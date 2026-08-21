//! Logos officiels des cryptomonnaies, téléchargés une fois puis servis depuis
//! le disque.
//!
//! La source fournit l'URL de l'image avec chaque cotation. On la récupère
//! hors du chemin critique — jamais pendant un tick de barre — et on la range
//! dans le cache sous l'identifiant de la monnaie. Les tailles sont modestes
//! (une dizaine de kilo-octets) et le téléchargement n'a lieu qu'une fois.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::model::Quote;

/// Hôte autorisé pour les images. Une URL venant d'ailleurs est ignorée :
/// la réponse de l'API n'est pas une raison suffisante pour aller chercher un
/// fichier n'importe où.
const IMAGE_HOST: &str = "https://coin-images.coingecko.com/";

/// Plafond de téléchargement. Les logos font une dizaine de kilo-octets ;
/// au-delà de cette marge, on refuse.
const MAX_ICON_BYTES: u64 = 256 * 1024;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(6);

/// Signature d'un fichier PNG : le contenu est vérifié avant d'être écrit,
/// pour qu'un chemin de cache ne puisse jamais désigner autre chose.
const PNG_MAGIC: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// Répertoire des logos dans le cache.
#[must_use]
pub fn dir(cache_dir: &Path) -> PathBuf {
    cache_dir.join("icons")
}

/// Emplacement du logo d'une cryptomonnaie, qu'il existe ou non.
#[must_use]
pub fn path(cache_dir: &Path, id: &str) -> PathBuf {
    dir(cache_dir).join(format!("{id}.png"))
}

/// Renseigne [`Quote::icon`] pour chaque cotation dont le logo est déjà en
/// cache. N'accède jamais au réseau : appelable depuis un tick de barre.
pub fn attach(cache_dir: &Path, quotes: &mut [Quote]) {
    for quote in quotes {
        let file = path(cache_dir, quote.id.as_str());
        if file.is_file() {
            quote.icon = file.to_str().map(ToOwned::to_owned);
        }
    }
}

/// Télécharge les logos manquants, puis renseigne [`Quote::icon`].
///
/// Chaque échec est silencieux et sans conséquence : l'affichage retombe sur
/// la pastille de repli. À n'appeler que là où quelques centaines de
/// millisecondes ne se voient pas — mode `--watch`, `refresh`, fenêtre.
pub fn fetch_missing(cache_dir: &Path, quotes: &mut [Quote]) {
    let mut agent = None;
    for quote in quotes.iter_mut() {
        quote.icon = ensure(
            cache_dir,
            quote.id.as_str(),
            quote.image.as_deref(),
            &mut agent,
        );
    }
}

/// Chemin du logo d'une cryptomonnaie, téléchargé si besoin, ou `None` si le
/// cache est vide et que l'URL est absente, hors du domaine autorisé, ou
/// injoignable.
///
/// L'agent est créé à la première requête et réutilisé ensuite : une liste de
/// résultats de recherche n'ouvre ainsi qu'une seule connexion.
#[must_use]
pub fn ensure(
    cache_dir: &Path,
    id: &str,
    url: Option<&str>,
    agent: &mut Option<ureq::Agent>,
) -> Option<String> {
    let file = path(cache_dir, id);
    if file.is_file() {
        return file.to_str().map(ToOwned::to_owned);
    }
    let url = url.filter(|url| url.starts_with(IMAGE_HOST))?;
    let agent = agent.get_or_insert_with(build_agent);
    download(agent, url, &file).ok()?;
    file.to_str().map(ToOwned::to_owned)
}

fn build_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_global(Some(TOTAL_TIMEOUT))
        .user_agent(concat!("cryptobar/", env!("CARGO_PKG_VERSION")))
        .max_redirects(0)
        .build()
        .into()
}

/// Récupère une image et l'écrit atomiquement à sa place définitive.
fn download(agent: &ureq::Agent, url: &str, destination: &Path) -> std::io::Result<()> {
    let bytes = agent
        .get(url)
        .call()
        .map_err(|e| std::io::Error::other(e.to_string()))?
        .body_mut()
        .with_config()
        .limit(MAX_ICON_BYTES)
        .read_to_vec()
        .map_err(|e| std::io::Error::other(e.to_string()))?;

    if !bytes.starts_with(PNG_MAGIC) {
        return Err(std::io::Error::other("réponse qui n'est pas un PNG"));
    }

    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Écriture atomique : un fichier partiel ne peut pas être servi.
    let temporary = destination.with_extension("png.tmp");
    {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
    }
    std::fs::rename(&temporary, destination)
}

#[cfg(test)]
// Un test qui ne trouve pas ce qu'il vient d'écrire doit s'arrêter net.
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::model::CoinId;

    fn quote(id: &str) -> Quote {
        Quote {
            id: CoinId::parse(id).unwrap(),
            symbol: "X".into(),
            name: "X".into(),
            price: 1.0,
            change_1h: None,
            change_24h: None,
            change_7d: None,
            sparkline: Vec::new(),
            image: Some(format!("{IMAGE_HOST}coins/images/1/large/x.png")),
            market_cap: None,
            volume_24h: None,
            rank: None,
            ath: None,
            ath_date: None,
            icon: None,
        }
    }

    #[test]
    fn un_logo_deja_present_est_rattache_sans_reseau() {
        let temporary = std::env::temp_dir().join("cryptobar-icons-attach");
        let _ = std::fs::remove_dir_all(&temporary);
        std::fs::create_dir_all(dir(&temporary)).unwrap();
        std::fs::write(path(&temporary, "bitcoin"), PNG_MAGIC).unwrap();

        let mut quotes = vec![quote("bitcoin"), quote("ethereum")];
        attach(&temporary, &mut quotes);

        let [bitcoin, ethereum] = quotes.as_slice() else {
            panic!("deux cotations attendues");
        };
        assert!(bitcoin.icon.is_some());
        assert!(ethereum.icon.is_none());
        let _ = std::fs::remove_dir_all(&temporary);
    }

    #[test]
    fn une_url_hors_du_domaine_autorise_est_ignoree() {
        let temporary = std::env::temp_dir().join("cryptobar-icons-host");
        let _ = std::fs::remove_dir_all(&temporary);
        let mut hostile = quote("bitcoin");
        hostile.image = Some("https://exemple.invalide/logo.png".into());
        let mut quotes = vec![hostile];

        fetch_missing(&temporary, &mut quotes);

        assert!(quotes.first().is_some_and(|q| q.icon.is_none()));
        assert!(!path(&temporary, "bitcoin").exists());
    }
}

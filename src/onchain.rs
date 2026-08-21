//! Suivi de portefeuilles en lecture seule.
//!
//! Le principe est celui d'un « watch-only » : on colle une **adresse
//! publique**, cryptobar lit le solde sur la chaîne et le valorise. Aucune clé
//! privée, aucune phrase de récupération, aucune signature — [`Wallet::validate`]
//! refuse d'ailleurs explicitement tout ce qui y ressemble. Le programme ne
//! peut pas dépenser, seulement regarder.
//!
//! Trois chaînes, trois points d'accès publics sans clé d'API : mempool.space
//! pour Bitcoin, un RPC Ethereum public, le RPC Solana mainnet. Les soldes sont
//! mis en cache sur disque, si bien que la barre n'ouvre jamais de connexion :
//! elle relit un fichier.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::model::CoinId;

/// Chaîne suivie. La liste est courte à dessein : chaque entrée demande un
/// point d'accès public, sans clé, et un décodage propre du solde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Chain {
    Bitcoin,
    Ethereum,
    Solana,
}

impl Chain {
    /// Toutes les chaînes, dans l'ordre d'affichage.
    pub const ALL: [Self; 3] = [Self::Bitcoin, Self::Ethereum, Self::Solana];

    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::Bitcoin => "bitcoin",
            Self::Ethereum => "ethereum",
            Self::Solana => "solana",
        }
    }

    /// Symbole de l'actif natif de la chaîne.
    #[must_use]
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Bitcoin => "BTC",
            Self::Ethereum => "ETH",
            Self::Solana => "SOL",
        }
    }

    /// Identifiant CoinGecko de l'actif natif : c'est par lui que le solde
    /// rejoint le portefeuille.
    #[must_use]
    pub fn coin(self) -> CoinId {
        CoinId::trusted(self.slug())
    }
}

impl std::str::FromStr for Chain {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "bitcoin" | "btc" => Ok(Self::Bitcoin),
            "ethereum" | "eth" => Ok(Self::Ethereum),
            "solana" | "sol" => Ok(Self::Solana),
            other => Err(format!("chaîne inconnue : {other}")),
        }
    }
}

/// Un portefeuille suivi, tel qu'il est écrit dans la configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wallet {
    pub chain: Chain,
    /// Adresse publique. Voir [`Wallet::validate`] : rien d'autre n'est accepté.
    pub address: String,
    /// Nom affiché, sinon l'adresse abrégée fait l'affaire.
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default = "crate::config::default_true")]
    pub enabled: bool,
}

impl Wallet {
    /// Vérifie que l'adresse a la forme attendue par sa chaîne.
    ///
    /// Le contrôle est volontairement strict : une adresse mal formée ne
    /// produirait qu'une requête inutile, et surtout on veut fermer la porte à
    /// tout ce qui ressemblerait à un secret collé par erreur.
    ///
    /// # Errors
    /// Décrit ce qui cloche, en français, pour un affichage direct.
    pub fn validate(&self) -> Result<(), String> {
        let address = self.address.trim();
        if address.is_empty() {
            return Err("adresse vide".to_owned());
        }
        if address.chars().any(char::is_whitespace) {
            // Une suite de mots séparés par des espaces, c'est une phrase de
            // récupération. On refuse net plutôt que de l'écrire sur disque.
            return Err("une adresse publique ne contient pas d'espace".to_owned());
        }
        let lowered = address.to_ascii_lowercase();
        for secret in [
            "xprv",
            "yprv",
            "zprv",
            "0x0000000000000000000000000000000000000000",
        ] {
            if lowered.starts_with(secret) {
                return Err("cette valeur n'est pas une adresse publique".to_owned());
            }
        }
        match self.chain {
            Chain::Bitcoin => validate_bitcoin(address),
            Chain::Ethereum => validate_ethereum(address),
            Chain::Solana => validate_solana(address),
        }
    }

    /// Nom affiché : l'étiquette si elle existe, sinon l'adresse abrégée.
    #[must_use]
    pub fn title(&self) -> String {
        match self
            .label
            .as_deref()
            .map(str::trim)
            .filter(|l| !l.is_empty())
        {
            Some(label) => label.to_owned(),
            None => shorten(&self.address),
        }
    }
}

/// Abrège une adresse en gardant ses extrémités, comme le font les explorateurs.
#[must_use]
pub fn shorten(address: &str) -> String {
    let chars: Vec<char> = address.chars().collect();
    if chars.len() <= 16 {
        return address.to_owned();
    }
    let head: String = chars.iter().take(6).collect();
    let tail: String = chars
        .iter()
        .rev()
        .take(6)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{head}…{tail}")
}

fn validate_bitcoin(address: &str) -> Result<(), String> {
    let lowered = address.to_ascii_lowercase();
    // Bech32 (bc1…) : minuscules, sans les caractères ambigus « 1 b i o ».
    if lowered.starts_with("bc1") {
        let body = lowered.get(3..).unwrap_or_default();
        let ok = (11..=87).contains(&body.len())
            && body
                .chars()
                .all(|c| c.is_ascii_digit() || c.is_ascii_lowercase())
            && !body.contains(['b', 'i', 'o'])
            && !body.contains('1');
        return ok
            .then_some(())
            .ok_or_else(|| "adresse bech32 mal formée".to_owned());
    }
    // Base58 hérité : commence par 1 (P2PKH) ou 3 (P2SH).
    let ok = address.starts_with(['1', '3'])
        && (26..=35).contains(&address.chars().count())
        && address.chars().all(is_base58);
    ok.then_some(())
        .ok_or_else(|| "adresse Bitcoin non reconnue".to_owned())
}

fn validate_ethereum(address: &str) -> Result<(), String> {
    let ok = address.len() == 42
        && address.starts_with("0x")
        && address
            .get(2..)
            .is_some_and(|body| body.chars().all(|c| c.is_ascii_hexdigit()));
    ok.then_some(())
        .ok_or_else(|| "adresse Ethereum attendue : 0x suivi de 40 caractères".to_owned())
}

fn validate_solana(address: &str) -> Result<(), String> {
    let ok = (32..=44).contains(&address.chars().count()) && address.chars().all(is_base58);
    ok.then_some(())
        .ok_or_else(|| "adresse Solana non reconnue".to_owned())
}

/// Alphabet base58 : ni zéro, ni O, ni I, ni l — trop faciles à confondre.
fn is_base58(c: char) -> bool {
    c.is_ascii_alphanumeric() && !matches!(c, '0' | 'O' | 'I' | 'l')
}

/// Solde lu sur la chaîne, dans l'unité de l'actif natif.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Balance {
    pub chain: Chain,
    pub address: String,
    /// Quantité, en BTC, ETH ou SOL selon la chaîne.
    pub amount: f64,
    /// Instant Unix de la dernière lecture réussie.
    pub fetched_at: u64,
}

/// Somme des soldes par cryptomonnaie, prête à rejoindre le portefeuille.
#[must_use]
pub fn totals(balances: &[Balance]) -> BTreeMap<CoinId, f64> {
    let mut totals: BTreeMap<CoinId, f64> = BTreeMap::new();
    for balance in balances.iter().filter(|b| b.amount.is_finite()) {
        *totals.entry(balance.chain.coin()).or_insert(0.0) += balance.amount;
    }
    totals
}

const USER_AGENT: &str = concat!(
    "cryptobar/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/Akayashuu/cryptobar)"
);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(10);
/// Les réponses attendues font quelques centaines d'octets ; ce plafond
/// protège d'un serveur qui déraille sans jamais gêner un usage normal.
const MAX_RESPONSE_BYTES: u64 = 64 * 1024;
/// Durée de validité d'un solde en cache. Une chaîne se règle en minutes, pas
/// en secondes : interroger plus souvent ne servirait qu'à charger les serveurs
/// publics qui nous rendent service gratuitement.
const TTL: Duration = Duration::from_secs(300);

fn endpoint(chain: Chain) -> String {
    let (var, default) = match chain {
        Chain::Bitcoin => ("CRYPTOBAR_BTC_API", "https://mempool.space/api"),
        Chain::Ethereum => ("CRYPTOBAR_ETH_RPC", "https://ethereum-rpc.publicnode.com"),
        Chain::Solana => ("CRYPTOBAR_SOL_RPC", "https://api.mainnet-beta.solana.com"),
    };
    match std::env::var(var) {
        Ok(base) if !base.trim().is_empty() => base.trim().trim_end_matches('/').to_owned(),
        _ => default.to_owned(),
    }
}

/// Lecteur de soldes, avec son cache disque.
pub struct Watcher {
    agent: ureq::Agent,
    path: PathBuf,
}

impl Watcher {
    #[must_use]
    pub fn new(cache_dir: &std::path::Path) -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_global(Some(TOTAL_TIMEOUT))
            .user_agent(USER_AGENT)
            .max_redirects(0)
            .build();
        Self {
            agent: config.into(),
            path: cache_dir.join("wallets.json"),
        }
    }

    /// Soldes en cache, sans aucun accès réseau. C'est ce que lit la barre.
    #[must_use]
    pub fn cached(&self, wallets: &[Wallet]) -> Vec<Balance> {
        let stored = self.read();
        wallets
            .iter()
            .filter(|wallet| wallet.enabled)
            .filter_map(|wallet| stored.get(&wallet.address).cloned())
            .collect()
    }

    /// Soldes à jour : les entrées encore fraîches viennent du cache, les
    /// autres de la chaîne. Une lecture qui échoue laisse la valeur précédente
    /// en place plutôt que d'effacer le portefeuille.
    #[must_use]
    pub fn refresh(&self, wallets: &[Wallet]) -> Vec<Balance> {
        let mut stored = self.read();
        let now = now();
        let mut changed = false;

        for wallet in wallets.iter().filter(|wallet| wallet.enabled) {
            let fresh = stored
                .get(&wallet.address)
                .is_some_and(|balance| now.saturating_sub(balance.fetched_at) < TTL.as_secs());
            if fresh || wallet.validate().is_err() {
                continue;
            }
            if let Ok(amount) = self.fetch(wallet.chain, &wallet.address) {
                stored.insert(
                    wallet.address.clone(),
                    Balance {
                        chain: wallet.chain,
                        address: wallet.address.clone(),
                        amount,
                        fetched_at: now,
                    },
                );
                changed = true;
            }
        }

        // Le cache ne garde que les portefeuilles encore déclarés : retirer une
        // adresse de la configuration doit l'effacer du disque.
        let known: std::collections::BTreeSet<&str> =
            wallets.iter().map(|w| w.address.as_str()).collect();
        let before = stored.len();
        stored.retain(|address, _| known.contains(address.as_str()));
        if changed || stored.len() != before {
            let _ = self.write(&stored);
        }
        self.cached(wallets)
    }

    /// Interroge la chaîne pour une adresse.
    fn fetch(&self, chain: Chain, address: &str) -> Result<f64, String> {
        let base = endpoint(chain);
        match chain {
            Chain::Bitcoin => {
                let url = format!("{base}/address/{address}");
                decode_bitcoin(&self.get(&url)?)
            }
            Chain::Ethereum => {
                let body = self.rpc(&base, "eth_getBalance", &[address.into(), "latest".into()])?;
                decode_ethereum(&body)
            }
            Chain::Solana => {
                let body = self.rpc(&base, "getBalance", &[address.into()])?;
                decode_solana(&body)
            }
        }
    }

    fn get(&self, url: &str) -> Result<String, String> {
        self.agent
            .get(url)
            .call()
            .map_err(|e| e.to_string())?
            .body_mut()
            .with_config()
            .limit(MAX_RESPONSE_BYTES)
            .read_to_string()
            .map_err(|e| e.to_string())
    }

    fn rpc(&self, url: &str, method: &str, params: &[serde_json::Value]) -> Result<String, String> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });
        self.agent
            .post(url)
            .send_json(&request)
            .map_err(|e| e.to_string())?
            .body_mut()
            .with_config()
            .limit(MAX_RESPONSE_BYTES)
            .read_to_string()
            .map_err(|e| e.to_string())
    }

    fn read(&self) -> BTreeMap<String, Balance> {
        // Un cache illisible est traité comme absent : jamais une erreur.
        std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    fn write(&self, balances: &BTreeMap<String, Balance>) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            crate::paths::ensure_dir(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let bytes = serde_json::to_vec(balances)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, &self.path)
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Solde Bitcoin : la différence entre ce qui est entré et ce qui est sorti,
/// confirmé comme en attente, convertie de satoshis en BTC.
fn decode_bitcoin(body: &str) -> Result<f64, String> {
    #[derive(Deserialize, Default)]
    struct Stats {
        #[serde(default)]
        funded_txo_sum: i64,
        #[serde(default)]
        spent_txo_sum: i64,
    }
    #[derive(Deserialize)]
    struct Address {
        #[serde(default)]
        chain_stats: Stats,
        #[serde(default)]
        mempool_stats: Stats,
    }
    let parsed: Address = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let sats = parsed
        .chain_stats
        .funded_txo_sum
        .saturating_sub(parsed.chain_stats.spent_txo_sum)
        .saturating_add(parsed.mempool_stats.funded_txo_sum)
        .saturating_sub(parsed.mempool_stats.spent_txo_sum);
    Ok(units(sats_to_f64(sats), 100_000_000.0))
}

/// Solde Ethereum : un entier hexadécimal en wei.
fn decode_ethereum(body: &str) -> Result<f64, String> {
    #[derive(Deserialize)]
    struct Reply {
        #[serde(default)]
        result: Option<String>,
    }
    let parsed: Reply = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let raw = parsed
        .result
        .ok_or_else(|| "réponse sans solde".to_owned())?;
    let digits = raw.trim_start_matches("0x");
    let wei = u128::from_str_radix(digits, 16).map_err(|e| e.to_string())?;
    Ok(units(wei_to_f64(wei), 1e18))
}

/// Solde Solana : un entier de lamports.
fn decode_solana(body: &str) -> Result<f64, String> {
    #[derive(Deserialize)]
    struct Value {
        #[serde(default)]
        value: Option<u64>,
    }
    #[derive(Deserialize)]
    struct Reply {
        #[serde(default)]
        result: Option<Value>,
    }
    let parsed: Reply = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let lamports = parsed
        .result
        .and_then(|result| result.value)
        .ok_or_else(|| "réponse sans solde".to_owned())?;
    Ok(units(lamports_to_f64(lamports), 1e9))
}

/// Conversion d'une unité de base vers l'unité affichée.
fn units(base: f64, divisor: f64) -> f64 {
    if divisor.abs() < f64::EPSILON {
        return 0.0;
    }
    let value = base / divisor;
    if value.is_finite() { value } else { 0.0 }
}

// Les trois conversions ci-dessous perdent des chiffres au-delà de 2^53, ce qui
// n'arrive qu'à des soldes irréels ; l'affichage se compte en décimales, pas en
// satoshis.
#[allow(clippy::cast_precision_loss)]
fn sats_to_f64(value: i64) -> f64 {
    value as f64
}

#[allow(clippy::cast_precision_loss)]
fn wei_to_f64(value: u128) -> f64 {
    value as f64
}

#[allow(clippy::cast_precision_loss)]
fn lamports_to_f64(value: u64) -> f64 {
    value as f64
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn wallet(chain: Chain, address: &str) -> Wallet {
        Wallet {
            chain,
            address: address.to_owned(),
            label: None,
            enabled: true,
        }
    }

    #[test]
    fn une_adresse_bitcoin_valide_est_acceptee() {
        assert!(
            wallet(Chain::Bitcoin, "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq")
                .validate()
                .is_ok()
        );
        assert!(
            wallet(Chain::Bitcoin, "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa")
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn une_adresse_bitcoin_mal_formee_est_refusee() {
        assert!(wallet(Chain::Bitcoin, "bc1!!!").validate().is_err());
        assert!(wallet(Chain::Bitcoin, "2Nabc").validate().is_err());
    }

    #[test]
    fn une_adresse_ethereum_doit_faire_quarante_hexadecimaux() {
        assert!(
            wallet(
                Chain::Ethereum,
                "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"
            )
            .validate()
            .is_ok()
        );
        assert!(wallet(Chain::Ethereum, "0x1234").validate().is_err());
    }

    #[test]
    fn une_adresse_solana_est_du_base58() {
        assert!(
            wallet(
                Chain::Solana,
                "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM"
            )
            .validate()
            .is_ok()
        );
        assert!(wallet(Chain::Solana, "0OIl").validate().is_err());
    }

    #[test]
    fn une_phrase_de_recuperation_est_refusee() {
        let phrase = wallet(Chain::Bitcoin, "abandon abandon abandon ability");
        let error = phrase.validate().expect_err("doit être refusée");
        assert!(error.contains("espace"), "{error}");
    }

    #[test]
    fn une_cle_privee_etendue_est_refusee() {
        let error = wallet(Chain::Bitcoin, "xprv9s21ZrQH143K3QTDL4LXw2F7Hek")
            .validate()
            .expect_err("doit être refusée");
        assert!(error.contains("publique"), "{error}");
    }

    #[test]
    fn le_solde_bitcoin_se_compte_en_btc() {
        let body = r#"{"chain_stats":{"funded_txo_sum":150000000,"spent_txo_sum":50000000},
                       "mempool_stats":{"funded_txo_sum":0,"spent_txo_sum":0}}"#;
        let amount = decode_bitcoin(body).expect("solde lisible");
        assert!((amount - 1.0).abs() < 1e-9, "{amount}");
    }

    #[test]
    fn le_solde_bitcoin_tient_compte_du_mempool() {
        let body = r#"{"chain_stats":{"funded_txo_sum":100000000,"spent_txo_sum":0},
                       "mempool_stats":{"funded_txo_sum":0,"spent_txo_sum":100000000}}"#;
        assert!(decode_bitcoin(body).expect("solde lisible").abs() < 1e-12);
    }

    #[test]
    fn le_solde_ethereum_se_lit_en_hexadecimal() {
        let body = r#"{"jsonrpc":"2.0","id":1,"result":"0x0de0b6b3a7640000"}"#;
        let amount = decode_ethereum(body).expect("solde lisible");
        assert!((amount - 1.0).abs() < 1e-9, "{amount}");
    }

    #[test]
    fn une_reponse_ethereum_sans_resultat_est_une_erreur() {
        let body = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32000}}"#;
        assert!(decode_ethereum(body).is_err());
    }

    #[test]
    fn le_solde_solana_se_compte_en_lamports() {
        let body = r#"{"jsonrpc":"2.0","result":{"context":{"slot":1},"value":2500000000},"id":1}"#;
        let amount = decode_solana(body).expect("solde lisible");
        assert!((amount - 2.5).abs() < 1e-9, "{amount}");
    }

    #[test]
    fn les_soldes_se_cumulent_par_cryptomonnaie() {
        let balances = [
            Balance {
                chain: Chain::Bitcoin,
                address: "a".to_owned(),
                amount: 0.5,
                fetched_at: 0,
            },
            Balance {
                chain: Chain::Bitcoin,
                address: "b".to_owned(),
                amount: 0.25,
                fetched_at: 0,
            },
        ];
        let totals = totals(&balances);
        let sum = totals.get(&Chain::Bitcoin.coin()).copied().unwrap_or(0.0);
        assert!((sum - 0.75).abs() < 1e-9);
    }

    #[test]
    fn une_adresse_longue_saffiche_abregee() {
        let short = shorten("bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq");
        assert!(short.starts_with("bc1qar"), "{short}");
        assert!(short.ends_with("wf5mdq"), "{short}");
    }
}

//! Official cryptocurrency logos, downloaded once then served from disk.
//!
//! The source hands the image URL over with every quote. It is fetched off the
//! critical path — never during a bar tick — and filed in the cache under the
//! coin id. The sizes are modest (about ten kilobytes) and the download happens
//! only once.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::model::Quote;

/// Allowed host for the images. A URL coming from anywhere else is ignored:
/// the API response is not reason enough to go fetch a file just anywhere.
const IMAGE_HOST: &str = "https://coin-images.coingecko.com/";

/// Download ceiling. The logos weigh about ten kilobytes; past that margin, we
/// refuse.
const MAX_ICON_BYTES: u64 = 256 * 1024;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(6);

/// Signature of a PNG file: the content is checked before being written, so
/// that a cache path can never point at anything else.
const PNG_MAGIC: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// Logo directory inside the cache.
#[must_use]
pub fn dir(cache_dir: &Path) -> PathBuf {
    cache_dir.join("icons")
}

/// Location of a cryptocurrency logo, whether it exists or not.
#[must_use]
pub fn path(cache_dir: &Path, id: &str) -> PathBuf {
    dir(cache_dir).join(format!("{id}.png"))
}

/// Fills [`Quote::icon`] in for every quote whose logo is already cached. Never
/// touches the network: callable from a bar tick.
pub fn attach(cache_dir: &Path, quotes: &mut [Quote]) {
    for quote in quotes {
        let file = path(cache_dir, quote.id.as_str());
        if file.is_file() {
            quote.icon = file.to_str().map(ToOwned::to_owned);
        }
    }
}

/// Downloads the missing logos, then fills [`Quote::icon`] in.
///
/// Every failure is silent and harmless: the display falls back to the badge.
/// Only call it where a few hundred milliseconds go unnoticed — `--watch` mode,
/// `refresh`, the window.
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

/// Path of a cryptocurrency logo, downloaded when needed, or `None` when the
/// cache is empty and the URL is missing, off the allowed domain, or
/// unreachable.
///
/// The agent is created on the first request then reused: a list of search
/// results therefore opens a single connection.
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

/// Fetches an image and writes it atomically to its final place.
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
        return Err(std::io::Error::other("response that is not a PNG"));
    }

    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Atomic write: a partial file can never be served.
    let temporary = destination.with_extension("png.tmp");
    {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
    }
    std::fs::rename(&temporary, destination)
}

#[cfg(test)]
// A test that cannot find what it just wrote has to stop dead.
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
    fn an_already_present_logo_is_attached_without_network() {
        let temporary = std::env::temp_dir().join("cryptobar-icons-attach");
        let _ = std::fs::remove_dir_all(&temporary);
        std::fs::create_dir_all(dir(&temporary)).unwrap();
        std::fs::write(path(&temporary, "bitcoin"), PNG_MAGIC).unwrap();

        let mut quotes = vec![quote("bitcoin"), quote("ethereum")];
        attach(&temporary, &mut quotes);

        let [bitcoin, ethereum] = quotes.as_slice() else {
            panic!("expected two quotes");
        };
        assert!(bitcoin.icon.is_some());
        assert!(ethereum.icon.is_none());
        let _ = std::fs::remove_dir_all(&temporary);
    }

    #[test]
    fn a_url_off_the_allowed_domain_is_ignored() {
        let temporary = std::env::temp_dir().join("cryptobar-icons-host");
        let _ = std::fs::remove_dir_all(&temporary);
        let mut hostile = quote("bitcoin");
        hostile.image = Some("https://example.invalid/logo.png".into());
        let mut quotes = vec![hostile];

        fetch_missing(&temporary, &mut quotes);

        assert!(quotes.first().is_some_and(|q| q.icon.is_none()));
        assert!(!path(&temporary, "bitcoin").exists());
    }
}

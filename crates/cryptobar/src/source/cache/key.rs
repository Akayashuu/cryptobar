use crate::model::{CoinId, Currency};

/// Safe file name derived from a logical key. The key only holds already
/// validated characters, but it is filtered again so that no path can escape
/// the cache directory.
pub(super) fn sanitize(key: &str) -> String {
    key.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .take(120)
        .collect()
}

pub(super) fn markets_key(ids: &[CoinId], vs: &Currency) -> String {
    format!(
        "markets-{}-{}",
        vs.as_str(),
        ids.iter().map(CoinId::as_str).collect::<Vec<_>>().join("_")
    )
}

/// Shortens an address keeping both ends, the way explorers do.
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

pub(super) fn validate_bitcoin(address: &str) -> Result<(), String> {
    let lowered = address.to_ascii_lowercase();
    // Bech32 (bc1…): lowercase, without the ambiguous characters `1 b i o`.
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
            .ok_or_else(|| "malformed bech32 address".to_owned());
    }
    // Legacy base58: starts with 1 (P2PKH) or 3 (P2SH).
    let ok = address.starts_with(['1', '3'])
        && (26..=35).contains(&address.chars().count())
        && address.chars().all(is_base58);
    ok.then_some(())
        .ok_or_else(|| "unrecognised Bitcoin address".to_owned())
}

pub(super) fn validate_ethereum(address: &str) -> Result<(), String> {
    let ok = address.len() == 42
        && address.starts_with("0x")
        && address
            .get(2..)
            .is_some_and(|body| body.chars().all(|c| c.is_ascii_hexdigit()));
    ok.then_some(())
        .ok_or_else(|| "expected an Ethereum address: 0x followed by 40 characters".to_owned())
}

pub(super) fn validate_solana(address: &str) -> Result<(), String> {
    let ok = (32..=44).contains(&address.chars().count()) && address.chars().all(is_base58);
    ok.then_some(())
        .ok_or_else(|| "unrecognised Solana address".to_owned())
}

/// Base58 alphabet: no zero, no O, no I, no l — too easy to confuse.
fn is_base58(c: char) -> bool {
    c.is_ascii_alphanumeric() && !matches!(c, '0' | 'O' | 'I' | 'l')
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::onchain::{Chain, Wallet};

    fn wallet(chain: Chain, address: &str) -> Wallet {
        Wallet {
            chain,
            address: address.to_owned(),
            label: None,
            enabled: true,
        }
    }

    #[test]
    fn a_valid_bitcoin_address_is_accepted() {
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
    fn a_malformed_bitcoin_address_is_rejected() {
        assert!(wallet(Chain::Bitcoin, "bc1!!!").validate().is_err());
        assert!(wallet(Chain::Bitcoin, "2Nabc").validate().is_err());
    }

    #[test]
    fn an_ethereum_address_must_be_forty_hex_digits() {
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
    fn a_solana_address_is_base58() {
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
    fn a_recovery_phrase_is_rejected() {
        let phrase = wallet(Chain::Bitcoin, "abandon abandon abandon ability");
        let error = phrase.validate().expect_err("must be rejected");
        assert!(error.contains("space"), "{error}");
    }

    #[test]
    fn an_extended_private_key_is_rejected() {
        let error = wallet(Chain::Bitcoin, "xprv9s21ZrQH143K3QTDL4LXw2F7Hek")
            .validate()
            .expect_err("must be rejected");
        assert!(error.contains("public"), "{error}");
    }

    #[test]
    fn a_long_address_displays_shortened() {
        let short = shorten("bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq");
        assert!(short.starts_with("bc1qar"), "{short}");
        assert!(short.ends_with("wf5mdq"), "{short}");
    }
}

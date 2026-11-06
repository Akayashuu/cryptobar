use serde::{Deserialize, Serialize};

use super::address::{shorten, validate_bitcoin, validate_ethereum, validate_solana};
use super::chain::Chain;
use super::error::OnchainError;

/// A watched wallet, as written in the configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wallet {
    pub chain: Chain,
    /// Public address. See [`Wallet::validate`]: nothing else is accepted.
    pub address: String,
    /// Display name; otherwise the shortened address will do.
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default = "crate::config::default_true")]
    pub enabled: bool,
}

impl Wallet {
    /// Checks that the address has the shape its chain expects.
    ///
    /// The check is deliberately strict: a malformed address would only produce
    /// a useless request, and above all we want to shut the door on anything
    /// that looks like a secret pasted by mistake.
    ///
    /// # Errors
    /// Describes what is wrong, ready to be displayed as is.
    pub fn validate(&self) -> Result<(), OnchainError> {
        let address = self.address.trim();
        if address.is_empty() {
            return Err(OnchainError::EmptyAddress);
        }
        if address.chars().any(char::is_whitespace) {
            return Err(OnchainError::ContainsSpace);
        }
        let lowered = address.to_ascii_lowercase();
        for secret in [
            "xprv",
            "yprv",
            "zprv",
            "0x0000000000000000000000000000000000000000",
        ] {
            if lowered.starts_with(secret) {
                return Err(OnchainError::NotPublic);
            }
        }
        match self.chain {
            Chain::Bitcoin => validate_bitcoin(address),
            Chain::Ethereum => validate_ethereum(address),
            Chain::Solana => validate_solana(address),
        }
    }

    /// Display name: the label when there is one, otherwise the short address.
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

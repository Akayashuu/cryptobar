//! Surgical editing of the configuration file.
//!
//! The graphical window writes into the same `config.toml` the user edits by
//! hand. Rewriting the file from the serialised struct would wipe its comments
//! and its ordering; the TOML document is therefore edited in place with
//! `toml_edit`, which only touches the targeted value.
//!
//! Every change is validated by [`Config::parse`] **before** it reaches the
//! disk: a rejected setting leaves the file untouched.

mod alerts;
mod coins;
mod editor;
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod fixtures;
mod settings;
mod wallets;

pub use editor::Editor;

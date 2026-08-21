//! Interfaces graphiques optionnelles.
//!
//! Tout ce module est derrière la fonctionnalité `popup` : une compilation
//! `--no-default-features` produit un binaire sans aucune dépendance GTK.

#[cfg(feature = "popup")]
pub mod popup;

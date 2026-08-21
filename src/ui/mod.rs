//! Optional graphical interfaces.
//!
//! The whole module sits behind the `popup` feature: a `--no-default-features`
//! build produces a binary with no GTK dependency at all.

#[cfg(feature = "popup")]
pub mod popup;

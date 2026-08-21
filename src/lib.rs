//! cryptobar — cours de cryptomonnaies pour Waybar.
//!
//! La logique vit dans cette bibliothèque ; le binaire ne fait qu'analyser les
//! arguments et appeler les commandes. Cela rend chaque module testable isolément.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::indexing_slicing,
    clippy::integer_division,
    clippy::arithmetic_side_effects,
    clippy::exit
)]

pub mod config;
pub mod model;
pub mod paths;
pub mod render;

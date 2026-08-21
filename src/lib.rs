//! cryptobar — cours de cryptomonnaies pour Waybar.
//!
//! La logique vit dans cette bibliothèque ; le binaire ne fait qu'analyser les
//! arguments et appeler les commandes. Cela rend chaque module testable isolément.

pub mod app;
pub mod cli;
pub mod commands;
pub mod config;
pub mod model;
pub mod paths;
pub mod render;
pub mod source;
pub mod state;
pub mod ui;
pub mod waybar_signal;

//! GTK4 window: detailed prices, vector curve, search and settings.
//!
//! This is the configuration surface of the project. Everything `config.toml`
//! accepts is set here, and every change is written back to that same file —
//! comments preserved — by [`crate::config_edit`].
//!
//! The network never runs on the graphical thread: requests leave on a
//! dedicated thread and come back through a channel the GTK loop drains. The
//! window therefore stays smooth even when the API takes a second to answer.

mod alerts;
mod chart;
mod export;
mod hover;
mod identity;
mod model;
mod parse;
mod portfolio;
mod prices;
mod search;
mod settings;
mod status;
mod style;
mod update;
mod widgets;
mod window;
mod worker;

pub use window::show;

//! cryptobar — cryptocurrency prices for your bar, terminal and desktop.
//!
//! The logic lives in this library; the binary only parses arguments and calls
//! into the commands. That keeps every module testable in isolation.

pub mod alerts;
pub mod app;
pub mod cli;
pub mod commands;
pub mod config;
pub mod config_edit;
pub mod model;
pub mod onchain;
pub mod paths;
pub mod portfolio;
pub mod render;
pub mod source;
pub mod state;
pub mod ui;
pub mod waybar_signal;

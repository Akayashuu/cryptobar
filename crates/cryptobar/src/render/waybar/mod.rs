//! Serialisation of the Waybar custom module protocol.
//!
//! Waybar expects one line of JSON holding `text`, `tooltip` and `class`.
//! `text` and `tooltip` are read as Pango markup: anything coming from the
//! network therefore has to be escaped.
//!
//! Other bars (Quickshell, Ags, Eww) consume the same JSON but display plain
//! text. [`Markup::None`] then produces output without a single tag or entity.

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod fixtures;
mod markup;
mod module;
mod payload;
mod rows;
mod series;
mod summary;
mod tooltip;

pub use markup::{Markup, escape_pango};
pub use module::render;
pub use payload::{AlertRow, CoinRow, HoldingRow, Module, PortfolioSummary};

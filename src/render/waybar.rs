//! Serialisation of the Waybar custom module protocol.
//!
//! Waybar expects one line of JSON holding `text`, `tooltip` and `class`.
//! `text` and `tooltip` are read as Pango markup: anything coming from the
//! network therefore has to be escaped.
//!
//! Other bars — Quickshell, Ags, Eww — consume the same JSON but display plain
//! text. [`Markup::None`] then produces output without a single tag or entity.

use serde::Serialize;

use crate::config::Config;
use crate::model::Quote;
use crate::portfolio;

use super::{badge, format, sparkline};

/// Number of points kept in the series handed to rich bars: enough to draw a
/// readable curve, few enough for the JSON line to stay short at every
/// interval.
const SERIES_POINTS: usize = 48;

/// One quote, split into display-ready fields.
///
/// Bars able to compose their own layout — Quickshell, Ags, Eww — use them to
/// draw an aligned list and a curve, where `tooltip` would only hand them a
/// block of text to parse.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CoinRow {
    pub symbol: String,
    pub name: String,
    pub price: String,
    pub change_1h: String,
    pub change_24h: String,
    pub change_7d: String,
    /// "up", "down" or "flat", following the 24 h change.
    pub trend: &'static str,
    /// Prices over the last seven days, normalised to [0, 1]: the consumer
    /// multiplies by whatever height it has.
    pub series: Vec<f64>,
    /// Quote currently highlighted in the bar.
    pub active: bool,
    /// Local path of the official logo, empty when not cached yet. Consumers
    /// able to display an image prefer it to the badge.
    pub icon: String,
    /// Glyph of the fallback badge, a single character.
    pub glyph: String,
    /// Brand colour of the disc, `#rrggbb`.
    pub color: String,
    /// Colour readable on top of [`CoinRow::color`], `#rrggbb`.
    pub foreground: String,
}

/// One portfolio line, ready to display.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct HoldingRow {
    pub symbol: String,
    /// Quantity held, as read from the watched addresses.
    pub quantity: String,
    /// Value of the line in the quote currency.
    pub value: String,
    /// Share of the portfolio, within [0, 1]: the consumer turns it into an
    /// arc, a bar or a percentage, depending on the room it has.
    pub share: f64,
    pub color: String,
}

/// Valued portfolio, absent when no balance is known.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PortfolioSummary {
    pub total: String,
    /// Gain or loss over 24 h, already formatted with its sign.
    pub change_24h: String,
    pub change_24h_pct: String,
    /// True when the portfolio is up over 24 h.
    pub positive: bool,
    pub positions: Vec<HoldingRow>,
}

/// An alert currently crossed.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AlertRow {
    pub symbol: String,
    /// "above", "below" or "change".
    pub kind: &'static str,
    pub title: String,
    pub body: String,
}

/// Payload of a Waybar module.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Module {
    pub text: String,
    pub tooltip: String,
    pub class: String,
    /// Extra fields Waybar ignores, useful to bars able to compose their own
    /// layout (Quickshell in vertical mode, where the full text would not fit
    /// the available width).
    pub symbol: String,
    pub price: String,
    pub percent: String,
    pub sparkline: String,
    /// Trend of the active quote: "up", "down", "flat" or "error".
    pub trend: &'static str,
    /// Data served from the cache, network unavailable.
    pub stale: bool,
    /// Logo and fallback badge of the active quote.
    pub icon: String,
    pub glyph: String,
    pub color: String,
    pub foreground: String,
    /// Every watched quote, in configuration order.
    pub coins: Vec<CoinRow>,
    /// Alerts crossed right now. Empty most of the time: that is what lets the
    /// bar light an indicator up without recomputing anything.
    pub alerts: Vec<AlertRow>,
    /// Valued portfolio, `null` when no watched address holds a balance.
    pub portfolio: Option<PortfolioSummary>,
}

/// Brings a series into [0, 1]. A flat series sits in the middle rather than
/// dividing by a zero amplitude.
fn normalize(points: &[f64]) -> Vec<f64> {
    let finite: Vec<f64> = points.iter().copied().filter(|v| v.is_finite()).collect();
    let reduced = sparkline::downsample(&finite, SERIES_POINTS);
    let Some(min) = reduced.iter().copied().reduce(f64::min) else {
        return Vec::new();
    };
    let max = reduced.iter().copied().fold(min, f64::max);
    let span = max - min;
    if span <= 0.0 {
        return vec![0.5; reduced.len()];
    }
    // Three decimals are enough for a pixel-accurate plot and divide by five
    // the length of the JSON line emitted at every interval.
    reduced
        .iter()
        .map(|value| ((value - min) / span * 1000.0).round() / 1000.0)
        .collect()
}

impl Module {
    /// Serialises to one line of JSON.
    ///
    /// # Errors
    /// Impossible in practice: the struct only holds strings.
    pub fn to_line(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// How markup is handled in the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Markup {
    /// Pango tags and escaped entities: what Waybar expects.
    #[default]
    Pango,
    /// Plain text, for bars that interpret no markup at all.
    None,
}

impl Markup {
    /// Applies the escaping matching untrusted data.
    fn escape(self, raw: &str) -> String {
        match self {
            Self::Pango => escape_pango(raw),
            // Control characters are dropped in every case: they would break
            // the expected JSON line.
            Self::None => raw
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .collect(),
        }
    }

    /// Opens and closes a bold run, or nothing at all.
    fn bold(self, raw: &str) -> String {
        match self {
            Self::Pango => format!("<b>{raw}</b>"),
            Self::None => raw.to_owned(),
        }
    }

    /// Formats a quiet note.
    fn italic(self, raw: &str) -> String {
        match self {
            Self::Pango => format!("<i>{raw}</i>"),
            Self::None => raw.to_owned(),
        }
    }
}

impl std::str::FromStr for Markup {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "pango" => Ok(Self::Pango),
            "none" | "plain" => Ok(Self::None),
            other => Err(format!(
                "unknown markup: {other}; expected `pango` or `none`"
            )),
        }
    }
}

/// Escapes the characters Pango would read as markup.
#[must_use]
pub fn escape_pango(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\'' => out.push_str("&apos;"),
            '"' => out.push_str("&quot;"),
            // Control characters would break the expected JSON line.
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// Builds the module from the active quote and the full list.
#[must_use]
pub fn render(
    quotes: &[Quote],
    active: usize,
    config: &Config,
    stale: bool,
    markup: Markup,
    onchain: &std::collections::BTreeMap<crate::model::CoinId, f64>,
) -> Module {
    let Some(quote) = quotes.get(active).or_else(|| quotes.first()) else {
        return Module {
            text: "crypto —".to_owned(),
            tooltip: "No data available".to_owned(),
            class: "error".to_owned(),
            symbol: "—".to_owned(),
            price: String::new(),
            percent: String::new(),
            sparkline: String::new(),
            trend: "error",
            stale,
            icon: String::new(),
            glyph: "●".to_owned(),
            color: "#8d99ae".to_owned(),
            foreground: "#ffffff".to_owned(),
            coins: Vec::new(),
            alerts: Vec::new(),
            portfolio: None,
        };
    };

    let decimals = config.decimals();
    let triggers = crate::alerts::evaluate(config, quotes);
    let wallet = portfolio::compute(quotes, onchain);
    let badge = badge::of(&quote.symbol);
    let symbol = markup.escape(&quote.symbol.to_uppercase());
    let price = markup.escape(&format::price(quote.price, &config.currency, decimals));
    let percent = markup.escape(&format::percent(quote.change_24h));
    let spark = if config.sparkline && !quote.sparkline.is_empty() {
        sparkline::sparkline(&quote.sparkline, config.sparkline_width)
    } else {
        String::new()
    };

    let mut text = format!("{symbol} {price}");
    if !spark.is_empty() {
        text.push(' ');
        text.push_str(&spark);
    }
    text.push(' ');
    text.push_str(&percent);

    Module {
        text,
        tooltip: tooltip(
            quotes,
            active,
            config,
            stale,
            markup,
            wallet.as_ref(),
            &triggers,
        ),
        class: class(quote, stale, !triggers.is_empty()),
        symbol,
        price,
        percent,
        sparkline: spark,
        trend: quote.trend().class(),
        stale,
        icon: quote.icon.clone().unwrap_or_default(),
        glyph: badge.glyph,
        color: badge.color,
        foreground: badge.foreground,
        coins: rows(quotes, active, config, markup),
        alerts: triggers
            .iter()
            .map(|trigger| AlertRow {
                symbol: markup.escape(&trigger.symbol),
                kind: trigger.kind.slug(),
                title: markup.escape(&trigger.title),
                body: markup.escape(&trigger.body),
            })
            .collect(),
        portfolio: wallet.map(|wallet| summary(&wallet, config, markup)),
    }
}

fn summary(wallet: &portfolio::Portfolio, config: &Config, markup: Markup) -> PortfolioSummary {
    let decimals = config.decimals();
    let money = |value: f64| markup.escape(&format::price(value, &config.currency, decimals));
    PortfolioSummary {
        total: money(wallet.total),
        change_24h: match wallet.change_24h {
            // The sign is explicit: "+€12" and "€12" do not read the same in
            // the corner of a screen.
            Some(gain) if gain >= 0.0 => format!("+{}", money(gain)),
            Some(gain) => money(gain),
            None => "—".to_owned(),
        },
        change_24h_pct: markup.escape(&format::percent(wallet.change_24h_pct)),
        positive: wallet.change_24h.is_some_and(|gain| gain >= 0.0),
        positions: wallet
            .positions
            .iter()
            .map(|position| HoldingRow {
                symbol: markup.escape(&position.symbol.to_uppercase()),
                quantity: markup.escape(&format::quantity(position.quantity)),
                value: money(position.value),
                share: (position.share * 1000.0).round() / 1000.0,
                color: position.color.clone(),
            })
            .collect(),
    }
}

fn rows(quotes: &[Quote], active: usize, config: &Config, markup: Markup) -> Vec<CoinRow> {
    let decimals = config.decimals();
    quotes
        .iter()
        .enumerate()
        .map(|(index, quote)| {
            let badge = badge::of(&quote.symbol);
            CoinRow {
                symbol: markup.escape(&quote.symbol.to_uppercase()),
                name: markup.escape(&quote.name),
                price: markup.escape(&format::price(quote.price, &config.currency, decimals)),
                change_1h: markup.escape(&format::percent(quote.change_1h)),
                change_24h: markup.escape(&format::percent(quote.change_24h)),
                change_7d: markup.escape(&format::percent(quote.change_7d)),
                trend: quote.trend().class(),
                series: normalize(&quote.sparkline),
                active: index == active,
                icon: quote.icon.clone().unwrap_or_default(),
                glyph: badge.glyph,
                color: badge.color,
                foreground: badge.foreground,
            }
        })
        .collect()
}

fn class(quote: &Quote, stale: bool, alerting: bool) -> String {
    let mut class = quote.trend().class().to_owned();
    if stale {
        class.push_str(" stale");
    }
    if alerting {
        class.push_str(" alert");
    }
    class
}

fn tooltip(
    quotes: &[Quote],
    active: usize,
    config: &Config,
    stale: bool,
    markup: Markup,
    wallet: Option<&portfolio::Portfolio>,
    triggers: &[crate::alerts::Trigger],
) -> String {
    let decimals = config.decimals();
    let mut lines = Vec::with_capacity(quotes.len().saturating_add(2));
    for (index, quote) in quotes.iter().enumerate() {
        let marker = if index == active { "▸" } else { " " };
        lines.push(format!(
            "{marker} {}  {}   1h {}  24h {}  7d {}",
            markup.bold(&markup.escape(&quote.symbol.to_uppercase())),
            markup.escape(&format::price(quote.price, &config.currency, decimals)),
            markup.escape(&format::percent(quote.change_1h)),
            markup.escape(&format::percent(quote.change_24h)),
            markup.escape(&format::percent(quote.change_7d)),
        ));
    }
    if let Some(wallet) = wallet {
        lines.push(String::new());
        let total = markup.escape(&format::price(wallet.total, &config.currency, decimals));
        let gain = markup.escape(&format::percent(wallet.change_24h_pct));
        lines.push(format!(
            "{}  {total}   24h {gain}",
            markup.bold("Portfolio")
        ));
    }
    for trigger in triggers {
        lines.push(String::new());
        lines.push(format!("⚠ {}", markup.escape(&trigger.title)));
    }
    if stale {
        lines.push(String::new());
        lines.push(markup.italic("stale data — network unavailable"));
    }
    lines.join("\n")
}

#[cfg(test)]
// A failing test has to stop dead; normalisation puts exactly 0.0 and 1.0 on
// the extremes, so the strict comparison is exact there.
#[allow(clippy::unwrap_used, clippy::panic, clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::model::CoinId;

    fn config() -> Config {
        Config::parse("coins = [\"bitcoin\", \"ethereum\"]").unwrap()
    }

    fn quote(symbol: &str, price: f64, change: Option<f64>) -> Quote {
        Quote {
            id: CoinId::parse("bitcoin").unwrap(),
            symbol: symbol.to_owned(),
            name: symbol.to_owned(),
            price,
            change_1h: Some(0.5),
            change_24h: change,
            change_7d: Some(-2.0),
            sparkline: vec![1.0, 2.0, 3.0, 2.0],
            image: None,
            market_cap: None,
            volume_24h: None,
            rank: None,
            ath: None,
            ath_date: None,
            icon: None,
        }
    }

    #[test]
    fn the_text_holds_symbol_price_and_change() {
        let module = render(
            &[quote("btc", 42_000.0, Some(1.5))],
            0,
            &config(),
            false,
            Markup::Pango,
            &std::collections::BTreeMap::new(),
        );
        assert!(module.text.contains("BTC"));
        assert!(module.text.contains("42"));
        assert!(module.text.contains("1.5"), "text: {}", module.text);
    }

    #[test]
    fn the_class_follows_the_trend() {
        let cfg = config();
        assert_eq!(
            render(
                &[quote("btc", 1.0, Some(1.0))],
                0,
                &cfg,
                false,
                Markup::Pango,
                &std::collections::BTreeMap::new(),
            )
            .class,
            "up"
        );
        assert_eq!(
            render(
                &[quote("btc", 1.0, Some(-1.0))],
                0,
                &cfg,
                false,
                Markup::Pango,
                &std::collections::BTreeMap::new(),
            )
            .class,
            "down"
        );
    }

    #[test]
    fn the_stale_state_joins_the_class_and_the_tooltip() {
        let module = render(
            &[quote("btc", 1.0, Some(1.0))],
            0,
            &config(),
            true,
            Markup::Pango,
            &std::collections::BTreeMap::new(),
        );
        assert!(module.class.contains("stale"));
        assert!(module.tooltip.contains("stale"));
    }

    #[test]
    fn an_empty_list_yields_an_error_module_not_a_panic() {
        let module = render(
            &[],
            0,
            &config(),
            false,
            Markup::Pango,
            &std::collections::BTreeMap::new(),
        );
        assert_eq!(module.class, "error");
    }

    #[test]
    fn an_out_of_bounds_index_falls_back_to_the_first_quote() {
        let module = render(
            &[quote("btc", 1.0, Some(1.0))],
            9,
            &config(),
            false,
            Markup::Pango,
            &std::collections::BTreeMap::new(),
        );
        assert!(module.text.contains("BTC"));
    }

    #[test]
    fn pango_markup_is_escaped() {
        let module = render(
            &[quote("<b>&x", 1.0, Some(1.0))],
            0,
            &config(),
            false,
            Markup::Pango,
            &std::collections::BTreeMap::new(),
        );
        assert!(!module.text.contains("<b>"));
        assert!(module.text.contains("&lt;"));
        assert!(module.text.contains("&amp;"));
    }

    #[test]
    fn the_tooltip_marks_the_active_quote() {
        let quotes = vec![quote("btc", 1.0, Some(1.0)), quote("eth", 2.0, Some(1.0))];
        let tooltip = render(
            &quotes,
            1,
            &config(),
            false,
            Markup::Pango,
            &std::collections::BTreeMap::new(),
        )
        .tooltip;
        let second = tooltip.lines().nth(1).unwrap();
        assert!(second.starts_with('▸'));
    }

    #[test]
    fn the_output_is_json_on_a_single_line() {
        let line = render(
            &[quote("btc", 1.0, Some(1.0))],
            0,
            &config(),
            false,
            Markup::Pango,
            &std::collections::BTreeMap::new(),
        )
        .to_line()
        .unwrap();
        assert_eq!(line.lines().count(), 1);
        assert!(line.starts_with('{'));
    }

    #[test]
    fn the_markup_free_mode_yields_no_tag_and_no_entity() {
        let quotes = vec![quote("<b>&x", 1.0, Some(1.0)), quote("eth", 2.0, Some(1.0))];
        let module = render(
            &quotes,
            0,
            &config(),
            true,
            Markup::None,
            &std::collections::BTreeMap::new(),
        );
        for field in [&module.text, &module.tooltip] {
            // The rendering adds no tag at all: neither <b> nor <i>.
            assert!(!field.contains("<b>"), "leftover tag: {field}");
            assert!(!field.contains("<i>"), "leftover tag: {field}");
            assert!(!field.contains("&amp;"), "leftover entity: {field}");
        }
        // The network data comes out literal: it is up to the consumer to
        // display it as plain text (Quickshell: `textFormat: Text.PlainText`).
        assert!(module.text.contains("<B>&X"), "text: {}", module.text);
    }

    #[test]
    fn the_markup_free_mode_still_neutralises_controls() {
        let module = render(
            &[quote("a\nb", 1.0, Some(1.0))],
            0,
            &config(),
            false,
            Markup::None,
            &std::collections::BTreeMap::new(),
        );
        assert!(!module.text.contains('\n'));
    }

    #[test]
    fn the_markup_parses_from_a_string() {
        assert_eq!("pango".parse(), Ok(Markup::Pango));
        assert_eq!("none".parse(), Ok(Markup::None));
        assert!("bold".parse::<Markup>().is_err());
    }

    #[test]
    fn the_structured_fields_come_with_the_text() {
        let module = render(
            &[quote("btc", 42_000.0, Some(1.5))],
            0,
            &config(),
            false,
            Markup::None,
            &std::collections::BTreeMap::new(),
        );
        assert_eq!(module.symbol, "BTC");
        assert!(module.price.contains("42"));
        assert!(module.percent.contains("1.5"));
        assert!(!module.sparkline.is_empty());
        // The text stays the concatenation of the fields: no divergence is
        // possible between the two representations.
        assert_eq!(
            module.text,
            format!(
                "{} {} {} {}",
                module.symbol, module.price, module.sparkline, module.percent
            )
        );
    }

    #[test]
    fn every_quote_yields_a_structured_row() {
        let quotes = vec![
            quote("btc", 42_000.0, Some(1.5)),
            quote("eth", 2_000.0, Some(-1.0)),
        ];
        let module = render(
            &quotes,
            1,
            &config(),
            false,
            Markup::None,
            &std::collections::BTreeMap::new(),
        );
        let [first, second] = module.coins.as_slice() else {
            panic!("expected two quotes, got {}", module.coins.len());
        };
        assert_eq!(first.symbol, "BTC");
        assert!(!first.active);
        assert!(second.active);
        assert_eq!(second.trend, "down");
        assert_eq!(module.trend, "down");
    }

    #[test]
    fn the_published_series_fit_within_zero_and_one() {
        let module = render(
            &[quote("btc", 1.0, Some(1.0))],
            0,
            &config(),
            false,
            Markup::None,
            &std::collections::BTreeMap::new(),
        );
        let Some(row) = module.coins.first() else {
            panic!("expected one quote");
        };
        let series = &row.series;
        assert!(!series.is_empty());
        assert!(series.iter().all(|value| (0.0..=1.0).contains(value)));
        // The extremes touch the bounds: the curve fills the whole height.
        assert!(series.iter().copied().any(|value| value == 0.0));
        assert!(series.iter().copied().any(|value| value == 1.0));
    }

    #[test]
    fn a_flat_series_sits_in_the_middle() {
        assert_eq!(normalize(&[3.0, 3.0, 3.0]), vec![0.5, 0.5, 0.5]);
        assert!(normalize(&[]).is_empty());
        assert!(normalize(&[f64::NAN]).is_empty());
    }

    #[test]
    fn a_long_series_is_reduced() {
        let long: Vec<f64> = (0..500).map(f64::from).collect();
        assert_eq!(normalize(&long).len(), SERIES_POINTS);
    }

    #[test]
    fn control_characters_are_neutralised() {
        assert_eq!(escape_pango("a\nb"), "a b");
    }
}

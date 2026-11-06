use serde::Serialize;

/// One quote, split into display-ready fields.
///
/// Bars able to compose their own layout (Quickshell, Ags, Eww) use them to
/// draw an aligned list and a curve, where `tooltip` would only hand them a
/// block of text to parse.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CoinRow {
    /// Ticker symbol.
    pub symbol: String,
    /// Human name.
    pub name: String,
    /// Price, formatted with its currency.
    pub price: String,
    /// Hourly change, formatted, or the missing-value marker.
    pub change_1h: String,
    /// 24 h change, formatted with its sign.
    pub change_24h: String,
    /// Seven-day change, formatted with its sign.
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
    /// Ticker symbol of the position.
    pub symbol: String,
    /// Quantity held, as read from the watched addresses.
    pub quantity: String,
    /// Value of the line in the quote currency.
    pub value: String,
    /// Share of the portfolio, within [0, 1]: the consumer turns it into an
    /// arc, a bar or a percentage, depending on the room it has.
    pub share: f64,
    /// Brand colour of the allocation ring, `#rrggbb`.
    pub color: String,
}

/// Valued portfolio, absent when no balance is known.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PortfolioSummary {
    /// Total value, formatted with its currency.
    pub total: String,
    /// Gain or loss over 24 h, already formatted with its sign.
    pub change_24h: String,
    /// 24 h change in percent, formatted with its sign.
    pub change_24h_pct: String,
    /// True when the portfolio is up over 24 h.
    pub positive: bool,
    /// Lines of the portfolio, by decreasing value.
    pub positions: Vec<HoldingRow>,
}

/// An alert currently crossed.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AlertRow {
    /// Ticker symbol of the coin that crossed the condition.
    pub symbol: String,
    /// "above", "below" or "change".
    pub kind: &'static str,
    /// Notification title, short and readable at a glance.
    pub title: String,
    /// Notification body, with the price and the threshold.
    pub body: String,
}

/// Payload of a Waybar module.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Module {
    /// Text shown by the bar, with its markup.
    pub text: String,
    /// Multi-line tooltip, with the markup of the bar.
    pub tooltip: String,
    /// CSS class of the module, following the trend of the active quote.
    pub class: String,
    /// Extra fields Waybar ignores, useful to bars able to compose their own
    /// layout (Quickshell in vertical mode, where the full text would not fit
    /// the available width).
    pub symbol: String,
    /// Price of the active quote, formatted with its currency.
    pub price: String,
    /// 24 h change of the active quote, formatted with its sign.
    pub percent: String,
    /// Sparkline of the active quote, drawn with block characters.
    pub sparkline: String,
    /// Trend of the active quote: "up", "down", "flat" or "error".
    pub trend: &'static str,
    /// Data served from the cache, network unavailable.
    pub stale: bool,
    /// Logo and fallback badge of the active quote.
    pub icon: String,
    /// Glyph of the fallback badge of the active quote.
    pub glyph: String,
    /// Brand colour of the badge of the active quote, `#rrggbb`.
    pub color: String,
    /// Colour readable on top of `color`, `#rrggbb`.
    pub foreground: String,
    /// Every watched quote, in configuration order.
    pub coins: Vec<CoinRow>,
    /// Alerts crossed right now. Empty most of the time: that is what lets the
    /// bar light an indicator up without recomputing anything.
    pub alerts: Vec<AlertRow>,
    /// Valued portfolio, `null` when no watched address holds a balance.
    pub portfolio: Option<PortfolioSummary>,
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

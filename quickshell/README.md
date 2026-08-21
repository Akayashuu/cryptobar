# Quickshell integration

Bar widget for [Quickshell](https://quickshell.outfoxxed.me/), designed for the
`ii` configuration of [dots-hyprland](https://github.com/end-4/dots-hyprland).
Both the horizontal and the vertical bar are supported.

## Installation

```bash
./quickshell/install.sh          # into ~/.config/quickshell/ii
QS_DIR=/another/path ./quickshell/install.sh
qs -c ii kill && qs -c ii -d     # restart to load the widget
```

The script is idempotent: it looks for its own `// cryptobar` marker and never
wires anything twice. It backs up every upstream file it modifies as
`<file>.bak-cryptobar` before its first change.

> A dots-hyprland update overwrites `Config.qml`, `BarContent.qml` and
> `VerticalBarContent.qml`. Run the script again afterwards; the widgets
> themselves live in files that do not belong to upstream.

## Files

| File | Role |
| --- | --- |
| `services/Crypto.qml` | Singleton: a single `cryptobar bar --watch` for the whole session, whatever the number of screens. Exposes `text`, `symbol`, `price`, `percent`, `sparkline`, `tooltip`, `trend`, `stale`, `alerts` and `portfolio`. |
| `modules/ii/bar/CryptoSparkline.qml` | Canvas for the seven-day curve, a solid line above a gradient fill. |
| `modules/ii/bar/CryptoWidget.qml` | Horizontal bar: logo, symbol, price, curve and change, background tinted by the trend, alert badge and a transition when the coin changes. |
| `modules/ii/bar/CryptoWidgetPopup.qml` | Panel shared by both orientations: one row per cryptocurrency, plus the crossed alerts and the valued portfolio. |
| `modules/ii/verticalBar/VerticalCryptoWidget.qml` | Vertical bar: logo, price and change, deliberately stripped down — the available width does not forgive clutter. |

## Interactions

| Gesture | Effect |
| --- | --- |
| Scroll up / down | Next / previous coin |
| Left click | Detailed window (GTK) |
| Right click | Chart in a terminal |
| Middle click | Forced refresh |
| Hover | Every quote, with 1 h / 24 h / 7 d |

After an action the widget reads the current value back immediately instead of
waiting for the next interval.

## Options

In `~/.config/illogical-impulse/config.json`:

```json
{ "bar": { "crypto": { "enable": true, "binary": "" } } }
```

An empty `binary` points at `~/.local/bin/cryptobar`, the default install
location. An absolute path is required: Quickshell does not inherit the `PATH`
of an interactive shell.

The watched coins, the currency, the interval and the sparkline width are set
in `~/.config/cryptobar/config.toml` (`cryptobar config init`).

## Other bars

The widget consumes the output of `cryptobar bar --markup none`: one JSON line
per interval, with no Pango markup. Ags and Eww can use it as it is. Waybar
uses the default mode, `--markup pango`.

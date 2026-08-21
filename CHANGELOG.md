# Changelog

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
semantic versioning.

## [0.2.1] — 2026-08-22

### Changed

- The window is navigated through tabs — Prices, Portfolio, Alerts, Settings —
  instead of a single column where the portfolio and the settings fell below
  the fold, with nothing to say they had to be scrolled to.

### Fixed

- The window stylesheet is loaded above `~/.config/gtk-4.0/gtk.css`: a desktop
  theme that repaints every button no longer decides how the tabs look.

## [0.2.0] — 2026-08-22

### Added

- Read-only portfolio watched by **public address**: balances read from
  mempool.space (Bitcoin), `ethereum-rpc.publicnode.com` (Ethereum) and
  `api.mainnet-beta.solana.com` (Solana), with no API key. A private key or a
  recovery phrase is rejected before anything is written to disk.
- `cryptobar portfolio` command: valuation, shares and 24 h gain in the
  terminal.
- Price alerts (`[[alerts]]`) on an upper threshold, a lower threshold and the
  24 h change amplitude, re-arming when the price comes back under the
  threshold, with a desktop notification from `cryptobar bar --watch`.
- `cryptobar select <N>` command, to display a given cryptocurrency.
- Complete GTK4 window: cryptocurrency search with logos, curve over five
  ranges, PNG and CSV export, list of the watched cryptocurrencies, portfolio,
  alerts and grouped settings — the whole configuration is editable from the
  window, preserving the comments and the ordering of the TOML file.
- Official cryptocurrency logos, cached on disk, in the window as well as in
  the Quickshell widgets.
- Quickshell integration for dots-hyprland (`quickshell/`): singleton service
  shared by every screen, horizontal widget, vertical widget, tooltip,
  idempotent installer.
- `cryptobar bar --markup none`: output without Pango markup, for the bars that
  display plain text.
- Extended JSON line: `symbol`, `price`, `percent`, `sparkline`, `trend`,
  `stale`, `icon`, `glyph`, `color`, `foreground`, `coins` (every watched
  cryptocurrency with its series normalised within `[0, 1]`), `alerts` and
  `portfolio`.

### Changed

- The Quickshell widgets draw the curve on a canvas and show a native panel
  instead of a monospace text block. The vertical bar is reduced to the logo,
  the price and the change.

## [0.1.0] — 2026-08-21

First release.

### Added

- Waybar module: Unicode sparkline, scroll to cycle, detailed tooltip with the
  1 h / 24 h / 7 d changes, CSS classes following the trend.
- Braille chart in the terminal, sized to its actual dimensions, with a
  labelled ordinate axis.
- GTK4 window with a vector curve, behind an optional feature enabled by
  default.
- CoinGecko id search (`cryptobar coins <term>`).
- Configuration management (`init`, `path`, `check`) with strict validation and
  explicit messages.
- Disk cache with a TTL, atomic writes, advisory lock and exponential backoff;
  degrades to stale data rather than to an empty module.
- Instant Waybar refresh through `SIGRTMIN+n`.

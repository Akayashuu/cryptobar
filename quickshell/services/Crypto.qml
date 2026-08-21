pragma Singleton

import qs.modules.common
import QtQuick
import Quickshell
import Quickshell.Io

/**
 * Cryptocurrency prices, served by the `cryptobar` binary.
 *
 * A single process serves the whole session: the service is a singleton, so
 * several screens or several widgets multiply neither the network calls nor
 * the memory footprint. The binary runs in `--watch` mode and emits one JSON
 * line per interval, as set in ~/.config/cryptobar/config.toml; no QML timer
 * wakes it up.
 */
Singleton {
    id: root

    /// Path to the binary. Quickshell does not inherit the interactive PATH:
    /// a bare `cryptobar` fails when the binary lives in ~/.local/bin, which is
    /// where `install.sh` puts it by default.
    readonly property string binary: {
        const configured = Config.options.bar.crypto?.binary ?? "";
        return configured.length > 0 ? configured : `${Quickshell.env("HOME")}/.local/bin/cryptobar`;
    }

    /// Full line, ready to display: "BTC €66,007 ▁▂▅█ +6.0 %".
    property string text: "—"
    /// Every watched quote, one per line, with 1 h / 24 h / 7 d.
    property string tooltip: qsTr("Waiting for prices…")
    /// Separate fields, for layouts too narrow for `text`.
    property string symbol: "—"
    property string price: ""
    property string percent: ""
    property string sparkline: ""
    /// 24 h trend: "up", "down", "flat" or "error".
    property string trend: ""
    /// Data served from the cache: the network is unavailable.
    property bool stale: false
    /// Series of the active coin, normalised within [0, 1].
    property var series: []
    /// The full active quote (symbol, logo, glyph, colours, series), or `null`
    /// until a first line arrives. `icon` is already taken by the trend
    /// function: widgets hand this object to CryptoCoinIcon.
    property var active: null
    /// Every watched quote: { symbol, name, price, change_1h,
    /// change_24h, change_7d, trend, series, active, icon, glyph, color,
    /// foreground }.
    property var coins: []
    /// Thresholds crossed right now: { symbol, kind, title, body }. Empty most
    /// of the time — this is what lights up the badge in the bar.
    property var alerts: []
    /// Valued portfolio, or `null` when no public address is watched:
    /// { total, change_24h, change_24h_pct, positive, positions }.
    property var portfolio: null

    /// True until a first line arrives.
    readonly property bool loading: root.trend.length === 0

    /// True when at least one threshold is currently crossed.
    readonly property bool alerting: root.alerts.length > 0

    signal failed(string reason)

    // The binary guarantees a valid JSON line; a truncated one (process killed
    // mid-write) must never break the bar.
    function ingest(line) {
        const trimmed = line.trim();
        if (trimmed.length === 0)
            return;
        let payload;
        try {
            payload = JSON.parse(trimmed);
        } catch (e) {
            return; // Unusable line: keep the last known value.
        }
        root.text = payload.text ?? "—";
        root.tooltip = payload.tooltip ?? "";
        root.symbol = payload.symbol ?? "—";
        root.price = payload.price ?? "";
        root.percent = payload.percent ?? "";
        root.sparkline = payload.sparkline ?? "";
        root.coins = payload.coins ?? [];
        root.stale = payload.stale ?? false;
        root.trend = payload.trend ?? "flat";
        root.alerts = payload.alerts ?? [];
        root.portfolio = payload.portfolio ?? null;
        const active = root.coins.find(coin => coin.active) ?? null;
        root.active = active;
        root.series = active?.series ?? [];
    }

    /// Runs a subcommand (`next`, `refresh`, `popup`…) then reads the current
    /// value back, for immediate feedback without waiting for the interval.
    function run(argv) {
        actionProc.command = [root.binary].concat(argv);
        actionProc.running = true;
    }

    function next() { root.run(["next"]); }
    function prev() { root.run(["prev"]); }
    function refresh() { root.run(["refresh"]); }
    function openPopup() { root.run(["popup"]); }
    function openChart() { root.run(["chart", "--window"]); }
    /// Makes the coin at the given index in the configured list active.
    function select(index) { root.run(["select", String(index)]); }

    /// Material arrow matching a trend.
    function icon(trend) {
        return trend === "up" ? "trending_up" : trend === "down" ? "trending_down" : "trending_flat";
    }

    /// Colour matching a trend, from the theme palette.
    function color(trend) {
        return trend === "up" ? Appearance.m3colors.m3primary : trend === "down" ? Appearance.colors.colError : Appearance.colors.colOnSurfaceVariant;
    }

    Process {
        id: watchProc
        command: [root.binary, "bar", "--watch", "--markup", "none"]
        running: true
        stdout: SplitParser {
            splitMarker: "\n"
            onRead: data => root.ingest(data)
        }
        // Network down, binary reinstalled, package upgraded: restart without
        // hammering the system.
        onExited: (exitCode, exitStatus) => {
            if (exitCode !== 0)
                root.failed(qsTr("cryptobar stopped (code %1)").arg(exitCode));
            restartTimer.restart();
        }
    }

    Timer {
        id: restartTimer
        interval: 5000
        onTriggered: watchProc.running = true
    }

    Process {
        id: actionProc
        onExited: readProc.running = true
    }

    Process {
        id: readProc
        command: [root.binary, "bar", "--markup", "none"]
        stdout: SplitParser {
            splitMarker: "\n"
            onRead: data => root.ingest(data)
        }
    }
}

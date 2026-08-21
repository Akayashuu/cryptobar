pragma Singleton

import qs.modules.common
import QtQuick
import Quickshell
import Quickshell.Io

/**
 * Cours des cryptomonnaies, fournis par le binaire `cryptobar`.
 *
 * Un seul processus dessert toute la session : le service est un singleton,
 * donc plusieurs écrans ou plusieurs widgets ne multiplient ni les appels
 * réseau ni la consommation mémoire. Le binaire tourne en mode `--watch` et
 * émet une ligne de JSON à chaque intervalle défini dans
 * ~/.config/cryptobar/config.toml ; aucun timer QML ne le réveille.
 */
Singleton {
    id: root

    /// Chemin du binaire. Quickshell n'hérite pas du PATH interactif : un
    /// simple « cryptobar » échoue si le binaire vit dans ~/.local/bin, qui
    /// est l'emplacement d'installation par défaut de `install.sh`.
    readonly property string binary: {
        const configured = Config.options.bar.crypto?.binary ?? "";
        return configured.length > 0 ? configured : `${Quickshell.env("HOME")}/.local/bin/cryptobar`;
    }

    /// Ligne complète prête à afficher : « BTC 66 007 € ▁▂▅█ +6,0 % ».
    property string text: "—"
    /// Toutes les cotations suivies, une par ligne, avec 1 h / 24 h / 7 j.
    property string tooltip: qsTr("En attente des cours…")
    /// Champs séparés, pour les mises en page trop étroites pour `text`.
    property string symbol: "—"
    property string price: ""
    property string percent: ""
    property string sparkline: ""
    /// Tendance sur 24 h : "up", "down", "flat", ou "error".
    property string trend: ""
    /// Données servies depuis le cache : le réseau est indisponible.
    property bool stale: false
    /// Série de la pièce active, normalisée dans [0, 1].
    property var series: []
    /// Cotation active complète (symbole, logo, glyphe, couleurs, série), ou
    /// `null` tant qu'aucune ligne n'a été reçue. `icon` est déjà pris par la
    /// fonction de tendance : les widgets passent cet objet à CryptoCoinIcon.
    property var active: null
    /// Toutes les cotations suivies : { symbol, name, price, change_1h,
    /// change_24h, change_7d, trend, series, active, icon, glyph, color,
    /// foreground }.
    property var coins: []
    /// Alertes franchies à cet instant : { symbol, kind, title, body }. Vide
    /// la plupart du temps — c'est ce qui allume la pastille de la barre.
    property var alerts: []
    /// Portefeuille valorisé, ou `null` si aucune adresse publique n'est
    /// suivie : { total, change_24h, change_24h_pct, positive, positions }.
    property var portfolio: null

    /// Vrai tant qu'aucune ligne n'a été reçue.
    readonly property bool loading: root.trend.length === 0

    /// Vrai si au moins un seuil est franchi en ce moment.
    readonly property bool alerting: root.alerts.length > 0

    signal failed(string reason)

    // Le binaire garantit une ligne de JSON valide ; une ligne tronquée
    // (processus tué en pleine écriture) ne doit jamais casser la barre.
    function ingest(line) {
        const trimmed = line.trim();
        if (trimmed.length === 0)
            return;
        let payload;
        try {
            payload = JSON.parse(trimmed);
        } catch (e) {
            return; // Ligne inexploitable : on garde la dernière valeur connue.
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

    /// Exécute une sous-commande (`next`, `refresh`, `popup`…) puis relit la
    /// valeur courante, pour un retour immédiat sans attendre l'intervalle.
    function run(argv) {
        actionProc.command = [root.binary].concat(argv);
        actionProc.running = true;
    }

    function next() { root.run(["next"]); }
    function prev() { root.run(["prev"]); }
    function refresh() { root.run(["refresh"]); }
    function openPopup() { root.run(["popup"]); }
    function openChart() { root.run(["chart", "--window"]); }
    /// Rend active la pièce d'indice donné dans la liste configurée.
    function select(index) { root.run(["select", String(index)]); }

    /// Flèche Material correspondant à une tendance.
    function icon(trend) {
        return trend === "up" ? "trending_up" : trend === "down" ? "trending_down" : "trending_flat";
    }

    /// Couleur correspondant à une tendance, dans la palette du thème.
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
        // Réseau coupé, binaire réinstallé, mise à jour du paquet : on relance
        // sans marteler le système.
        onExited: (exitCode, exitStatus) => {
            if (exitCode !== 0)
                root.failed(qsTr("cryptobar s'est arrêté (code %1)").arg(exitCode));
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

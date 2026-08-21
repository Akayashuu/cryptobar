#!/usr/bin/env bash
# Installs the cryptobar Quickshell integration into a dots-hyprland ("ii")
# configuration.
#
# The widget files are copied as they are. The three upstream files —
# Config.qml, BarContent.qml, VerticalBarContent.qml — receive an idempotent
# addition: the script detects its own marker and applies it only once. A
# dots-hyprland update overwrites them: run this script again afterwards.
set -euo pipefail

QS_DIR="${QS_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/quickshell/ii}"
SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MARK="// cryptobar"

die() { printf '%s\n' "$*" >&2; exit 1; }

[ -d "$QS_DIR" ] || die "Quickshell configuration not found: $QS_DIR"
[ -f "$QS_DIR/modules/common/Config.qml" ] || die "$QS_DIR does not look like a dots-hyprland \"ii\" configuration."

backup() { cp -n "$1" "$1.bak-cryptobar" 2>/dev/null || true; }

install -Dm644 "$SRC/services/Crypto.qml" "$QS_DIR/services/Crypto.qml"
install -Dm644 "$SRC/modules/ii/bar/CryptoWidget.qml" "$QS_DIR/modules/ii/bar/CryptoWidget.qml"
install -Dm644 "$SRC/modules/ii/bar/CryptoCoinIcon.qml" "$QS_DIR/modules/ii/bar/CryptoCoinIcon.qml"
install -Dm644 "$SRC/modules/ii/bar/CryptoSparkline.qml" "$QS_DIR/modules/ii/bar/CryptoSparkline.qml"
install -Dm644 "$SRC/modules/ii/bar/CryptoWidgetPopup.qml" "$QS_DIR/modules/ii/bar/CryptoWidgetPopup.qml"
install -Dm644 "$SRC/modules/ii/verticalBar/VerticalCryptoWidget.qml" "$QS_DIR/modules/ii/verticalBar/VerticalCryptoWidget.qml"
echo "Widgets installed."

patch_file() {
    local file="$1" anchor="$2" addition="$3"
    [ -f "$file" ] || die "Missing file: $file"
    if grep -qF "$MARK" "$file"; then
        echo "Already wired: ${file#"$QS_DIR"/}"
        return
    fi
    grep -qF "$anchor" "$file" || die "Anchor point not found in ${file#"$QS_DIR"/}; wire it by hand (see quickshell/README.md)."
    backup "$file"
    ANCHOR="$anchor" ADDITION="$addition" python3 - "$file" <<'PY'
import os, sys
path = sys.argv[1]
anchor, addition = os.environ["ANCHOR"], os.environ["ADDITION"]
with open(path, encoding="utf-8") as handle:
    text = handle.read()
# A single occurrence: the anchor is picked to be unique in each upstream file.
with open(path, "w", encoding="utf-8") as handle:
    handle.write(text.replace(anchor, anchor + addition, 1))
PY
    echo "Wired: ${file#"$QS_DIR"/}"
}

# This option is declared BEFORE its anchor: a unique case, handled apart.
if ! grep -qF "$MARK" "$QS_DIR/modules/common/Config.qml"; then
    backup "$QS_DIR/modules/common/Config.qml"
    python3 - "$QS_DIR/modules/common/Config.qml" <<'PY'
import sys
path = sys.argv[1]
anchor = "                property JsonObject indicators: JsonObject {"
addition = """                property JsonObject crypto: JsonObject { // cryptobar
                    property bool enable: true
                    property string binary: "" // Empty: ~/.local/bin/cryptobar
                }
"""
with open(path, encoding="utf-8") as handle:
    text = handle.read()
if anchor not in text:
    sys.exit("Anchor point not found in Config.qml")
with open(path, "w", encoding="utf-8") as handle:
    handle.write(text.replace(anchor, addition + anchor, 1))
PY
    echo "Wired: modules/common/Config.qml"
fi

patch_file "$QS_DIR/modules/ii/bar/BarContent.qml" \
'                sourceComponent: BarGroup {
                    WeatherBar {}
                }
            }' \
'

            // Cryptocurrency prices
            Loader { // cryptobar
                Layout.leftMargin: 4
                active: Config.options.bar.crypto?.enable ?? true

                sourceComponent: BarGroup {
                    CryptoWidget {
                        Layout.alignment: Qt.AlignVCenter
                    }
                }
            }'

patch_file "$QS_DIR/modules/ii/verticalBar/VerticalBarContent.qml" \
'            BatteryIndicator {
                visible: Battery.available
                Layout.fillWidth: true
                Layout.fillHeight: false
            }' \
'

            HorizontalBarSeparator {
                visible: cryptoLoader.active
            }

            // Cryptocurrency prices
            Loader { // cryptobar
                id: cryptoLoader
                active: Config.options.bar.crypto?.enable ?? true
                Layout.fillWidth: true
                Layout.fillHeight: false
                sourceComponent: VerticalCryptoWidget {}
            }'

echo
echo "Done. Restart Quickshell to load the widget:"
echo "    qs -c ii kill && qs -c ii -d"

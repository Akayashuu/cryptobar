#!/usr/bin/env bash
# Installe l'intégration Quickshell de cryptobar dans une configuration
# dots-hyprland (« ii »).
#
# Les trois fichiers du widget sont copiés tels quels. Les trois fichiers
# amont — Config.qml, BarContent.qml, VerticalBarContent.qml — reçoivent un
# ajout idempotent : le script détecte sa propre marque et ne l'applique
# qu'une fois. Une mise à jour de dots-hyprland les écrase : relancer ce
# script après coup.
set -euo pipefail

QS_DIR="${QS_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/quickshell/ii}"
SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MARK="// cryptobar"

die() { printf '%s\n' "$*" >&2; exit 1; }

[ -d "$QS_DIR" ] || die "Configuration Quickshell introuvable : $QS_DIR"
[ -f "$QS_DIR/modules/common/Config.qml" ] || die "$QS_DIR ne ressemble pas à une configuration dots-hyprland « ii »."

backup() { cp -n "$1" "$1.bak-cryptobar" 2>/dev/null || true; }

install -Dm644 "$SRC/services/Crypto.qml" "$QS_DIR/services/Crypto.qml"
install -Dm644 "$SRC/modules/ii/bar/CryptoWidget.qml" "$QS_DIR/modules/ii/bar/CryptoWidget.qml"
install -Dm644 "$SRC/modules/ii/bar/CryptoWidgetPopup.qml" "$QS_DIR/modules/ii/bar/CryptoWidgetPopup.qml"
install -Dm644 "$SRC/modules/ii/verticalBar/VerticalCryptoWidget.qml" "$QS_DIR/modules/ii/verticalBar/VerticalCryptoWidget.qml"
echo "Widgets installés."

patch_file() {
    local file="$1" anchor="$2" addition="$3"
    [ -f "$file" ] || die "Fichier absent : $file"
    if grep -qF "$MARK" "$file"; then
        echo "Déjà câblé : ${file#"$QS_DIR"/}"
        return
    fi
    grep -qF "$anchor" "$file" || die "Point d'ancrage introuvable dans ${file#"$QS_DIR"/} ; câblage manuel nécessaire (voir quickshell/README.md)."
    backup "$file"
    ANCHOR="$anchor" ADDITION="$addition" python3 - "$file" <<'PY'
import os, sys
path = sys.argv[1]
anchor, addition = os.environ["ANCHOR"], os.environ["ADDITION"]
with open(path, encoding="utf-8") as handle:
    text = handle.read()
# Une seule occurrence : l'ancre est choisie unique dans chaque fichier amont.
with open(path, "w", encoding="utf-8") as handle:
    handle.write(text.replace(anchor, anchor + addition, 1))
PY
    echo "Câblé : ${file#"$QS_DIR"/}"
}

# L'option se déclare AVANT son ancre : cas unique, traité à part.
if ! grep -qF "$MARK" "$QS_DIR/modules/common/Config.qml"; then
    backup "$QS_DIR/modules/common/Config.qml"
    python3 - "$QS_DIR/modules/common/Config.qml" <<'PY'
import sys
path = sys.argv[1]
anchor = "                property JsonObject indicators: JsonObject {"
addition = """                property JsonObject crypto: JsonObject { // cryptobar
                    property bool enable: true
                    property string binary: "" // Vide : ~/.local/bin/cryptobar
                }
"""
with open(path, encoding="utf-8") as handle:
    text = handle.read()
if anchor not in text:
    sys.exit("Point d'ancrage introuvable dans Config.qml")
with open(path, "w", encoding="utf-8") as handle:
    handle.write(text.replace(anchor, addition + anchor, 1))
PY
    echo "Câblé : modules/common/Config.qml"
fi

patch_file "$QS_DIR/modules/ii/bar/BarContent.qml" \
'                sourceComponent: BarGroup {
                    WeatherBar {}
                }
            }' \
'

            // Cours des cryptomonnaies
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

            // Cours des cryptomonnaies
            Loader { // cryptobar
                id: cryptoLoader
                active: Config.options.bar.crypto?.enable ?? true
                Layout.fillWidth: true
                Layout.fillHeight: false
                sourceComponent: VerticalCryptoWidget {}
            }'

echo
echo "Terminé. Redémarrez Quickshell pour charger le widget :"
echo "    qs -c ii kill && qs -c ii -d"

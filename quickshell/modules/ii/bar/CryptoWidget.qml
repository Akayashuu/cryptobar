import qs.modules.common
import qs.modules.common.widgets
import qs.services
import QtQuick
import QtQuick.Layouts

/**
 * Cours de la pièce active pour la barre horizontale.
 *
 * Molette : pièce suivante / précédente. Clic : fenêtre détaillée.
 * Clic droit : graphique dans un terminal. Clic milieu : rafraîchissement.
 */
Item {
    id: root

    readonly property color accent: Crypto.trend === "up" ? Appearance.m3colors.m3primary : Crypto.trend === "down" ? Appearance.colors.colError : Appearance.colors.colOnLayer1

    implicitWidth: rowLayout.implicitWidth
    implicitHeight: Appearance.sizes.barHeight

    RowLayout {
        id: rowLayout
        anchors.centerIn: parent
        spacing: 4

        MaterialSymbol {
            text: Crypto.stale ? "cloud_off" : "currency_bitcoin"
            iconSize: Appearance.font.pixelSize.larger
            color: root.accent
        }

        StyledText {
            // `--markup none` produit du texte brut : Qt ne doit pas y
            // chercher du HTML.
            textFormat: Text.PlainText
            font.pixelSize: Appearance.font.pixelSize.small
            color: root.accent
            opacity: Crypto.stale ? 0.6 : 1
            text: Crypto.text
        }
    }

    MouseArea {
        id: mouseArea
        anchors.fill: parent
        hoverEnabled: !Config.options.bar.tooltips.clickToShow
        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton

        onPressed: event => {
            if (event.button === Qt.LeftButton)
                Crypto.openPopup();
            else if (event.button === Qt.RightButton)
                Crypto.openChart();
            else if (event.button === Qt.MiddleButton)
                Crypto.refresh();
        }

        onWheel: event => {
            if (event.angleDelta.y > 0)
                Crypto.next();
            else
                Crypto.prev();
            event.accepted = true;
        }

        CryptoWidgetPopup {
            hoverTarget: mouseArea
        }
    }
}

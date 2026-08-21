import qs.modules.common
import qs.modules.common.widgets
import qs.services
import QtQuick
import QtQuick.Layouts
import qs.modules.ii.bar as Bar

/**
 * Variante pour la barre verticale : la largeur disponible ne permet pas
 * d'afficher la ligne complète, on empile donc symbole, prix et variation.
 */
Item {
    id: root

    readonly property color accent: Crypto.trend === "up" ? Appearance.m3colors.m3primary : Crypto.trend === "down" ? Appearance.colors.colError : Appearance.colors.colOnLayer1

    implicitWidth: Appearance.sizes.verticalBarWidth
    implicitHeight: column.implicitHeight

    Column {
        id: column
        anchors.centerIn: parent
        spacing: 0

        StyledText {
            anchors.horizontalCenter: parent.horizontalCenter
            font.pixelSize: Appearance.font.pixelSize.smaller
            color: root.accent
            text: Crypto.symbol
        }

        StyledText {
            anchors.horizontalCenter: parent.horizontalCenter
            width: root.width
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
            textFormat: Text.PlainText
            font.pixelSize: Appearance.font.pixelSize.smallest
            color: Appearance.colors.colOnLayer1
            opacity: Crypto.stale ? 0.6 : 1
            text: Crypto.price
        }

        StyledText {
            anchors.horizontalCenter: parent.horizontalCenter
            font.pixelSize: Appearance.font.pixelSize.smallest
            color: root.accent
            text: Crypto.percent
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

        Bar.CryptoWidgetPopup {
            hoverTarget: mouseArea
        }
    }
}

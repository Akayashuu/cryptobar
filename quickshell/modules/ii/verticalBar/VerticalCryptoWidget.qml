import qs.modules.common
import qs.modules.common.widgets
import qs.services
import QtQuick
import qs.modules.ii.bar as Bar

/**
 * Variante pour la barre verticale : la largeur disponible ne permet pas
 * d'afficher la ligne complète, on empile donc symbole, prix, courbe et
 * variation.
 */
Item {
    id: root

    property color accent: Crypto.stale ? Appearance.colors.colOnSurfaceVariant : Crypto.color(Crypto.trend)

    implicitWidth: Appearance.sizes.verticalBarWidth
    implicitHeight: column.implicitHeight

    Behavior on accent {
        animation: Appearance.animation.elementMoveFast.colorAnimation.createObject(this)
    }

    Column {
        id: column
        anchors.centerIn: parent
        spacing: 1

        Bar.CryptoCoinIcon {
            anchors.horizontalCenter: parent.horizontalCenter
            visible: !Crypto.stale && !Crypto.loading
            coin: Crypto.active
            size: Appearance.font.pixelSize.large
        }

        StyledText {
            anchors.horizontalCenter: parent.horizontalCenter
            font.pixelSize: Appearance.font.pixelSize.smaller
            font.weight: Font.DemiBold
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

        Bar.CryptoSparkline {
            anchors.horizontalCenter: parent.horizontalCenter
            visible: Crypto.series.length > 1
            implicitWidth: root.width - 8
            implicitHeight: 12
            series: Crypto.series
            stroke: root.accent
        }

        Row {
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: 1

            MaterialSymbol {
                anchors.verticalCenter: parent.verticalCenter
                iconSize: Appearance.font.pixelSize.smaller
                color: root.accent
                text: Crypto.stale ? "cloud_off" : Crypto.icon(Crypto.trend)
            }

            StyledText {
                anchors.verticalCenter: parent.verticalCenter
                textFormat: Text.PlainText
                font.pixelSize: Appearance.font.pixelSize.smallest
                color: root.accent
                text: Crypto.percent
            }
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

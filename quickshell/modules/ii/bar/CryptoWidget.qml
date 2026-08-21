import qs.modules.common
import qs.modules.common.widgets
import qs.services
import QtQuick
import QtQuick.Layouts

/**
 * Cotation active dans la barre horizontale : pastille de tendance, symbole,
 * prix, courbe sur sept jours et variation sur 24 h.
 */
Item {
    id: root

    property color accent: Crypto.stale ? Appearance.colors.colOnSurfaceVariant : Crypto.color(Crypto.trend)

    implicitWidth: row.implicitWidth
    implicitHeight: Appearance.sizes.barHeight

    Behavior on accent {
        animation: Appearance.animation.elementMoveFast.colorAnimation.createObject(this)
    }

    RowLayout {
        id: row
        anchors.centerIn: parent
        spacing: 5

        // Le logo officiel remplace l'icône générique dès qu'il est en cache ;
        // l'icône ne reste que pour les deux états qui n'ont pas de pièce à
        // montrer : hors ligne et premier chargement.
        Item {
            Layout.alignment: Qt.AlignVCenter
            implicitWidth: Appearance.font.pixelSize.larger
            implicitHeight: Appearance.font.pixelSize.larger

            MaterialSymbol {
                anchors.centerIn: parent
                visible: Crypto.stale || Crypto.loading
                iconSize: Appearance.font.pixelSize.larger
                color: root.accent
                text: Crypto.stale ? "cloud_off" : "sync"

                RotationAnimator on rotation {
                    running: Crypto.loading && !Crypto.stale
                    from: 0
                    to: 360
                    duration: 1200
                    loops: Animation.Infinite
                    onRunningChanged: if (!running) target.rotation = 0
                }
            }

            CryptoCoinIcon {
                anchors.centerIn: parent
                visible: !Crypto.stale && !Crypto.loading
                coin: Crypto.active
                size: Appearance.font.pixelSize.larger
            }
        }

        StyledText {
            Layout.alignment: Qt.AlignVCenter
            font.pixelSize: Appearance.font.pixelSize.smaller
            font.weight: Font.DemiBold
            color: root.accent
            text: Crypto.symbol
        }

        StyledText {
            Layout.alignment: Qt.AlignVCenter
            textFormat: Text.PlainText
            font.pixelSize: Appearance.font.pixelSize.small
            color: Appearance.colors.colOnLayer1
            opacity: Crypto.stale ? 0.6 : 1
            text: Crypto.price
        }

        CryptoSparkline {
            Layout.alignment: Qt.AlignVCenter
            visible: Crypto.series.length > 1
            implicitWidth: 34
            implicitHeight: Appearance.font.pixelSize.large
            series: Crypto.series
            stroke: root.accent
        }

        MaterialSymbol {
            Layout.alignment: Qt.AlignVCenter
            iconSize: Appearance.font.pixelSize.normal
            color: root.accent
            text: Crypto.icon(Crypto.trend)
        }

        StyledText {
            Layout.alignment: Qt.AlignVCenter
            textFormat: Text.PlainText
            font.pixelSize: Appearance.font.pixelSize.smaller
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

        CryptoWidgetPopup {
            hoverTarget: mouseArea
        }
    }
}

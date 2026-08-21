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

    implicitWidth: row.implicitWidth + 16
    implicitHeight: Appearance.sizes.barHeight

    Behavior on accent {
        animation: Appearance.animation.elementMoveFast.colorAnimation.createObject(this)
    }

    // Fond teinté par la tendance : assez léger pour rester lisible sur
    // n'importe quel fond de barre, assez net pour se voir du coin de l'œil.
    Rectangle {
        anchors.fill: parent
        anchors.topMargin: 3
        anchors.bottomMargin: 3
        radius: Appearance.rounding.small
        color: Qt.rgba(root.accent.r, root.accent.g, root.accent.b, Crypto.stale || Crypto.loading ? 0.06 : 0.14)

        Behavior on color {
            animation: Appearance.animation.elementMoveFast.colorAnimation.createObject(this)
        }
    }

    // Changement de pièce : la ligne glisse et se fond plutôt que de sauter,
    // ce qui laisse à l'œil le temps de suivre le symbole qui change.
    Connections {
        target: Crypto
        function onSymbolChanged() {
            swap.restart();
        }
    }

    SequentialAnimation {
        id: swap
        PropertyAction { target: row; property: "opacity"; value: 0 }
        PropertyAction { target: row; property: "anchors.verticalCenterOffset"; value: 6 }
        ParallelAnimation {
            NumberAnimation { target: row; property: "opacity"; to: 1; duration: 180; easing.type: Easing.OutCubic }
            NumberAnimation { target: row; property: "anchors.verticalCenterOffset"; to: 0; duration: 180; easing.type: Easing.OutCubic }
        }
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

            // Pastille d'alerte : un seuil est franchi en ce moment. Elle bat
            // lentement, le temps qu'on la remarque, sans happer le regard.
            Rectangle {
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.rightMargin: -1
                anchors.topMargin: -1
                visible: Crypto.alerting
                implicitWidth: 7
                implicitHeight: 7
                radius: height / 2
                color: Appearance.colors.colError
                border.width: 1
                border.color: Appearance.colors.colLayer1

                SequentialAnimation on opacity {
                    running: Crypto.alerting
                    loops: Animation.Infinite
                    NumberAnimation { to: 0.35; duration: 900; easing.type: Easing.InOutSine }
                    NumberAnimation { to: 1; duration: 900; easing.type: Easing.InOutSine }
                }
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

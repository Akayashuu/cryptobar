import qs.modules.common
import qs.modules.common.widgets
import qs.services
import QtQuick
import qs.modules.ii.bar as Bar

/**
 * Variant for the vertical bar. Forty pixels of width cannot carry a full
 * row: only the essentials remain — the logo, which already says which coin
 * this is, the price, and the 24 h change. The symbol, the curve and the
 * tinted background stay with the horizontal bar and the tooltip, where they
 * have the room to be readable.
 */
Item {
    id: root

    property color accent: Crypto.stale ? Appearance.colors.colOnSurfaceVariant : Crypto.color(Crypto.trend)

    implicitWidth: Appearance.sizes.verticalBarWidth
    implicitHeight: column.implicitHeight

    Behavior on accent {
        animation: Appearance.animation.elementMoveFast.colorAnimation.createObject(this)
    }

    Connections {
        target: Crypto
        function onSymbolChanged() {
            swap.restart();
        }
    }

    SequentialAnimation {
        id: swap
        PropertyAction { target: column; property: "opacity"; value: 0 }
        NumberAnimation { target: column; property: "opacity"; to: 1; duration: 180; easing.type: Easing.OutCubic }
    }

    Column {
        id: column
        anchors.centerIn: parent
        spacing: 1

        Item {
            anchors.horizontalCenter: parent.horizontalCenter
            implicitWidth: Appearance.font.pixelSize.large
            implicitHeight: Appearance.font.pixelSize.large

            Bar.CryptoCoinIcon {
                anchors.centerIn: parent
                visible: !Crypto.stale && !Crypto.loading
                coin: Crypto.active
                size: Appearance.font.pixelSize.large
            }

            // Alert badge: a threshold is crossed right now.
            Rectangle {
                anchors.right: parent.right
                anchors.top: parent.top
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

        Row {
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: 1

            MaterialSymbol {
                anchors.verticalCenter: parent.verticalCenter
                // The arrow only signals the offline cache: the sign of the
                // percentage already tells the direction of the change.
                visible: Crypto.stale
                iconSize: Appearance.font.pixelSize.smaller
                color: root.accent
                text: "cloud_off"
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

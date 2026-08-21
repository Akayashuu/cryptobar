import qs.modules.common
import qs.modules.common.widgets
import QtQuick

/**
 * Visual identity of a cryptocurrency: its official logo as soon as it is
 * cached, otherwise a coloured badge carrying its glyph.
 *
 * The fallback is not only a safety net on first start: it also covers the
 * currencies for which the source publishes no image.
 */
Item {
    id: root

    /// One entry of `Crypto.coins`.
    property var coin: null
    property real size: 16

    implicitWidth: root.size
    implicitHeight: root.size

    Rectangle {
        anchors.fill: parent
        radius: width / 2
        visible: image.status !== Image.Ready
        color: root.coin?.color ?? Appearance.colors.colOnSurfaceVariant

        StyledText {
            anchors.centerIn: parent
            font.pixelSize: Math.round(root.size * 0.62)
            font.weight: Font.Bold
            color: root.coin?.foreground ?? Appearance.colors.colOnLayer1
            text: root.coin?.glyph ?? ""
        }
    }

    Image {
        id: image
        anchors.fill: parent
        // Decoding at display size avoids keeping a 250 px image in memory for
        // a 16 px render.
        sourceSize.width: Math.round(root.size * 2)
        sourceSize.height: Math.round(root.size * 2)
        fillMode: Image.PreserveAspectFit
        asynchronous: true
        smooth: true
        cache: true
        source: (root.coin?.icon ?? "") === "" ? "" : `file://${root.coin.icon}`
    }
}

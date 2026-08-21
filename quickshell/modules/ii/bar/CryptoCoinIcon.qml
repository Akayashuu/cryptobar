import qs.modules.common
import qs.modules.common.widgets
import QtQuick

/**
 * Identité visuelle d'une cryptomonnaie : son logo officiel dès qu'il est en
 * cache, sinon une pastille colorée portant son glyphe.
 *
 * Le repli n'est pas seulement un filet de sécurité au premier démarrage : il
 * couvre aussi les monnaies dont la source ne publie pas d'image.
 */
Item {
    id: root

    /// Une entrée de `Crypto.coins`.
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
        // Décoder à la taille d'affichage évite de garder une image de 250 px
        // en mémoire pour un rendu de 16.
        sourceSize.width: Math.round(root.size * 2)
        sourceSize.height: Math.round(root.size * 2)
        fillMode: Image.PreserveAspectFit
        asynchronous: true
        smooth: true
        cache: true
        source: (root.coin?.icon ?? "") === "" ? "" : `file://${root.coin.icon}`
    }
}

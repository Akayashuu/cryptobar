import qs.modules.common
import qs.modules.common.widgets
import qs.services
import QtQuick
import QtQuick.Layouts

/**
 * Tableau de bord des cotations suivies : une ligne par pièce, la ligne active
 * mise en avant, avec la courbe des sept derniers jours et les variations.
 */
StyledPopup {
    id: root

    // Largeurs fixes : les colonnes restent alignées d'une ligne à l'autre sans
    // recourir à un GridLayout, dont le délégué de Repeater serait à plat.
    readonly property int curveWidth: 44
    readonly property int logoSize: 16
    readonly property int symbolWidth: 42
    readonly property int priceWidth: 88
    readonly property int changeWidth: 62

    /// Ligne de trois variations, réutilisée par l'en-tête et par les cotations.
    component ChangeCells: Row {
        id: changeCells
        property var labels: []
        property color tint: Appearance.colors.colOnSurfaceVariant
        property int weight: Font.Normal
        spacing: 0

        Repeater {
            model: changeCells.labels
            StyledText {
                required property string modelData
                width: root.changeWidth
                horizontalAlignment: Text.AlignRight
                textFormat: Text.PlainText
                font.pixelSize: Appearance.font.pixelSize.smaller
                font.weight: changeCells.weight
                color: changeCells.tint
                text: modelData
            }
        }
    }

    ColumnLayout {
        anchors.centerIn: parent
        spacing: 6

        StyledPopupHeaderRow {
            icon: Crypto.stale ? "cloud_off" : "monitoring"
            label: Crypto.stale ? Translation.tr("Crypto — cache hors ligne") : Translation.tr("Crypto")
        }

        Column {
            Layout.fillWidth: true
            spacing: 3

            Row { // En-tête des colonnes
                spacing: 8

                Item {
                    width: root.curveWidth + root.logoSize + root.symbolWidth + root.priceWidth + 28
                    height: 1
                }

                ChangeCells {
                    labels: ["1 h", "24 h", "7 j"]
                }
            }

            Repeater {
                model: Crypto.coins

                delegate: Rectangle {
                    id: line
                    required property var modelData
                    readonly property color accent: Crypto.color(line.modelData.trend)

                    // La largeur vient des colonnes fixes, pas du parent :
                    // la lier au Column créerait une boucle de liaison, le
                    // Column tirant sa largeur de ses enfants.
                    width: cells.implicitWidth + 8
                    height: cells.implicitHeight + 6
                    radius: Appearance.rounding.verysmall
                    color: line.modelData.active ? Appearance.colors.colLayer2 : "transparent"

                    Row {
                        id: cells
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.left: parent.left
                        anchors.leftMargin: 4
                        spacing: 8

                        CryptoCoinIcon {
                            anchors.verticalCenter: parent.verticalCenter
                            coin: line.modelData
                            size: root.logoSize
                        }

                        CryptoSparkline {
                            anchors.verticalCenter: parent.verticalCenter
                            width: root.curveWidth
                            height: 14
                            series: line.modelData.series
                            stroke: line.accent
                            thickness: 1.2
                        }

                        StyledText {
                            anchors.verticalCenter: parent.verticalCenter
                            width: root.symbolWidth
                            horizontalAlignment: Text.AlignLeft
                            font.pixelSize: Appearance.font.pixelSize.smaller
                            font.weight: line.modelData.active ? Font.DemiBold : Font.Normal
                            color: line.accent
                            text: line.modelData.symbol
                        }

                        StyledText {
                            anchors.verticalCenter: parent.verticalCenter
                            width: root.priceWidth
                            horizontalAlignment: Text.AlignRight
                            elide: Text.ElideRight
                            textFormat: Text.PlainText
                            font.pixelSize: Appearance.font.pixelSize.smaller
                            font.weight: line.modelData.active ? Font.DemiBold : Font.Normal
                            color: Appearance.colors.colOnLayer1
                            text: line.modelData.price
                        }

                        ChangeCells {
                            anchors.verticalCenter: parent.verticalCenter
                            labels: [line.modelData.change_1h, line.modelData.change_24h, line.modelData.change_7d]
                            tint: line.accent
                            weight: line.modelData.active ? Font.DemiBold : Font.Normal
                        }
                    }
                }
            }
        }

        StyledText {
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignLeft
            wrapMode: Text.Wrap
            font.pixelSize: Appearance.font.pixelSize.smallest
            color: Appearance.colors.colOnSurfaceVariant
            text: Translation.tr("Molette : pièce suivante • Clic droit : graphique • Clic milieu : rafraîchir")
        }
    }
}

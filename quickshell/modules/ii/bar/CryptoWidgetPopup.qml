import qs.modules.common
import qs.modules.common.widgets
import qs.services
import QtQuick
import QtQuick.Layouts

/**
 * Dashboard of the watched quotes: one row per coin, the active row
 * highlighted, with the seven-day curve and the changes.
 */
StyledPopup {
    id: root

    // Fixed widths: the columns stay aligned from one row to the next without a
    // GridLayout, whose Repeater delegate would be flattened.
    readonly property int curveWidth: 44
    readonly property int logoSize: 16
    readonly property int symbolWidth: 42
    readonly property int priceWidth: 88
    readonly property int changeWidth: 62

    /// Row of three changes, reused by the header and by the quotes.
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
            label: Crypto.stale ? Translation.tr("Crypto — offline cache") : Translation.tr("Crypto")
        }

        Column {
            Layout.fillWidth: true
            spacing: 3

            Row { // Column header
                spacing: 8

                Item {
                    width: root.curveWidth + root.logoSize + root.symbolWidth + root.priceWidth + 28
                    height: 1
                }

                ChangeCells {
                    labels: ["1 h", "24 h", "7 d"]
                }
            }

            Repeater {
                model: Crypto.coins

                delegate: Rectangle {
                    id: line
                    required property var modelData
                    readonly property color accent: Crypto.color(line.modelData.trend)

                    // The width comes from the fixed columns, not from the
                    // parent: binding it to the Column would create a binding
                    // loop, since the Column sizes itself from its children.
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

        // Crossed alerts: nothing most of the time, one row per threshold
        // otherwise. Same content as the desktop notification, kept at hand
        // after it has gone.
        Column {
            Layout.fillWidth: true
            visible: Crypto.alerts.length > 0
            spacing: 2

            Repeater {
                model: Crypto.alerts

                delegate: Row {
                    required property var modelData
                    spacing: 6

                    MaterialSymbol {
                        anchors.verticalCenter: parent.verticalCenter
                        iconSize: Appearance.font.pixelSize.normal
                        color: Appearance.colors.colError
                        text: modelData.kind === "below" ? "trending_down" : modelData.kind === "above" ? "trending_up" : "bolt"
                    }

                    StyledText {
                        anchors.verticalCenter: parent.verticalCenter
                        textFormat: Text.PlainText
                        font.pixelSize: Appearance.font.pixelSize.smaller
                        color: Appearance.colors.colError
                        text: modelData.title
                    }
                }
            }
        }

        // Portfolio: absent until a public address is watched.
        Column {
            Layout.fillWidth: true
            visible: Crypto.portfolio !== null
            spacing: 3

            Row {
                spacing: 8

                StyledText {
                    anchors.verticalCenter: parent.verticalCenter
                    font.pixelSize: Appearance.font.pixelSize.large
                    font.weight: Font.DemiBold
                    textFormat: Text.PlainText
                    color: Appearance.colors.colOnLayer1
                    text: Crypto.portfolio?.total ?? ""
                }

                StyledText {
                    anchors.verticalCenter: parent.verticalCenter
                    textFormat: Text.PlainText
                    font.pixelSize: Appearance.font.pixelSize.smaller
                    color: (Crypto.portfolio?.positive ?? true) ? Appearance.m3colors.m3primary : Appearance.colors.colError
                    text: (Crypto.portfolio?.change_24h ?? "") + " " + (Crypto.portfolio?.change_24h_pct ?? "")
                }
            }

            Repeater {
                model: Crypto.portfolio?.positions ?? []

                delegate: Row {
                    required property var modelData
                    spacing: 8

                    // Share of the portfolio: a bar beats one more percentage
                    // in an already busy row.
                    Rectangle {
                        anchors.verticalCenter: parent.verticalCenter
                        implicitWidth: 40
                        implicitHeight: 4
                        radius: 2
                        color: Appearance.colors.colLayer2

                        Rectangle {
                            width: Math.max(2, parent.width * (modelData.share ?? 0))
                            height: parent.height
                            radius: parent.radius
                            color: modelData.color
                        }
                    }

                    StyledText {
                        anchors.verticalCenter: parent.verticalCenter
                        width: root.symbolWidth
                        font.pixelSize: Appearance.font.pixelSize.smaller
                        color: Appearance.colors.colOnLayer1
                        text: modelData.symbol
                    }

                    StyledText {
                        anchors.verticalCenter: parent.verticalCenter
                        width: root.priceWidth
                        horizontalAlignment: Text.AlignRight
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        font.pixelSize: Appearance.font.pixelSize.smaller
                        color: Appearance.colors.colOnSurfaceVariant
                        text: modelData.quantity
                    }

                    StyledText {
                        anchors.verticalCenter: parent.verticalCenter
                        width: root.changeWidth
                        horizontalAlignment: Text.AlignRight
                        textFormat: Text.PlainText
                        font.pixelSize: Appearance.font.pixelSize.smaller
                        color: Appearance.colors.colOnLayer1
                        text: modelData.value
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
            text: Translation.tr("Scroll: next coin • Right click: chart • Middle click: refresh")
        }
    }
}

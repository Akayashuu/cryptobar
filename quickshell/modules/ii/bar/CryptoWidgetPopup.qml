import qs.modules.common
import qs.modules.common.widgets
import qs.services
import QtQuick
import QtQuick.Layouts

/**
 * Infobulle détaillée : toutes les cotations suivies, avec les variations
 * 1 h / 24 h / 7 j telles que `cryptobar` les met en forme.
 */
StyledPopup {
    id: root

    ColumnLayout {
        anchors.centerIn: parent
        spacing: 4

        StyledPopupHeaderRow {
            icon: Crypto.stale ? "cloud_off" : "monitoring"
            label: Crypto.stale ? qsTr("Cours (cache)") : qsTr("Cours")
        }

        StyledText {
            Layout.fillWidth: true
            textFormat: Text.PlainText
            horizontalAlignment: Text.AlignLeft
            wrapMode: Text.NoWrap
            font.family: Appearance.font.family.monospace
            color: Appearance.colors.colOnSurfaceVariant
            text: Crypto.tooltip
        }

        StyledText {
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignLeft
            font.pixelSize: Appearance.font.pixelSize.smaller
            color: Appearance.colors.colOnSurfaceVariant
            text: qsTr("Molette : changer de pièce · Clic : fenêtre · Clic droit : graphique · Clic milieu : rafraîchir")
        }
    }
}

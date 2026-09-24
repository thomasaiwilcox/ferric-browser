import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: settings
    required property var browserWindow
    required property bool settingsVisible
    required property bool temporary
    required property string searchText
    required property string notice
    required property string spellcheckStatus
    required property bool spellcheckDictionariesMissing
    required property var userscriptRows
    required property var settingsModel
    signal closeRequested()
    signal refreshRequested()
    signal installUserscriptRequested()
    signal temporaryChanged(bool temporary)
    signal searchChanged(string text)
    signal userscriptEnabledRequested(string name, bool enabled)
    signal userscriptRemovalRequested(string name)
    signal settingApplyRequested(var row, var value)
    signal settingResetRequested(var row)

    function setUserscriptEnabled(name, enabled) {
        userscriptInventory.setEnabled(name, enabled)
    }

    anchors.centerIn: parent
    width: Math.min(980, parent.width - 70)
    height: Math.min(680, parent.height - 80)
    z: 76
    visible: settingsVisible
    focus: visible
    Accessible.role: Accessible.Dialog
    Accessible.name: "Settings"
    onVisibleChanged: if (visible) forceActiveFocus()
    color: browserWindow.panelColor
    border.color: browserWindow.accentColor
    border.width: 1

    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape) {
            settings.closeRequested()
            event.accepted = true
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 14
        spacing: 8

        RowLayout {
            Layout.fillWidth: true
            Label {
                Layout.fillWidth: true
                text: "Settings"
                color: settings.browserWindow.primaryTextColor
                font.bold: true
                Accessible.name: "Settings"
            }
            Button {
                text: "Refresh"
                Accessible.name: "Refresh settings"
                onClicked: settings.refreshRequested()
            }
            Button {
                text: "Install userscript"
                Accessible.name: "Install userscript manifest"
                onClicked: settings.installUserscriptRequested()
            }
            Button {
                text: "Close"
                Accessible.name: "Close settings"
                onClicked: settings.closeRequested()
            }
        }

        RowLayout {
            Layout.fillWidth: true
            CheckBox {
                text: "Temporary (memory only)"
                checked: settings.temporary
                Accessible.name: "Apply settings temporarily"
                onToggled: settings.temporaryChanged(checked)
            }
            Label {
                Layout.fillWidth: true
                text: "Persistent edits never rewrite the authored config.toml."
                color: settings.browserWindow.secondaryTextColor
                wrapMode: Text.WordWrap
            }
        }

        TextField {
            Layout.fillWidth: true
            text: settings.searchText
            placeholderText: "Filter settings by key, description, scope, or apply time"
            Accessible.name: "Search settings"
            Accessible.role: Accessible.EditableText
            onTextChanged: settings.searchChanged(text)
            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    settings.closeRequested()
                    event.accepted = true
                }
            }
        }

        Label {
            Layout.fillWidth: true
            text: "Supported fields below use the validated typed schema. Site-scoped values remain available through explicit site-rule commands; this manager edits the active global/profile runtime layer."
            color: settings.browserWindow.secondaryTextColor
            wrapMode: Text.WordWrap
        }

        Label {
            Layout.fillWidth: true
            visible: settings.notice.length > 0
            text: settings.notice
            color: settings.notice.indexOf("error") >= 0
                   ? settings.browserWindow.errorColor : settings.browserWindow.successColor
            wrapMode: Text.WordWrap
            Accessible.name: "Settings status"
        }

        Label {
            Layout.fillWidth: true
            text: settings.spellcheckStatus
            color: settings.spellcheckDictionariesMissing
                   ? settings.browserWindow.warningColor : settings.browserWindow.mutedTextColor
            wrapMode: Text.WordWrap
            Accessible.name: "Spellcheck dictionary status"
        }

        Label {
            Layout.fillWidth: true
            text: "Installed userscripts"
            color: settings.browserWindow.primaryTextColor
            font.bold: true
            Accessible.name: "Installed userscripts"
        }

        FerricUserscriptInventory {
            id: userscriptInventory
            Layout.fillWidth: true
            Layout.preferredHeight: implicitHeight
            browserWindow: settings.browserWindow
            rows: settings.userscriptRows
            onEnabledRequested: function(name, enabled) {
                settings.userscriptEnabledRequested(name, enabled)
            }
            onRemovalRequested: function(name) { settings.userscriptRemovalRequested(name) }
        }

        Label {
            Layout.fillWidth: true
            visible: settings.browserWindow.temporaryProfile
            text: "Userscript installation and enable changes are unavailable in private or ephemeral profiles."
            color: settings.browserWindow.mutedTextColor
            wrapMode: Text.WordWrap
        }

        FerricSettingRows {
            Layout.fillWidth: true
            Layout.fillHeight: true
            browserWindow: settings.browserWindow
            settingsModel: settings.settingsModel
            onApplyRequested: function(row, value) { settings.settingApplyRequested(row, value) }
            onResetRequested: function(row) { settings.settingResetRequested(row) }
        }
    }
}

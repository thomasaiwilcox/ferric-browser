import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: bindingHelp
    required property var browserWindow
    signal closeRequested()
    signal refreshRequested()
    signal searchChanged(string text)

    anchors.centerIn: parent
    width: Math.min(980, parent.width - 70)
    height: Math.min(650, parent.height - 90)
    z: 74
    visible: browserWindow.bindingHelpVisible
    focus: visible
    Accessible.role: Accessible.Dialog
    Accessible.name: "Binding help"
    onVisibleChanged: if (visible) forceActiveFocus()
    color: browserWindow.panelColor
    border.color: browserWindow.accentColor
    border.width: 1

    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape) {
            closeRequested()
            event.accepted = true
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 14
        spacing: 8
        RowLayout {
            Layout.fillWidth: true
            Label { Layout.fillWidth: true; text: "Keyboard help"; color: bindingHelp.browserWindow.primaryTextColor; font.bold: true; Accessible.name: "Keyboard help" }
            Button { text: "Refresh"; Accessible.name: "Refresh keyboard help"; onClicked: bindingHelp.refreshRequested() }
            Button { text: "Close"; Accessible.name: "Close keyboard help"; onClicked: bindingHelp.closeRequested() }
        }
        TextField {
            id: helpSearchInput
            Layout.fillWidth: true
            text: bindingHelp.browserWindow.bindingHelpSearch
            placeholderText: "Search commands, keys, modes, or descriptions"
            Accessible.name: "Search keyboard help"
            Accessible.role: Accessible.EditableText
            onTextChanged: bindingHelp.searchChanged(text)
            Keys.onPressed: function(event) { if (event.key === Qt.Key_Escape) { bindingHelp.closeRequested(); event.accepted = true } }
        }
        Label {
            Layout.fillWidth: true
            text: "Effective bindings are generated from the active validated trie. ‘unbound’ commands remain available through the command line."
            color: bindingHelp.browserWindow.secondaryTextColor
            wrapMode: Text.WordWrap
        }
        ScrollView {
            id: bindingHelpScroll
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            ListView {
                id: bindingHelpList
                width: bindingHelpScroll.availableWidth
                height: bindingHelpScroll.availableHeight
                clip: true
                model: bindingHelp.browserWindow.bindingHelpRows
                spacing: 4
                delegate: Rectangle {
                    width: bindingHelpList.width
                    height: modelData.kind === "heading" ? 30 : 76
                    color: modelData.kind === "heading" ? bindingHelp.browserWindow.surfaceColor
                         : modelData.kind === "conflict" ? Qt.darker(bindingHelp.browserWindow.errorColor, 2.0)
                         : bindingHelp.browserWindow.surfaceColor
                    radius: 3
                    Accessible.name: modelData.kind === "heading" ? modelData.title : modelData.mode + " " + modelData.command + " " + modelData.keys
                    Label {
                        anchors.fill: parent; anchors.margins: 8
                        visible: modelData.kind === "heading"; text: modelData.title
                        color: bindingHelp.browserWindow.accentColor; font.bold: true; verticalAlignment: Text.AlignVCenter
                    }
                    RowLayout {
                        anchors.fill: parent; anchors.margins: 8
                        visible: modelData.kind !== "heading"; spacing: 8
                        Label { Layout.preferredWidth: 100; text: modelData.mode; color: modelData.kind === "conflict" ? bindingHelp.browserWindow.warningColor : bindingHelp.browserWindow.mutedTextColor; elide: Text.ElideRight }
                        Label { Layout.preferredWidth: 180; text: modelData.command; color: bindingHelp.browserWindow.primaryTextColor; font.bold: true; elide: Text.ElideRight }
                        ColumnLayout {
                            Layout.fillWidth: true; spacing: 2
                            Label { Layout.fillWidth: true; text: modelData.keys; color: modelData.kind === "conflict" ? bindingHelp.browserWindow.warningColor : bindingHelp.browserWindow.accentColor; elide: Text.ElideRight }
                            Label { Layout.fillWidth: true; text: modelData.description; color: bindingHelp.browserWindow.secondaryTextColor; elide: Text.ElideRight }
                        }
                        Label { Layout.preferredWidth: 120; text: modelData.source + " · " + modelData.count; color: bindingHelp.browserWindow.mutedTextColor; elide: Text.ElideRight }
                    }
                }
            }
        }
    }
}

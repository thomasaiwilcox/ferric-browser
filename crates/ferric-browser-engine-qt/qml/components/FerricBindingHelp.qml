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
        ColumnLayout {
            Layout.fillWidth: true
            Label {
                Layout.fillWidth: true
                text: "Keyboard help"
                color: bindingHelp.browserWindow.primaryTextColor
                font.bold: true
                Accessible.name: "Keyboard help"
                Accessible.description: "Unbound commands remain available through the command line"
            }
            RowLayout {
                Layout.fillWidth: true
                Button { text: "Refresh"; Accessible.name: "Refresh keyboard help"; onClicked: bindingHelp.refreshRequested() }
                Button { text: "Close"; Accessible.name: "Close keyboard help"; onClicked: bindingHelp.closeRequested() }
                Item { Layout.fillWidth: true }
            }
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
        ScrollView {
            id: bindingHelpScroll
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            ListView {
                id: bindingHelpList
                objectName: "bindingHelpList"
                width: bindingHelpScroll.availableWidth
                height: bindingHelpScroll.availableHeight
                clip: true
                model: bindingHelp.browserWindow.bindingHelpRows
                spacing: 4
                delegate: Rectangle {
                    objectName: "bindingHelpRow"
                    width: bindingHelpList.width
                    height: modelData.kind === "heading"
                            ? headingLabel.implicitHeight + 16
                            : bindingDetails.implicitHeight + 16
                    color: modelData.kind === "heading" ? bindingHelp.browserWindow.surfaceColor
                         : modelData.kind === "conflict" ? Qt.darker(bindingHelp.browserWindow.errorColor, 2.0)
                         : bindingHelp.browserWindow.surfaceColor
                    radius: 3
                    Accessible.name: modelData.kind === "heading" ? modelData.title : modelData.mode + " " + modelData.command + " " + modelData.keys
                    Label {
                        id: headingLabel
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: 8
                        visible: modelData.kind === "heading"
                        text: modelData.kind === "heading" ? modelData.title : ""
                        color: bindingHelp.browserWindow.accentColor
                        font.bold: true
                        wrapMode: Text.WordWrap
                    }
                    ColumnLayout {
                        id: bindingDetails
                        objectName: "bindingDetails"
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: 8
                        visible: modelData.kind !== "heading"
                        spacing: 2
                        Label {
                            Layout.fillWidth: true
                            text: modelData.command
                            color: bindingHelp.browserWindow.primaryTextColor
                            font.bold: true
                            wrapMode: Text.WordWrap
                        }
                        Label {
                            Layout.fillWidth: true
                            text: modelData.keys
                            color: modelData.kind === "conflict"
                                   ? bindingHelp.browserWindow.warningColor
                                   : bindingHelp.browserWindow.accentColor
                            wrapMode: Text.WordWrap
                        }
                        Label {
                            Layout.fillWidth: true
                            text: modelData.description
                            color: bindingHelp.browserWindow.secondaryTextColor
                            wrapMode: Text.WordWrap
                        }
                        Label {
                            Layout.fillWidth: true
                            text: modelData.mode + " · " + modelData.source
                                  + " · " + modelData.count
                            color: bindingHelp.browserWindow.mutedTextColor
                            wrapMode: Text.WordWrap
                        }
                    }
                }
            }
        }
    }
}

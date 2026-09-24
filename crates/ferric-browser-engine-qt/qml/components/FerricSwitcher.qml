import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: switcher
    required property var browserWindow
    required property bool switcherVisible
    required property var results
    property alias query: switcherInput.text
    signal queryChanged(string query)
    signal closeRequested()
    signal activationRequested(int index)
    signal actionRequested(int index, string action)

    function focusInput() {
        switcherInput.forceActiveFocus()
    }

    anchors.centerIn: parent
    width: Math.min(860, parent.width - Math.max(40, browserWindow.chromeRowHeight * 4))
    height: Math.min(520, parent.height - Math.max(90, browserWindow.chromeRowHeight * 6))
    z: 60
    visible: switcherVisible
    focus: visible
    Accessible.role: Accessible.Dialog
    Accessible.name: "Universal switcher"
    Accessible.description: "Search up to " + browserWindow.switcherMaxResults()
        + " browser items; use arrow keys, Page Up, Page Down, Home, End, and Enter to choose"
    color: browserWindow.panelColor
    border.color: browserWindow.accentColor

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 12
        spacing: 8

        RowLayout {
            Layout.fillWidth: true
            Label {
                Layout.fillWidth: true
                text: "Switcher"
                color: switcher.browserWindow.primaryTextColor
                font.bold: true
                Accessible.name: "Universal switcher heading"
            }
            Label {
                text: "Ctrl-P · Esc"
                color: switcher.browserWindow.mutedTextColor
            }
            Label {
                text: switcher.results.length + " results"
                color: switcher.browserWindow.mutedTextColor
                Accessible.name: text
                Accessible.role: Accessible.StatusBar
                Accessible.description: "Current universal switcher result count"
            }
        }

        TextField {
            id: switcherInput
            Layout.fillWidth: true
            placeholderText: "Search tabs, windows, contexts, commands, history, marks, sessions, downloads"
            Accessible.name: "Universal switcher search"
            Accessible.role: Accessible.EditableText
            Accessible.editable: true
            onTextChanged: switcher.queryChanged(text)
            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    switcher.closeRequested()
                    event.accepted = true
                } else if (event.key === Qt.Key_Down) {
                    switcherResultsList.moveBy(1)
                    event.accepted = true
                } else if (event.key === Qt.Key_Up) {
                    switcherResultsList.moveBy(-1)
                    event.accepted = true
                } else if (event.key === Qt.Key_PageDown) {
                    switcherResultsList.moveBy(8)
                    event.accepted = true
                } else if (event.key === Qt.Key_PageUp) {
                    switcherResultsList.moveBy(-8)
                    event.accepted = true
                } else if (event.key === Qt.Key_Home) {
                    switcherResultsList.moveToBeginning()
                    event.accepted = true
                } else if (event.key === Qt.Key_End) {
                    switcherResultsList.moveToEnd()
                    event.accepted = true
                } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                    switcher.activationRequested(switcherResultsList.currentIndex)
                    event.accepted = true
                }
            }
        }

        FerricSwitcherResults {
            id: switcherResultsList
            Layout.fillWidth: true
            Layout.fillHeight: true
            browserWindow: switcher.browserWindow
            results: switcher.results
            onActivationRequested: function(index) { switcher.activationRequested(index) }
            onActionRequested: function(index, action) { switcher.actionRequested(index, action) }
        }
    }
}

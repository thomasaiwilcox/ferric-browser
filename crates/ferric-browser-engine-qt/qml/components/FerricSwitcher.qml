import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FerricModalSurface {
    id: switcher
    required property bool switcherVisible
    required property var results
    property alias query: switcherInput.text
    signal queryChangeRequested(string query)
    signal closeRequested()
    signal activationRequested(int index)
    signal actionRequested(int index, string action)

    function focusInput() {
        switcherInput.forceActiveFocus()
    }

    visible: switcherVisible
    commandText: ":switcher"
    title: "Universal switcher"
    keyHelp: "↑/↓ select  ·  pgup/pgdn page  ·  enter open  ·  esc close"
    dialogWidth: 860 * scale
    dialogHeight: 520 * scale
    stackingOrder: 60
    initialFocusItem: switcherInput
    onDismissRequested: closeRequested()
    Accessible.description: "Search up to " + browserWindow.switcherMaxResults()
        + " browser items; use arrow keys, Page Up, Page Down, Home, End, and Enter to choose"

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 0
        spacing: 8

        RowLayout {
            Layout.fillWidth: true
            Item { Layout.fillWidth: true }
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
            placeholderText: "Search tabs, windows, contexts, commands, actions, history, marks, sessions, downloads"
            Accessible.name: "Universal switcher search"
            Accessible.role: Accessible.EditableText
            Accessible.editable: true
            onTextChanged: switcher.queryChangeRequested(text)
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

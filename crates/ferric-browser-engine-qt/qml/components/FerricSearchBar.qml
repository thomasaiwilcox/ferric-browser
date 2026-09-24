import QtQuick
import QtQuick.Controls

// Search chrome renders a supplied search snapshot and emits user intents.
// Search execution and page-engine effects stay at the composition root.
Rectangle {
    id: searchBar
    required property var browserWindow
    required property bool searchVisible
    required property bool searchBackward
    required property string searchText
    signal searchChanged(string text)
    signal accepted()
    signal escapeRequested()
    signal nextRequested(bool backward)

    function focusInput() {
        searchLine.forceActiveFocus()
    }

    anchors.left: parent.left
    anchors.right: parent.right
    anchors.bottom: parent.bottom
    height: browserWindow.inputBarHeight
    z: 20
    visible: searchVisible
    color: browserWindow.surfaceColor
    opacity: browserWindow.chromeOpacity

    Rectangle {
        id: searchPrefixBackground
        anchors.left: parent.left
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        width: searchPrefix.implicitWidth + 16
        color: searchBar.browserWindow.warningColor

        Label {
            id: searchPrefix
            anchors.centerIn: parent
            text: searchBar.searchBackward ? "?" : "/"
            color: searchBar.browserWindow.contrastText(parent.color)
            font.bold: true
            Accessible.ignored: true
        }
    }

    TextField {
        id: searchLine
        anchors.left: searchPrefixBackground.right
        anchors.right: searchBackwardButton.left
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        leftPadding: 8
        rightPadding: 8
        topPadding: 0
        bottomPadding: 0
        text: searchBar.searchText
        color: searchBar.browserWindow.primaryTextColor
        selectionColor: searchBar.browserWindow.selectionColor
        selectedTextColor: searchBar.browserWindow.selectionTextColor
        placeholderText: searchBar.searchBackward ? "search backward" : "search"
        placeholderTextColor: searchBar.browserWindow.mutedTextColor
        background: Rectangle { color: "transparent" }
        Accessible.name: searchBar.searchBackward ? "Search backward" : "Search forward"
        Accessible.role: Accessible.EditableText
        Accessible.editable: true
        focus: searchBar.searchVisible

        onVisibleChanged: if (visible) forceActiveFocus()
        onTextChanged: searchBar.searchChanged(text)
        onAccepted: searchBar.accepted()
        Keys.onPressed: function(event) {
            if (event.key === Qt.Key_Escape) {
                searchBar.escapeRequested()
                event.accepted = true
            }
        }
    }

    ToolButton {
        id: searchBackwardButton
        anchors.right: searchForwardButton.left
        anchors.verticalCenter: parent.verticalCenter
        text: "↑"
        width: searchBar.browserWindow.inputBarHeight
        height: searchBar.browserWindow.inputBarHeight
        padding: 0
        background: Rectangle {
            color: parent.hovered ? searchBar.browserWindow.panelColor : "transparent"
        }
        contentItem: Text {
            text: parent.text
            color: searchBar.browserWindow.mutedTextColor
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }
        Accessible.name: "Find previous match"
        onClicked: {
            searchBar.nextRequested(true)
            searchBar.focusInput()
        }
    }

    ToolButton {
        id: searchForwardButton
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        text: "↓"
        width: searchBar.browserWindow.inputBarHeight
        height: searchBar.browserWindow.inputBarHeight
        padding: 0
        background: Rectangle {
            color: parent.hovered ? searchBar.browserWindow.panelColor : "transparent"
        }
        contentItem: Text {
            text: parent.text
            color: searchBar.browserWindow.mutedTextColor
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }
        Accessible.name: "Find next match"
        onClicked: {
            searchBar.nextRequested(false)
            searchBar.focusInput()
        }
    }
}

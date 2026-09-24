import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

RowLayout {
    id: journeySearch
    required property string searchText
    required property bool currentOnly
    signal searchRequested(string text)
    signal clearRequested()
    signal currentRequested()
    signal allRequested()

    TextField {
        id: journeySearchField
        Layout.fillWidth: true
        placeholderText: "Search title, URL, transition, or source"
        Accessible.name: "Search journey records"
        text: journeySearch.searchText
        onAccepted: journeySearch.searchRequested(text)
    }
    Button {
        text: "Search"
        Accessible.name: "Search journey records"
        onClicked: journeySearch.searchRequested(journeySearchField.text)
    }
    Button {
        text: "Clear"
        Accessible.name: "Clear journey search"
        onClicked: journeySearch.clearRequested()
    }
    Button {
        text: "Current"
        checkable: true
        checked: journeySearch.currentOnly
        Accessible.name: "Show current journey node only"
        onClicked: journeySearch.currentRequested()
    }
    Button {
        text: "All"
        enabled: journeySearch.currentOnly
        Accessible.name: "Show all journey records"
        onClicked: journeySearch.allRequested()
    }
}

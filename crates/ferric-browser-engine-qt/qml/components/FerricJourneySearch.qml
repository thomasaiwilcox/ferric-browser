import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ColumnLayout {
    id: journeySearch
    required property string searchText
    required property bool currentOnly
    property bool pending: false
    signal searchRequested(string text)
    signal clearRequested()
    signal currentRequested()
    signal allRequested()

    FerricTextField {
        id: journeySearchField
        objectName: "journeySearchInput"
        Layout.fillWidth: true
        placeholderText: "Search pages and navigation details"
        Accessible.name: "Search journey records"
        text: journeySearch.searchText
        readOnly: journeySearch.pending
        onAccepted: if (!journeySearch.pending) journeySearch.searchRequested(text)
    }
    Flow {
        Layout.fillWidth: true
        Layout.preferredHeight: childrenRect.height
        spacing: 6
        Button {
            text: "Search"
            enabled: !journeySearch.pending
            Accessible.name: "Search journey records"
            onClicked: journeySearch.searchRequested(journeySearchField.text)
        }
        Button {
            text: "Clear"
            enabled: !journeySearch.pending
            Accessible.name: "Clear journey search"
            onClicked: journeySearch.clearRequested()
        }
        Button {
            text: "Current page"
            enabled: !journeySearch.pending
            checkable: true
            checked: journeySearch.currentOnly
            Accessible.name: "Show the current page only"
            onClicked: journeySearch.currentRequested()
        }
        Button {
            text: "All pages"
            enabled: !journeySearch.pending && journeySearch.currentOnly
            Accessible.name: "Show all journey records"
            onClicked: journeySearch.allRequested()
        }
    }
}

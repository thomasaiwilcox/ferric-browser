import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: libraryManager
    required property var browserWindow
    required property bool managerVisible
    required property string libraryKind
    required property bool graphMode
    required property int totalEntries
    required property int page
    required property int pageCount
    required property int pageSize
    required property string journeySearchText
    required property bool journeyCurrentOnly
    required property string pendingDelete
    required property var entries
    required property var graphNodes
    required property var graphEntries
    required property var graphLineData
    required property var profilesModel
    required property bool profilesLoading
    signal closeRequested()
    signal graphToggleRequested()
    signal exportRequested()
    signal pageChangeRequested(int delta)
    signal journeySearchRequested(string text)
    signal journeyClearRequested()
    signal journeyCurrentRequested()
    signal journeyAllRequested()
    signal entryOpenRequested(string entryKind, string entryId)
    signal privateHistoryTransferRequested(string entryId, string title, string url)
    signal entryEditRequested(string entryKind, string entryId, string value)
    signal entryDeleteRequested(string entryKind, string entryId)
    signal journeyReopenRequested(string nodeId, string target)
    signal journeyExpandRequested(string nodeId)
    signal journeyExportFileRequested()
    signal journeyExportCancelRequested()
    signal privateHistoryReopenRequested(string profileName)
    signal privateHistoryBookmarkRequested(string profileName, string profileLabel)
    signal privateHistoryCancelRequested()

    function markEditSaved(entryKind, entryId) {
        libraryEntryList.markEditSaved(entryKind, entryId)
    }

    function requestGraphPaint() {
        journeyGraph.requestPaint()
    }

    anchors.centerIn: parent
    width: Math.min(820, parent.width - 100)
    height: Math.min(520, parent.height - 120)
    z: 55
    visible: managerVisible
    focus: visible
    Accessible.role: Accessible.Dialog
    Accessible.name: "Library manager"
    onVisibleChanged: if (visible) forceActiveFocus()
    color: browserWindow.panelColor
    border.color: browserWindow.borderColor

    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape) {
            libraryManager.closeRequested()
            event.accepted = true
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 12
        spacing: 8

        RowLayout {
            Layout.fillWidth: true
            Label {
                Layout.fillWidth: true
                text: libraryManager.libraryKind
                color: libraryManager.browserWindow.primaryTextColor
                font.bold: true
            }
            Button {
                visible: libraryManager.libraryKind === "journey"
                text: libraryManager.graphMode ? "Outline" : "Relationships"
                Accessible.name: text + " view"
                onClicked: libraryManager.graphToggleRequested()
            }
            Button {
                visible: libraryManager.libraryKind === "journey"
                text: "Export"
                Accessible.name: "Export journey records"
                onClicked: libraryManager.exportRequested()
            }
            Button {
                visible: libraryManager.libraryKind !== "journey"
                         && libraryManager.totalEntries > libraryManager.pageSize
                text: "Previous"
                enabled: libraryManager.page > 0
                Accessible.name: "Previous library page"
                onClicked: libraryManager.pageChangeRequested(-1)
            }
            Label {
                visible: libraryManager.libraryKind !== "journey"
                         && libraryManager.totalEntries > libraryManager.pageSize
                text: "Page " + (libraryManager.page + 1) + " / " + libraryManager.pageCount
                color: libraryManager.browserWindow.mutedTextColor
                Accessible.name: text
            }
            Button {
                visible: libraryManager.libraryKind !== "journey"
                         && libraryManager.totalEntries > libraryManager.pageSize
                text: "Next"
                enabled: libraryManager.page + 1 < libraryManager.pageCount
                Accessible.name: "Next library page"
                onClicked: libraryManager.pageChangeRequested(1)
            }
            Button {
                text: "Close"
                Accessible.name: "Close library manager"
                onClicked: libraryManager.closeRequested()
            }
        }

        Label {
            Layout.fillWidth: true
            text: "Profile-local records; URLs are displayed in their sanitized form."
            color: libraryManager.browserWindow.mutedTextColor
        }

        FerricJourneySearch {
            Layout.fillWidth: true
            visible: libraryManager.libraryKind === "journey"
            searchText: libraryManager.journeySearchText
            currentOnly: libraryManager.journeyCurrentOnly
            onSearchRequested: function(text) { libraryManager.journeySearchRequested(text) }
            onClearRequested: libraryManager.journeyClearRequested()
            onCurrentRequested: libraryManager.journeyCurrentRequested()
            onAllRequested: libraryManager.journeyAllRequested()
        }

        FerricLibraryEntries {
            id: libraryEntryList
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: !libraryManager.graphMode || libraryManager.libraryKind !== "journey"
            browserWindow: libraryManager.browserWindow
            entries: libraryManager.entries
            libraryKind: libraryManager.libraryKind
            pendingDelete: libraryManager.pendingDelete
            onOpenRequested: function(entryKind, entryId) {
                libraryManager.entryOpenRequested(entryKind, entryId)
            }
            onPrivateHistoryTransferRequested: function(entryId, title, url) {
                libraryManager.privateHistoryTransferRequested(entryId, title, url)
            }
            onEditRequested: function(entryKind, entryId, value) {
                libraryManager.entryEditRequested(entryKind, entryId, value)
            }
            onDeleteRequested: function(entryKind, entryId) {
                libraryManager.entryDeleteRequested(entryKind, entryId)
            }
            onJourneyReopenRequested: function(nodeId, target) {
                libraryManager.journeyReopenRequested(nodeId, target)
            }
            onJourneyExpandRequested: function(nodeId) {
                libraryManager.journeyExpandRequested(nodeId)
            }
        }

        FerricJourneyGraph {
            id: journeyGraph
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: libraryManager.graphMode && libraryManager.libraryKind === "journey"
            browserWindow: libraryManager.browserWindow
            graphNodes: libraryManager.graphNodes
            graphEntries: libraryManager.graphEntries
            lineData: libraryManager.graphLineData
        }
    }

    FerricJourneyExportPreview {
        browserWindow: libraryManager.browserWindow
        onChooseFileRequested: libraryManager.journeyExportFileRequested()
        onCancelRequested: libraryManager.journeyExportCancelRequested()
    }

    FerricPrivateHistoryTransfer {
        browserWindow: libraryManager.browserWindow
        profilesModel: libraryManager.profilesModel
        profilesLoading: libraryManager.profilesLoading
        onReopenRequested: function(profileName) {
            libraryManager.privateHistoryReopenRequested(profileName)
        }
        onBookmarkRequested: function(profileName, profileLabel) {
            libraryManager.privateHistoryBookmarkRequested(profileName, profileLabel)
        }
        onCancelRequested: libraryManager.privateHistoryCancelRequested()
    }
}

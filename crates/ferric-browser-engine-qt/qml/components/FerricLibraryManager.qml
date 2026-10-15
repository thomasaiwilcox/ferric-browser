import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../scripts/ChromePresentation.js" as ChromePresentation

FerricModalSurface {
    id: libraryManager
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
    property bool listPending: false
    property string listError: ""
    property var feedback: ({id: "", state: "idle", message: "", detail: ""})
    property string submittedId: ""
    property string submittedKind: ""
    property string submittedKey: ""
    property string submittedValue: ""
    property bool submittedDelete: false
    property string rejectionMessage: ""
    property string rejectionDetail: ""
    readonly property bool operationPending: feedback.state === "pending"
    readonly property real graphNodeWidth: journeyGraph.nodeWidth
    readonly property real graphNodeHeight: journeyGraph.nodeHeight
    signal graphLayoutRequested()
    signal refreshRequested()
    signal deleteCancelled()
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

    function mutationAccepted(id, kind, key, value, deletion) {
        submittedDelete = deletion === true
        submittedId = id; submittedKind = kind; submittedKey = key; submittedValue = value
        rejectionMessage = ""; rejectionDetail = ""
        applyFeedback()
    }
    function requestRejected(message, detail) { rejectionMessage = message; rejectionDetail = detail }
    function applyFeedback() {
        if (!submittedId || feedback.id !== submittedId || (feedback.state !== "succeeded" && feedback.state !== "failed")) return
        if (feedback.state === "succeeded") {
            libraryEntryList.markEditSaved(submittedKind, submittedKey, submittedValue)
            if (submittedDelete) focusTimer.restart()
        }
        submittedId = ""
    }
    function requestGraphPaint() { journeyGraph.requestPaint() }
    function focusPrimaryControl() {
        if (libraryEntryList.visible) libraryEntryList.focusList()
        else { journeyGraph.showBeginning(); journeyGraph.forceActiveFocus() }
    }
    onFeedbackChanged: applyFeedback()
    onLibraryKindChanged: deleteCancelled()
    onGraphModeChanged: if (visible) focusTimer.restart()
    onGraphNodeWidthChanged: graphLayoutRequested()
    onGraphNodeHeightChanged: graphLayoutRequested()
    onVisibleChanged: {
        deleteCancelled()
        if (visible) focusTimer.restart()
    }
    Component.onCompleted: if (visible) focusTimer.restart()
    Timer { id: focusTimer; interval: 0; onTriggered: if (libraryManager.visible) libraryManager.focusPrimaryControl() }

    visible: managerVisible
    commandText: libraryKind === "actions" ? ":action-list" : libraryKind === "bookmarks" ? ":bookmark-list" : libraryKind === "quickmarks" ? ":quickmark-list" : libraryKind === "context-routes" ? ":context-route list" : ":" + libraryKind
    title: ChromePresentation.libraryTitle(libraryKind)
    message: libraryKind === "history" && (browserWindow.temporaryProfile || browserWindow.ephemeralProfile)
        ? "History in this private session." : ChromePresentation.libraryDescription(libraryKind)
    keyHelp: "j/k or ↑/↓ browse  ·  enter " + (["actions", "context-routes"].indexOf(libraryKind) >= 0 ? "details" : "activate") + "  ·  tab actions  ·  esc close"
    dialogWidth: 880 * scale
    dialogHeight: 620 * scale
    stackingOrder: 55
    onDismissRequested: closeRequested()

    Component {
        id: libraryControls
        ColumnLayout {
            width: libraryManager.bodyItem.width
            spacing: 8 * libraryManager.scale
            FerricJourneySearch {
                Layout.fillWidth: true
                visible: libraryManager.libraryKind === "journey"
                searchText: libraryManager.journeySearchText
                currentOnly: libraryManager.journeyCurrentOnly
                pending: libraryManager.listPending
                onSearchRequested: function(text) { libraryManager.journeySearchRequested(text) }
                onClearRequested: libraryManager.journeyClearRequested()
                onCurrentRequested: libraryManager.journeyCurrentRequested()
                onAllRequested: libraryManager.journeyAllRequested()
            }
            FerricManagerFeedback {
                objectName: "libraryOperationFeedback"
                Layout.fillWidth: true
                browserWindow: libraryManager.browserWindow
                message: libraryManager.rejectionMessage || libraryManager.feedback.message || ""
                detail: libraryManager.rejectionMessage ? libraryManager.rejectionDetail : libraryManager.feedback.detail || ""
                pending: libraryManager.operationPending
                failed: libraryManager.rejectionMessage.length > 0 || libraryManager.feedback.state === "failed"
            }
            FerricManagerFeedback {
                objectName: "libraryLoadingFeedback"
                Layout.fillWidth: true
                browserWindow: libraryManager.browserWindow
                message: libraryManager.listPending ? "Loading library…" : libraryManager.listError.length > 0 ? "The library couldn't be loaded. Refresh to try again." : ""
                detail: libraryManager.listError
                pending: libraryManager.listPending
                failed: !libraryManager.listPending && libraryManager.listError.length > 0
            }
            Label {
                objectName: "libraryEmptyState"
                Layout.fillWidth: true
                visible: libraryManager.totalEntries === 0 && !libraryManager.listPending && libraryManager.listError.length === 0
                text: ChromePresentation.libraryEmptyMessage(libraryManager.libraryKind)
                textFormat: Text.PlainText
                color: libraryManager.browserWindow.secondaryTextColor
                wrapMode: Text.Wrap
            }
            Item { implicitHeight: 6 * libraryManager.scale }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8 * libraryManager.scale
        Flow {
            objectName: "libraryToolbar"
            Layout.fillWidth: true
            Layout.preferredHeight: childrenRect.height
            spacing: 6 * libraryManager.scale
            Button { visible: libraryManager.libraryKind === "journey"; text: libraryManager.graphMode ? "Outline" : "Relationships"; onClicked: libraryManager.graphToggleRequested() }
            Button { visible: libraryManager.libraryKind === "journey"; text: "Export"; Accessible.name: "Export journey records"; onClicked: libraryManager.exportRequested() }
            Button { text: "Refresh"; enabled: !libraryManager.listPending; onClicked: { libraryManager.deleteCancelled(); libraryManager.refreshRequested() } }
            Button {
                visible: libraryManager.libraryKind !== "journey" && libraryManager.totalEntries > libraryManager.pageSize
                text: "Previous"; enabled: libraryManager.page > 0
                Accessible.name: "Previous library page"
                onClicked: libraryManager.pageChangeRequested(-1)
            }
            Label {
                visible: libraryManager.libraryKind !== "journey" && libraryManager.totalEntries > libraryManager.pageSize
                text: "Page " + (libraryManager.page + 1) + " / " + libraryManager.pageCount
                color: libraryManager.browserWindow.mutedTextColor
                height: implicitHeight + 8 * libraryManager.scale
                verticalAlignment: Text.AlignVCenter
            }
            Button {
                visible: libraryManager.libraryKind !== "journey" && libraryManager.totalEntries > libraryManager.pageSize
                text: "Next"; enabled: libraryManager.page + 1 < libraryManager.pageCount
                Accessible.name: "Next library page"
                onClicked: libraryManager.pageChangeRequested(1)
            }
            Button { objectName: "libraryCloseButton"; text: "Close"; Accessible.name: "Close library manager"; onClicked: libraryManager.closeRequested() }
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
            operationPending: libraryManager.operationPending
            controlsHeader: libraryControls
            onFavouriteRequested: function(id) {
                if (libraryManager.browserWindow.primaryBrowserUi.start_page_pick_bookmark(id)) {
                    libraryManager.closeRequested()
                    libraryManager.browserWindow.completeBrowserKeyAction(libraryManager.browserWindow.primaryBrowserUi, libraryManager.browserWindow)
                }
            }
            onOpenRequested: function(kind, id) { libraryManager.entryOpenRequested(kind, id) }
            onPrivateHistoryTransferRequested: function(id, title, url) { libraryManager.privateHistoryTransferRequested(id, title, url) }
            onEditRequested: function(kind, id, value) { libraryManager.entryEditRequested(kind, id, value) }
            onDeleteRequested: function(kind, id) { libraryManager.entryDeleteRequested(kind, id) }
            onJourneyReopenRequested: function(id, target) { libraryManager.journeyReopenRequested(id, target) }
            onJourneyExpandRequested: function(id) { libraryManager.journeyExpandRequested(id) }
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
            controlsHeader: libraryControls
        }
    }

    FerricJourneyExportPreview {
        browserWindow: libraryManager.browserWindow
        onChooseFileRequested: libraryManager.journeyExportFileRequested()
        onCancelRequested: libraryManager.journeyExportCancelRequested()
    }

    FerricPrivateHistoryTransfer {
        browserWindow: libraryManager.browserWindow
        transferPending: libraryManager.browserWindow.privateBookmarkTransferPending === true
        transferMessage: String(libraryManager.browserWindow.privateBookmarkTransferMessage || "")
        transferDetail: String(libraryManager.browserWindow.privateBookmarkTransferDetail || "")
        transferFailed: libraryManager.browserWindow.privateBookmarkTransferFailed === true
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

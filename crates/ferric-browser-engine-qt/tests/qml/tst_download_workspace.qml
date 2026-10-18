import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"

TestCase {
    id: test
    name: "DownloadWorkspace"
    when: windowShown
    property var workspace: null
    property var openedUi: null
    property string openedUri: ""
    property string requestedAction: ""
    property string requestedId: ""
    property int sourceLookups: 0
    ApplicationWindow {
        id: host
        width: 900
        height: 700
        visible: true
        property real chromeScale: 1
        property color backgroundColor: "#181825"
        property color panelColor: "#1e1e2e"
        property color surfaceColor: "#313244"
        property color primaryTextColor: "#cdd6f4"
        property color secondaryTextColor: "#bac2de"
        property color mutedTextColor: "#a6adc8"
        property color accentColor: "#89b4fa"
        property color warningColor: "#f9e2af"
        property color borderColor: "#585b70"
        property var profileResourceRegistry: QtObject {
            property var downloadSource: ({supported:true})
            function downloadFor(ui, id) { compare(ui, transferUi); return null }
        }
        function boundedPageDialogText(text) { return String(text || "") }
        function openExternalUri(ui, uri) { test.openedUi = ui; test.openedUri = uri }
        function executePendingEngineAction() {}
    }
    QtObject {
        id: transferUi
        property var rows: []
        property string history: "ready"
        property bool library_pending: false
        property string library_error: ""
        property bool profile_bootstrap_pending: false
        property string status_text: ""
        property string failedRequest: ""
        function fail_download_request(token) { failedRequest = token; return true }
        function download_request_source_index(token) { ++test.sourceLookups; return -1 }
        property string download_desktop_uri: "file:///owned/Report.pdf"
        function take_download_request() { return false }
        function download_rows() { return rows }
        function download_history_status() { return history }
        function refresh_download_history() { history = "loading" }
        function download_desktop_action(id, reveal) { test.requestedId = id; return true }
        function execute_ui_action(action, id) { test.requestedAction = action; test.requestedId = id; return true }
    }
    Component {
        id: fixture
        FerricDownloadWorkspace {
            browserWindow: host
            hostWindow: host
            rootWindow: host
            browserUi: transferUi
        }
    }
    Component {
        id: initiatorFixture
        FerricDownloadInitiator {
            rootWindow: host
            hostWindow: host
            browserUi: transferUi
        }
    }
    Component {
        id: retiredHelperFixture
        QtObject {
            property string phase: "retiring"
            property bool viewRetired: true
            property bool adopted: false
            function stop() {}
        }
    }
    function test_retired_controller_releases_admission_without_its_cancelled_callback() {
        var initiator = createTemporaryObject(initiatorFixture, host.contentItem)
        var helper = createTemporaryObject(retiredHelperFixture, initiator)
        verify(initiator && helper)
        initiator.ownedHelpers = [helper]
        compare(initiator.ownedHelpers.length, 1)
        initiator.release(helper)
        compare(initiator.ownedHelpers.length, 0)
        compare(initiator.helpers.length, 0)
        verify(!initiator.busy)
    }
    function test_unavailable_downloads_settle_before_creating_a_source() {
        host.profileResourceRegistry.downloadSource = ({supported:false})
        var initiator = createTemporaryObject(initiatorFixture, host.contentItem)
        verify(initiator)
        verify(!initiator.start("unsupported-build-request", "http://127.0.0.1/unused"))
        compare(transferUi.failedRequest, "unsupported-build-request")
        verify(transferUi.status_text.indexOf("Downloads are unavailable") >= 0)
        compare(test.sourceLookups, 0)
        compare(initiator.helpers.length, 0)
        verify(!initiator.busy)
    }
    function record(id, state) {
        return {id:id,state:state,bytes:"2048",destination:"/owned/Report.pdf",displayName:"Report <final>.pdf",
                active:state === "in-progress",retryAvailable:state === "cancelled" || state === "interrupted",fileReady:state === "completed",finishing:false}
    }
    function init() {
        test.sourceLookups = 0
        transferUi.failedRequest = ""
        transferUi.status_text = ""
        host.profileResourceRegistry.downloadSource = ({supported:true})
        transferUi.rows = []
        transferUi.history = "ready"
        transferUi.library_pending = false
        transferUi.library_error = ""
        transferUi.profile_bootstrap_pending = false
        test.openedUi = null
        test.openedUri = ""
        test.requestedAction = ""
        test.requestedId = ""
        workspace = createTemporaryObject(fixture, host.contentItem)
        verify(workspace)
    }
    function cleanup() { workspace.close(); workspace = null }
    function row() {
        var list = findChild(workspace, "downloadList")
        tryCompare(list, "count", 1)
        tryVerify(function() { return list.itemAtIndex(0) !== null })
        return list.itemAtIndex(0)
    }
    function test_live_controls_keep_the_invoking_ui_when_history_fails() {
        transferUi.rows = [record("live", "in-progress")]
        transferUi.history = "unavailable"
        workspace.show()
        var delegate = row()
        compare(findChild(delegate, "downloadFileName").text, "Report <final>.pdf")
        mouseClick(findChild(delegate, "downloadPauseButton"))
        compare(requestedAction, "browser.download.pause")
        compare(requestedId, "live")
        mouseClick(findChild(delegate, "downloadCancelButton"))
        compare(requestedAction, "browser.download.cancel")
    }
    function test_rate_samples_are_released_when_transport_or_row_disappears() {
        workspace.rateSamples = ({live:{bytes:123,time:1,rate:7},removed:{bytes:456,time:1,rate:7}})
        var value = record("live", "completed")
        value.total = 2048
        transferUi.rows = [value]
        workspace.refresh()
        compare(Object.keys(workspace.rateSamples).length, 0)
        compare(workspace.downloadsModel.get(0).speed, 0)
        compare(workspace.downloadsModel.get(0).total, 2048)
        transferUi.rows = []
        workspace.refresh()
        compare(workspace.downloadsModel.count, 0)
    }
    function test_known_completed_file_stays_usable_during_history_refresh_and_failure() {
        transferUi.rows = [record("complete", "completed")]
        transferUi.history = "loading"
        workspace.show()
        var open = findChild(row(), "downloadOpenButton")
        verify(open.enabled)
        transferUi.history = "unavailable"
        workspace.refresh()
        verify(open.enabled)
        mouseClick(open)
        compare(requestedId, "complete")
        compare(openedUi, transferUi)
        compare(openedUri, "file:///owned/Report.pdf")
    }
    function test_progress_keeps_delegate_identity_and_details_disclosure() {
        transferUi.rows = [record("live", "in-progress")]
        workspace.show()
        var original = row()
        mouseClick(findChild(original, "downloadDetailsButton"))
        verify(original.detailsExpanded)
        var updated = record("live", "paused")
        updated.active = true
        updated.bytes = "4096"
        transferUi.rows = [updated]
        workspace.refresh()
        compare(row(), original)
        verify(original.detailsExpanded)
        compare(original.bytes, "4096")
        compare(findChild(original, "downloadPauseButton").text, "Resume")
    }
    function test_private_empty_state_is_truthful_and_new_profile_clears_rows() {
        transferUi.history = "not-stored"
        workspace.show()
        var empty = findChild(workspace, "downloadsEmptyState")
        verify(empty.visible)
        verify(empty.text.indexOf("No downloads yet") >= 0)
        transferUi.rows = [record("private", "completed")]
        workspace.refresh()
        compare(workspace.downloadsModel.count, 1)
        workspace.metrics("private", {receivedBytes:2048,totalBytes:4096,interruptReasonString:""})
        verify(Object.keys(workspace.rateSamples).length > 0)
        var other = createTemporaryObject(fixture,host.contentItem)
        verify(other)
        compare(Object.keys(other.rateSamples).length,0)
        transferUi.profile_bootstrap_pending = true
        tryCompare(workspace, "active", false)
        compare(Object.keys(workspace.rateSamples).length,0)
        compare(workspace.downloadsModel.count, 0)
    }
    function test_finishing_file_can_be_cancelled_and_cannot_be_opened() {
        var value = record("finishing", "in-progress")
        value.finishing = true
        value.active = true
        transferUi.rows = [value]
        workspace.show()
        var delegate = row()
        verify(!findChild(delegate, "downloadPauseButton").visible)
        verify(!findChild(delegate, "downloadOpenButton").enabled)
        var cancel = findChild(delegate, "downloadCancelButton")
        verify(cancel.visible)
        mouseClick(cancel)
        compare(requestedAction, "browser.download.cancel")
        compare(requestedId, "finishing")
    }
    function test_terminal_details_survive_the_native_object() {
        var value = record("failed", "interrupted")
        value.total = "8192"
        value.reason = "The file couldn't be saved. Check the save folder and retry."
        transferUi.rows = [value]
        workspace.show()
        compare(row().total, 8192)
        compare(row().reason, value.reason)
    }
}

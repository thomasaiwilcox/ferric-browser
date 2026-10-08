import QtQuick
import QtTest
import QtWebEngine
import "../../qml/components"

TestCase {
    name: "VersioningRestoreCoordinator"
    when: windowShown

    QtObject {
        id: testProfile
    }

    QtObject {
        id: testView
        property var profile: testProfile
        property url url: "https://example.test/"
        property bool loading: true
        property bool visible: true
        property bool enabled: true
        property int lifecycleState: WebEngineView.LifecycleState.Active
        property int versioningNavigationEpoch: 1
        property var pendingCallback: null

        function runJavaScript(script, worldId, callback) {
            pendingCallback = callback
        }
    }

    QtObject {
        id: testHostWindow
        property int width: 640
        property int height: 480
        property var browserProfile: testProfile
        property int browserScriptWorld: 1
        property bool temporaryProfile: false
        property bool ephemeralProfile: false
        property bool versioningRestoreEditableDialogVisible: false

        function tabViewAt(index) {
            return index === 0 ? testView : null
        }

        function activeWebView() {
            return testView
        }
    }

    QtObject {
        id: testBrowserUi
        property int active_tab_index: 0
        property int tab_count: 1
        property string current_url: testView.url.toString()
        property string status_text: ""
        property int discardedRequests: 0
        property var discardedRequestIds: []
        property string editablePreviewJson: ""
        property string confirmedEditableRequest: ""

        function tab_index_for_id(tabId) {
            return tabId === "destination-tab" ? 0 : -1
        }

        function tab_id_for_index(index) {
            return index === 0 ? "destination-tab" : ""
        }

        function discard_versioned_restore_sidecar(requestId) {
            discardedRequests += 1
            discardedRequestIds = discardedRequestIds.concat([String(requestId)])
        }

        function versioned_restore_editable_preview(requestId) {
            return requestId === "123" ? editablePreviewJson : ""
        }

        function confirm_versioned_restore_editable_preview(requestId) {
            confirmedEditableRequest = String(requestId)
            return true
        }

        function versioned_restore_geometry_capture_request(requestId, tabId) {
            if (requestId !== "123" || tabId !== "destination-tab")
                return ""
            return JSON.stringify({
                expected_origin: "https://example.test",
                expected_route: "/",
                route_fingerprint: "a".repeat(64),
                capture_fingerprint: "b".repeat(64),
                nonce: "0123456789abcdef0123456789abcdef",
                max_nodes: 4096,
                max_candidates: 64,
                max_ms: 50,
                include_controls: false,
                include_editable: false,
                include_selection: false,
                include_media: false
            })
        }
    }

    Component {
        id: coordinatorComponent
        FerricVersioningRestoreCoordinator { }
    }

    Component {
        id: hostComponent
        FerricVersioningRestoreHost { }
    }

    function init() {
        testView.loading = true
        testView.versioningNavigationEpoch = 1
        testView.pendingCallback = null
        testBrowserUi.active_tab_index = 0
        testBrowserUi.discardedRequests = 0
        testBrowserUi.discardedRequestIds = []
        testBrowserUi.editablePreviewJson = JSON.stringify({
            items: [{ kind: "content-editable", value: "<b>synthetic text-only draft</b>" }],
            omitted: 0
        })
        testBrowserUi.confirmedEditableRequest = ""
        testBrowserUi.status_text = ""
    }

    function test_host_defers_loading_until_first_request_and_reuses_coordinator() {
        var host = createTemporaryObject(hostComponent, this, {
            hostWindow: testHostWindow,
            browserUi: testBrowserUi
        })
        verify(host !== null)
        verify(!host.active)
        compare(host.source.toString(), "")
        compare(host.item, null)

        verify(host.handleSidecarReady(
                   "versioning-sidecar-ready\t123\tdestination-tab", false))
        verify(host.active)
        compare(host.status, Loader.Ready)
        var coordinator = host.item
        verify(coordinator !== null)
        compare(coordinator.requestId, "123")
        compare(coordinator.destinationTabId, "destination-tab")
        verify(!coordinator.editableRestoreEnabled)

        verify(host.handleSidecarReady(
                   "versioning-sidecar-ready\t456\tdestination-tab", false))
        compare(host.item, coordinator)
        compare(coordinator.requestId, "456")
        compare(testBrowserUi.discardedRequestIds, ["123"])
        coordinator.clear(true)
    }

    function test_host_forwards_one_shot_editable_consent() {
        var host = createTemporaryObject(hostComponent, this, {
            hostWindow: testHostWindow,
            browserUi: testBrowserUi
        })
        verify(host !== null)
        verify(host.handleSidecarReady(
                   "versioning-sidecar-ready\t123\tdestination-tab", true))
        compare(host.item.previewItems.length, 1)
        verify(testHostWindow.versioningRestoreEditableDialogVisible)
        verify(!host.item.editableRestoreEnabled)
        host.item.finishEditableReview(false)

        verify(host.handleSidecarReady(
                   "versioning-sidecar-ready\t456\tdestination-tab", false))
        compare(host.item.previewItems.length, 0)
        verify(!testHostWindow.versioningRestoreEditableDialogVisible)
        compare(host.item.requestId, "456")
        host.item.clear(true)
    }

    function test_readiness_window_times_out_and_discards_exact_sidecar() {
        var coordinator = coordinatorComponent.createObject(this, {
            hostWindow: testHostWindow,
            browserUi: testBrowserUi
        })
        verify(coordinator !== null)

        coordinator.startGeometry("123", "destination-tab")
        for (var attempt = 0; attempt < 26; ++attempt)
            coordinator.advanceGeometry()

        compare(coordinator.requestId, "")
        compare(testBrowserUi.discardedRequests, 1)
        compare(testBrowserUi.status_text,
                "Geometry restore skipped: page was not ready in time")
        coordinator.destroy()
    }

    function test_stale_destination_callback_discards_sidecar_before_apply() {
        var coordinator = coordinatorComponent.createObject(this, {
            hostWindow: testHostWindow,
            browserUi: testBrowserUi
        })
        verify(coordinator !== null)
        testView.loading = false

        coordinator.startGeometry("123", "destination-tab")
        coordinator.advanceGeometry()
        verify(testView.pendingCallback !== null)

        testBrowserUi.active_tab_index = 1
        testView.pendingCallback("{}")

        compare(coordinator.requestId, "")
        compare(testBrowserUi.discardedRequests, 1)
        compare(testBrowserUi.discardedRequestIds.length, 1)
        compare(testBrowserUi.discardedRequestIds[0], "123")
        coordinator.destroy()
    }

    function test_old_callback_cannot_clear_a_newer_restore_request() {
        var coordinator = coordinatorComponent.createObject(this, {
            hostWindow: testHostWindow,
            browserUi: testBrowserUi
        })
        verify(coordinator !== null)
        testView.loading = false

        coordinator.startGeometry("123", "destination-tab")
        coordinator.advanceGeometry()
        verify(testView.pendingCallback !== null)
        var oldCallback = testView.pendingCallback

        coordinator.clear(true)
        coordinator.startGeometry("456", "destination-tab")
        oldCallback("{}")

        compare(coordinator.requestId, "456")
        compare(coordinator.destinationTabId, "destination-tab")
        compare(testBrowserUi.discardedRequests, 2)
        compare(testBrowserUi.discardedRequestIds.length, 2)
        compare(testBrowserUi.discardedRequestIds[0], "123")
        compare(testBrowserUi.discardedRequestIds[1], "123")
        coordinator.destroy()
    }

    function test_contenteditable_preview_is_visible_but_remains_display_only() {
        var coordinator = coordinatorComponent.createObject(this, {
            hostWindow: testHostWindow,
            browserUi: testBrowserUi
        })
        verify(coordinator !== null)
        coordinator.editableRestoreEnabled = true

        verify(coordinator.handleSidecarReady(
                   "versioning-sidecar-ready\t123\tdestination-tab"))
        compare(coordinator.previewItems.length, 1)
        compare(coordinator.previewItems[0].kind, "content-editable")
        compare(coordinator.previewItems[0].displayValue,
                "<b>synthetic text-only draft</b>")
        compare(coordinator.previewRefillableCount, 0)
        verify(testHostWindow.versioningRestoreEditableDialogVisible)
        var previewRow = coordinator.editablePreviewList.itemAt(0)
        verify(previewRow !== null)
        var draftText = previewRow.displayValueControl
        verify(draftText !== null)
        verify(draftText.readOnly)
        verify(draftText.selectByMouse)
        compare(draftText.textFormat, TextEdit.PlainText)
        var confirmButton = findChild(
                    coordinator, "versioningEditableRestoreConfirmButton")
        verify(confirmButton !== null)
        compare(confirmButton.text, "Continue without refill")

        coordinator.finishEditableReview(false)
        compare(testBrowserUi.confirmedEditableRequest, "")
        compare(coordinator.previewItems.length, 0)
        verify(!testHostWindow.versioningRestoreEditableDialogVisible)
        coordinator.destroy()
    }
}

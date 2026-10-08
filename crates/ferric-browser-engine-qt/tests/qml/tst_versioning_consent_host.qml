import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"

TestCase {
    name: "VersioningConsentHost"
    when: windowShown
    width: 900
    height: 700

    ApplicationWindow {
        id: hostWindow
        visible: true
        width: 900
        height: 700
        property real chromeScale: 1
        property color panelColor: "#181825"
        property color borderColor: "#585b70"
        property color mutedTextColor: "#a6adc8"
        property color warningColor: "#f9e2af"
        property color accentColor: "#89b4fa"
        property color primaryTextColor: "#cdd6f4"
        property color secondaryTextColor: "#bac2de"
        property color selectionTextColor: "#cdd6f4"
        property color selectionColor: "#45475a"
        property color surfaceColor: "#313244"
        property color errorColor: "#f38ba8"
        property var nativePageCaptureDisclosure: []
        property var nativeStashDisclosure: []
        property var browserUi: adapter
        property int scheduled: 0
        function scheduleRuntimeWork(delay) {
            compare(delay, 0)
            scheduled += 1
        }
    }

    QtObject {
        id: adapter
        property string called: ""
        property string receivedId: ""
        property bool result: true
        function accept_native_page_capture_disclosure(operationId) {
            called = "accept-page"
            receivedId = operationId
            return result
        }
        function decline_native_page_capture_disclosure(operationId) {
            called = "decline-page"
            receivedId = operationId
            return result
        }
        function decline_native_stash_disclosure(operationId) {
            called = "decline-stash"
            receivedId = operationId
            return result
        }
    }

    Component {
        id: consentFactory
        FerricVersioningConsentHost { browserWindow: hostWindow }
    }

    function init() {
        hostWindow.nativePageCaptureDisclosure = []
        hostWindow.nativeStashDisclosure = []
        hostWindow.scheduled = 0
        adapter.called = ""
        adapter.receivedId = ""
        adapter.result = true
    }

    function showPageConsent() {
        hostWindow.nativePageCaptureDisclosure = ["page-operation", "Synthetic disclosure"]
        var host = createTemporaryObject(consentFactory, hostWindow.contentItem)
        verify(host)
        tryCompare(host, "status", Loader.Ready)
        verify(host.item)
        return host
    }

    function test_page_accept_routes_exact_operation_and_releases_dialog() {
        var host = showPageConsent()
        host.item.requestAction("accept")
        compare(adapter.called, "accept-page")
        compare(adapter.receivedId, "page-operation")
        compare(hostWindow.nativePageCaptureDisclosure.length, 0)
        compare(hostWindow.scheduled, 1)
        tryCompare(host, "active", false)
        compare(host.item, null)
    }

    function test_page_cancel_routes_exact_operation() {
        var host = showPageConsent()
        host.item.cancel()
        compare(adapter.called, "decline-page")
        compare(adapter.receivedId, "page-operation")
        compare(hostWindow.nativePageCaptureDisclosure.length, 0)
        compare(hostWindow.scheduled, 1)
    }

    function test_rejected_native_decision_clears_stale_disclosure_and_refreshes() {
        var host = showPageConsent()
        adapter.result = false
        host.item.requestAction("accept")
        compare(adapter.called, "accept-page")
        compare(hostWindow.nativePageCaptureDisclosure.length, 0)
        compare(hostWindow.scheduled, 1)
        tryCompare(host, "active", false)
    }

    function test_rejected_page_decline_refreshes_from_rust() {
        var host = showPageConsent()
        adapter.result = false
        host.item.cancel()
        compare(adapter.called, "decline-page")
        compare(hostWindow.nativePageCaptureDisclosure.length, 0)
        compare(hostWindow.scheduled, 1)
        tryCompare(host, "active", false)
    }

    function test_stale_page_decision_cannot_act_on_replacement_disclosure() {
        var host = showPageConsent()
        hostWindow.nativePageCaptureDisclosure = ["replacement-operation", "New disclosure"]
        host.item.acceptRequested("page-operation")
        compare(adapter.called, "")
        compare(hostWindow.nativePageCaptureDisclosure[0], "replacement-operation")
        compare(hostWindow.scheduled, 0)
    }

    function test_stash_cannot_be_approved_and_cancel_keeps_page_visible() {
        hostWindow.nativeStashDisclosure = ["stash-operation", "Synthetic retention disclosure"]
        var host = createTemporaryObject(consentFactory, hostWindow.contentItem)
        verify(host)
        tryCompare(host, "status", Loader.Ready)
        compare(host.item.acceptEnabled, false)
        host.item.requestAction("accept")
        compare(adapter.called, "")
        host.item.cancel()
        compare(adapter.called, "decline-stash")
        compare(adapter.receivedId, "stash-operation")
        compare(hostWindow.nativeStashDisclosure.length, 0)
        compare(hostWindow.scheduled, 1)
    }

    function test_incomplete_disclosure_does_not_construct_dialog() {
        hostWindow.nativePageCaptureDisclosure = ["page-operation"]
        var host = createTemporaryObject(consentFactory, hostWindow.contentItem)
        verify(host)
        compare(host.active, false)
        compare(host.item, null)
        compare(adapter.called, "")
    }
}

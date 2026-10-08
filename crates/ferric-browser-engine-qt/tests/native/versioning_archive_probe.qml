import QtQuick
import QtQuick.Window
import QtWebEngine
import "../../qml/components" as FerricComponents
import "../../qml/scripts/NativePageCaptureCallbacks.js" as NativePageCaptureCallbacks

// Loaded only by versioning_archive_probe.cpp. The disk-backed source profile
// uses paths beneath its disposable operation directory; this deliberately
// does not use Ferric's normal profile.
Window {
    id: root
    objectName: "archiveProbe"
    title: "FerricArchiveProbe"
    width: 640
    height: 480
    visible: true

    property string expectedSavePath: ""
    property string ordinaryDirectory: ""
    property string ordinaryFileName: "ordinary.txt"
    property bool sourceLoaded: false
    property bool sourceFailed: false
    property string sourceCurrentUrl: sourceView.url.toString()
    property bool sourceJavascriptEnabled: true
    property int sourceLinkNavigations: 0
    property int sourceFormNavigations: 0
    property bool sourceControlInspected: false
    property bool sourceControlClicked: false
    property bool archiveLoaded: false
    property bool archiveFailed: false
    property int saveRequests: 0
    property int ordinaryRequests: 0
    property int saveFinishedState: -1
    property bool saveFinishedIsFinished: false
    property bool cancelNextSave: false
    property int cancelledRequestState: -1
    property bool cancelledRequestIsFinished: false
    property int cancelledRequestId: -1
    property int cancelledFinishState: -1
    property bool cancelledFinishIsFinished: false
    property int activeSaveId: -1
    property int activeSaveState: -1
    property bool activeSaveIsFinished: false
    property var activeSaveItem: null
    property int ordinaryFinishedState: -1
    property int lastFinishedDownloadId: -1
    property int lastFinishedDownloadState: -1
    property bool saveMatchedView: false
    property bool saveMatchedPath: false
    property bool saveMatchedFormat: false
    property var nativePageCaptureSave: null
    property var nativePageCaptureRetiredDownloads: ({})
    property var nativePageCaptureRetiredDownloadOrder: []
    property var nativePageCaptureRetiredPaths: ({})
    property var nativePageCaptureRetiredPathOrder: []
    property var nativePageCapturePoisonedTabIds: []
    property int productionSaveRequestedId: -1
    property int productionSaveFinishedState: -1
    property int productionSaveAdmissionCalls: 0
    property bool productionSaveAdmissionExact: false
    property bool productionSaveRefuseNext: false
    property bool productionSaveRejected: false
    property bool productionSaveRetiredExact: false
    property bool productionSaveSameTabRetryBlocked: false
    property int productionSaveCancelledOperations: 0
    property int productionSaveRefusedId: -1
    property int productionSaveRefusedFinishedState: -1
    property int productionSaveRefusedStateAfterCancel: -1
    property bool productionSaveRefusedIsFinishedAfterCancel: false
    property bool holdOrdinaryForProductionSave: false
    property bool ordinaryRequestHeld: false
    property var heldOrdinaryRequest: null
    property bool ordinaryReleasedWhileSavePending: false
    property int archiveDownloadRequests: 0
    property int archivePopupRequests: 0
    property int archiveRejectedNavigations: 0
    property int archiveRejectedRedirectNavigations: 0
    property int archiveFormNavigations: 0
    property int archiveRedirectNavigations: 0
    property string lastFormNavigationUrl: ""
    property bool diagnosticAllowRemoteNavigation: false
    property bool archiveInitialNavigationPending: false
    property string lastNavigationUrl: ""
    property string archiveTitle: archiveView.title
    property bool archiveFocusOnNavigationEnabled: archiveView.settings.focusOnNavigationEnabled
    property bool archiveJavascriptEnabled: archiveView.settings.javascriptEnabled
    property bool archiveLocalRemoteEnabled: archiveView.settings.localContentCanAccessRemoteUrls
    property bool archiveLocalFilesEnabled: archiveView.settings.localContentCanAccessFileUrls
    property bool archiveHasActiveFocus: archiveView.activeFocus
    property bool productionArchiveMode: false
    property string productionArchiveUrl: ""
    property bool productionArchiveLoaded: false
    property bool productionArchiveFailed: false
    property bool productionArchiveBlanked: false
    property bool productionArchiveBlankTimedOut: false
    property string productionArchiveTitle: productionArchiveLoader.item
                                             ? productionArchiveLoader.item.archiveTitle : ""
    property var productionArchiveItem: productionArchiveLoader.item
    property int productionArchiveRejected: productionArchiveLoader.item
                                             ? productionArchiveLoader.item.rejectedNavigationCount : 0
    property color backgroundColor: "#181825"
    property color mutedTextColor: "#a6adc8"
    property bool integratedArchiveMode: false
    property bool integratedArchivePresented: false
    property int integratedCleanupCalls: 0
    property bool integratedCleanupExact: true
    property int integratedRuntimeSchedules: 0
    property bool integratedFailNextClose: false
    property bool integratedArchiveActive: integratedArchiveLoader.item
                                           ? integratedArchiveLoader.item.active : false

    function scheduleRuntimeWork(delay) {
        if (delay === 0) integratedRuntimeSchedules++
    }

    WebEngineProfile {
        id: sourceEngineProfile
        offTheRecord: false
        storageName: "FerricArchiveProbeSource"
        persistentStoragePath: archiveProbeSourceProfileStoragePath
        cachePath: archiveProbeSourceProfileCachePath
        httpCacheType: WebEngineProfile.DiskHttpCache
    }
    WebEngineProfile { id: archiveEngineProfile; offTheRecord: true }
    property var sourceProfile: sourceEngineProfile
    property var archiveProfile: archiveEngineProfile

    QtObject {
        id: productionCaptureBridge
        property string status_text: ""
        function note_native_page_capture_download_requested(operation, id, path,
                                                              isSavePage, isMhtml, permitMatches) {
            root.productionSaveAdmissionCalls++
            root.productionSaveAdmissionExact = operation === "native-probe"
                    && /^\d{1,20}$/.test(id)
                    && path === root.expectedSavePath
                    && isSavePage && isMhtml && permitMatches
            var accepted = root.productionSaveAdmissionExact
                    && !root.productionSaveRefuseNext
            root.productionSaveRefuseNext = false
            return accepted
        }
        function cancel_native_page_capture_request(operation) {
            root.productionSaveCancelledOperations++
            root.nativePageCaptureSave = null
        }
    }

    function isRetiredNativePageCapturePath(path) {
        return NativePageCaptureCallbacks.isRetiredPath(root, path)
    }
    function retireNativePageCaptureDownload(id) {
        return NativePageCaptureCallbacks.retireDownload(root, id, 128)
    }
    function retireNativePageCapturePath(path) {
        return NativePageCaptureCallbacks.retirePath(root, path, 128)
    }
    function rejectNativePageCaptureRequest(capture, download, id) {
        productionSaveRefusedId = Number(id)
        var rejected = NativePageCaptureCallbacks.rejectCaptureRequest(
                    root, productionCaptureBridge, capture, download, id)
        productionSaveSameTabRetryBlocked =
                NativePageCaptureCallbacks.captureTabIsPoisoned(
                    root, "probe-source-tab")
                && !productionCaptureMayStartTab("probe-source-tab")
        productionSaveRefusedStateAfterCancel = download.state
        productionSaveRefusedIsFinishedAfterCancel = download.isFinished
        productionSaveRetiredExact = rejected
                && nativePageCaptureRetiredDownloads[id] === true
                && nativePageCaptureRetiredPaths[capture.path] === true
        productionSaveRejected = rejected
        return rejected
    }
    function productionCaptureMayStartTab(tabId) {
        return !NativePageCaptureCallbacks.captureTabIsPoisoned(root, tabId)
    }
    function retireNativePageCaptureWithoutFinish(capture, download, state) {
        return NativePageCaptureCallbacks.retireTerminalCapture(
                    root, productionCaptureBridge, capture, download, state)
    }
    function discardNativePageCaptureMutationProbe(capture) {}
    function downloadStateName(download) {
        if (download.state === WebEngineDownloadRequest.DownloadCancelled)
            return "cancelled"
        if (download.state === WebEngineDownloadRequest.DownloadInterrupted)
            return "interrupted"
        if (download.state === WebEngineDownloadRequest.DownloadCompleted)
            return "completed"
        return "in-progress"
    }
    Connections {
        target: root.sourceProfile
        function onDownloadRequested(item) {
            if (NativePageCaptureCallbacks.routeSaveRequest(
                        root, productionCaptureBridge, item,
                        WebEngineDownloadRequest.MimeHtmlSaveFormat)) {
                if (root.nativePageCaptureSave
                        && root.nativePageCaptureSave.download === item) {
                    root.productionSaveRequestedId = item.id
                    if (root.heldOrdinaryRequest) {
                        var held = root.heldOrdinaryRequest
                        root.heldOrdinaryRequest = null
                        root.ordinaryRequestHeld = false
                        root.holdOrdinaryForProductionSave = false
                        Qt.callLater(function() {
                            root.ordinaryReleasedWhileSavePending =
                                    root.productionSaveFinishedState < 0
                            held.downloadDirectory = root.ordinaryDirectory
                            held.downloadFileName = root.ordinaryFileName
                            held.accept()
                        })
                    }
                }
                return
            }
            if (item.isSavePageDownload) {
                root.saveRequests++
                root.saveMatchedView = item.view === sourceView
                root.saveMatchedPath =
                        item.downloadDirectory + "/" + item.downloadFileName
                        === root.expectedSavePath
                root.saveMatchedFormat = item.savePageFormat
                        === WebEngineDownloadRequest.MimeHtmlSaveFormat
                // WebEngineView.save() auto-accepts its page-save request.
                if (root.cancelNextSave) {
                    root.cancelNextSave = false
                    root.cancelledRequestId = item.id
                    item.cancel()
                    root.cancelledRequestState = item.state
                    root.cancelledRequestIsFinished = item.isFinished
                } else {
                    root.activeSaveId = item.id
                    root.activeSaveItem = item
                    root.activeSaveState = item.state
                    root.activeSaveIsFinished = item.isFinished
                    item.stateChanged.connect(function(state) {
                        if (item.id === root.activeSaveId) {
                            root.activeSaveState = state
                            root.activeSaveIsFinished = item.isFinished
                        }
                    })
                }
            } else {
                root.ordinaryRequests++
                if (root.holdOrdinaryForProductionSave) {
                    root.heldOrdinaryRequest = item
                    root.ordinaryRequestHeld = true
                    return
                }
                item.downloadDirectory = root.ordinaryDirectory
                item.downloadFileName = root.ordinaryFileName
                item.accept()
            }
        }
        function onDownloadFinished(item) {
            root.lastFinishedDownloadId = item.id
            root.lastFinishedDownloadState = item.state
            if (item.id === root.productionSaveRefusedId)
                root.productionSaveRefusedFinishedState = item.state
            if (item.id === root.productionSaveRequestedId) {
                root.productionSaveFinishedState = item.state
                root.productionSaveRequestedId = -1
                root.nativePageCaptureSave = null
                return
            }
            if (item.isSavePageDownload) {
                if (item.id === root.cancelledRequestId) {
                    root.cancelledFinishState = item.state
                    root.cancelledFinishIsFinished = item.isFinished
                } else if (item.id === root.activeSaveId) {
                    root.saveFinishedState = item.state
                    root.saveFinishedIsFinished = item.isFinished
                }
            }
            else
                root.ordinaryFinishedState = item.state
        }
    }

    Connections {
        target: root.archiveProfile
        function onDownloadRequested(item) {
            root.archiveDownloadRequests++
            item.cancel()
        }
    }

    WebEngineView {
        id: sourceView
        anchors.fill: parent
        profile: root.sourceProfile
        settings.javascriptEnabled: root.sourceJavascriptEnabled
        settings.localContentCanAccessRemoteUrls: false
        onNavigationRequested: function(request) {
            if (request.url.toString().endsWith("/link")) {
                root.sourceLinkNavigations++
                request.action = WebEngineNavigationRequest.IgnoreRequest
            } else if (request.url.toString().includes("/form")) {
                root.sourceFormNavigations++
                request.action = WebEngineNavigationRequest.IgnoreRequest
            }
        }
        onLoadingChanged: function(request) {
            if (request.status === WebEngineView.LoadSucceededStatus)
                root.sourceLoaded = true
            else if (request.status === WebEngineView.LoadFailedStatus)
                root.sourceFailed = true
        }
    }

    WebEngineView {
        id: archiveView
        anchors.fill: parent
        visible: false
        profile: root.archiveProfile
        activeFocusOnPress: true
        settings.focusOnNavigationEnabled: true
        settings.javascriptEnabled: false
        settings.javascriptCanOpenWindows: false
        settings.localContentCanAccessRemoteUrls: false
        settings.localContentCanAccessFileUrls: false
        onLoadingChanged: function(request) {
            if (request.status === WebEngineView.LoadSucceededStatus)
                root.archiveLoaded = true
            else if (request.status === WebEngineView.LoadFailedStatus)
                root.archiveFailed = true
        }
        onNavigationRequested: function(request) {
            root.lastNavigationUrl = request.url.toString()
            var isRedirect = request.navigationType
                    === WebEngineNavigationRequest.RedirectNavigation
            var targetsExternalRedirect = root.lastNavigationUrl.endsWith("/redirect")
            if (request.hasFormData
                    || request.navigationType === WebEngineNavigationRequest.FormSubmittedNavigation) {
                root.archiveFormNavigations++
                root.lastFormNavigationUrl = request.url.toString()
            }
            if (isRedirect || targetsExternalRedirect)
                root.archiveRedirectNavigations++
            if (root.archiveInitialNavigationPending
                    && root.lastNavigationUrl === root.allowedArchiveUrl) {
                root.archiveInitialNavigationPending = false
            } else if (!(root.diagnosticAllowRemoteNavigation
                         && root.lastNavigationUrl.startsWith("http://127.0.0.1:"))) {
                root.archiveRejectedNavigations++
                if (isRedirect || targetsExternalRedirect)
                    root.archiveRejectedRedirectNavigations++
                request.action = WebEngineNavigationRequest.IgnoreRequest
            }
        }
        onNewWindowRequested: function(request) {
            root.archivePopupRequests++
            // No acceptAsNewWindow: public API denies the popup.
        }
    }

    Loader {
        id: productionArchiveLoader
        anchors.fill: parent
        active: root.productionArchiveMode
        sourceComponent: Component {
            FerricComponents.FerricVersioningArchiveView {
                authorizedArchiveUrl: root.productionArchiveUrl
                onArchiveReadyChanged: {
                    if (archiveReady) Qt.callLater(openAuthorizedArchive)
                }
                onArchiveLoaded: function(title) { root.productionArchiveLoaded = true }
                onArchiveLoadFailed: root.productionArchiveFailed = true
                onArchiveBlanked: root.productionArchiveBlanked = true
                onArchiveBlankingTimedOut: root.productionArchiveBlankTimedOut = true
            }
        }
    }

    QtObject {
        id: integratedArchiveAdapter
        property string versioning_archive_view_url: ""
        property string versioning_archive_view_operation: "operation:15dff4c6-e7a7-449c-9d50-737a560a78c7"
        property string versioning_archive_source_url: "https://source.example.test/private?token=synthetic"
        property string versioning_archive_captured_at_ms: "1000"
        property string versioning_archive_fidelity_label: "Synthetic archive fidelity"

        function close_versioned_archive_view(operation) {
            root.integratedCleanupCalls++
            root.integratedCleanupExact = root.integratedCleanupExact
                    && operation === versioning_archive_view_operation
            if (!root.integratedCleanupExact) return false
            if (root.integratedFailNextClose) {
                root.integratedFailNextClose = false
                return false
            }
            versioning_archive_view_url = ""
            return true
        }
    }

    Loader {
        id: integratedArchiveLoader
        anchors.fill: parent
        active: root.integratedArchiveMode
        sourceComponent: Component {
            FerricComponents.FerricVersioningArchiveHost {
                browserWindow: root
                browserUi: integratedArchiveAdapter
                onArchivePresented: root.integratedArchivePresented = true
            }
        }
    }

    property string allowedArchiveUrl: ""
    function startSource(url) {
        sourceLoaded = false
        sourceFailed = false
        sourceView.visible = true
        archiveView.visible = false
        sourceView.url = url
    }
    function submitPostSource() {
        sourceLoaded = false
        sourceFailed = false
        sourceView.runJavaScript("document.getElementById('post-form').submit()")
    }
    function focusSource() { sourceView.forceActiveFocus() }
    function inspectSourceControl() {
        sourceControlInspected = false
        sourceView.runJavaScript("document.getElementById('control').getAttribute('data-clicked') === 'yes'",
                                 function(result) {
                                     sourceControlClicked = result === true
                                     sourceControlInspected = true
                                 })
    }
    function saveSource(path) {
        saveFinishedState = -1
        saveFinishedIsFinished = false
        activeSaveState = -1
        activeSaveIsFinished = false
        expectedSavePath = path
        sourceView.save(path, WebEngineDownloadRequest.MimeHtmlSaveFormat)
    }
    function saveSourceThroughProductionRoute(path) {
        expectedSavePath = path
        productionSaveRequestedId = -1
        productionSaveFinishedState = -1
        productionSaveAdmissionCalls = 0
        productionSaveAdmissionExact = false
        productionSaveRejected = false
        productionSaveRetiredExact = false
        productionSaveSameTabRetryBlocked = false
        productionSaveCancelledOperations = 0
        productionSaveRefusedId = -1
        productionSaveRefusedFinishedState = -1
        productionSaveRefusedStateAfterCancel = -1
        productionSaveRefusedIsFinishedAfterCancel = false
        nativePageCaptureSave = {
            operationId: "native-probe",
            profile: sourceProfile,
            path: path,
            view: sourceView,
            tabId: "probe-source-tab",
            downloadId: "",
            download: null,
            downloadFinished: false
        }
        sourceView.save(path, WebEngineDownloadRequest.MimeHtmlSaveFormat)
    }
    function refuseProductionSave(path) {
        productionSaveRefuseNext = true
        saveSourceThroughProductionRoute(path)
    }
    function startHeldOrdinaryDownload() {
        holdOrdinaryForProductionSave = true
        ordinaryRequestHeld = false
        heldOrdinaryRequest = null
        ordinaryReleasedWhileSavePending = false
        ordinaryFinishedState = -1
        startOrdinaryDownload()
    }
    function cancelSourceSave(path) {
        cancelledRequestState = -1
        cancelledRequestIsFinished = false
        cancelledFinishState = -1
        cancelledFinishIsFinished = false
        cancelNextSave = true
        saveSource(path)
    }
    function cancelActiveSave() {
        if (activeSaveItem)
            activeSaveItem.cancel()
    }
    function startOrdinaryDownload() {
        sourceView.runJavaScript("document.querySelector('#ordinary').click()")
    }
    function openArchive(url) {
        archiveLoaded = false
        archiveFailed = false
        allowedArchiveUrl = url
        archiveInitialNavigationPending = true
        sourceView.visible = false
        archiveView.visible = true
        if (archiveView.url.toString() === url)
            archiveView.reload()
        else
            archiveView.url = url
    }
    function focusArchive() { archiveView.forceActiveFocus() }
    function openProductionArchive(url) {
        if (productionArchiveMode && !productionArchiveBlanked) return false
        productionArchiveMode = false
        productionArchiveUrl = url
        productionArchiveLoaded = false
        productionArchiveFailed = false
        productionArchiveBlanked = false
        productionArchiveBlankTimedOut = false
        productionArchiveMode = true
        return productionArchiveLoader.item !== null
    }
    function closeProductionArchive() {
        if (!productionArchiveLoader.item) return false
        productionArchiveLoader.item.closeArchive()
        return true
    }
    function openIntegratedArchive(url) {
        productionArchiveMode = false
        integratedArchivePresented = false
        integratedCleanupCalls = 0
        integratedCleanupExact = true
        integratedRuntimeSchedules = 0
        integratedArchiveAdapter.versioning_archive_view_url = url
        integratedArchiveMode = true
        return integratedArchiveLoader.item !== null
    }
    function failNextIntegratedArchiveClose() {
        integratedFailNextClose = true
    }
    function closeIntegratedArchive() {
        if (!integratedArchiveLoader.item) return false
        integratedArchiveLoader.item.closeArchive()
        return true
    }
}

import QtQuick
import QtQuick.Controls
import QtTest
import QtWebEngine
import "../../qml/components" as FerricComponents
import "../../qml/scripts/ProfileDownloadRouting.js" as ProfileDownloadRouting

TestCase {
    id: probe
    name: "NativeProfileDownload"
    when: windowShown
    width: 640
    height: 400

    property string outputDirectory: "__FERRIC_PROFILE_DOWNLOAD_DIR__"
    property string slowDownloadUrl: "__FERRIC_PROFILE_DOWNLOAD_URL__"
    property var disposableProfile: null
    property var firstView: null
    property var secondView: null
    property var requested: []
    property var finished: []
    property var routedRequests: []
    property var routedFinished: []

    ApplicationWindow {
        id: testWindow
        width: 640
        height: 400
        visible: true
    }

    WebEngineProfilePrototype {
        id: profilePrototype
        storageName: ""
    }

    QtObject {
        id: sourceHost
        property var browserProfile: probe.disposableProfile
        property var tabViews: []
        property var activeDownloads: ({})
        property var pendingDownloadRequests: ({})
        property var downloadCompletionRequests: ({})
        property var nativePageCaptureSave: null
        property var nativePageCaptureRetiredDownloads: ({})
        property var nativePageCaptureRetiredPaths: ({})
        property var versioningWindowOwner: null
        property var popupWindowRegistry: []
    }

    QtObject {
        id: restoredHost
        property var browserProfile: probe.disposableProfile
        property var tabViews: []
        property var activeDownloads: ({})
        property var pendingDownloadRequests: ({})
        property var downloadCompletionRequests: ({})
        property var nativePageCaptureSave: null
        property var nativePageCaptureRetiredDownloads: ({})
        property var nativePageCaptureRetiredPaths: ({})
        property var popupWindowRegistry: []
        property var versioningWindowOwner: null
        property bool visible: true
        property bool shutdownApproved: false
        signal versioningDiscoveryChanged()
    }

    FerricComponents.FerricVersioningWindowOwner {
        id: restoredWindowOwner
    }

    Connections {
        target: probe.disposableProfile
        function onDownloadRequested(download) {
            var routed = probe.resolveNativeDownloadOwner(download)
            var owner = ProfileDownloadRouting.resolve(
                        download, routed.view, sourceHost, routed.restoreHost,
                        [], [], probe.disposableProfile)
            probe.routedRequests.push(owner)
            if (owner) {
                owner.downloadCompletionRequests[String(download.id)] = download
                owner.activeDownloads[String(download.id)] = download
            }
            probe.requested.push({
                request: download,
                id: String(download.id),
                view: routed.view,
                exactPermitPath: routed.exactPermitPath,
                savePage: download.isSavePageDownload
            })
            if (String(download.url) === probe.slowDownloadUrl) {
                download.downloadDirectory = probe.outputDirectory
                download.downloadFileName = "synthetic-slow.bin"
                download.accept()
            }
        }
        function onDownloadFinished(download) {
            var routed = probe.resolveNativeDownloadOwner(download)
            for (var requestIndex = 0;
                 requestIndex < probe.requested.length; ++requestIndex) {
                var priorRequest = probe.requested[requestIndex]
                if (priorRequest.id === String(download.id)
                        && !priorRequest.exactPermitPath) {
                    routed.view = priorRequest.view
                    break
                }
            }
            probe.routedFinished.push(ProfileDownloadRouting.resolve(
                        download, routed.view, sourceHost, routed.restoreHost,
                        [], [], probe.disposableProfile))
            var id = String(download.id)
            if (sourceHost.downloadCompletionRequests[id] === download)
                delete sourceHost.downloadCompletionRequests[id]
            if (restoredHost.downloadCompletionRequests[id] === download)
                delete restoredHost.downloadCompletionRequests[id]
            if (sourceHost.activeDownloads[id] === download)
                delete sourceHost.activeDownloads[id]
            if (restoredHost.activeDownloads[id] === download)
                delete restoredHost.activeDownloads[id]
            probe.finished.push({
                request: download,
                id: String(download.id),
                view: routed.view,
                exactPermitPath: routed.exactPermitPath,
                finished: download.isFinished
            })
        }
    }

    Component {
        id: viewFactory
        WebEngineView {
            profile: probe.disposableProfile
            width: 300
            height: 300
            visible: true
            property bool loaded: false
            property string loadFailure: ""
            onLoadingChanged: function(request) {
                if (request.status === WebEngineView.LoadSucceededStatus)
                    loaded = true
                else if (request.status === WebEngineView.LoadFailedStatus)
                    loadFailure = request.errorString
            }
        }
    }

    function loadView(html) {
        var view = createTemporaryObject(viewFactory, testWindow.contentItem)
        verify(view !== null)
        view.loadHtml(html, "https://example.test/download-fixture")
        tryVerify(function() { return view.loaded || view.loadFailure.length > 0 }, 15000)
        compare(view.loadFailure, "")
        return view
    }

    function resolveNativeDownloadOwner(download) {
        var ownerByPath = restoredWindowOwner.hostForDownload(
                    download, null, sourceHost)
        if (ownerByPath === undefined)
            return { view: null, restoreHost: undefined, exactPermitPath: false }
        if (ownerByPath) {
            var capture = ownerByPath.nativePageCaptureSave
            var downloadPath = download.isSavePageDownload === true
                    ? String(download.downloadDirectory || "") + "/"
                      + String(download.downloadFileName || "") : ""
            var exactPermitPath = capture
                    && capture.profile === disposableProfile
                    && capture.path === downloadPath
            return {
                view: null,
                restoreHost: ownerByPath,
                exactPermitPath: exactPermitPath === true
            }
        }
        var activeCapture = restoredWindowOwner.activePageCaptureHost(
                    disposableProfile)
        if (activeCapture === undefined)
            return { view: null, restoreHost: undefined, exactPermitPath: false }
        if (activeCapture) {
            return { view: null, restoreHost: activeCapture, exactPermitPath: false }
        }
        var view = download.view
        return {
            view: view,
            restoreHost: restoredWindowOwner.hostForDownload(download, view, sourceHost),
            exactPermitPath: false
        }
    }

    function test_one_profile_delivers_each_view_request_and_finish_once() {
        if (outputDirectory.indexOf("__FERRIC_") === 0) {
            skip("Use cargo xtask test versioning-profile-download with a disposable output directory")
            return
        }
        disposableProfile = profilePrototype.instance()
        verify(disposableProfile !== null)
        firstView = loadView("<html><body>first synthetic page</body></html>")
        secondView = loadView("<html><body>second synthetic page</body></html>")
        sourceHost.tabViews = [firstView]
        restoredHost.tabViews = [secondView]
        sourceHost.versioningWindowOwner = restoredWindowOwner
        restoredWindowOwner.hostWindow = sourceHost
        verify(restoredWindowOwner.retain(restoredHost))

        var firstCapturePath = outputDirectory + "/first.mhtml"
        sourceHost.nativePageCaptureSave = {
            profile: disposableProfile,
            path: firstCapturePath
        }
        firstView.save(firstCapturePath,
                       WebEngineDownloadRequest.MimeHtmlSaveFormat)
        tryVerify(function() {
            return requested.length === 1 && finished.length === 1
        }, 20000)
        compare(requested[0].view, null)
        compare(finished[0].view, null)
        verify(requested[0].exactPermitPath)
        verify(finished[0].exactPermitPath)
        compare(finished[0].request, requested[0].request)
        compare(finished[0].id, requested[0].id)
        verify(requested[0].savePage)
        verify(finished[0].finished)
        compare(routedRequests[0], sourceHost)
        compare(routedFinished[0], sourceHost)
        sourceHost.nativePageCaptureSave = null

        var capturePath = outputDirectory + "/second.mhtml"
        restoredHost.nativePageCaptureSave = {
            profile: disposableProfile,
            path: capturePath
        }
        compare(restoredWindowOwner.activePageCaptureHost(disposableProfile), restoredHost)
        secondView.save(outputDirectory + "/second.mhtml",
                        WebEngineDownloadRequest.MimeHtmlSaveFormat)
        tryVerify(function() {
            return requested.length === 2 && finished.length === 2
        }, 20000)
        compare(requested[1].view, null)
        compare(finished[1].view, null)
        verify(requested[1].exactPermitPath)
        verify(finished[1].exactPermitPath)
        compare(finished[1].request, requested[1].request)
        compare(finished[1].id, requested[1].id)
        verify(requested[1].savePage)
        verify(finished[1].finished)
        verify(requested[0].id !== requested[1].id)
        compare(routedRequests[1], restoredHost)
        compare(routedFinished[1], restoredHost)
        restoredHost.nativePageCaptureSave = null
        sourceHost.tabViews = [firstView, secondView]
        compare(ProfileDownloadRouting.resolve(
                    requested[1].request, secondView, sourceHost, restoredHost,
                    [], [], disposableProfile), undefined)
        sourceHost.tabViews = [firstView]
        var forgedTerminal = { id: requested[1].id }
        compare(restoredWindowOwner.hostForDownload(forgedTerminal, null), null)
        compare(ProfileDownloadRouting.resolve(
                    forgedTerminal, null, sourceHost, null,
                    [], [], disposableProfile), null)

        secondView.url = slowDownloadUrl
        tryVerify(function() { return requested.length === 3 }, 15000)
        compare(requested[2].view, secondView)
        compare(routedRequests[2], restoredHost)
        verify(!requested[2].request.isFinished)
        tryVerify(function() {
            return requested[2].request.receivedBytes > 0
                    && !requested[2].request.isFinished
        }, 10000)
        requested[2].request.cancel()
        tryVerify(function() { return finished.length === 3 }, 10000)
        compare(finished[2].request, requested[2].request)
        compare(routedFinished[2], restoredHost)
        verify(finished[2].finished)
        compare(finished[2].request.state,
                WebEngineDownloadRequest.DownloadCancelled)
        console.log("FERRIC_PROFILE_DOWNLOAD_RESULT:3/3")
    }
}

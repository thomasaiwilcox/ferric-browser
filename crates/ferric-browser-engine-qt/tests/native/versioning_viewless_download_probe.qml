import QtQuick
import QtQuick.Window
import QtWebEngine
import "../../qml/scripts/ProfileDownloadRouting.js" as ProfileDownloadRouting

Window {
    id: root
    width: 480
    height: 320
    visible: true

    property string probeStoragePath: viewlessProbeStoragePath
    property string probeCachePath: viewlessProbeCachePath
    property string probeOutputDirectory: viewlessProbeOutputDirectory
    property string probeDownloadUrl: viewlessProbeDownloadUrl
    property bool sourceLoaded: false
    property string sourceFailure: ""
    property int viewlessDownloadRequests: 0
    property int viewlessDownloadFinished: 0
    property int viewlessDownloadState: -1
    property bool viewlessRoutedToExactProfileOwner: false
    property bool viewlessFinishedWithExactOwner: false
    property bool viewlessRequestRefused: false

    WebEngineProfile {
        id: sourceProfile
        offTheRecord: false
        storageName: "ferric-viewless-download-probe"
        persistentStoragePath: root.probeStoragePath
        cachePath: root.probeCachePath
    }
    property var profileForProbe: sourceProfile

    QtObject {
        id: downloadOwner
        property var browserProfile: sourceProfile
        property var activeDownloads: ({})
        property var downloadCompletionRequests: ({})
        property bool shutdownApproved: false
    }
    Connections {
        target: sourceProfile
        function onDownloadRequested(item) {
            if (String(item.url) !== root.probeDownloadUrl)
                return

            root.viewlessDownloadRequests++
            var owner = ProfileDownloadRouting.boundProfileOwner(
                        item, downloadOwner, sourceProfile, false)
            if (owner !== downloadOwner
                    || ProfileDownloadRouting.retainAndCancelIfShutdownApproved(
                        owner, item)) {
                root.viewlessRequestRefused = true
                if (item && typeof item.cancel === "function")
                    item.cancel()
                return
            }

            var id = String(item.id)
            downloadOwner.activeDownloads[id] = item
            item.downloadDirectory = root.probeOutputDirectory
            item.downloadFileName = "viewless.bin"
            item.accept()
            root.viewlessRoutedToExactProfileOwner =
                    downloadOwner.downloadCompletionRequests[id] === item
                    && downloadOwner.activeDownloads[id] === item
        }
        function onDownloadFinished(item) {
            if (String(item.url) !== root.probeDownloadUrl)
                return

            var id = String(item.id)
            root.viewlessFinishedWithExactOwner =
                    downloadOwner.downloadCompletionRequests[id] === item
                    && downloadOwner.activeDownloads[id] === item
            if (downloadOwner.downloadCompletionRequests[id] === item)
                delete downloadOwner.downloadCompletionRequests[id]
            if (downloadOwner.activeDownloads[id] === item)
                delete downloadOwner.activeDownloads[id]
            root.viewlessDownloadState = item.state
            root.viewlessDownloadFinished++
        }
    }

    WebEngineView {
        id: extensionView
        anchors.fill: parent
        profile: sourceProfile
        onLoadingChanged: function(request) {
            if (request.status === WebEngineView.LoadSucceededStatus)
                root.sourceLoaded = true
            else if (request.status === WebEngineView.LoadFailedStatus)
                root.sourceFailure = request.errorString
        }
    }

    function openExtensionPopup(url) {
        sourceLoaded = false
        sourceFailure = ""
        extensionView.url = url
    }
}

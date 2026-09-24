import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import QtWebEngine
import io.github.ferricbrowser 1.0

ApplicationWindow {
    id: popupWindow
    required property var rootWindow
    width: 1024
    height: 720
    visible: true
    title: popupWindow.popupContextName.length > 0
           ? "Ferric Browser popup · context "
             + (popupWindow.popupContextLabel.length > 0
                ? popupWindow.popupContextLabel : popupWindow.popupContextName)
           : "Ferric Browser popup"
    color: rootWindow.backgroundColor
    font: rootWindow.font
    palette: rootWindow.palette
    property var popupRequest
    property var popupProfile
    property var popupRequestInterceptor
    property var popupPermissionUi
    property string popupJourneyToken: ""
    property bool popupPrivateProfile: false
    property bool popupEphemeralProfile: false
    property string popupProfileName: "unknown"
    property string popupContextName: ""
    property string popupContextLabel: ""
    property var popupPermissionPromptSurface: null
    property var popupDesktopMediaSurface: null
    property var popupCaptureIndicatorSurface: null
    property var popupRendererFailureSurface: null
    property var pendingFileDialogRequest: null
    property var pendingFileDialogView: null
    property bool pendingFileDialogWaitingForPortal: false
    property double pendingFileDialogPortalDeadlineMs: 0
    property bool windowShutdownApproved: false
    property bool windowShutdownPromptVisible: false
    property bool windowShutdownPagePromptVisible: false
    property bool windowShutdownStoragePromptVisible: false
    property string windowShutdownPagePromptReason: ""
    property int windowShutdownPageProbeGeneration: 0
    property var activeDownloads: ({})
    property var pendingDownloadRequests: ({})
    property string pendingDownloadId: ""
    property string pendingDownloadSuggestedName: ""

    Timer {
        id: popupShutdownPageProbeTimer
        interval: 2500
        repeat: false
        onTriggered: {
            popupWindow.windowShutdownPageProbeGeneration += 1
            popupWindow.windowShutdownPagePromptReason = "Page state check timed out."
            popupWindow.windowShutdownPagePromptVisible = true
            if (popupWindow.popupPermissionUi) {
                popupWindow.popupPermissionUi.status_text =
                        "Page state check timed out; choose Close anyway"
            }
        }
    }

    onClosing: function(close) {
        if (popupWindow.windowShutdownApproved) {
            return
        }
        close.accepted = false
        popupWindow.beginQuitRequest()
    }

    function hasActiveDownloads() {
        if (popupWindow.pendingDownloadId.length > 0) {
            return true
        }
        for (var pendingKey in popupWindow.pendingDownloadRequests) {
            if (popupWindow.pendingDownloadRequests[pendingKey]) {
                return true
            }
        }
        for (var key in popupWindow.activeDownloads) {
            var download = popupWindow.activeDownloads[key]
            if (download && !download.isFinished) {
                return true
            }
        }
        return false
    }

    function checkPageStateBeforeQuit() {
        var generation = ++popupWindow.windowShutdownPageProbeGeneration
        if (!popupView || popupView.lifecycleState !== WebEngineView.LifecycleState.Active) {
            popupWindow.finalizeQuit()
            return
        }
        popupShutdownPageProbeTimer.restart()
        popupView.runJavaScript(rootWindow.shutdownPageProbeScript(), function(result) {
            if (generation !== popupWindow.windowShutdownPageProbeGeneration) {
                return
            }
            popupShutdownPageProbeTimer.stop()
            if (result === "clean") {
                popupWindow.finalizeQuit()
                return
            }
            popupWindow.windowShutdownPagePromptReason = result === "dirty"
                    ? "This page has unsaved form or editor state."
                    : "Ferric Browser could not verify this page before closing."
            popupWindow.windowShutdownPagePromptVisible = true
        })
    }

    function cancelPageStateProbe() {
        popupWindow.windowShutdownPageProbeGeneration += 1
        popupShutdownPageProbeTimer.stop()
        popupWindow.windowShutdownPagePromptVisible = false
    }

    function beginQuitRequest() {
        if (popupWindow.hasActiveDownloads()
                || rootWindow.hasActiveShutdownRequestsFor(
                    popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi, popupWindow)) {
            popupWindow.windowShutdownPromptVisible = true
            return
        }
        popupWindow.checkPageStateBeforeQuit()
    }

    function finalizeQuit() {
        if (popupWindow.popupPermissionUi
                && popupWindow.popupJourneyToken.length > 0
                && !popupWindow.popupPermissionUi.close_popup_tab(
                    popupWindow.popupJourneyToken)) {
            return
        }
        popupWindow.cancelPageStateProbe()
        popupWindow.windowShutdownPromptVisible = false
        popupWindow.windowShutdownApproved = true
        popupWindow.close()
    }

    function cancelDownloadsAndQuit() {
        if (popupWindow.pendingDownloadId.length > 0) {
            popupWindow.cancelPendingDownload()
        }
        var ui = popupWindow.popupDownloadUi()
        for (var pendingKey in popupWindow.pendingDownloadRequests) {
            var pendingDownload = popupWindow.pendingDownloadRequests[pendingKey]
            if (pendingDownload) {
                rootWindow.resolveQtRequest(ui, pendingDownload, "download", "cancel", [])
                ui.update_download(
                    String(pendingKey), "cancelled", pendingDownload.receivedBytes, true)
            }
        }
        popupWindow.pendingDownloadRequests = ({})
        for (var key in popupWindow.activeDownloads) {
            var download = popupWindow.activeDownloads[key]
            if (download && !download.isFinished) {
                rootWindow.resolveQtRequest(ui, download, "download", "cancel", [])
            }
        }
        popupWindow.activeDownloads = ({})
        rootWindow.cancelShutdownRequestsFor(
                    popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi, popupWindow)
        popupWindow.beginQuitRequest()
    }

    Rectangle {
        anchors.centerIn: parent
        width: Math.min(560, popupWindow.width - 80)
        height: Math.min(190 * rootWindow.chromeScale, popupWindow.height - 32)
        z: 100
        visible: popupWindow.windowShutdownPromptVisible
        color: rootWindow.panelColor
        border.color: rootWindow.warningColor
        border.width: 2

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 16
            spacing: 12
            Label {
                Layout.fillWidth: true
                text: "Popup browser work is still running"
                color: rootWindow.primaryTextColor
                font.bold: true
            }
            Label {
                Layout.fillWidth: true
                text: "Finish or cancel active popup work before closing."
                color: rootWindow.secondaryTextColor
                wrapMode: Text.WordWrap
            }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                Button {
                    text: "Keep window open"
                    Accessible.name: "Keep popup window open"
                    onClicked: {
                        popupWindow.windowShutdownPromptVisible = false
                        rootWindow.abortApplicationShutdown()
                        if (popupWindow.popupPermissionUi) {
                            popupWindow.popupPermissionUi.status_text = "Popup shutdown cancelled"
                        }
                    }
                }
                Button {
                    text: "Cancel active work and close"
                    Accessible.name: "Close popup window anyway"
                    onClicked: popupWindow.cancelDownloadsAndQuit()
                }
            }
        }
    }

    Rectangle {
        anchors.centerIn: parent
        width: Math.min(560, popupWindow.width - 80)
        height: Math.min(220 * rootWindow.chromeScale, popupWindow.height - 32)
        z: 100
        visible: popupWindow.windowShutdownPagePromptVisible
        color: rootWindow.panelColor
        border.color: rootWindow.warningColor
        border.width: 2

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 16
            spacing: 12
            Label {
                Layout.fillWidth: true
                text: "Popup page state may be lost"
                color: rootWindow.primaryTextColor
                font.bold: true
            }
            Label {
                Layout.fillWidth: true
                text: popupWindow.windowShutdownPagePromptReason
                      + " Close anyway may lose that state."
                color: rootWindow.secondaryTextColor
                wrapMode: Text.WordWrap
            }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                Button {
                    text: "Keep window open"
                    Accessible.name: "Keep popup window open"
                    onClicked: {
                        popupWindow.cancelPageStateProbe()
                        rootWindow.abortApplicationShutdown()
                        if (popupWindow.popupPermissionUi) {
                            popupWindow.popupPermissionUi.status_text = "Popup shutdown cancelled"
                        }
                    }
                }
                Button {
                    text: "Close anyway"
                    Accessible.name: "Close popup window despite page state"
                    onClicked: popupWindow.finalizeQuit()
                }
            }
        }
    }

    FileDialog {
        id: popupFileChooser
        title: "Choose file"
        onAccepted: popupWindow.acceptFileDialog()
        onRejected: popupWindow.rejectFileDialog()
    }

    FolderDialog {
        id: popupFolderChooser
        title: "Choose folder"
        onAccepted: popupWindow.acceptFolderDialog()
        onRejected: popupWindow.rejectFileDialog()
    }

    FileDialog {
        id: popupDownloadChooser
        title: "Choose download destination"
        fileMode: FileDialog.SaveFile
        nameFilters: ["All files (*)"]
        onAccepted: popupWindow.acceptPendingDownload()
        onRejected: popupWindow.cancelPendingDownload()
    }

    function popupDownloadUi() {
        return popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi
    }

    function attachPopupDownloadStateUpdates(download, id) {
        download.stateChanged.connect(function() {
            popupWindow.popupDownloadUi().update_download(
                id, rootWindow.downloadStateName(download), download.receivedBytes, false)
        })
        download.isPausedChanged.connect(function() {
            popupWindow.popupDownloadUi().update_download(
                id, rootWindow.downloadStateName(download), download.receivedBytes, false)
        })
    }

    function acceptPendingDownload() {
        var id = popupWindow.pendingDownloadId
        var download = popupWindow.pendingDownloadRequests[id]
        var ui = popupWindow.popupDownloadUi()
        var selectedPath = popupDownloadChooser.selectedFile.toLocalFile()
        if (!download) {
            popupWindow.pendingDownloadId = ""
            rootWindow.restoreOverlayFocus()
            return
        }
        var finalName = ui.accept_download_path(id, selectedPath)
        if (finalName.length === 0) {
            rootWindow.resolveQtRequest(ui, download, "download", "cancel", [])
            ui.update_download(id, "cancelled", download.receivedBytes, true)
        } else {
            var separator = selectedPath.lastIndexOf("/")
            var directory = separator > 0 ? selectedPath.slice(0, separator) : "/"
            var stagingDirectory = rootWindow.downloadStagingDirectory(ui, id)
            if (stagingDirectory.length === 0) {
                rootWindow.resolveQtRequest(ui, download, "download", "cancel", [])
                ui.update_download(id, "cancelled", download.receivedBytes, true)
                delete popupWindow.pendingDownloadRequests[id]
                popupWindow.pendingDownloadId = ""
                popupWindow.pendingDownloadSuggestedName = ""
                rootWindow.restoreOverlayFocus()
                return
            }
            download.downloadDirectory = stagingDirectory
            download.downloadFileName = finalName
            popupWindow.activeDownloads[id] = download
            popupWindow.attachPopupDownloadStateUpdates(download, id)
            rootWindow.resolveQtRequest(ui, download, "download", "accept", [])
            ui.update_download(id, "in-progress", 0, false)
        }
        delete popupWindow.pendingDownloadRequests[id]
        popupWindow.pendingDownloadId = ""
        popupWindow.pendingDownloadSuggestedName = ""
        rootWindow.restoreOverlayFocus()
    }

    function cancelPendingDownload() {
        var id = popupWindow.pendingDownloadId
        var download = popupWindow.pendingDownloadRequests[id]
        var ui = popupWindow.popupDownloadUi()
        if (download) {
            rootWindow.resolveQtRequest(ui, download, "download", "cancel", [])
            ui.update_download(id, "cancelled", download.receivedBytes, true)
        }
        delete popupWindow.pendingDownloadRequests[id]
        popupWindow.pendingDownloadId = ""
        popupWindow.pendingDownloadSuggestedName = ""
        rootWindow.restoreOverlayFocus()
    }

    function handleDownloadRequested(download) {
        var id = String(download.id)
        var ui = popupWindow.popupDownloadUi()
        var safeName = ui.offer_download(
                    id, download.url.toString(), download.suggestedFileName)
        var requestedPath = download.savePageFormat === WebEngineDownloadRequest.MimeHtmlSaveFormat
                ? ui.take_save_page_path() : ""
        if (requestedPath.length > 0) {
            var requestedName = ui.accept_download_path(id, requestedPath)
            if (requestedName.length === 0) {
                rootWindow.resolveQtRequest(ui, download, "download", "cancel", [])
                ui.update_download(id, "cancelled", download.receivedBytes, true)
                return
            }
            var requestedSeparator = requestedPath.lastIndexOf("/")
            var requestedDirectory = requestedSeparator > 0
                    ? requestedPath.slice(0, requestedSeparator) : "/"
            var stagingDirectory = rootWindow.downloadStagingDirectory(ui, id)
            if (stagingDirectory.length === 0) {
                rootWindow.resolveQtRequest(ui, download, "download", "cancel", [])
                ui.update_download(id, "cancelled", download.receivedBytes, true)
                return
            }
            download.downloadDirectory = stagingDirectory
            download.downloadFileName = requestedName
            popupWindow.activeDownloads[id] = download
            popupWindow.attachPopupDownloadStateUpdates(download, id)
            rootWindow.resolveQtRequest(ui, download, "download", "accept", [])
            ui.update_download(id, "in-progress", 0, false)
            return
        }
        var directory = ui.default_download_directory()
        if (!rootWindow.downloadsAskDestination()) {
            popupWindow.activeDownloads[id] = download
            var finalName = ui.accept_download(id, directory, safeName)
            if (finalName.length === 0) {
                delete popupWindow.activeDownloads[id]
                rootWindow.resolveQtRequest(ui, download, "download", "cancel", [])
                ui.update_download(id, "cancelled", download.receivedBytes, true)
                return
            }
            var stagingDirectory = rootWindow.downloadStagingDirectory(ui, id)
            if (stagingDirectory.length === 0) {
                delete popupWindow.activeDownloads[id]
                rootWindow.resolveQtRequest(ui, download, "download", "cancel", [])
                ui.update_download(id, "cancelled", download.receivedBytes, true)
                return
            }
            download.downloadDirectory = stagingDirectory
            download.downloadFileName = finalName
            popupWindow.attachPopupDownloadStateUpdates(download, id)
            rootWindow.resolveQtRequest(ui, download, "download", "accept", [])
            ui.update_download(id, "in-progress", 0, false)
            return
        }
        if (popupWindow.pendingDownloadId.length > 0) {
            rootWindow.resolveQtRequest(ui, download, "download", "cancel", [])
            ui.update_download(id, "cancelled", download.receivedBytes, true)
            return
        }
        popupWindow.pendingDownloadRequests[id] = download
        popupWindow.pendingDownloadId = id
        popupWindow.pendingDownloadSuggestedName = safeName
        rootWindow.captureOverlayFocus(popupWindow, popupView)
        ui.update_download(id, "selecting-destination", 0, false)
        popupDownloadChooser.currentFile = rootWindow.fileUrlForPath(directory + "/" + safeName)
        popupDownloadChooser.open()
    }

    function handleDownloadFinished(download) {
        var id = String(download.id)
        var ui = popupWindow.popupDownloadUi()
        var state = rootWindow.downloadStateName(download)
        if (state === "completed" && !ui.finalize_download(id)) {
            state = "interrupted"
        } else if (state !== "completed") {
            ui.discard_download_staging(id)
        }
        ui.update_download(id, state, download.receivedBytes, true)
        delete popupWindow.activeDownloads[id]
        delete popupWindow.pendingDownloadRequests[id]
        if (popupWindow.pendingDownloadId === id) {
            popupWindow.pendingDownloadId = ""
            popupWindow.pendingDownloadSuggestedName = ""
        }
    }

    Timer {
        id: popupFilePortalTimer
        interval: 50
        repeat: true
        running: false
        onTriggered: popupWindow.maybeOpenPendingFileDialog()
    }

    function popupFileDialogPaths() {
        var paths = []
        var urls = popupFileChooser.fileMode === FileDialog.OpenFiles
                ? popupFileChooser.selectedFiles
                : [popupFileChooser.selectedFile]
        for (var i = 0; i < urls.length; ++i) {
            var path = urls[i].toLocalFile()
            if (!path || urls[i].scheme !== "file") {
                return []
            }
            paths.push(path)
        }
        return paths
    }

    function popupFolderDialogPaths() {
        var url = popupFolderChooser.selectedFolder
        var path = url && url.scheme === "file" ? url.toLocalFile() : ""
        return path ? [path] : []
    }

    function clearPopupFileDialog(cancelRequest, message) {
        var request = popupWindow.pendingFileDialogRequest
        var hadRequest = !!request
        var ui = popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi
        popupWindow.pendingFileDialogRequest = null
        popupWindow.pendingFileDialogView = null
        popupWindow.pendingFileDialogWaitingForPortal = false
        popupWindow.pendingFileDialogPortalDeadlineMs = 0
        popupFilePortalTimer.stop()
        if (popupFileChooser.visible) {
            popupFileChooser.close()
        }
        if (popupFolderChooser.visible) {
            popupFolderChooser.close()
        }
        if (hadRequest) {
            rootWindow.restoreOverlayFocus()
        }
        if (cancelRequest && request) {
            rootWindow.resolveQtRequest(ui, request, "file-dialog", "dialogReject", [])
        }
        if (message && ui) {
            ui.status_text = message
        }
    }

    function acceptFileDialog() {
        var request = popupWindow.pendingFileDialogRequest
        var ui = popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi
        var paths = popupWindow.popupFileDialogPaths()
        popupWindow.pendingFileDialogRequest = null
        popupWindow.pendingFileDialogView = null
        if (!request || paths.length === 0) {
            if (request) {
                rootWindow.resolveQtRequest(ui, request, "file-dialog", "dialogReject", [])
            }
            if (request) {
                rootWindow.restoreOverlayFocus()
            }
            ui.status_text = "File selection returned no local paths"
            return
        }
        rootWindow.resolveQtRequest(ui, request, "file-dialog", "dialogAccept", [paths])
        rootWindow.restoreOverlayFocus()
        ui.status_text = paths.length === 1
                ? "File selected"
                : paths.length + " files selected"
    }

    function acceptFolderDialog() {
        var request = popupWindow.pendingFileDialogRequest
        var ui = popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi
        var paths = popupWindow.popupFolderDialogPaths()
        popupWindow.pendingFileDialogRequest = null
        popupWindow.pendingFileDialogView = null
        if (!request || paths.length === 0) {
            if (request) {
                rootWindow.resolveQtRequest(ui, request, "file-dialog", "dialogReject", [])
            }
            if (request) {
                rootWindow.restoreOverlayFocus()
            }
            ui.status_text = "Folder selection returned no local path"
            return
        }
        rootWindow.resolveQtRequest(ui, request, "file-dialog", "dialogAccept", [paths])
        rootWindow.restoreOverlayFocus()
        ui.status_text = "Folder selected"
    }

    function rejectFileDialog() {
        var request = popupWindow.pendingFileDialogRequest
        var ui = popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi
        if (!request) {
            return
        }
        popupWindow.pendingFileDialogRequest = null
        popupWindow.pendingFileDialogView = null
        rootWindow.resolveQtRequest(ui, request, "file-dialog", "dialogReject", [])
        rootWindow.restoreOverlayFocus()
        ui.status_text = "File selection cancelled"
    }

    function handleFileDialogRequested(request) {
        if (popupWindow.pendingFileDialogRequest) {
            popupWindow.clearPopupFileDialog(true,
                    "Previous file selection cancelled")
            rootWindow.resolveQtRequest(
                popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi,
                request, "file-dialog", "dialogReject", [])
            (popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi).status_text =
                    "Another file selection is already open"
            return
        }
        rootWindow.captureOverlayFocus(popupWindow, popupView)
        popupWindow.pendingFileDialogRequest = request
        popupWindow.pendingFileDialogView = popupView
        var requestUi = popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi
        if (rootWindow.desktopPortalMode(requestUi) === "required") {
            var status = rootWindow.desktopPortalCapabilityStatus(requestUi, "file_chooser")
            if (status === "not-probed") {
                rootWindow.primaryBrowserUi.probe_desktop_portals()
                status = rootWindow.desktopPortalCapabilityStatus(requestUi, "file_chooser")
            }
            if (status === "pending" || status === "not-probed") {
                popupWindow.pendingFileDialogWaitingForPortal = true
                popupWindow.pendingFileDialogPortalDeadlineMs = Date.now() + 5000
                requestUi.status_text = "Checking required desktop portal…"
                popupFilePortalTimer.start()
                return
            }
            if (status !== "available") {
                popupWindow.clearPopupFileDialog(
                    true, "Required desktop portal unavailable; file selection cancelled")
                return
            }
        }
        popupWindow.pendingFileDialogWaitingForPortal = false
        popupFilePortalTimer.stop()
        if (request.mode === FileDialogRequest.FileModeUploadFolder) {
            popupFolderChooser.open()
            return
        }
        popupFileChooser.fileMode = request.mode === FileDialogRequest.FileModeOpenMultiple
                ? FileDialog.OpenFiles
                : request.mode === FileDialogRequest.FileModeSave
                    ? FileDialog.SaveFile
                    : FileDialog.OpenFile
        popupFileChooser.nameFilters = rootWindow.fileDialogNameFilters(request)
        popupFileChooser.currentFile = request.defaultFileName.length > 0
                ? request.defaultFileName
                : ""
        popupFileChooser.title = request.mode === FileDialogRequest.FileModeSave
                ? "Save file"
                : request.mode === FileDialogRequest.FileModeOpenMultiple
                    ? "Choose files"
                    : "Choose file"
        popupFileChooser.open()
    }

    function maybeOpenPendingFileDialog() {
        if (!popupWindow.pendingFileDialogWaitingForPortal
                || !popupWindow.pendingFileDialogRequest) {
            popupFilePortalTimer.stop()
            return
        }
        if (Date.now() >= popupWindow.pendingFileDialogPortalDeadlineMs) {
            popupWindow.clearPopupFileDialog(
                true, "Desktop portal check timed out; file selection cancelled")
            return
        }
        var ui = popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi
        var status = rootWindow.desktopPortalCapabilityStatus(ui, "file_chooser")
        if (status === "pending" || status === "not-probed") {
            return
        }
        if (status !== "available") {
            popupWindow.clearPopupFileDialog(
                true, "Required desktop portal unavailable; file selection cancelled")
            return
        }
        popupWindow.pendingFileDialogWaitingForPortal = false
        popupFilePortalTimer.stop()
        var request = popupWindow.pendingFileDialogRequest
        if (request.mode === FileDialogRequest.FileModeUploadFolder) {
            popupFolderChooser.open()
            return
        }
        popupFileChooser.fileMode = request.mode === FileDialogRequest.FileModeOpenMultiple
                ? FileDialog.OpenFiles
                : request.mode === FileDialogRequest.FileModeSave
                    ? FileDialog.SaveFile
                    : FileDialog.OpenFile
        popupFileChooser.nameFilters = rootWindow.fileDialogNameFilters(request)
        popupFileChooser.currentFile = request.defaultFileName.length > 0
                ? request.defaultFileName : ""
        popupFileChooser.title = request.mode === FileDialogRequest.FileModeSave
                ? "Save file"
                : request.mode === FileDialogRequest.FileModeOpenMultiple
                    ? "Choose files"
                    : "Choose file"
        popupFileChooser.open()
    }

    function clearFileDialogForView(view) {
        if (popupWindow.pendingFileDialogView === view) {
            popupWindow.clearPopupFileDialog(
                true, "File selection cancelled by navigation")
        }
    }

    WebEngineView {
        id: popupView
        property var pageUserScriptNames: []
        property string pageUserScriptReloadUrl: ""
        property string pageDialogDocumentKey: ""
        property int pageDialogCount: 0
        property bool pageDialogSuppressed: false
        property bool rendererFailed: false
        property int rendererFailureCount: 0
        property double rendererFailureAt: 0
        property string rendererFailureSafeUrl: ""
        property string rendererFailureReason: ""
        property int rendererFailureExitCode: 0
        // Navigation-scoped content settings are snapshotted per
        // document. A configuration reload must not mutate the
        // already-loaded page's engine policy.
        property var effectiveSiteSettings: ({ values: {}, matched_rules: [] })
        function refreshEffectiveSiteSettings() {
            effectiveSiteSettings = rootWindow.siteRuleSettingsFor(
                popupWindow.popupPermissionUi, popupView.url.toString())
        }
        Connections {
            target: popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi
            function onSite_experiment_kindChanged() {
                popupView.refreshEffectiveSiteSettings()
            }
        }
        anchors.fill: parent
        anchors.bottomMargin: rootWindow.statusBarHeight
        profile: popupWindow.popupProfile
        url: "about:blank"
        settings.javascriptEnabled: !!rootWindow.siteRuleValue(
            effectiveSiteSettings, "content.javascript", true)
        settings.autoLoadImages: !!rootWindow.siteRuleValue(
            effectiveSiteSettings, "content.images", true)
        settings.forceDarkMode: !!rootWindow.siteRuleValue(
            effectiveSiteSettings, "content.force_dark", false)
        settings.playbackRequiresUserGesture:
            rootWindow.siteRuleValue(effectiveSiteSettings, "content.autoplay", "engine-default")
            === "require-gesture"
        Accessible.name: "Popup web content"
        onUrlChanged: {
            refreshEffectiveSiteSettings()
            if (popupWindow.popupJourneyToken.length > 0) {
                (popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi).popup_navigation_url_changed(
                    popupWindow.popupJourneyToken, url.toString())
            }
        }
        onLoadingChanged: function(loadRequest) {
            if (loadRequest.status === WebEngineView.LoadStartedStatus) {
                rootWindow.clearPageDialogForView(popupView)
                rootWindow.clearClientCertificateForView(popupView)
                rootWindow.clearCertificateErrorForView(popupView)
                rootWindow.clearWebAuthForView(popupView)
                rootWindow.clearContextMenuForView(popupView)
                rootWindow.clearDesktopMediaForView(popupView)
                rootWindow.clearSiteDataClearForView(popupView)
                rootWindow.noteCaptureNavigation(popupView)
                popupWindow.clearFileDialogForView(popupView)
                rootWindow.clearRendererFailureForView(popupView)
                rootWindow.resetPageDialogBudget(popupView)
                if (popupWindow.popupRequestInterceptor) {
                    popupWindow.popupRequestInterceptor.clearSiteEvidence(popupView.url.host)
                }
                rootWindow.cancelPermissionForUi(popupWindow.popupPermissionUi)
                rootWindow.installPageUserscripts(
                    popupWindow.popupPermissionUi, popupView, popupView.url.toString(),
                    popupWindow.popupPrivateProfile)
                rootWindow.injectPageUserscripts(
                    popupWindow.popupPermissionUi, popupView, popupView.url.toString(),
                    popupWindow.popupPrivateProfile, "document_start")
                rootWindow.injectCosmeticRules(popupWindow.popupPermissionUi, popupView)
                if (popupWindow.popupJourneyToken.length > 0) {
                    (popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi).popup_navigation_started(
                        popupWindow.popupJourneyToken, loadRequest.url.toString())
                }
            } else if (loadRequest.status === WebEngineView.LoadSucceededStatus) {
                if (popupWindow.popupJourneyToken.length > 0) {
                    (popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi).popup_navigation_committed(
                        popupWindow.popupJourneyToken, popupView.url.toString(), popupView.title)
                    (popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi).popup_navigation_completed(
                        popupWindow.popupJourneyToken)
                }
                rootWindow.injectPageUserscripts(
                    popupWindow.popupPermissionUi, popupView, popupView.url.toString(),
                    popupWindow.popupPrivateProfile, "document_end")
                rootWindow.injectCosmeticRules(popupWindow.popupPermissionUi, popupView)
                Qt.callLater(function() {
                    rootWindow.injectPageUserscripts(
                        popupWindow.popupPermissionUi, popupView, popupView.url.toString(),
                        popupWindow.popupPrivateProfile, "document_idle")
                })
            } else if (loadRequest.status === WebEngineView.LoadFailedStatus) {
                if (popupWindow.popupJourneyToken.length > 0) {
                    (popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi).popup_navigation_failed(
                        popupWindow.popupJourneyToken)
                }
            }
        }

        onPermissionRequested: function(permissionRequest) {
            if (popupWindow.popupPermissionUi) {
                rootWindow.handleImmediatePermissionRequested(
                    popupWindow.popupPermissionUi,
                    permissionRequest,
                    popupWindow.popupPrivateProfile,
                    popupWindow)
            } else {
                rootWindow.resolveQtRequest(
                    popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi,
                    permissionRequest, "permission", "deny", [])
            }
        }
        onJavaScriptDialogRequested: function(request) {
            rootWindow.handleJavaScriptDialogRequested(
                popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi,
                popupView, request, popupWindow.popupPrivateProfile)
        }
        onAuthenticationDialogRequested: function(request) {
            rootWindow.handleAuthenticationDialogRequested(
                popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi,
                popupView, request, popupWindow.popupPrivateProfile)
        }
        onSelectClientCertificate: function(selection) {
            rootWindow.handleClientCertificateRequested(
                popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi,
                popupView, selection, popupWindow.popupPrivateProfile, popupWindow)
        }
        onCertificateError: function(error) {
            rootWindow.handleCertificateError(
                popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi,
                popupView, error, popupWindow)
        }
        onWebAuthUxRequested: function(request) {
            rootWindow.handleWebAuthRequested(
                popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi,
                popupView, request, popupWindow)
        }
        onContextMenuRequested: function(request) {
            rootWindow.handleContextMenuRequested(
                popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi,
                popupView, request, popupWindow)
        }
        onFileDialogRequested: function(request) {
            popupWindow.handleFileDialogRequested(request)
        }
        onDesktopMediaRequested: function(request) {
            rootWindow.handleDesktopMediaRequested(
                popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi,
                popupView, request, popupWindow)
        }
        onRenderProcessTerminated: function(terminationStatus, exitCode) {
            rootWindow.handleRendererProcessTerminated(
                popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi,
                popupView, popupWindow, -1, terminationStatus, exitCode)
        }

        Component.onCompleted: {
            refreshEffectiveSiteSettings()
            rootWindow.registerPopupWindow(
                        popupWindow, popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi, popupView)
            if (!popupWindow.popupPermissionPromptSurface) {
                popupWindow.popupPermissionPromptSurface =
                        rootWindow.permissionPromptFactory.createObject(
                            popupWindow.contentItem, { hostWindow: popupWindow })
            }
            if (!popupWindow.popupDesktopMediaSurface) {
                popupWindow.popupDesktopMediaSurface =
                        rootWindow.desktopMediaFactory.createObject(
                            popupWindow.contentItem, { hostWindow: popupWindow })
            }
            if (!popupWindow.popupCaptureIndicatorSurface) {
                popupWindow.popupCaptureIndicatorSurface =
                        rootWindow.captureIndicatorFactory.createObject(
                            popupWindow.contentItem, { hostWindow: popupWindow })
            }
            if (!popupWindow.popupRendererFailureSurface) {
                popupWindow.popupRendererFailureSurface =
                        rootWindow.rendererFailureFactory.createObject(
                            popupWindow.contentItem, { hostWindow: popupWindow })
            }
            rootWindow.installFocusObserver(popupView)
            if (popupWindow.popupRequest) {
                popupWindow.popupRequest.openIn(popupView)
            }
        }
        Component.onDestruction: {
            rootWindow.clearPageDialogForView(popupView)
            rootWindow.clearClientCertificateForView(popupView)
            rootWindow.clearCertificateErrorForView(popupView)
            rootWindow.clearWebAuthForView(popupView)
            rootWindow.clearContextMenuForView(popupView)
            rootWindow.clearDesktopMediaForView(popupView)
            rootWindow.clearCaptureSessionForView(popupView)
            popupWindow.clearFileDialogForView(popupView)
            rootWindow.clearSiteDataClearForView(popupView)
            rootWindow.cancelPermissionForUi(popupWindow.popupPermissionUi)
        }
    }

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: rootWindow.statusBarHeight
        z: 10
        visible: rootWindow.statusbarMode === "always"
        color: rootWindow.surfaceColor
        opacity: rootWindow.chromeOpacity

        Label {
            anchors.fill: parent
            anchors.leftMargin: 10
            verticalAlignment: Text.AlignVCenter
            color: rootWindow.contextStatusColor(
                       popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi,
                       rootWindow.secondaryTextColor)
            text: (popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi).mode + " · "
                  + (popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi).status_text
                  + rootWindow.statusDetails(
                      popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi,
                      popupWindow, popupView,
                      popupWindow.popupPrivateProfile,
                      popupWindow.popupProfileName,
                      popupWindow.popupEphemeralProfile)
            Accessible.name: "Browser status bar"
            Accessible.role: Accessible.StatusBar
        }
    }

    Component.onDestruction: {
        rootWindow.unregisterPopupWindow(popupWindow)
        if (!popupWindow.windowShutdownApproved
                && popupWindow.popupPermissionUi
                && popupWindow.popupJourneyToken.length > 0) {
            popupWindow.popupPermissionUi.close_popup_tab(
                popupWindow.popupJourneyToken)
        }
        if (popupWindow.popupJourneyToken.length > 0) {
            (popupWindow.popupPermissionUi || rootWindow.primaryBrowserUi).release_popup_journey_token(
                popupWindow.popupJourneyToken)
        }
        popupWindow.clearPopupFileDialog(true, "File selection cancelled")
        rootWindow.cancelPermissionForUi(popupWindow.popupPermissionUi)
        if (rootWindow.pendingDesktopMediaHost === popupWindow) {
            rootWindow.clearDesktopMediaRequest(true)
        }
        rootWindow.clearCaptureSessionsForHost(popupWindow)
        if (popupWindow.popupPermissionPromptSurface) {
            popupWindow.popupPermissionPromptSurface.destroy()
            popupWindow.popupPermissionPromptSurface = null
        }
        if (popupWindow.popupDesktopMediaSurface) {
            popupWindow.popupDesktopMediaSurface.destroy()
            popupWindow.popupDesktopMediaSurface = null
        }
        if (popupWindow.popupCaptureIndicatorSurface) {
            popupWindow.popupCaptureIndicatorSurface.destroy()
            popupWindow.popupCaptureIndicatorSurface = null
        }
        if (popupWindow.popupRendererFailureSurface) {
            popupWindow.popupRendererFailureSurface.destroy()
            popupWindow.popupRendererFailureSurface = null
        }
    }
}

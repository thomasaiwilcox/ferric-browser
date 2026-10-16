import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import QtWebEngine
import io.github.ferricbrowser 1.0
import "../scripts/ChromePresentation.js" as ChromePresentation

ApplicationWindow {
    required property var rootWindow
    id: secondaryWindow
    readonly property var modalInputScope: FerricModalInputController { hostWindow: secondaryWindow }
    readonly property alias browserKeyRouterObject: secondaryKeyRouter
    width: 1180
    height: 760
    visible: true
    title: secondaryWindow.windowEphemeralProfile
           ? "Ferric Browser · " + secondaryWindow.windowProfileLabel
           : "Ferric Browser · " + secondaryWindow.windowProfileName
    color: secondaryAppearance.backgroundColor
    font: secondaryAppearance.font
    palette.window: secondaryAppearance.backgroundColor
    palette.windowText: secondaryAppearance.primaryTextColor
    palette.base: secondaryAppearance.surfaceColor
    palette.alternateBase: secondaryAppearance.panelColor
    palette.text: secondaryAppearance.primaryTextColor
    palette.button: secondaryAppearance.surfaceColor
    palette.buttonText: secondaryAppearance.primaryTextColor
    palette.highlight: secondaryAppearance.selectionColor
    palette.highlightedText: secondaryAppearance.selectionTextColor
    palette.placeholderText: secondaryAppearance.mutedTextColor
    palette.link: secondaryAppearance.accentColor
    property string windowStartupUrl: "about:blank"
    readonly property alias windowUi: secondaryUi
    readonly property alias chromeAppearance: secondaryAppearance
    readonly property string displayScaleLabel: ChromePresentation.displayScaleLabel(
                                                   Screen.devicePixelRatio, Screen.logicalPixelDensity)
    signal bookmarkTransferStarted(var destinationUi, bool queued)
    property string windowBookmarkTransferUrl: ""
    property string windowBookmarkTransferTitle: ""
    property string windowProfileName: "secondary"
    property string windowStartupContext: ""
    property bool windowStartupContextRestore: false
    property bool windowStartupRoutePreflighted: true
    property string windowProfileLabel: "Secondary"
    property string windowStorageBasePath: rootWindow.storageBasePath
    property bool windowPrivateProfile: false
    property bool windowEphemeralProfile: false
    property string windowEphemeralInvocationToken: ""
    property var windowSharedProfile: null
    property var windowInheritedUi: null
    property var windowSharedRequestInterceptor: null
    property bool windowPopup: false
    property var windowPopupRequest: null
    property var windowPopupOpenerUi: null
    property string windowPopupToken: ""
    FerricPopupOwnerBridge { id: popupOwnerBridge }
    property var windowTransferView: null
    property var windowTransferSourceUi: null
    property var windowTransferSourceHost: null
    property string windowTransferOperationId: ""
    property string windowTransferPayload: ""
    property var windowPreparedTransferFallback: null
    property var windowDetachedView: null
    property var devToolsWindow: null
    property bool devToolsVisible: false
    property var devToolsAttachedView: secondaryDevToolsLoader.item
    readonly property bool windowTransferMode: windowTransferView !== null
    readonly property bool inputBarActive: secondaryUi.mode === "command"
                                           || secondaryUi.mode === "search"
    property bool commandNoticeVisible: false
    property string commandNoticeText: ""
    property bool commandNoticeError: false
    property string pendingInteractiveCommand: ""
    readonly property bool statusBarVisible:
        (secondaryWindow.commandNoticeVisible && !secondaryWindow.inputBarActive)
        || secondaryAppearance.statusBarVisibleForMode(secondaryUi.mode)
    readonly property real bottomControlsHeight:
        inputBarActive ? secondaryAppearance.inputBarHeight
                       : (statusBarVisible ? secondaryAppearance.statusBarHeight : 0)
    readonly property real bottomChromeHeight: bottomControlsHeight + secondaryProfileNotice.height
    readonly property var activeView: tabViewAt(secondaryUi.active_tab_index)
    readonly property alias hintWorkspace: secondaryHintWorkspace
    readonly property bool rapidHintConfirmationVisible: secondaryHintWorkspace.rapidHintConfirmationVisible
    readonly property alias tabsModel: secondaryTabs
    property alias tabViews: secondaryTabController.tabViews
    function tabViewAt(index) { return index >= 0 && index < tabViews.length ? tabViews[index] : null }
    function activeWebView() { return activeView }
    function syncTabModel() { secondaryTabController.syncTabModel() }
    readonly property bool browserKeyFocusActive:
        secondaryWindow.profileBootstrapComplete
        && !secondaryWindow.inputBarActive
        && !secondaryProfileNotice.inputActive
        && !(rootWindow.copyNoticeInputActive && rootWindow.copyNoticeHost === secondaryWindow)
        && !secondaryWindow.devToolsVisible
        && !secondaryHintWorkspace.rapidHintConfirmationVisible
        && !secondaryUi.target_search_active
        && !rootWindow.presentedPromptActiveFor(secondaryUi)
        && !(rootWindow.rendererFailureVisible && rootWindow.rendererFailureHost === secondaryWindow)
        && !secondarySettings.active
        && !secondaryManagers.active
        && !(activeView && activeView.nativeUrl === "rb://start" && activeView.startPage && activeView.startPage.inputActive)
        && !windowCloseConfirmationVisible
        && !windowShutdownPromptVisible
        && !windowShutdownPagePromptVisible
        && !windowShutdownStoragePromptVisible
        && !secondaryUi.external_navigation_visible
    readonly property bool windowTransientProfile:
        windowPrivateProfile || windowEphemeralProfile
    property var windowProfileResources: null
    readonly property var windowWebEngineProfile:
        windowProfileResources ? windowProfileResources.profile : null
    readonly property var windowRequestInterceptor:
        windowProfileResources ? windowProfileResources.interceptor : null
    property var permissionPromptSurface: null
    property var desktopMediaSurface: null
    property var captureIndicatorSurface: null
    property var rendererFailureSurface: null
    property var pendingFileDialogRequest: null
    property var pendingFileDialogView: null
    property bool pendingFileDialogWaitingForPortal: false
    property double pendingFileDialogPortalDeadlineMs: 0
    property bool profileBootstrapComplete: false
    property bool runtimeReleased: false
    property bool windowShutdownApproved: false
    property bool windowCloseConfirmationVisible: false
    property bool windowShutdownPromptVisible: false
    property bool windowShutdownPagePromptVisible: false
    property bool windowShutdownStoragePromptVisible: false
    property string windowShutdownPagePromptReason: ""
    property int windowShutdownPageProbeGeneration: 0
    property var activeDownloads: ({})
    property var pendingDownloadRequests: ({})
    property string pendingDownloadId: ""
    property string pendingDownloadSuggestedName: ""
    property bool caretSelecting: false

    Timer {
        id: secondaryCommandNoticeTimer
        interval: 3500
        repeat: false
        onTriggered: secondaryWindow.commandNoticeVisible = false
    }

    function showCommandNotice(text, isError) {
        var message = String(text || "").trim()
        if (message.length === 0 || message === "Normal mode") {
            return
        }
        secondaryWindow.commandNoticeText = message
        secondaryWindow.commandNoticeError = !!isError
        secondaryWindow.commandNoticeVisible = true
        secondaryCommandNoticeTimer.restart()
    }

    FerricWebEngineSurfaceRecovery {
        hostWindow: secondaryWindow
        enabled: rootWindow.nativeWayland && !rootWindow.softwareRendering
        views: [
            secondaryWindow.activeView,
            secondaryDevToolsLoader.active ? secondaryDevToolsLoader.item : null
        ]
    }

    BrowserKeyRouter {
        id: secondaryKeyRouter
        transientInsert: secondaryUi.mode === "insert"
        passNextKey: secondaryUi.mode === "pass-through-once"
        passThrough: secondaryUi.mode === "pass-through"
        inputEpoch: secondaryUi.input_epoch
        onPassNextKeyDispatched: function(epoch) {
            if (secondaryUi.mode === "pass-through-once" && secondaryUi.input_epoch === epoch) {
                secondaryUi.escape()
            }
        }
        onTransientEnterDispatched: function(epoch) {
            if (secondaryUi.mode === "insert" && secondaryUi.input_epoch === epoch) {
                secondaryUi.escape()
            }
        }
        targetWindow: secondaryWindow
        enabled: secondaryWindow.active
                 && secondaryWindow.browserKeyFocusActive
                 && (secondaryUi.mode === "normal"
                     || secondaryUi.mode === "hint"
                     || secondaryUi.mode === "grid"
                     || secondaryUi.mode === "caret"
                     || secondaryUi.mode === "insert"
                     || secondaryUi.mode === "insert-locked"
                     || secondaryUi.mode === "pass-through"
                     || secondaryUi.mode === "pass-through-once")
        onClipboardShortcutOverride: function(key, modifiers) {
            var logical = rootWindow.logicalNormalKeyText({key: key, modifiers: modifiers, text: ""})
            if (!rootWindow.nativePromptActiveFor(secondaryUi) && secondaryUi.clipboard_shortcut_browser_owned(logical)) secondaryKeyRouter.acceptCurrentEvent()
        }
        onKeyPressed: function(text, key, modifiers, isAutoRepeat, browserOwnedRepeat) {
            var event = {
                text: text,
                key: key,
                modifiers: modifiers,
                isAutoRepeat: !!isAutoRepeat,
                browserOwnedRepeat: !!browserOwnedRepeat,
                accepted: false
            }
            if (rootWindow.handleBrowserKey(
                        secondaryUi, secondaryWindow, event)) {
                secondaryKeyRouter.acceptCurrentEvent()
                if (secondaryUi.mode === "insert" || secondaryUi.mode === "insert-locked") {
                    rootWindow.probeActiveFocus(secondaryUi, secondaryUi.active_tab_index,
                                               secondaryWindow.activeView)
                }
            }
        }
        onKeyReleased: function(key, modifiers) {
            if (rootWindow.handleBrowserKeyRelease(
                        secondaryUi, secondaryWindow, key, modifiers)) {
                secondaryKeyRouter.acceptCurrentEvent()
            }
        }
    }

    onActiveChanged: {
        secondaryUi.set_ipc_window_active(active)
        if (!active && rootWindow.copyNoticeHost === secondaryWindow) rootWindow.copyNoticeVisible = false
        if (!active && secondaryUi.mode === "pass-through-once") {
            secondaryUi.escape()
        }
    }

    Timer {
        id: secondaryShutdownPageProbeTimer
        interval: 2500
        repeat: false
        onTriggered: {
            secondaryWindow.windowShutdownPageProbeGeneration += 1
            secondaryWindow.windowShutdownPagePromptReason = "Page state check timed out."
            secondaryWindow.windowShutdownPagePromptVisible = true
            secondaryUi.status_text =
                    "Page state check timed out; choose Close anyway"
        }
    }

    onClosing: function(close) {
        if (secondaryWindow.windowShutdownApproved) {
            secondaryWindow.releaseWindowResources()
            return
        }
        close.accepted = false
        secondaryWindow.beginQuitRequest()
    }

    function hasActiveDownloads() {
        if (secondaryWindow.pendingDownloadId.length > 0) {
            return true
        }
        for (var pendingKey in secondaryWindow.pendingDownloadRequests) {
            if (secondaryWindow.pendingDownloadRequests[pendingKey]) {
                return true
            }
        }
        for (var key in secondaryWindow.activeDownloads) {
            var download = secondaryWindow.activeDownloads[key]
            if (download && !download.isFinished) {
                return true
            }
        }
        return false
    }

    function checkPageStateBeforeQuit() {
        var generation = ++secondaryWindow.windowShutdownPageProbeGeneration
        var view = secondaryWindow.activeView
        if (!view || view.lifecycleState !== WebEngineView.LifecycleState.Active) {
            secondaryWindow.finalizeQuit()
            return
        }
        secondaryShutdownPageProbeTimer.restart()
        rootWindow.runBrowserScript(view, rootWindow.shutdownPageProbeScript(), function(result) {
            if (generation !== secondaryWindow.windowShutdownPageProbeGeneration) {
                return
            }
            secondaryShutdownPageProbeTimer.stop()
            if (result === "clean") {
                secondaryWindow.finalizeQuit()
                return
            }
            secondaryWindow.windowShutdownPagePromptReason = result === "dirty"
                    ? "This page has unsaved form or editor state."
                    : "Ferric Browser could not verify this page before closing."
            secondaryWindow.windowShutdownPagePromptVisible = true
        }, 0)
    }

    function cancelPageStateProbe() {
        secondaryWindow.windowShutdownPageProbeGeneration += 1
        secondaryShutdownPageProbeTimer.stop()
        secondaryWindow.windowShutdownPagePromptVisible = false
    }

    function beginQuitRequest() {
        if (secondaryWindow.windowCloseConfirmationVisible
                || secondaryWindow.windowShutdownPromptVisible
                || secondaryWindow.windowShutdownPagePromptVisible
                || secondaryWindow.windowShutdownStoragePromptVisible) {
            return
        }
        secondaryWindow.windowCloseConfirmationVisible = true
        secondaryUi.status_text = "Confirm window close"
    }

    function continueQuitRequest() {
        secondaryWindow.windowCloseConfirmationVisible = false
        if (secondaryWindow.hasActiveDownloads()
                || rootWindow.hasActiveShutdownRequestsFor(secondaryUi, secondaryWindow)) {
            secondaryWindow.windowShutdownPromptVisible = true
            return
        }
        secondaryWindow.checkPageStateBeforeQuit()
    }

    function finalizeQuit() {
        if (!secondaryUi.flush_durable_state()) {
            secondaryWindow.windowShutdownStoragePromptVisible = true
            return
        }
        if (!secondaryUi.request_shutdown()) {
            return
        }
        secondaryWindow.cancelPageStateProbe()
        secondaryWindow.windowShutdownPromptVisible = false
        secondaryWindow.windowShutdownStoragePromptVisible = false
        secondaryWindow.windowShutdownApproved = true
        secondaryWindow.close()
    }

    function cancelDownloadsAndQuit() {
        if (secondaryWindow.pendingDownloadId.length > 0) {
            secondaryWindow.cancelPendingDownload()
        }
        for (var pendingKey in secondaryWindow.pendingDownloadRequests) {
            var pendingDownload = secondaryWindow.pendingDownloadRequests[pendingKey]
            if (pendingDownload) {
                rootWindow.resolveQtRequest(secondaryUi, pendingDownload, "download", "cancel", [])
                secondaryUi.update_download(
                    String(pendingKey), "cancelled", pendingDownload.receivedBytes, true)
            }
        }
        secondaryWindow.pendingDownloadRequests = ({})
        for (var key in secondaryWindow.activeDownloads) {
            var download = secondaryWindow.activeDownloads[key]
            if (download && !download.isFinished) {
                rootWindow.resolveQtRequest(secondaryUi, download, "download", "cancel", [])
            }
        }
        secondaryWindow.activeDownloads = ({})
        rootWindow.cancelShutdownRequestsFor(secondaryUi, secondaryWindow)
        secondaryWindow.continueQuitRequest()
    }

    FerricFileDialogSurfaces {
        id: secondaryDialogSurfaces
        onUserscriptManifestAccepted: secondarySettings.installManifest(userscriptManifestSelectedFile)
        onEngineFileAccepted: secondaryWindow.acceptFileDialog()
        onEngineFileRejected: secondaryWindow.rejectFileDialog()
        onEngineFolderAccepted: secondaryWindow.acceptFolderDialog()
        onEngineFolderRejected: secondaryWindow.rejectFileDialog()
    }

    Timer {
        id: secondaryFilePortalTimer
        interval: 50
        repeat: true
        onTriggered: secondaryWindow.maybeOpenPendingFileDialog()
    }

    FileDialog {
        id: secondaryDownloadChooser
        title: "Choose download destination"
        fileMode: FileDialog.SaveFile
        nameFilters: ["All files (*)"]
        onAccepted: secondaryWindow.acceptPendingDownload()
        onRejected: secondaryWindow.cancelPendingDownload()
    }

    function acceptPendingDownload() {
        var id = secondaryWindow.pendingDownloadId
        var download = secondaryWindow.pendingDownloadRequests[id]
        var selectedPath = rootWindow.primaryBrowserUi.local_file_path(secondaryDownloadChooser.selectedFile)
        if (!download) {
            secondaryWindow.pendingDownloadId = ""
            rootWindow.restoreOverlayFocus(secondaryWindow)
            return
        }
        var finalName = secondaryUi.accept_download_path(id, selectedPath)
        if (finalName.length === 0) {
            rootWindow.resolveQtRequest(secondaryUi, download, "download", "cancel", [])
            secondaryUi.update_download(id, "cancelled", download.receivedBytes, true)
        } else {
            var separator = selectedPath.lastIndexOf("/")
            var directory = separator > 0 ? selectedPath.slice(0, separator) : "/"
            var stagingDirectory = rootWindow.downloadStagingDirectory(secondaryUi, id)
            if (stagingDirectory.length === 0) {
                rootWindow.resolveQtRequest(secondaryUi, download, "download", "cancel", [])
                secondaryUi.update_download(id, "cancelled", download.receivedBytes, true)
                delete secondaryWindow.pendingDownloadRequests[id]
                secondaryWindow.pendingDownloadId = ""
                secondaryWindow.pendingDownloadSuggestedName = ""
                rootWindow.restoreOverlayFocus(secondaryWindow)
                return
            }
            download.downloadDirectory = stagingDirectory
            download.downloadFileName = finalName
            secondaryWindow.activeDownloads[id] = download
            download.stateChanged.connect(function() {
                secondaryUi.update_download(
                    id, secondaryWindow.secondaryDownloadStateName(download),
                    download.receivedBytes, false)
            })
            rootWindow.resolveQtRequest(secondaryUi, download, "download", "accept", [])
            secondaryUi.update_download(id, "in-progress", 0, false)
        }
        delete secondaryWindow.pendingDownloadRequests[id]
        secondaryWindow.pendingDownloadId = ""
        secondaryWindow.pendingDownloadSuggestedName = ""
        rootWindow.restoreOverlayFocus(secondaryWindow)
    }

    function cancelPendingDownload() {
        var id = secondaryWindow.pendingDownloadId
        var download = secondaryWindow.pendingDownloadRequests[id]
        if (download) {
            rootWindow.resolveQtRequest(secondaryUi, download, "download", "cancel", [])
            secondaryUi.update_download(id, "cancelled", download.receivedBytes, true)
        }
        delete secondaryWindow.pendingDownloadRequests[id]
        secondaryWindow.pendingDownloadId = ""
        secondaryWindow.pendingDownloadSuggestedName = ""
        rootWindow.restoreOverlayFocus(secondaryWindow)
    }

    function handleDownloadRequested(download) {
        var id = String(download.id)
        var safeName = secondaryUi.offer_download(id, download.url.toString(), download.suggestedFileName)
        var requestedPath = download.savePageFormat === WebEngineDownloadRequest.MimeHtmlSaveFormat
                ? secondaryUi.take_save_page_path() : ""
        if (requestedPath.length > 0) {
            var requestedName = secondaryUi.accept_download_path(id, requestedPath)
            if (requestedName.length === 0) {
                rootWindow.resolveQtRequest(secondaryUi, download, "download", "cancel", [])
                secondaryUi.update_download(id, "cancelled", download.receivedBytes, true)
                return
            }
            var requestedSeparator = requestedPath.lastIndexOf("/")
            var requestedDirectory = requestedSeparator > 0
                    ? requestedPath.slice(0, requestedSeparator) : "/"
            var stagingDirectory = rootWindow.downloadStagingDirectory(secondaryUi, id)
            if (stagingDirectory.length === 0) {
                rootWindow.resolveQtRequest(secondaryUi, download, "download", "cancel", [])
                secondaryUi.update_download(id, "cancelled", download.receivedBytes, true)
                return
            }
            download.downloadDirectory = stagingDirectory
            download.downloadFileName = requestedName
            secondaryWindow.activeDownloads[id] = download
            download.stateChanged.connect(function() {
                secondaryUi.update_download(id, secondaryWindow.secondaryDownloadStateName(download), download.receivedBytes, false)
            })
            rootWindow.resolveQtRequest(secondaryUi, download, "download", "accept", [])
            secondaryUi.update_download(id, "in-progress", 0, false)
            return
        }
        var directory = secondaryUi.default_download_directory()
        if (rootWindow.downloadsAskDestination(secondaryUi)) {
            if (secondaryWindow.pendingDownloadId.length > 0) {
                rootWindow.resolveQtRequest(secondaryUi, download, "download", "cancel", [])
                secondaryUi.update_download(id, "cancelled", download.receivedBytes, true)
                return
            }
            secondaryWindow.pendingDownloadRequests[id] = download
            secondaryWindow.pendingDownloadId = id
            secondaryWindow.pendingDownloadSuggestedName = safeName
            rootWindow.captureOverlayFocus(secondaryWindow, secondaryWindow.activeView)
            secondaryUi.update_download(id, "selecting-destination", 0, false)
            secondaryDownloadChooser.currentFile = rootWindow.fileUrlForPath(
                directory + "/" + safeName)
            secondaryDownloadChooser.open()
            return
        }
        var finalName = secondaryUi.accept_download(id, directory, safeName)
        if (finalName.length === 0) {
            rootWindow.resolveQtRequest(secondaryUi, download, "download", "cancel", [])
            return
        }
        var stagingDirectory = rootWindow.downloadStagingDirectory(secondaryUi, id)
        if (stagingDirectory.length === 0) {
            delete secondaryWindow.activeDownloads[id]
            rootWindow.resolveQtRequest(secondaryUi, download, "download", "cancel", [])
            secondaryUi.update_download(id, "cancelled", download.receivedBytes, true)
            return
        }
        download.downloadDirectory = stagingDirectory
        download.downloadFileName = finalName
        download.stateChanged.connect(function() {
            secondaryUi.update_download(id, secondaryWindow.secondaryDownloadStateName(download), download.receivedBytes, false)
        })
        rootWindow.resolveQtRequest(secondaryUi, download, "download", "accept", [])
        secondaryWindow.activeDownloads[id] = download
        secondaryUi.update_download(id, "in-progress", 0, false)
    }
    function handleDownloadFinished(download) {
        var id = String(download.id)
        var state = secondaryWindow.secondaryDownloadStateName(download)
        if (state === "completed" && !secondaryUi.finalize_download(id)) {
            state = "interrupted"
        } else if (state !== "completed") {
            secondaryUi.discard_download_staging(id)
        }
        delete secondaryWindow.activeDownloads[id]
        delete secondaryWindow.pendingDownloadRequests[id]
        if (secondaryWindow.pendingDownloadId === id) {
            secondaryWindow.pendingDownloadId = ""
            secondaryWindow.pendingDownloadSuggestedName = ""
        }
        secondaryUi.update_download(id, state, download.receivedBytes, true)
    }

    BrowserUi {
        id: secondaryUi
        hint_chrome_available: true
        status_text: "Ready"
        onRuntime_work_available: secondaryWindow.scheduleRuntimeWork(0)
        onStatus_textChanged: {
            secondaryWindow.scheduleRuntimeWork(0)
            if (secondaryWindow.commandNoticeVisible
                    && secondaryUi.mode !== "command"
                    && secondaryUi.status_text !== "Normal mode") {
                var loweredStatus = secondaryUi.status_text.toLowerCase()
                secondaryWindow.showCommandNotice(
                    secondaryUi.status_text,
                    loweredStatus.indexOf("failed") >= 0
                    || loweredStatus.indexOf("error") >= 0
                    || loweredStatus.indexOf("rejected") >= 0)
            }
        }
    }

    FerricWindowAppearance {
        id: secondaryAppearance
        browserUi: secondaryUi
        hostWindow: secondaryWindow
        themeWindow: rootWindow
    }
    FerricSettingsWorkspace {
        id: secondarySettings
        objectName: "secondarySettingsWorkspace"
        browserWindow: secondaryAppearance
        hostWindow: secondaryWindow
        browserUi: secondaryUi
        transientProfile: secondaryWindow.windowTransientProfile
        profileLabel: secondaryWindow.windowProfileLabel
        spellcheckStatus: rootWindow.spellcheckStatusText(secondaryUi)
        spellcheckDictionariesMissing: rootWindow.spellcheckDictionaryStatus(secondaryUi).missing.length > 0
        onOpening: rootWindow.captureOverlayFocus(secondaryWindow, secondaryWindow.activeView)
        onClosed: rootWindow.restoreOverlayFocus(secondaryWindow)
        onInstallManifestRequested: secondaryDialogSurfaces.openUserscriptManifest()
    }

    FerricShutdownDecisionDialog {
        hostWindow: secondaryWindow
        themeWindow: rootWindow
        promptVisible: secondaryWindow.windowCloseConfirmationVisible
        commandText: ":window-close"
        title: "Close this browser window?"
        message: {
            var count = secondaryUi.tab_count
            return "This will close this window and its " + count
                    + (count === 1 ? " open tab." : " open tabs.")
        }
        keepLabel: "Keep window open"
        proceedLabel: "Close window"
        proceedAccessibleName: "Close browser window"
        dialogHeight: 220 * rootWindow.chromeScale
        onKeepRequested: {
            secondaryWindow.windowCloseConfirmationVisible = false
            secondaryUi.status_text = "Window close cancelled"
        }
        onProceedRequested: {
            secondaryWindow.windowCloseConfirmationVisible = false
            secondaryWindow.continueQuitRequest()
        }
    }

    FerricShutdownDecisionDialog {
        hostWindow: secondaryWindow
        themeWindow: rootWindow
        promptVisible: secondaryWindow.windowShutdownPromptVisible
        commandText: ":window-close"
        title: "Active browser work is still running"
        message: "Cancel active work before closing, or keep this window open."
        keepLabel: "Keep window open"
        proceedLabel: "Cancel and close"
        dialogHeight: 190 * rootWindow.chromeScale
        onKeepRequested: {
            secondaryWindow.windowShutdownPromptVisible = false
            rootWindow.abortApplicationShutdown()
            secondaryUi.status_text = "Shutdown cancelled"
        }
        onProceedRequested: {
            secondaryWindow.windowShutdownPromptVisible = false
            secondaryWindow.cancelDownloadsAndQuit()
        }
    }

    FerricShutdownDecisionDialog {
        hostWindow: secondaryWindow
        themeWindow: rootWindow
        promptVisible: secondaryWindow.windowShutdownPagePromptVisible
        commandText: ":window-close"
        title: "Page state may be lost"
        message: secondaryWindow.windowShutdownPagePromptReason
                 + " Close anyway may lose that state."
        keepLabel: "Keep window open"
        proceedLabel: "Close anyway"
        proceedAccessibleName: "Close anyway despite page state"
        dialogHeight: 220 * rootWindow.chromeScale
        onKeepRequested: {
            secondaryWindow.cancelPageStateProbe()
            rootWindow.abortApplicationShutdown()
            secondaryUi.status_text = "Shutdown cancelled"
        }
        onProceedRequested: {
            secondaryWindow.windowShutdownPagePromptVisible = false
            secondaryWindow.finalizeQuit()
        }
    }

    FerricShutdownDecisionDialog {
        hostWindow: secondaryWindow
        themeWindow: rootWindow
        promptVisible: secondaryWindow.windowShutdownStoragePromptVisible
        commandText: ":window-close"
        title: "Durable profile state could not be flushed"
        message: "The window remains open so its session and profile data are not abandoned. Retry the close after checking storage availability, or keep it open."
        keepLabel: "Keep window open"
        proceedLabel: "Retry close"
        keepAccessibleName: "Keep window open after storage flush failure"
        dialogBorderColor: rootWindow.errorColor
        dialogHeight: 220 * rootWindow.chromeScale
        stackingOrder: 101
        onKeepRequested: {
            secondaryWindow.windowShutdownStoragePromptVisible = false
            rootWindow.abortApplicationShutdown()
            secondaryUi.status_text = "Shutdown cancelled; durable state was retained"
        }
        onProceedRequested: {
            secondaryWindow.windowShutdownStoragePromptVisible = false
            secondaryWindow.finalizeQuit()
        }
    }

    Connections {
        target: rootWindow.primaryBrowserUi
        function onConfig_jsonChanged() {
            secondaryUi.set_startup_configuration(
                        rootWindow.primaryBrowserUi.config_json, rootWindow.primaryBrowserUi.config_base_json,
                        rootWindow.primaryBrowserUi.cli_overrides_json,
                        secondaryUi.profile_overrides_json,
                        rootWindow.primaryBrowserUi.config_path, secondaryUi.config_source)
        }
        function onConfig_base_jsonChanged() {
            secondaryUi.set_startup_configuration(
                        secondaryUi.config_json, rootWindow.primaryBrowserUi.config_base_json,
                        secondaryUi.cli_overrides_json,
                        secondaryUi.profile_overrides_json,
                        secondaryUi.config_path, secondaryUi.config_source)
        }
        function onContexts_jsonChanged() {
            secondaryUi.set_contexts_configuration(rootWindow.primaryBrowserUi.contexts_json)
        }
    }

    Connections {
        target: secondaryUi
function onContext_route_idChanged() {
    rootWindow.showContextRoute(secondaryUi)
}
function onContext_workspaceChanged() {
    rootWindow.routeContextWorkspace(secondaryUi)
}
        function onExternal_navigation_visibleChanged() {
            if (secondaryUi.external_navigation_visible) {
                rootWindow.captureOverlayFocus(secondaryWindow, secondaryWindow.activeView)
            } else {
                rootWindow.restoreOverlayFocus(secondaryWindow)
            }
        }
    }

    FerricExternalNavigationDialog {
        id: secondaryExternalNavigationPopup
        browserWindow: rootWindow
        browserUi: secondaryUi
        hostWindow: secondaryWindow
        confirmedActionHandler: function() {
            secondaryWindow.applySwitcherEngineAction()
        }
    }

    function secondaryDownloadStateName(download) {
        if (download.state === WebEngineDownloadRequest.DownloadInProgress) {
            return "in-progress"
        }
        if (download.state === WebEngineDownloadRequest.DownloadCompleted) {
            return "completed"
        }
        if (download.state === WebEngineDownloadRequest.DownloadCancelled) {
            return "cancelled"
        }
        if (download.state === WebEngineDownloadRequest.DownloadInterrupted) {
            return "interrupted"
        }
        return "offered"
    }

    function secondaryFileDialogPaths() {
        var paths = []
        var urls = secondaryDialogSurfaces.engineFileMode === FileDialog.OpenFiles
                ? secondaryDialogSurfaces.engineFileSelectedFiles
                : [secondaryDialogSurfaces.engineFileSelectedFile]
        for (var i = 0; i < urls.length; ++i) {
            var path = rootWindow.primaryBrowserUi.local_file_path(urls[i])
            if (!path) {
                return []
            }
            paths.push(path)
        }
        return paths
    }

    function secondaryFolderDialogPaths() {
        var url = secondaryDialogSurfaces.engineFolderSelectedFolder
        var path = rootWindow.primaryBrowserUi.local_file_path(url || "")
        return path ? [path] : []
    }

    function clearSecondaryFileDialog(cancelRequest, message) {
        var request = secondaryWindow.pendingFileDialogRequest
        var hadRequest = !!request
        secondaryWindow.pendingFileDialogRequest = null
        secondaryWindow.pendingFileDialogView = null
        secondaryWindow.pendingFileDialogWaitingForPortal = false
        secondaryWindow.pendingFileDialogPortalDeadlineMs = 0
        secondaryFilePortalTimer.stop()
        if (secondaryDialogSurfaces.engineFileVisible) {
            secondaryDialogSurfaces.closeEngineFile()
        }
        if (secondaryDialogSurfaces.engineFolderVisible) {
            secondaryDialogSurfaces.closeEngineFolder()
        }
        if (hadRequest) {
            rootWindow.restoreOverlayFocus(secondaryWindow)
        }
        if (cancelRequest && request) {
            rootWindow.resolveQtRequest(secondaryUi, request, "file-dialog", "dialogReject", [])
        }
        if (message) {
            secondaryUi.status_text = message
        }
    }

    function acceptFileDialog() {
        var request = secondaryWindow.pendingFileDialogRequest
        var paths = secondaryWindow.secondaryFileDialogPaths()
        secondaryWindow.pendingFileDialogRequest = null
        secondaryWindow.pendingFileDialogView = null
        if (!request || paths.length === 0) {
            if (request) {
                rootWindow.resolveQtRequest(secondaryUi, request, "file-dialog", "dialogReject", [])
            }
            if (request) {
                rootWindow.restoreOverlayFocus(secondaryWindow)
            }
            secondaryUi.status_text = "File selection returned no local paths"
            return
        }
        rootWindow.resolveQtRequest(secondaryUi, request, "file-dialog", "dialogAccept", [paths])
        rootWindow.restoreOverlayFocus(secondaryWindow)
        secondaryUi.status_text = paths.length === 1
                ? "File selected"
                : paths.length + " files selected"
    }

    function acceptFolderDialog() {
        var request = secondaryWindow.pendingFileDialogRequest
        var paths = secondaryWindow.secondaryFolderDialogPaths()
        secondaryWindow.pendingFileDialogRequest = null
        secondaryWindow.pendingFileDialogView = null
        if (!request || paths.length === 0) {
            if (request) {
                rootWindow.resolveQtRequest(secondaryUi, request, "file-dialog", "dialogReject", [])
            }
            if (request) {
                rootWindow.restoreOverlayFocus(secondaryWindow)
            }
            secondaryUi.status_text = "Folder selection returned no local path"
            return
        }
        rootWindow.resolveQtRequest(secondaryUi, request, "file-dialog", "dialogAccept", [paths])
        rootWindow.restoreOverlayFocus(secondaryWindow)
        secondaryUi.status_text = "Folder selected"
    }

    function rejectFileDialog() {
        var request = secondaryWindow.pendingFileDialogRequest
        if (!request) {
            return
        }
        secondaryWindow.pendingFileDialogRequest = null
        secondaryWindow.pendingFileDialogView = null
        rootWindow.resolveQtRequest(secondaryUi, request, "file-dialog", "dialogReject", [])
        rootWindow.restoreOverlayFocus(secondaryWindow)
        secondaryUi.status_text = "File selection cancelled"
    }

    function handleFileDialogRequested(request) {
        if (!request) return
        request.accepted = true
        if (secondaryWindow.pendingFileDialogRequest) {
            secondaryWindow.clearSecondaryFileDialog(true,
                    "Previous file selection cancelled")
            rootWindow.resolveQtRequest(
                secondaryUi, request, "file-dialog", "dialogReject", [])
            secondaryUi.status_text = "Another file selection is already open"
            return
        }
        rootWindow.captureOverlayFocus(secondaryWindow, secondaryWindow.activeView)
        secondaryWindow.pendingFileDialogRequest = request
        secondaryWindow.pendingFileDialogView = secondaryWindow.activeView
        var requestUi = secondaryUi
        if (rootWindow.desktopPortalMode(requestUi) === "required") {
            var status = rootWindow.desktopPortalCapabilityStatus(requestUi, "file_chooser")
            if (status === "not-probed") {
                rootWindow.primaryBrowserUi.probe_desktop_portals()
                status = rootWindow.desktopPortalCapabilityStatus(requestUi, "file_chooser")
            }
            if (status === "pending" || status === "not-probed") {
                secondaryWindow.pendingFileDialogWaitingForPortal = true
                secondaryWindow.pendingFileDialogPortalDeadlineMs = Date.now() + 5000
                requestUi.status_text = "Checking required desktop portal…"
                secondaryFilePortalTimer.start()
                return
            }
            if (status !== "available") {
                secondaryWindow.clearSecondaryFileDialog(
                    true, "Required desktop portal unavailable; file selection cancelled")
                return
            }
        }
        secondaryWindow.pendingFileDialogWaitingForPortal = false
        secondaryFilePortalTimer.stop()
        if (request.mode === FileDialogRequest.FileModeUploadFolder) {
            secondaryDialogSurfaces.openEngineFolder()
            return
        }
        secondaryDialogSurfaces.engineFileMode = request.mode === FileDialogRequest.FileModeOpenMultiple
                ? FileDialog.OpenFiles
                : request.mode === FileDialogRequest.FileModeSave
                    ? FileDialog.SaveFile
                    : FileDialog.OpenFile
        secondaryDialogSurfaces.engineFileNameFilters = rootWindow.fileDialogNameFilters(request)
        secondaryDialogSurfaces.engineFileCurrentFile = request.defaultFileName.length > 0
                ? request.defaultFileName
                : ""
        secondaryDialogSurfaces.engineFileTitle = request.mode === FileDialogRequest.FileModeSave
                ? "Save file"
                : request.mode === FileDialogRequest.FileModeOpenMultiple
                    ? "Choose files"
                    : "Choose file"
        secondaryDialogSurfaces.openEngineFile()
    }

    function maybeOpenPendingFileDialog() {
        if (!secondaryWindow.pendingFileDialogWaitingForPortal
                || !secondaryWindow.pendingFileDialogRequest) {
            secondaryFilePortalTimer.stop()
            return
        }
        if (Date.now() >= secondaryWindow.pendingFileDialogPortalDeadlineMs) {
            secondaryWindow.clearSecondaryFileDialog(
                true, "Desktop portal check timed out; file selection cancelled")
            return
        }
        var ui = secondaryUi
        var status = rootWindow.desktopPortalCapabilityStatus(ui, "file_chooser")
        if (status === "pending" || status === "not-probed") {
            return
        }
        if (status !== "available") {
            secondaryWindow.clearSecondaryFileDialog(
                true, "Required desktop portal unavailable; file selection cancelled")
            return
        }
        secondaryWindow.pendingFileDialogWaitingForPortal = false
        secondaryFilePortalTimer.stop()
        var request = secondaryWindow.pendingFileDialogRequest
        if (request.mode === FileDialogRequest.FileModeUploadFolder) {
            secondaryDialogSurfaces.openEngineFolder()
            return
        }
        secondaryDialogSurfaces.engineFileMode = request.mode === FileDialogRequest.FileModeOpenMultiple
                ? FileDialog.OpenFiles
                : request.mode === FileDialogRequest.FileModeSave
                    ? FileDialog.SaveFile
                    : FileDialog.OpenFile
        secondaryDialogSurfaces.engineFileNameFilters = rootWindow.fileDialogNameFilters(request)
        secondaryDialogSurfaces.engineFileCurrentFile = request.defaultFileName.length > 0
                ? request.defaultFileName : ""
        secondaryDialogSurfaces.engineFileTitle = request.mode === FileDialogRequest.FileModeSave
                ? "Save file"
                : request.mode === FileDialogRequest.FileModeOpenMultiple
                    ? "Choose files"
                    : "Choose file"
        secondaryDialogSurfaces.openEngineFile()
    }

    function clearFileDialogForView(view) {
        if (secondaryWindow.pendingFileDialogView === view) {
            secondaryWindow.clearSecondaryFileDialog(
                true, "File selection cancelled by navigation")
        }
    }

    function applySwitcherEngineAction() {
        var action = secondaryUi.take_engine_action()
        if (!action || action.length === 0) {
            return
        }
        if (action.indexOf("new-window\t") === 0 || action.indexOf("context-window\t") === 0) {
            rootWindow.executeNewWindowAction(secondaryUi, action)
            return
        }
        if (action.indexOf("window-focus\t") === 0) {
            rootWindow.executeWindowFocusAction(secondaryUi, action)
            return
        }
        if (action === "show-settings" || action.indexOf("show-settings\t") === 0) {
            secondarySettings.show(action === "show-settings" ? undefined : action.slice("show-settings\t".length))
            return
        }
        if (action.indexOf("tab-close\t") === 0) {
            var tabCloseParts = action.split("\t")
            var tabCloseId = tabCloseParts.length >= 2 ? tabCloseParts[1] : ""
            var tabCloseIndex = secondaryUi.tab_index_for_id(tabCloseId)
            if (tabCloseIndex < 0) {
                secondaryUi.status_text = "Tab close target is stale"
                return
            }
            if (!secondaryWindow.closeTabAtIndex(tabCloseIndex)) {
                secondaryUi.status_text = "Tab close was rejected"
                return
            }
            return
        }
        if (action.indexOf("tab-close-active\t") === 0) {
            var tabCloseActiveParts = action.split("\t")
            var tabCloseCount = Number(tabCloseActiveParts[1])
            if (!Number.isFinite(tabCloseCount) || tabCloseCount < 1 || tabCloseCount > 100) {
                secondaryUi.status_text = "Tab close count is invalid"
                return
            }
            var closedCount = 0
            for (var tabCloseAttempt = 0; tabCloseAttempt < tabCloseCount; ++tabCloseAttempt) {
                var candidateIndex = -1
                var activeTabIndex = secondaryUi.active_tab_index
                if (activeTabIndex >= 0 && activeTabIndex < secondaryTabs.count
                        && !secondaryTabs.get(activeTabIndex).pinned) {
                    candidateIndex = activeTabIndex
                } else {
                    for (var tabCloseCandidate = 0;
                         tabCloseCandidate < secondaryTabs.count;
                         ++tabCloseCandidate) {
                        if (!secondaryTabs.get(tabCloseCandidate).pinned) {
                            candidateIndex = tabCloseCandidate
                            break
                        }
                    }
                }
                if (candidateIndex < 0 || !secondaryWindow.closeTabAtIndex(candidateIndex)) {
                    break
                }
                closedCount += 1
            }
            secondaryUi.status_text = closedCount > 0
                    ? "Closed " + closedCount + " tab" + (closedCount === 1 ? "" : "s")
                    : "No unpinned tabs available to close"
            return
        }
        if (action === "show-diagnostics") { rootWindow.showDiagnostics(secondaryUi); return }
        if (action === "show-profiles") { secondaryManagers.showProfiles(); return }
        if (action === "show-sessions") { secondaryManagers.showSessions(); return }
        if (action.indexOf("profile-delete\t") === 0) {
            secondaryManagers.showProfileDeletePreview(action.slice("profile-delete\t".length))
            return
        }
        if (action.indexOf("profile-window\t") === 0) {
            var profileWindowAction = action.split("\t")
            if (profileWindowAction.length >= 4) {
                rootWindow.openProfileWindow(profileWindowAction[1], profileWindowAction[2],
                    profileWindowAction.slice(3).join("\t"), secondaryUi.current_url)
                secondaryManagers.closeProfiles()
            }
            return
        }
        if (action.indexOf("ephemeral-window\t") === 0) {
            rootWindow.openEphemeralWindow(action.slice("ephemeral-window\t".length))
            return
        }
        if (action.indexOf("show-session-preview\t") === 0) {
            secondaryManagers.showSessionPreview(action.split("\t").slice(1).join("\t"), false)
            return
        }
        if (action.indexOf("edit\t") === 0) {
            rootWindow.applyNativeEditAction(secondaryUi, secondaryWindow.activeView, action)
            return
        }
        if (action.indexOf("command-prefill\t") === 0) {
            secondaryCommandBar.commandText = action.slice("command-prefill\t".length)
            secondaryCommandBar.cursorPosition = secondaryCommandBar.commandText.length
            secondaryCommandBar.focusInput()
            return
        }
        if (action === "quit-request") {
            rootWindow.beginQuitRequest(":quit")
            return
        }
        if (action === "window-close-request") {
            secondaryWindow.beginQuitRequest()
            return
        }
        if (action.indexOf("scroll-target\t") === 0) {
            var scrollTargetParts = action.split("\t")
            var scrollTargetIndex = scrollTargetParts.length === 3
                    ? secondaryUi.tab_index_for_id(scrollTargetParts[1]) : -1
            var scrollTargetView = secondaryWindow.tabViewAt(scrollTargetIndex)
            rootWindow.applyScrollTargetAction(
                secondaryUi, scrollTargetView,
                scrollTargetParts.length === 3 ? scrollTargetParts[2] : "",
                secondaryWindow)
            return
        }
        if (action === "tab-detach" || action.indexOf("tab-detach\t") === 0
                || action.indexOf("tab-give\t") === 0) {
            rootWindow.executeBrowserTransferAction(secondaryUi, secondaryWindow, action)
            return
        }
        if (action.indexOf("tab-suspend-request\t") === 0) {
            secondaryUi.status_text = "Only hidden tabs can be suspended"
            return
        }
        if (action.indexOf("tab-discard-request\t") === 0) {
            secondaryUi.status_text = "Only hidden tabs can be discarded"
            return
        }
        if (action.indexOf("tab-resume\t") === 0) {
            secondaryWindow.activeView.lifecycleState = WebEngineView.LifecycleState.Active
            secondaryUi.status_text = "Tab resumed; page state is live"
            return
        }
        if (action.indexOf("tab-mute\t") === 0) {
            var secondaryMuteParts = action.split("\t")
            var secondaryMuteIndex = secondaryUi.tab_index_for_id(secondaryMuteParts[1] || "")
            var secondaryTabId = secondaryUi.tab_id_for_index(secondaryMuteIndex)
            var secondaryMuted = secondaryMuteParts.length >= 3
                    && secondaryMuteParts[2] === "true"
            if (secondaryMuteIndex < 0 || secondaryMuteParts.length < 2
                    || secondaryMuteParts[1] !== secondaryTabId) {
                secondaryUi.status_text = "Tab mute target is stale"
                return
            }
            secondaryTabs.setProperty(secondaryMuteIndex, "muted", secondaryMuted)
            var mutedView = secondaryWindow.tabViewAt(secondaryMuteIndex)
            if (mutedView) mutedView.audioMuted = secondaryMuted
            secondaryUi.status_text = secondaryMuted ? "Tab muted" : "Tab unmuted"
            return
        }
        if (action.indexOf("navigate\t") === 0) {
            var navigation = action.split("\t")
            if (navigation.length >= 3) {
                var navigationIndex = Number(navigation[1])
                var navigationView = secondaryWindow.tabViewAt(navigationIndex)
                if (navigationView) {
                    secondaryTabs.setProperty(navigationIndex, "loaded", true)
                    navigationView.setContentUrl(navigation.slice(2).join("\t"), false)
                }
                return
            }
        }
        if (action === "start-page-cycle") {
            if (activeView.nativeUrl === "rb://start" && activeView.startPage) activeView.startPage.cycleView()
            return
        }
        if (action.indexOf("start-page-view\t") === 0 || action.indexOf("start-page-add\t") === 0) {
            var startParts = action.split("\t")
            if (activeView.nativeUrl === "rb://start" && activeView.startPage) {
                if (startParts[0] === "start-page-view") activeView.startPage.chooseView(startParts[1])
                else activeView.startPage.addCaptured(startParts[1], startParts.slice(2).join("\t"))
            }
            return
        }
        if (action.indexOf("reload\t") === 0 && activeView.nativeUrl.length > 0) {
            if (activeView.startPage) activeView.startPage.refresh()
            return
        }
        if (action.indexOf("reload\t") === 0) {
            var secondaryReloadParts = action.split("\t")
            if (secondaryReloadParts[1] === "true") {
                secondaryWindow.activeView.reloadAndBypassCache()
            } else {
                secondaryWindow.activeView.reload()
            }
            secondaryWindow.activeView.forceActiveFocus()
            return
        }
        if (action === "back" || action.indexOf("back\t") === 0
                || action === "forward" || action.indexOf("forward\t") === 0) {
            var historyParts = action.split("\t")
            var historyCount = Number(historyParts.length >= 2
                                      ? historyParts[1] : 1)
            var historyBack = historyParts[0] === "back"
            rootWindow.executeHistoryTraversal(secondaryWindow.activeView, historyBack, historyCount)
            secondaryWindow.activeView.forceActiveFocus()
            return
        }
        if (action === "stop") {
            secondaryWindow.activeView.stop()
            secondaryWindow.activeView.forceActiveFocus()
            return
        }
        if (action.indexOf("fullscreen\t") === 0) {
            var fullscreenState = action.split("\t")[1] || "toggle"
            var currentlyFullscreen = secondaryWindow.visibility === Window.FullScreen
            var enterFullscreen = fullscreenState === "on"
                    || (fullscreenState === "toggle" && !currentlyFullscreen)
            if (enterFullscreen) {
                secondaryWindow.showFullScreen()
                secondaryUi.status_text = "Fullscreen enabled"
            } else {
                secondaryWindow.showNormal()
                secondaryUi.status_text = "Fullscreen ended"
            }
            return
        }
        if (action.indexOf("reopen-window\t") === 0) {
            rootWindow.openReopenedWindow(secondaryUi, secondaryWindow, action)
            secondaryWindow.activeView.forceActiveFocus()
            return
        }
        if (action.indexOf("zoom\t") === 0) {
            var zoomParts = action.split("\t")
            if (zoomParts.length >= 3) {
                var zoomTabIndex = secondaryUi.tab_index_for_id(zoomParts[1])
                var zoomFactor = Number(zoomParts[2])
                if (zoomTabIndex >= 0 && Number.isFinite(zoomFactor)
                        && zoomFactor >= 0.25 && zoomFactor <= 5.0) {
                    secondaryTabs.setProperty(zoomTabIndex, "zoom", zoomFactor)
                    var zoomView = tabViewAt(zoomTabIndex)
                    if (zoomView) {
                        zoomView.zoomFactor = zoomFactor
                    }
                }
            }
            return
        }
        if (action.indexOf("find-next\t") === 0) {
            var secondaryFindParts = action.split("\t")
            var secondaryFindCount = Number(secondaryFindParts[1])
            var secondaryFindBackward = secondaryFindParts[2] === "true"
            if (Number.isFinite(secondaryFindCount)
                    && secondaryFindCount >= 1 && secondaryFindCount <= 100) {
                var secondaryFindFlags = rootWindow.searchFindFlags(
                    secondaryUi.search_text, secondaryFindParts[3], secondaryFindBackward)
                for (var secondaryFindIndex = 0;
                     secondaryFindIndex < secondaryFindCount;
                     ++secondaryFindIndex) {
                    secondaryWindow.activeView.findText(secondaryUi.search_text, secondaryFindFlags)
                }
            }
            secondaryWindow.activeView.forceActiveFocus()
            return
        }
        if (action.indexOf("find\t") === 0) {
            var secondaryFindRequest = action.split("\t")
            var secondaryFindFlags = rootWindow.searchFindFlags(
                secondaryUi.search_text, secondaryFindRequest[2],
                secondaryFindRequest[1] === "true")
            secondaryWindow.activeView.findText(secondaryUi.search_text, secondaryFindFlags)
            secondaryWindow.activeView.forceActiveFocus()
            return
        }
        if (action.indexOf("scroll\t") === 0
                || action.indexOf("scroll-page\t") === 0
                || action.indexOf("scroll-to\t") === 0) {
            var secondaryScrollParts = action.split("\t")
            if (secondaryScrollParts[0] === "scroll" && secondaryScrollParts.length >= 3) {
                rootWindow.runBrowserScript(secondaryWindow.activeView, rootWindow.scrollScript(
                    "scroll", secondaryScrollParts[1], false,
                    Number(secondaryScrollParts[2])))
            } else if (secondaryScrollParts[0] === "scroll-page"
                       && secondaryScrollParts.length >= 4) {
                rootWindow.runBrowserScript(secondaryWindow.activeView, rootWindow.scrollScript(
                    "scroll-page", secondaryScrollParts[1],
                    secondaryScrollParts[2] === "true",
                    Number(secondaryScrollParts[3])))
            } else if (secondaryScrollParts[0] === "scroll-to"
                       && secondaryScrollParts.length >= 2) {
                rootWindow.runBrowserScript(secondaryWindow.activeView, rootWindow.scrollScript(
                    "scroll-to", secondaryScrollParts[1], false, 1))
            }
            secondaryWindow.activeView.forceActiveFocus()
            return
        }
        if (action.indexOf("download-open\t") === 0) {
            rootWindow.openExternalUri(secondaryUi,
                                   action.split("\t").slice(1).join("\t"))
        }
        if (action.indexOf("save-page\t") === 0) {
            var secondarySaveParts = action.split("\t")
            if (secondarySaveParts.length >= 3) {
                var secondarySavePath = secondaryUi.prepare_save_page(
                    secondarySaveParts.slice(2).join("\t"))
                if (secondarySavePath.length > 0) {
                    secondaryWindow.activeView.save(
                        secondarySavePath,
                        WebEngineDownloadRequest.MimeHtmlSaveFormat)
                    secondaryUi.status_text =
                            "Page save requested; completion is tracked in Downloads"
                } else {
                    secondaryUi.take_save_page_path()
                }
            }
            secondaryWindow.activeView.forceActiveFocus()
            return
        }
        if (action.indexOf("view-source\t") === 0) {
            var secondarySourceParts = action.split("\t")
            if (secondarySourceParts.length >= 3) {
                var secondarySourceUrl = secondarySourceParts.slice(2).join("\t")
                secondaryWindow.activeView.url = "view-source:" + secondarySourceUrl
                secondaryUi.status_text = "Viewing page source"
            }
            secondaryWindow.activeView.forceActiveFocus()
            return
        }
        if (action === "jseval") {
            var secondaryEvalId = String(secondaryUi.jseval_tab_id)
            var secondaryEvalIndex = secondaryUi.tab_index_for_id(secondaryEvalId)
            var secondaryEvalView = secondaryWindow.tabViewAt(secondaryEvalIndex)
            var secondaryEvalScript = String(secondaryUi.jseval_script)
            var secondaryEvalWorld = secondaryUi.jseval_world === "page"
                    ? WebEngineScript.MainWorld : rootWindow.browserScriptWorld
            if (!secondaryEvalView || secondaryEvalScript.length === 0
                    || secondaryEvalScript.length > 65536) {
                secondaryUi.status_text =
                        "JavaScript evaluation target is stale or invalid"
                return
            }
            rootWindow.runBrowserScript(secondaryEvalView, secondaryEvalScript,
                function() {
                    secondaryUi.status_text = "JavaScript evaluation completed"
                }, secondaryEvalWorld)
            secondaryEvalView.forceActiveFocus()
            return
        }
        if (action.indexOf("devtools\t") === 0) {
            var secondaryDevtoolsParts = action.split("\t")
            var secondaryDetach = secondaryDevtoolsParts.length >= 2
                    && secondaryDevtoolsParts[1] === "true"
            if (!secondaryDetach) {
                {
                    secondaryWindow.devToolsVisible =
                            !secondaryWindow.devToolsVisible
                    secondaryUi.status_text = secondaryWindow.devToolsVisible
                            ? "DevTools attached" : "DevTools closed"
                }
            } else if (secondaryWindow.activeView) {
                if (secondaryWindow.devToolsWindow) {
                    secondaryWindow.devToolsWindow.close()
                    secondaryWindow.devToolsWindow = null
                }
                var secondaryDevtools = rootWindow.devToolsWindowFactory.createObject(null, {
                    inspectView: secondaryWindow.activeView
                })
                if (secondaryDevtools) {
                    secondaryWindow.devToolsWindow = secondaryDevtools
                    secondaryWindow.activeView.devToolsView =
                            secondaryDevtools.inspectorView
                    secondaryDevtools.show()
                    secondaryUi.status_text = "DevTools detached"
                } else {
                    secondaryUi.status_text = "DevTools window could not be created"
                }
            }
            secondaryWindow.activeView.forceActiveFocus()
            return
        }
        if (action.indexOf("print\t") === 0) {
            var secondaryPrintParts = action.split("\t")
            var secondaryPrintPath = secondaryUi.prepare_print_job()
            if (secondaryPrintPath.length > 0 && secondaryWindow.activeView) {
                secondaryWindow.activeView.printToPdf(function(success) {
                    secondaryUi.finish_print_job(secondaryPrintPath, success)
                })
            } else {
                secondaryUi.finish_print_job(secondaryPrintPath, false)
            }
            secondaryWindow.activeView.forceActiveFocus()
            return
        }
        if (action.indexOf("print-pdf\t") === 0) {
            var secondaryPdfParts = action.split("\t")
            if (secondaryPdfParts.length >= 3) {
                var secondaryPdfPath = secondaryUi.prepare_print_pdf(
                    secondaryPdfParts.slice(2).join("\t"))
                if (secondaryPdfPath.length > 0 && secondaryWindow.activeView) {
                    secondaryWindow.activeView.printToPdf(secondaryPdfPath)
                } else {
                    secondaryUi.finish_print_pdf(
                        secondaryPdfParts.slice(2).join("\t"), false)
                }
            }
            return
        }
        if (action.indexOf("external-open\t") === 0) {
            rootWindow.openExternalUri(secondaryUi,
                                   action.split("\t").slice(1).join("\t"))
            return
        }
        secondaryUi.status_text = "Browser action is unavailable in this window: "
                + action.split("\t")[0]
        secondaryWindow.activeView.forceActiveFocus()
    }

    function scheduleRuntimeWork(delay) {
        if (secondaryWindow.runtimeReleased) return
        var boundedDelay = Math.max(0, Math.min(60000, Number(delay) || 0))
        if (!secondaryRuntimeWorkTimer.running
                || boundedDelay < secondaryRuntimeWorkTimer.interval) {
            secondaryRuntimeWorkTimer.interval = Math.max(1, boundedDelay)
            secondaryRuntimeWorkTimer.restart()
        }
    }

    function processRuntimeWork() {
        if (secondaryWindow.runtimeReleased) return
        secondaryUi.poll_ipc()
        secondaryWindow.syncTabModel()
        secondaryWindow.applySwitcherEngineAction()
        secondaryUi.poll_config()
        secondaryManagers.presentCommandRequest()
        secondaryWindow.syncTabModel()
        rootWindow.updateBrowserWindowView(secondaryUi, secondaryWindow.activeView)
        secondaryWindow.maybeOpenPendingFileDialog()
        secondaryUi.refresh_operations()
        secondaryUi.tick_bindings()
        secondaryUi.tick_site_doctor_experiment()
        secondaryUi.checkpoint_session()

        if (secondaryUi.take_caret_request()) {
            var caretToken = secondaryUi.caret_request_token
            var caretOperation = secondaryUi.caret_request_operation
            if (secondaryWindow.activeView) {
                rootWindow.runBrowserScript(
                    secondaryWindow.activeView,
                    rootWindow.caretScript(caretOperation,
                                           secondaryWindow.caretSelecting),
                    function(value) {
                        var response = value || {error: "caret script returned no result"}
                        if (response.selecting !== undefined) {
                            secondaryWindow.caretSelecting = response.selecting
                        }
                        secondaryUi.deliver_caret(caretToken, JSON.stringify(response))
                    })
            } else {
                secondaryUi.deliver_caret(
                    caretToken, JSON.stringify({error: "document view unavailable"}))
            }
        }

        var editorRequest = secondaryUi.take_editor_request()
        if (editorRequest.length > 0) {
            if (secondaryWindow.activeView) {
                rootWindow.runBrowserScript(
                    secondaryWindow.activeView, rootWindow.editorScript(),
                    function(value) {
                        secondaryUi.deliver_editor(
                            editorRequest,
                            JSON.stringify(value
                                           || {error: "editor script returned no result"}))
                    })
            } else {
                secondaryUi.deliver_editor(
                    editorRequest,
                    JSON.stringify({error: "document view unavailable"}))
            }
        }

        if (secondaryUi.take_editor_completion()) {
            var editorToken = secondaryUi.editor_completion_token
            var editorOriginal = secondaryUi.editor_completion_original
            var editorUpdated = secondaryUi.editor_completion_updated
            var editorError = secondaryUi.editor_completion_error
            var editorStderr = secondaryUi.editor_completion_stderr
            if (editorError.length > 0) {
                if (editorStderr.length > 0) {
                    editorError += " (stderr: " + editorStderr + ")"
                }
                secondaryUi.deliver_editor_apply(
                    editorToken, JSON.stringify({error: editorError}))
            } else if (secondaryWindow.activeView) {
                rootWindow.runBrowserScript(
                    secondaryWindow.activeView,
                    rootWindow.editorApplyScript(editorOriginal, editorUpdated),
                    function(value) {
                        secondaryUi.deliver_editor_apply(
                            editorToken,
                            JSON.stringify(value
                                           || {error: "editor apply returned no result"}))
                    })
            } else {
                secondaryUi.deliver_editor_apply(
                    editorToken,
                    JSON.stringify({error: "document view unavailable"}))
            }
        }

        var selectionToken = secondaryUi.take_selection_request()
        if (selectionToken.length > 0) {
            if (secondaryWindow.activeView) {
                rootWindow.readSelection(secondaryUi, secondaryWindow.activeView, selectionToken)
            } else {
                secondaryUi.deliver_selection(
                    selectionToken,
                    JSON.stringify({error: "document view unavailable"}))
            }
        }

        if (secondaryUi.take_download_request()) {
            if (secondaryWindow.activeView) {
                rootWindow.runBrowserScript(
                    secondaryWindow.activeView,
                    rootWindow.downloadLinkScript(secondaryUi.download_request_url),
                    function() {
                        secondaryUi.complete_download_request(
                            secondaryUi.download_request_token, true)
                    })
            } else {
                secondaryUi.complete_download_request(
                    secondaryUi.download_request_token, false)
            }
        }

        var clipboardRequest = secondaryUi.take_clipboard_request()
        if (clipboardRequest.length > 0) {
            rootWindow.copyToClipboard(
                clipboardRequest,
                !secondaryUi.clipboard_request_sensitive,
                secondaryUi.clipboard_request_primary, secondaryUi)
        }

        secondaryWindow.applySwitcherEngineAction()
        var nextDelay = secondaryUi.maintenance_delay_ms()
        if (nextDelay >= 0) {
            secondaryWindow.scheduleRuntimeWork(nextDelay)
        }
    }

    Timer {
        id: secondaryRuntimeWorkTimer
        interval: 1
        repeat: false
        running: false
        onTriggered: secondaryWindow.processRuntimeWork()
    }

    header: ToolBar {
        height: 0
        visible: false
        RowLayout {
            anchors.fill: parent
            anchors.margins: 8
            spacing: 8

            ToolButton {
                text: "‹"
                Accessible.name: "Back"
                onClicked: {
                    if (secondaryWindow.activeView.contentCanGoBack) {
                        secondaryUi.back()
                        secondaryWindow.executePendingEngineAction()
                    } else {
                        secondaryUi.status_text = "History boundary reached"
                    }
                }
            }
            ToolButton {
                text: "›"
                Accessible.name: "Forward"
                onClicked: {
                    if (secondaryWindow.activeView.contentCanGoForward) {
                        secondaryUi.forward()
                        secondaryWindow.executePendingEngineAction()
                    } else {
                        secondaryUi.status_text = "History boundary reached"
                    }
                }
            }
            FerricTextField {
                id: secondaryAddress
                Layout.fillWidth: true
                property bool addressEditing: false
                function refreshAddressPresentation() {
                    if (!addressEditing) {
                        text = rootWindow.addressPresentation(
                            secondaryUi.display_url,
                            width / Math.max(1, font.pixelSize * 0.56))
                    }
                }
                text: rootWindow.addressPresentation(
                    secondaryUi.display_url,
                    width / Math.max(1, font.pixelSize * 0.56))
                placeholderText: "Enter a URL"
                Accessible.name: "Address"
                Accessible.description: secondaryUi.display_url
                Accessible.role: Accessible.EditableText
                Accessible.editable: true
                selectByMouse: true
                onActiveFocusChanged: {
                    addressEditing = activeFocus
                    if (addressEditing) {
                        text = secondaryUi.display_url
                        cursorPosition = text.length
                    } else {
                        refreshAddressPresentation()
                    }
                }
                onWidthChanged: refreshAddressPresentation()
                Connections {
                    target: secondaryUi
                    function onDisplay_urlChanged() {
                        secondaryAddress.refreshAddressPresentation()
                    }
                }
                onAccepted: {
                    secondaryUi.navigate(text)
                    secondaryWindow.executePendingEngineAction()
                }
            }
            Label {
                text: secondaryWindow.windowEphemeralProfile
                      ? secondaryWindow.windowProfileLabel
                      : (secondaryWindow.windowPrivateProfile
                         ? "private" : secondaryWindow.windowProfileName)
                color: rootWindow.primaryTextColor
                Accessible.name: "Profile"
            }
            ToolButton {
                text: "−"
                Accessible.name: "Zoom out"
                onClicked: {
                    if (secondaryUi.execute_ui_action("browser.tab.zoom", "out")) {
                        secondaryWindow.applySwitcherEngineAction()
                    }
                }
            }
            ToolButton {
                text: "100%"
                Accessible.name: "Reset page zoom"
                onClicked: {
                    if (secondaryUi.execute_ui_action("browser.tab.zoom", "reset")) {
                        secondaryWindow.applySwitcherEngineAction()
                    }
                }
            }
            ToolButton {
                text: "+"
                Accessible.name: "Zoom in"
                onClicked: {
                    if (secondaryUi.execute_ui_action("browser.tab.zoom", "in")) {
                        secondaryWindow.applySwitcherEngineAction()
                    }
                }
            }
        }
    }

    ListModel {
        id: secondaryTabs
        ListElement {
            tabId: ""; url: "about:blank"; title: "New tab"; loaded: true
            pinned: false; muted: false; zoom: 1.0; suspended: false; discarded: false
        }
    }
    property bool tabSwitchingVisible: false
    readonly property string tabPosition: secondaryUi.chrome_tab_position
    readonly property bool sideTabs: tabPosition === "left" || tabPosition === "right"
    readonly property real sideTabWidth: Math.max(160, Math.min(280, width * 0.22))
    readonly property bool tabStripVisible: secondaryUi.chrome_tabs_mode === "always"
        || (secondaryUi.chrome_tabs_mode === "multiple" && secondaryUi.tab_count > 1)
        || (secondaryUi.chrome_tabs_mode === "switching" && tabSwitchingVisible)
    function noteTabActivity() { tabSwitchingVisible = true; secondaryTabSwitchTimer.restart() }
    Timer { id: secondaryTabSwitchTimer; interval: 800; onTriggered: secondaryWindow.tabSwitchingVisible = false }
    FerricTabStrip {
        id: secondaryTabStrip
        x: secondaryWindow.tabPosition === "right" ? parent.width - width : 0
        y: secondaryWindow.tabPosition === "bottom" ? parent.height - secondaryWindow.bottomChromeHeight - height : 0
        width: secondaryWindow.sideTabs ? secondaryWindow.sideTabWidth : parent.width
        height: !visible ? 0 : secondaryWindow.sideTabs
                ? parent.height - secondaryWindow.bottomChromeHeight : secondaryAppearance.tabBarHeight
        z: 9
        vertical: secondaryWindow.sideTabs
        visible: secondaryWindow.tabStripVisible
        browserWindow: secondaryAppearance
        tabsModel: secondaryTabs
        activeTabIndex: secondaryUi.active_tab_index
        onSelectRequested: function(index) {
            secondaryUi.select_tab(index)
            secondaryWindow.syncTabModel()
            secondaryWindow.applySwitcherEngineAction()
        }
        onCloseRequested: function(index) { secondaryWindow.closeTabAtIndex(index) }
        onNewRequested: {
            secondaryUi.execute_command("tab-new")
            secondaryWindow.syncTabModel()
            secondaryWindow.applySwitcherEngineAction()
        }
    }
    StackLayout {
        id: secondaryViews
        anchors.fill: parent
        anchors.topMargin: secondaryWindow.tabStripVisible && secondaryWindow.tabPosition === "top" ? secondaryTabStrip.height : 0
        anchors.leftMargin: secondaryWindow.tabStripVisible && secondaryWindow.tabPosition === "left" ? secondaryTabStrip.width : 0
        anchors.rightMargin: secondaryWindow.tabStripVisible && secondaryWindow.tabPosition === "right" ? secondaryTabStrip.width : 0
        anchors.bottomMargin: secondaryWindow.bottomChromeHeight
            + (secondaryWindow.tabStripVisible && secondaryWindow.tabPosition === "bottom" ? secondaryTabStrip.height : 0)
        currentIndex: secondaryWindow.activeView ? secondaryWindow.activeView.StackLayout.index : -1
    }
    FerricHintWorkspace {
        id: secondaryHintWorkspace
        hostWindow: secondaryWindow
        rootWindow: secondaryWindow.rootWindow
        browserUi: secondaryUi
        activeView: secondaryWindow.activeView
        appearance: secondaryAppearance
        anchors.fill: secondaryViews
    }
    FerricTabViewController {
        id: secondaryTabController
        ready: secondaryWindow.profileBootstrapComplete && !secondaryWindow.runtimeReleased
        browserUi: secondaryUi
        browserHost: secondaryWindow
        tabs: secondaryTabs
        viewFactory: rootWindow.webViewFactory
        viewParent: secondaryViews
        profile: secondaryWindow.windowWebEngineProfile
        interceptor: secondaryWindow.windowRequestInterceptor
        transientProfile: secondaryWindow.windowTransientProfile
        onRestored: secondaryManagers.finishSessionRestore()
        onActivity: secondaryWindow.noteTabActivity()
    }

    Loader {
        id: secondaryDevToolsLoader
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: Math.min(280, parent.height * 0.55)
        active: secondaryWindow.devToolsVisible
        visible: active
        z: 20
        sourceComponent: Component {
            WebEngineView {
                id: secondaryInspector
                property var viewHost: secondaryWindow
                property var viewUi: secondaryUi
                anchors.fill: parent
                profile: secondaryWindow.activeView
                        ? secondaryWindow.activeView.profile
                        : secondaryWindow.windowWebEngineProfile
                inspectedView: secondaryWindow.activeView
                Accessible.name: "Attached developer tools"
                Component.onCompleted: rootWindow.profileResourceRegistry.trackView(secondaryInspector,viewHost,viewUi)
            }
        }
    }

    property int detachedTabIndex: -1
    function detachActiveViewForTransfer() {
        var index = secondaryUi.active_tab_index
        var view = tabViewAt(index)
        if (!view) return null
        secondaryWindow.devToolsVisible = false
        secondaryWindow.detachedTabIndex = index
        secondaryWindow.windowDetachedView = view
        tabViews = tabViews.slice(0, index).concat(tabViews.slice(index + 1))
        view.parent = null
        view.visible = false
        return view
    }
    function restoreDetachedView(view) {
        if (!view) return false
        if (tabViews.indexOf(view) >= 0) return true
        var sourceIndex = secondaryUi.tab_index_for_id(view.stableTabId)
        var index = Math.max(0, Math.min(sourceIndex >= 0 ? sourceIndex : detachedTabIndex, tabViews.length))
        if (tabViewAt(index) && tabViewAt(index).stableTabId === view.stableTabId)
            secondaryWindow.removeTabViewAt(index)
        view.viewUi = secondaryUi
        view.viewHost = secondaryWindow
        view.viewTabs = secondaryTabs
        tabViews = tabViews.slice(0, index).concat([view], tabViews.slice(index))
        view.parent = secondaryViews
        view.anchors.fill = undefined
        view.visible = true
        for (var i = 0; i < tabViews.length; ++i) {
            tabViews[i].tabIndex = i
            tabViews[i].viewModel = secondaryTabs.get(i)
        }
        windowDetachedView = null
        detachedTabIndex = -1
        return true
    }
    function prepareTransferFallback() {
        if (secondaryUi.tab_count !== 1 || windowPreparedTransferFallback) return true
        var fallback = rootWindow.webViewFactory.createObject(secondaryViews, {
            tabIndex: -1, viewUi: secondaryUi, viewHost: secondaryWindow,
            viewTabs: secondaryTabs,
            viewModel: { loaded:true, url:"about:blank", title:"New tab", muted:false,
                         zoom:1.0, scrollX:-1, scrollY:-1 },
            viewProfile: windowWebEngineProfile, viewInterceptor: windowRequestInterceptor,
            viewTransientProfile: windowTransientProfile, viewTransferred:false
        })
        if (!fallback) return false
        fallback.visible = false
        windowPreparedTransferFallback = fallback
        return true
    }
    function discardPreparedTransferFallback() {
        if (windowPreparedTransferFallback) {
            windowPreparedTransferFallback.destroy()
            windowPreparedTransferFallback = null
        }
    }
    function resetAfterTransferSource() {
        if (detachedTabIndex >= 0 && detachedTabIndex < secondaryTabs.count)
            secondaryTabs.remove(detachedTabIndex)
        if (windowPreparedTransferFallback) {
            var fallback = windowPreparedTransferFallback
            windowPreparedTransferFallback = null
            secondaryTabs.append({ tabId:secondaryUi.tab_id_for_index(0), url:"about:blank",
                title:"New tab", loaded:true, pinned:false, muted:false, zoom:1.0,
                suspended:false, discarded:false })
            fallback.stableTabId = secondaryUi.tab_id_for_index(0)
            fallback.tabIndex = 0
            fallback.viewModel = secondaryTabs.get(0)
            fallback.visible = true
            tabViews = tabViews.concat([fallback])
        }
        windowDetachedView = null
        detachedTabIndex = -1
        syncTabModel()
        rootWindow.updateBrowserWindowView(secondaryUi, activeView)
        return true
    }

    function closeTabAtIndex(index) {
        var view = tabViewAt(index)
        if (index < 0 || !view) {
            return false
        }
        rootWindow.clearPageDialogForView(view)
        rootWindow.clearClientCertificateForView(view)
        rootWindow.clearCertificateErrorForView(view)
        rootWindow.clearWebAuthForView(view)
        rootWindow.clearContextMenuForView(view)
        rootWindow.removePermissionGroups(function(group) { return group.ui === secondaryUi && group.tabIndex === index }, true)
        secondaryWindow.clearFileDialogForView(view)
        if (!secondaryUi.close_tab(index)) {
            return false
        }
        secondaryWindow.removeTabViewAt(index)
        secondaryTabs.remove(index)
        if (secondaryUi.tab_count > secondaryTabs.count) {
            secondaryTabs.append({ url: secondaryUi.initial_url, title: "New tab",
                          pinned: false, muted: false, zoom: 1.0,
                          suspended: false, discarded: false })
        }
        secondaryWindow.syncTabModel()
        return true
    }

    function submitInteractiveCommand(text) {
        if (!secondaryUi.execute_secondary_interactive_command(text)) {
            secondaryWindow.pendingInteractiveCommand = secondaryUi.command_retryable
                    ? text : ""
            secondaryCommandBar.showFeedback(secondaryUi.status_text,
                                             !secondaryUi.command_retryable)
            secondaryCommandBar.selectAllInput()
            return
        }
        secondaryWindow.pendingInteractiveCommand = ""
        var commandStatus = secondaryUi.status_text
        secondaryCommandBar.clearInput()
        secondaryWindow.applySwitcherEngineAction()
        commandStatus = secondaryUi.status_text
        if (secondaryUi.mode === "command") {
            secondaryUi.escape()
            secondaryUi.status_text = commandStatus
        }
        secondaryWindow.showCommandNotice(commandStatus, false)
    }

    FerricCommandLine {
        id: secondaryCommandBar
        browserWindow: secondaryAppearance
        commandVisible: secondaryUi.mode === "command"
        completionVisible: secondaryUi.completion_visible
        completionText: secondaryUi.completion_text
        completionValues: secondaryUi.completion_values
        completionStart: secondaryUi.completion_start
        completionEnd: secondaryUi.completion_end
        completionSelected: secondaryUi.completion_selected
        onCompletionUpdateRequested: function(text, cursorPosition) {
            secondaryUi.update_completion(text, cursorPosition)
        }
        onSubmitted: function(text) { secondaryWindow.submitInteractiveCommand(text) }
        onInputEdited: secondaryWindow.pendingInteractiveCommand = ""
        onEscapeRequested: {
            secondaryWindow.pendingInteractiveCommand = ""
            secondaryUi.escape()
        }
        onCompletionMoveRequested: function(delta) { secondaryUi.completion_move(delta) }
        onCompletionSelectRequested: function(index) { secondaryUi.completion_select(index) }
        onHistoryMoveRequested: function(delta, current) {
            secondaryCommandBar.commandText = delta < 0
                ? secondaryUi.command_history_previous(current)
                : secondaryUi.command_history_next(current)
            secondaryCommandBar.cursorPosition = secondaryCommandBar.commandText.length
        }
    }

    Connections {
        target: secondaryUi
        function onStorage_library_revisionChanged() {
            if (secondaryCommandBar.commandVisible) {
                secondaryUi.update_completion(secondaryCommandBar.commandText,
                                              secondaryCommandBar.cursorPosition)
            }
            if (secondaryWindow.pendingInteractiveCommand.length > 0
                    && secondaryCommandBar.commandVisible
                    && secondaryCommandBar.commandText
                       === secondaryWindow.pendingInteractiveCommand) {
                var command = secondaryWindow.pendingInteractiveCommand
                secondaryWindow.pendingInteractiveCommand = ""
                Qt.callLater(function() {
                    if (secondaryCommandBar.commandVisible
                            && secondaryCommandBar.commandText === command) {
                        secondaryWindow.submitInteractiveCommand(command)
                    }
                })
            }
        }
    }

    FerricProfileNotice {
        id: secondaryProfileNotice
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.bottomMargin: secondaryWindow.bottomControlsHeight
        browserWindow: secondaryAppearance
        hostWindow: secondaryWindow
        noticeText: secondaryUi.profile_startup_notice
        onDetailsRequested: rootWindow.showDiagnostics(secondaryUi, secondaryWindow)
    }

    Rectangle {
        id: secondarySearchBar
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: secondaryAppearance.inputBarHeight
        z: 20
        visible: secondaryUi.mode === "search"
        color: secondaryAppearance.surfaceColor

        Label {
            id: secondarySearchPrefix
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            width: secondaryAppearance.inputBarHeight
            text: secondaryUi.search_backward ? "?" : "/"
            color: rootWindow.warningColor
            font.bold: true
            horizontalAlignment: Text.AlignHCenter
            Accessible.ignored: true
        }

        FerricTextField {
            id: secondarySearchLine
            anchors.left: secondarySearchPrefix.right
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            leftPadding: 8
            rightPadding: 8
            topPadding: 0
            bottomPadding: 0
            text: secondaryUi.search_text
            color: secondaryAppearance.primaryTextColor
            selectionColor: secondaryAppearance.selectionColor
            selectedTextColor: secondaryAppearance.selectionTextColor
            placeholderText: "search"
            placeholderTextColor: secondaryAppearance.mutedTextColor
            background: Rectangle { color: "transparent" }
            focus: secondarySearchBar.visible
            Accessible.name: "Search"
            Accessible.role: Accessible.EditableText
            Accessible.editable: true
            onVisibleChanged: if (visible) forceActiveFocus()
            onTextChanged: secondaryUi.search_changed(text)
            onAccepted: {
                secondaryUi.search_next(secondaryUi.search_backward)
                secondaryUi.accept_search()
                secondaryWindow.applySwitcherEngineAction()
            }
            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    secondaryUi.escape()
                    event.accepted = true
                }
            }
        }
    }

    FerricWindowStatusBar {
        browserWindow: secondaryAppearance
        statusVisible: secondaryWindow.statusBarVisible
        mode: secondaryUi.mode
        displayUrl: secondaryUi.display_url
        hintPreviewText: secondaryUi.mode === "hint" && !secondaryUi.target_search_active ? secondaryHintWorkspace.previewText : ""
        statusText: secondaryWindow.commandNoticeVisible
                    ? secondaryWindow.commandNoticeText : secondaryUi.status_text
        profileLabel: secondaryWindow.windowEphemeralProfile ? "EPHEMERAL"
                      : secondaryWindow.windowPrivateProfile ? "PRIVATE"
                      : secondaryWindow.windowProfileName
        statusColor: secondaryWindow.commandNoticeVisible
                     && secondaryWindow.commandNoticeError
                     ? rootWindow.errorColor
                     : rootWindow.contextStatusColor(secondaryUi,
                                                     rootWindow.mutedTextColor)
        profileColor: rootWindow.contextStatusColor(secondaryUi, rootWindow.secondaryTextColor)
        accessibleDetails: rootWindow.statusDetails(
                               secondaryUi, secondaryWindow,
                               secondaryWindow.activeView,
                               secondaryWindow.windowPrivateProfile,
                               secondaryWindow.windowProfileName,
                               secondaryWindow.windowEphemeralProfile)
    }

    function removeTabViewAt(index) {
        var view = tabViewAt(index)
        if (!view) return
        tabViews = tabViews.slice(0, index).concat(tabViews.slice(index + 1))
        view.viewRetired = true
        view.viewModel = ({ loaded:false, url:"", muted:false, zoom:1.0 })
        view.visible = false
        view.enabled = false
        view.parent = null
        Qt.callLater(function() { if (view) view.destroy() })
    }

    function attachTransferredView(view, payload, sourceUi, sourceHost) {
        if (!view || typeof payload !== "string" || payload.length === 0) {
            return false
        }
        var sourceTabId = view.stableTabId
        var sourceUrl = view.url ? view.url.toString() : "about:blank"
        var contentHistory = sourceUi.content_history_export(view.tabIndex >= 0 ? view.tabIndex : sourceUi.active_tab_index)
        var transferData = null
        try {
            transferData = JSON.parse(payload)
        } catch (error) {
            transferData = null
        }
        if (!transferData || typeof transferData.tab_id !== "string"
                || !secondaryUi.adopt_tab_transfer(payload)) {
            return false
        }
        var adoptedTabId = secondaryUi.tab_id_for_index(secondaryUi.active_tab_index)
        secondaryWindow.syncTabModel()
        var index = secondaryUi.active_tab_index
        if (index < 0 || index >= secondaryTabs.count) {
            secondaryUi.rollback_tab_transfer(adoptedTabId)
            secondaryWindow.syncTabModel()
            return false
        }
        secondaryTabs.setProperty(index, "url", transferData.url || "about:blank")
        secondaryTabs.setProperty(index, "title", transferData.title || "New tab")
        secondaryTabs.setProperty(index, "loaded", true)
        secondaryTabs.setProperty(index, "muted", !!transferData.muted)
        secondaryTabs.setProperty(index, "zoom", Number(transferData.zoom_hundredths || 100) / 100.0)
        var placeholder = secondaryWindow.tabViewAt(index)
        if (!placeholder) {
            secondaryUi.rollback_tab_transfer(adoptedTabId)
            secondaryWindow.syncTabModel()
            return false
        }
        secondaryWindow.removeTabViewAt(index)
        if (contentHistory.length > 0) secondaryUi.content_history_import(secondaryUi.active_tab_index, contentHistory)
        view.viewUi = secondaryUi
        view.viewTabs = secondaryTabs
        view.viewModel = secondaryTabs.get(index)
        view.viewTransferred = false
        view.stableTabId = adoptedTabId
        if (view.viewHost !== undefined) {
            view.viewHost = secondaryWindow
        }
        if (view.viewIsSecondaryStatic === true) {
            view.url = sourceUrl
        }
        view.tabIndex = index
        view.anchors.fill = undefined
        view.parent = secondaryViews
        view.visible = true
        tabViews = tabViews.slice(0, index).concat([view], tabViews.slice(index))
        for (var reindex = index; reindex < tabViews.length; ++reindex) {
            tabViews[reindex].tabIndex = reindex
            tabViews[reindex].viewModel = secondaryTabs.get(reindex)
        }
        rootWindow.updateBrowserWindowView(secondaryUi, view)
        if (!rootWindow.completeDetachedSource(sourceUi, sourceHost, payload)) {
            tabViews = tabViews.slice(0, index).concat(tabViews.slice(index + 1))
            view.parent = null
            view.visible = false
            view.stableTabId = sourceTabId
            secondaryUi.rollback_tab_transfer(adoptedTabId)
            secondaryWindow.syncTabModel()
            return false
        }
        if (arguments.length >= 5 && typeof arguments[4] === "string"
                && arguments[4].length > 0 && sourceUi) {
            sourceUi.complete_transfer_operation(arguments[4], true)
        }
        view.forceActiveFocus()
        secondaryUi.status_text = "Live tab attached without navigation"
        return true
    }

    Timer {
        id: bookmarkTransferTimer
        interval: 50
        repeat: true
        onTriggered: {
            if (secondaryUi.profile_bootstrap_pending || !secondaryWindow.profileBootstrapComplete) {
                return
            }
            stop()
            var queued = secondaryUi.queue_bookmark_transfer(
                        secondaryWindow.windowBookmarkTransferUrl,
                        secondaryWindow.windowBookmarkTransferTitle)
            secondaryWindow.bookmarkTransferStarted(secondaryUi, queued)
            if (!queued) {
                secondaryUi.status_text = "Bookmark transfer failed"
            }
            secondaryWindow.windowBookmarkTransferUrl = ""
            secondaryWindow.windowBookmarkTransferTitle = ""
        }
    }

    Component.onCompleted: {
        windowProfileResources=rootWindow.profileResourceRegistry.ensure(
                    windowProfileName,windowTransientProfile,windowStorageBasePath,
                    windowSharedProfile,windowSharedRequestInterceptor)
        if (!rootWindow.profileResourceRegistry.retainWindow(windowProfileResources,secondaryWindow,secondaryUi)) {
            secondaryUi.status_text="This profile couldn't be opened. Try reopening the window."
            return
        }
        secondaryWindow.scheduleRuntimeWork(0)
        secondaryWindow.permissionPromptSurface =
                rootWindow.permissionPromptFactory.createObject(
                    secondaryWindow.contentItem, { hostWindow: secondaryWindow })
        secondaryWindow.desktopMediaSurface =
                rootWindow.desktopMediaFactory.createObject(
                    secondaryWindow.contentItem, { hostWindow: secondaryWindow })
        secondaryWindow.captureIndicatorSurface =
                rootWindow.captureIndicatorFactory.createObject(
                    secondaryWindow.contentItem, { hostWindow: secondaryWindow })
        secondaryWindow.rendererFailureSurface =
                rootWindow.rendererFailureFactory.createObject(
                    secondaryWindow.contentItem, { hostWindow: secondaryWindow })
        if (secondaryWindow.windowPopup) {
            if (!secondaryWindow.windowPopupRequest
                    || !popupOwnerBridge.initialize(secondaryWindow.windowPopupOpenerUi,
                                                    secondaryUi,secondaryWindow.windowPopupToken)) {
                secondaryWindow.rejectPopupAdmission()
                return
            }
            secondaryWindow.windowPopupOpenerUi=null
            secondaryWindow.completeProfileBootstrap()
            return
        }
        if (secondaryWindow.windowInheritedUi) {
            var inherited = popupOwnerBridge.inheritProfile(windowInheritedUi, secondaryUi, windowProfileName)
            windowInheritedUi = null
            if (!inherited) {
                secondaryWindow.rejectPopupAdmission()
                return
            }
            if (secondaryUi.profile_bootstrap_pending) secondaryProfileBootstrapTimer.start()
            else secondaryWindow.completeProfileBootstrap()
            return
        }
        secondaryWindow.syncTabModel()
        rootWindow.installFocusObserver(secondaryWindow.activeView)
        secondaryUi.set_startup_configuration(
                    rootWindow.primaryBrowserUi.config_json, rootWindow.primaryBrowserUi.config_base_json,
                    rootWindow.primaryBrowserUi.cli_overrides_json,
                    secondaryUi.profile_overrides_json,
                    rootWindow.primaryBrowserUi.config_path, rootWindow.primaryBrowserUi.config_source)
        secondaryUi.set_contexts_configuration(rootWindow.primaryBrowserUi.contexts_json)
        secondaryUi.configure_profile(
                                      secondaryWindow.windowPrivateProfile,
                                      secondaryWindow.windowEphemeralProfile,
                                      secondaryWindow.windowProfileLabel,
                                      secondaryWindow.windowProfileName,
                                      secondaryWindow.windowStorageBasePath)
        if (secondaryWindow.windowBookmarkTransferUrl.length > 0) {
            bookmarkTransferTimer.start()
        }
        secondaryUi.set_context_entry_reuse(secondaryWindow.windowStartupContextRestore)
        rootWindow.registerBrowserWindow(
                    secondaryWindow, secondaryUi, secondaryWindow.activeView,
                    secondaryWindow.windowWebEngineProfile,
                    secondaryWindow.windowProfileName,
                    secondaryWindow.windowEphemeralInvocationToken,
                    secondaryWindow.windowEphemeralProfile)
        secondaryProfileBootstrapTimer.start()
    }

    function rejectPopupAdmission() {
        profileBootstrapComplete=false
        Qt.callLater(function() {
            if (secondaryWindow.runtimeReleased) return
            secondaryWindow.windowShutdownApproved=true
            secondaryWindow.close()
            secondaryWindow.destroy()
        })
    }

    function registerWindowOwner() {
        if (windowPopup)
            return rootWindow.registerPopupWindow(secondaryWindow,secondaryUi,activeView)
        return rootWindow.registerBrowserWindow(
                    secondaryWindow, secondaryUi, activeView,windowWebEngineProfile,
                    windowProfileName,windowEphemeralInvocationToken,windowEphemeralProfile)
    }

    function clearPendingTransfer() {
        windowTransferView = null
        windowTransferSourceUi = null
        windowTransferSourceHost = null
        windowTransferOperationId = ""
        windowTransferPayload = ""
    }

    function cancelPendingTransfer() {
        if (windowTransferView && !rootWindow.runtimeDisposing) {
            rootWindow.restoreSourceView(windowTransferSourceUi,
                                         windowTransferSourceHost, windowTransferView)
            if (windowTransferSourceUi && windowTransferOperationId.length > 0)
                windowTransferSourceUi.complete_transfer_operation(windowTransferOperationId, false)
        }
        clearPendingTransfer()
    }

    Timer {
        id: secondaryProfileBootstrapTimer
        interval: 25
        repeat: true
        onTriggered: {
            if (secondaryUi.profile_bootstrap_pending) return
            stop()
            secondaryWindow.completeProfileBootstrap()
        }
    }

    function completeProfileBootstrap() {
        if (runtimeReleased || profileBootstrapComplete || !secondaryUi.profile_available()
                || (!windowPopup && secondaryUi.profile_bootstrap_pending)) return
        rootWindow.profileResourceRegistry.activateWindow(secondaryWindow)
        profileBootstrapComplete = true
        secondaryWindow.syncTabModel()
        if (!secondaryWindow.registerWindowOwner()) {
            if (windowPopup) rejectPopupAdmission()
            return
        }
        if (windowPopup) {
            // The native request exists only during its signal delivery.
            // The shared controller has created a blank view with the exact
            // inherited profile; accept before returning to WebEngine.
            var request=windowPopupRequest
            var view=activeView
            if (!request || !view || view.profile!==windowSharedProfile) {
                rejectPopupAdmission()
                return
            }
            request.openIn(view)
            windowPopupRequest=null
            return
        }
        if (secondaryWindow.windowTransferMode) {
            var bootstrapTabId = secondaryUi.tab_count === 1
                    ? secondaryUi.tab_id_for_index(0) : ""
            if (!secondaryWindow.attachTransferredView(
                        secondaryWindow.windowTransferView,
                        secondaryWindow.windowTransferPayload,
                        secondaryWindow.windowTransferSourceUi,
                        secondaryWindow.windowTransferSourceHost,
                        secondaryWindow.windowTransferOperationId)) {
                secondaryUi.status_text = "Live tab adoption failed"
                secondaryWindow.cancelPendingTransfer()
                return
            }
            secondaryWindow.clearPendingTransfer()
            var bootstrapTabIndex = secondaryUi.tab_index_for_id(bootstrapTabId)
            // The reducer may reuse an idle blank tab for the transfer.
            // Only close a bootstrap tab that remains separate from the live page.
            if (bootstrapTabIndex >= 0
                    && bootstrapTabId !== secondaryUi.tab_id_for_index(secondaryUi.active_tab_index))
                secondaryWindow.closeTabAtIndex(bootstrapTabIndex)
            return
        }
        if (secondaryWindow.windowStartupContext.length > 0) {
            if (windowStartupContextRestore) {
                secondaryUi.set_context_entry_reuse(true)
                secondaryUi.execute_command("context-enter " + windowStartupContext)
            } else if (!secondaryUi.assign_window_context(windowStartupContext)) {
                secondaryWindow.rejectPopupAdmission()
                return
            }
            rootWindow.routeContextWorkspace(secondaryUi)
        }
        if (secondaryWindow.windowStartupContextRestore) {
            secondaryWindow.applySwitcherEngineAction()
        } else if (secondaryWindow.windowStartupRoutePreflighted) {
            secondaryUi.navigate_without_context_route(
                        secondaryWindow.windowStartupUrl)
        } else {
            secondaryUi.navigate_initial(
                        secondaryWindow.windowStartupUrl, "external-open", false)
        }
        secondaryWindow.applySwitcherEngineAction()
    }
    Connections {
        target: secondaryUi
        function onActive_tab_indexChanged() {
            if (rootWindow.copyNoticeHost === secondaryWindow) rootWindow.copyNoticeVisible = false
            secondaryWindow.noteTabActivity(); secondaryWindow.scheduleRuntimeWork(0)
        }
        function onTab_countChanged() { secondaryWindow.scheduleRuntimeWork(0) }
    }

    readonly property alias profileSessionWorkspace: secondaryManagers
    FerricProfileSessionWorkspace {
        id: secondaryManagers
        objectName: "secondaryProfileSessionWorkspace"
        browserWindow: secondaryAppearance
        hostWindow: secondaryWindow
        browserUi: secondaryUi
        transientProfile: secondaryWindow.windowTransientProfile
        onOpening: rootWindow.captureOverlayFocus(secondaryWindow, secondaryWindow.activeView)
        onClosed: rootWindow.restoreOverlayFocus(secondaryWindow)
    }
    FerricSelectorValidationSurface {
        browserWindow: rootWindow
        browserUi: secondaryUi
    }

    function releaseWindowResources() {
        if (secondaryWindow.runtimeReleased) return
        secondaryWindow.runtimeReleased = true
        secondaryWindow.devToolsVisible = false
        rootWindow.discardOverlayFocus(secondaryWindow)
        if (rootWindow.contextRouteUi === secondaryUi) rootWindow.closeContextRoutePrompt()
        secondaryProfileBootstrapTimer.stop()
        secondaryWindow.cancelPendingTransfer()
        secondaryRuntimeWorkTimer.stop()
        secondaryFilePortalTimer.stop()
        bookmarkTransferTimer.stop()
        secondaryShutdownPageProbeTimer.stop()
        secondaryCommandNoticeTimer.stop()
        secondaryTabSwitchTimer.stop()
        var views = tabViews.slice(0)
        for (var index = 0; index < views.length; ++index) {
            var view = views[index]
            if (!view || views.indexOf(view) !== index) continue
            rootWindow.cancelScriptsForView(view)
            rootWindow.clearPageDialogForView(view)
            rootWindow.clearClientCertificateForView(view)
            rootWindow.clearCertificateErrorForView(view)
            rootWindow.clearWebAuthForView(view)
            rootWindow.clearContextMenuForView(view)
            rootWindow.clearDesktopMediaForView(view)
            rootWindow.clearCaptureSessionForView(view)
            rootWindow.clearRendererFailureForView(view)
            rootWindow.clearSiteDataClearForView(view)
            secondaryWindow.clearFileDialogForView(view)
        }
        rootWindow.removePermissionGroups(function(group) { return group.host === secondaryWindow }, true)
        if (secondaryWindow.devToolsWindow) {
            secondaryWindow.devToolsWindow.close()
            secondaryWindow.devToolsWindow = null
        }
        if (windowPopup) rootWindow.unregisterPopupWindow(secondaryWindow)
        else rootWindow.unregisterBrowserWindow(secondaryUi)
        rootWindow.clearCaptureSessionsForHost(secondaryWindow)
        if (rootWindow.pendingDesktopMediaHost === secondaryWindow) {
            rootWindow.clearDesktopMediaRequest(true)
        }
        if (secondaryWindow.permissionPromptSurface) {
            secondaryWindow.permissionPromptSurface.destroy()
            secondaryWindow.permissionPromptSurface = null
        }
        if (secondaryWindow.desktopMediaSurface) {
            secondaryWindow.desktopMediaSurface.destroy()
            secondaryWindow.desktopMediaSurface = null
        }
        if (secondaryWindow.captureIndicatorSurface) {
            secondaryWindow.captureIndicatorSurface.destroy()
            secondaryWindow.captureIndicatorSurface = null
        }
        if (secondaryWindow.rendererFailureSurface) {
            secondaryWindow.rendererFailureSurface.destroy()
            secondaryWindow.rendererFailureSurface = null
        }
        rootWindow.profileResourceRegistry.releaseWindow(secondaryWindow)
        secondaryUi.view_closed()
        secondaryUi.release_transient_resources()
    }
    Component.onDestruction: {
        if (!secondaryWindow.runtimeReleased && !rootWindow.runtimeDisposing) releaseWindowResources()
    }

}

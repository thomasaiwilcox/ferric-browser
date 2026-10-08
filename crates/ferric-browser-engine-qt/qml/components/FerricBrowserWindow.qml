import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import QtQml
import QtWebEngine
import io.github.ferricbrowser 1.0
import "../scripts/ShutdownPageProbeState.js" as ShutdownPageProbeState
import "../scripts/VersionedPageState.js" as VersionedPageState
import "../scripts/ProfileDownloadRouting.js" as ProfileDownloadRouting
import "../scripts/PopupWindowOwnership.js" as PopupWindowOwnership
import "../scripts/CleanBranchPopupOwnership.js" as CleanBranchPopupOwnership

ApplicationWindow {
    required property var rootWindow
    id: secondaryWindow
    width: 1180
    height: 760
    visible: !secondaryWindow.windowCleanBranch
    title: secondaryWindow.windowCleanBranch
           ? "Ferric Browser · Clean branch (parent active)"
           : secondaryWindow.windowEphemeralProfile
           ? "Ferric Browser · " + secondaryWindow.windowProfileLabel
           : "Ferric Browser · " + secondaryWindow.windowProfileName
    color: rootWindow.backgroundColor
    font: rootWindow.font
    palette: rootWindow.palette
    property string windowStartupUrl: "about:blank"
    property string windowRestoreCommitId: ""
    property bool windowRestoreMediaPositionEnabled: false
    property bool windowRestoreEditableDraftsEnabled: false
    property bool windowRestoreEditableReviewEnabled: false
    property string windowBookmarkTransferUrl: ""
    property string windowBookmarkTransferTitle: ""
    property string windowProfileName: "secondary"
    property string windowProfileOverridesJson: secondaryUi.profile_overrides_json
    property string windowStartupContext: ""
    property bool windowStartupContextRestore: false
    property bool windowStartupRoutePreflighted: true
    property string windowProfileLabel: "Secondary"
    property string windowStorageBasePath: rootWindow.storageBasePath
    property bool windowPrivateProfile: false
    property bool windowEphemeralProfile: false
    property string windowEphemeralInvocationToken: ""
    // Set only by the trusted clean-branch transaction. The branch retains
    // its own off-record profile and never reassigns the source view/profile.
    property string windowBranchToken: ""
    property string windowBoundBranchToken: ""
    property bool windowBranchTokenLocked: false
    readonly property bool windowCleanBranch: windowBranchTokenLocked
            ? windowBoundBranchToken.length > 0 : windowBranchToken.length > 0
    property bool windowCleanBranchPublished: false
    property bool windowCleanBranchNativeBound: false
    property bool windowCleanBranchNavigationAdmitted: false
    property bool windowCleanBranchLineagePublished: false
    property var windowBranchParentUi: null
    readonly property alias windowBranchUi: secondaryUi
    readonly property alias windowBranchProfile: secondaryProfile
    readonly property alias windowBranchInitialView: secondaryView
    // Additional native page views may be registered only by the future
    // window-branch host. The first-view lease remains the registry anchor.
    property var windowBranchGroupViews: []
    onWindowBranchTokenChanged: {
        if (windowBranchTokenLocked
                && windowBranchToken !== windowBoundBranchToken) {
            secondaryWindow.visible = false
            Qt.callLater(function() { secondaryWindow.destroy() })
        }
    }
    property var windowSharedProfile: null
    property var windowSharedRequestInterceptor: null
    property var windowTransferView: null
    property var windowTransferSourceUi: null
    property var windowTransferSourceHost: null
    property string windowTransferOperationId: ""
    property string windowTransferPayload: ""
    property var windowFallbackView: null
    property var windowPreparedTransferFallback: null
    property var windowDetachedView: null
    property var devToolsWindow: null
    property bool devToolsVisible: false
    property var devToolsAttachedView: secondaryDevToolsLoader.item
    property bool windowDetachedWasTransfer: false
    readonly property bool windowTransferMode: windowTransferView !== null
    readonly property bool inputBarActive: secondaryUi.mode === "command"
                                           || secondaryUi.mode === "search"
    property bool commandNoticeVisible: false
    property string commandNoticeText: ""
    property bool commandNoticeError: false
    property string pendingInteractiveCommand: ""
    property string branchExportOperation: ""
    property string branchExportDestination: ""
    property int branchExportPageCount: 0
    readonly property bool statusBarVisible:
        (secondaryWindow.commandNoticeVisible && !secondaryWindow.inputBarActive)
        || rootWindow.statusBarVisibleForMode(secondaryUi.mode)
    readonly property real bottomChromeHeight:
        inputBarActive ? rootWindow.inputBarHeight
                       : (statusBarVisible ? rootWindow.statusBarHeight : 0)
    readonly property var activeView: windowTransferMode
                                               ? windowTransferView
                                               : (windowFallbackView || secondaryView)
    readonly property bool browserKeyFocusActive:
        !rootWindow.browserChromeInputActive
        && !windowCloseConfirmationVisible
        && !windowShutdownPromptVisible
        && !windowShutdownPagePromptVisible
        && !windowShutdownStoragePromptVisible
        && !versioningEditableRestoreReview.visible
        && !secondaryUi.external_navigation_visible
    readonly property bool windowTransientProfile:
        windowPrivateProfile || windowEphemeralProfile || windowCleanBranch
    readonly property var windowWebEngineProfile:
        windowSharedProfile || secondaryProfile
    readonly property var windowRequestInterceptor:
        windowSharedProfile
        ? (windowSharedRequestInterceptor || rootWindow.primaryRequestInterceptor)
        : secondaryRequestInterceptor

    // A prepared branch is neither visible nor discoverable until VB-059 has
    // committed its lineage and child navigation policy. No fallback to a
    // normal-profile window is allowed if the exact lease disappears.
    function publishCleanBranchWindow() {
        if (!secondaryWindow.windowCleanBranch
                || secondaryWindow.windowCleanBranchPublished
                || !secondaryWindow.windowCleanBranchNativeBound
                || !secondaryWindow.windowCleanBranchLineagePublished
                || secondaryUi.profile_bootstrap_pending)
            return false
        var owner = rootWindow.cleanBranchOwner(secondaryWindow.windowBoundBranchToken)
        if (!owner || owner.host !== secondaryWindow
                || owner.profile !== secondaryProfile || owner.view !== secondaryView)
            return false
        if (!rootWindow.registerBrowserWindow(
                    secondaryWindow, secondaryUi, secondaryView,
                    secondaryProfile, secondaryWindow.windowProfileName,
                    secondaryWindow.windowBoundBranchToken, true))
            return false
        secondaryWindow.windowCleanBranchPublished = true
        secondaryWindow.show()
        return true
    }
    property var permissionPromptSurface: null
    property var desktopMediaSurface: null
    property var captureIndicatorSurface: null
    property var rendererFailureSurface: null
    property var pendingFileDialogRequest: null
    property var pendingFileDialogView: null
    property bool pendingFileDialogWaitingForPortal: false
    property double pendingFileDialogPortalDeadlineMs: 0
    property bool windowShutdownApproved: false
    property bool windowDestroyQueued: false
    property bool windowCloseConfirmationVisible: false
    property bool windowShutdownPromptVisible: false
    property bool windowShutdownWaitingForDownloadFinishes: false
    property bool windowShutdownPagePromptVisible: false
    property bool windowShutdownStoragePromptVisible: false
    property string windowShutdownPagePromptReason: ""
    property var windowShutdownPagePromptSnapshot: null
    property int windowShutdownPageProbeGeneration: 0
    property var activeDownloads: ({})
    property var pendingDownloadRequests: ({})
    property var downloadCompletionRequests: ({})
    property string pendingDownloadId: ""
    property bool caretSelecting: false
    property string versioningRestoreSidecarRequestId: ""
    property string versioningRestoreSidecarTabId: ""
    property int versioningRestoreSidecarAttempts: 0
    property int versioningRestoreSidecarGeneration: 0
    property bool versioningRestoreSidecarCaptureRunning: false
    property bool versioningOpenRestoreMediaPositionsRequested: false
    property bool versioningOpenRestoreEditableDraftsRequested: false
    property string versioningOpenRestoreCommitId: ""
    property double versioningOpenRestoreOptionsAt: 0
    property string versioningEditablePreviewRequestId: ""
    property string versioningEditablePreviewTabId: ""
    property var versioningEditablePreviewItems: []
    property int versioningEditablePreviewOmitted: 0

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
        targetWindow: secondaryWindow
        enabled: secondaryWindow.active
                 && secondaryWindow.browserKeyFocusActive
                 && (secondaryUi.mode === "normal"
                     || secondaryUi.mode === "hint"
                     || secondaryUi.mode === "grid"
                     || secondaryUi.mode === "caret"
                     || secondaryUi.mode === "insert"
                     || secondaryUi.mode === "pass-through")
        onKeyPressed: function(text, key, modifiers, isAutoRepeat) {
            var event = {
                text: text,
                key: key,
                modifiers: modifiers,
                isAutoRepeat: !!isAutoRepeat,
                accepted: false
            }
            if (rootWindow.handleBrowserKey(
                        secondaryUi, secondaryWindow, event)) {
                secondaryKeyRouter.acceptCurrentEvent()
            }
        }
    }

    Timer {
        id: secondaryShutdownPageProbeTimer
        interval: 2500
        repeat: false
        onTriggered: {
            secondaryWindow.windowShutdownPageProbeGeneration += 1
            secondaryWindow.showShutdownPagePrompt("Page state check timed out.")
            secondaryUi.status_text =
                    "Page state check timed out; choose Close anyway"
        }
    }

    Timer {
        id: versioningRestoreReadinessTimer
        interval: 200
        repeat: true
        onTriggered: secondaryWindow.advanceVersioningRestoreGeometry()
    }

    Timer {
        id: versioningEditablePreviewTimeout
        interval: 60000
        repeat: false
        onTriggered: secondaryWindow.finishVersioningEditableReview(false)
    }

    onClosing: function(close) {
        if (secondaryWindow.windowShutdownApproved) {
            if (secondaryWindow.versioningRestoreSidecarRequestId.length > 0) {
                secondaryWindow.clearVersioningRestoreGeometry(
                            secondaryWindow.versioningRestoreSidecarRequestId, true)
            }
            var previewRequestId = secondaryWindow.versioningEditablePreviewRequestId
            secondaryWindow.clearVersioningEditablePreview()
            if (previewRequestId.length > 0) {
                secondaryUi.discard_versioned_restore_sidecar(previewRequestId)
            }
            return
        }
        close.accepted = false
        secondaryWindow.beginQuitRequest()
    }

    onVisibleChanged: {
        if (!visible && windowShutdownApproved)
            requestApprovedWindowRelease()
    }

    function requestApprovedWindowRelease() {
        if (secondaryWindow.visible || !secondaryWindow.windowShutdownApproved
                || secondaryWindow.windowDestroyQueued)
            return false
        secondaryWindow.windowDestroyQueued = true
        Qt.callLater(function() {
            if (secondaryWindow.visible || !secondaryWindow.windowShutdownApproved) {
                secondaryWindow.windowDestroyQueued = false
                return
            }
            if (secondaryWindow.hasActiveDownloads()) {
                // Keep the exact profile owner alive until every retained
                // request receives its terminal callback.
                secondaryWindow.windowDestroyQueued = false
                return
            }
            if (secondaryWindow.hasOwnedPopupWindows()) {
                // Popup callbacks and UI operations remain owned by this
                // restore window until each popup has completed destruction.
                secondaryWindow.windowDestroyQueued = false
                return
            }
            var nestedOwner = secondaryWindow.versioningWindowOwner
            var parentOwner = nestedOwner ? nestedOwner.parentOwner : null
            if (parentOwner && typeof parentOwner.releaseClosed === "function") {
                if (!parentOwner.releaseClosed(secondaryWindow))
                    secondaryWindow.windowDestroyQueued = false
            } else {
                secondaryWindow.destroy()
            }
        })
        return true
    }

    function hasActiveDownloads() {
        if (secondaryWindow.pendingDownloadId.length > 0) {
            return true
        }
        return ProfileDownloadRouting.hasOutstandingDownloadRequests(
                    secondaryWindow.pendingDownloadRequests)
                || ProfileDownloadRouting.hasOutstandingDownloadRequests(
                    secondaryWindow.activeDownloads)
                || ProfileDownloadRouting.hasOutstandingDownloadRequests(
                    secondaryWindow.downloadCompletionRequests)
    }

    function hasOwnedPopupWindows() {
        return PopupWindowOwnership.hasOwnedPopupWindows(
                    rootWindow, secondaryWindow, secondaryUi)
    }

    function downloadOwnerFor(download, inspectView) {
        if (!download)
            return null
        // This secondary surface has no versioned page-capture path. Its own
        // profile signal can therefore be managed at profile scope without
        // reading Qt Quick's potentially viewless getter.
        try {
            var popupHosts = PopupWindowOwnership.snapshotPopupHosts(rootWindow)
            if (popupHosts === null)
                return undefined
            var exactOwner = ProfileDownloadRouting.requestOwnerForExactDownload(
                        download, secondaryWindow, popupHosts, secondaryProfile)
            if (exactOwner === undefined)
                return undefined
            if (exactOwner)
                return exactOwner
            if (inspectView !== false)
                return ProfileDownloadRouting.boundProfileOwner(
                            download, secondaryWindow, secondaryProfile,
                            download.isSavePageDownload === true)
            return null
        } catch (error) {
            return undefined
        }
    }

    // A shared-profile secondary receives its profile-wide signals through
    // the owning entry surface; a private profile still calls the same path.
    function handleDownloadRequested(download) {
        secondaryProfile.handleForwardedDownloadRequested(download)
    }

    function handleDownloadFinished(download) {
        secondaryProfile.handleForwardedDownloadFinished(download)
    }

    function showShutdownPagePrompt(reason) {
        secondaryShutdownPageProbeTimer.stop()
        secondaryWindow.windowShutdownPagePromptSnapshot =
                ShutdownPageProbeState.snapshotActive(
                    [secondaryWindow.activeView],
                    WebEngineView.LifecycleState.Active, true)
        secondaryWindow.windowShutdownPagePromptReason = reason
        secondaryWindow.windowShutdownPagePromptVisible = true
    }

    function confirmShutdownPagePrompt() {
        secondaryWindow.windowShutdownPagePromptVisible = false
        if (!ShutdownPageProbeState.stillCurrent(
                secondaryWindow.windowShutdownPagePromptSnapshot,
                [secondaryWindow.activeView], WebEngineView.LifecycleState.Active, true)) {
            secondaryWindow.windowShutdownPagePromptSnapshot = null
            secondaryWindow.checkPageStateBeforeQuit()
            return
        }
        secondaryWindow.windowShutdownPagePromptSnapshot = null
        secondaryWindow.finalizeQuit()
    }

    function checkPageStateBeforeQuit() {
        var generation = ++secondaryWindow.windowShutdownPageProbeGeneration
        secondaryWindow.windowShutdownPagePromptSnapshot = null
        var view = secondaryWindow.activeView
        if (!view || view.lifecycleState !== WebEngineView.LifecycleState.Active) {
            secondaryWindow.finalizeQuit()
            return
        }
        var snapshot = ShutdownPageProbeState.snapshotActive(
            [view], WebEngineView.LifecycleState.Active)
        if (!snapshot) {
            secondaryWindow.showShutdownPagePrompt(
                "A page or browser activity changed during the close check.")
            return
        }
        secondaryShutdownPageProbeTimer.restart()
        view.runJavaScript(rootWindow.shutdownPageProbeScript(), function(result) {
            if (generation !== secondaryWindow.windowShutdownPageProbeGeneration) {
                return
            }
            if (!ShutdownPageProbeState.stillCurrent(
                    snapshot, [secondaryWindow.activeView],
                    WebEngineView.LifecycleState.Active)) {
                secondaryWindow.showShutdownPagePrompt(
                    "A page or browser activity changed during the close check.")
                return
            }
            secondaryShutdownPageProbeTimer.stop()
            if (result === "clean") {
                secondaryWindow.finalizeQuit()
                return
            }
            secondaryWindow.showShutdownPagePrompt(result === "dirty"
                    ? "This page has unsaved form or editor state."
                    : "Ferric Browser could not verify this page before closing.")
        })
    }

    function cancelPageStateProbe() {
        secondaryWindow.windowShutdownPageProbeGeneration += 1
        secondaryShutdownPageProbeTimer.stop()
        secondaryWindow.windowShutdownPagePromptVisible = false
        secondaryWindow.windowShutdownPagePromptSnapshot = null
    }

    function clearVersioningRestoreGeometry(requestId, discard) {
        if (secondaryWindow.versioningRestoreSidecarRequestId !== requestId) {
            return false
        }
        versioningRestoreReadinessTimer.stop()
        secondaryWindow.versioningRestoreSidecarRequestId = ""
        secondaryWindow.versioningRestoreSidecarTabId = ""
        secondaryWindow.versioningRestoreSidecarAttempts = 0
        secondaryWindow.versioningRestoreSidecarCaptureRunning = false
        secondaryWindow.versioningRestoreSidecarGeneration += 1
        if (discard) {
            secondaryUi.discard_versioned_restore_sidecar(requestId)
        }
        return true
    }

    function queueVersioningRestoreGeometry(requestId, tabId) {
        if (!/^\d{1,20}$/.test(requestId) || requestId.length === 0
                || typeof tabId !== "string" || tabId.length > 64) {
            secondaryUi.status_text = "Geometry restore request was invalid"
            return false
        }
        if (secondaryWindow.versioningRestoreSidecarRequestId.length > 0) {
            secondaryWindow.clearVersioningRestoreGeometry(
                secondaryWindow.versioningRestoreSidecarRequestId, true)
        }
        secondaryWindow.versioningRestoreSidecarRequestId = requestId
        secondaryWindow.versioningRestoreSidecarTabId = tabId
        secondaryWindow.versioningRestoreSidecarAttempts = 0
        secondaryWindow.versioningRestoreSidecarCaptureRunning = false
        secondaryWindow.versioningRestoreSidecarGeneration += 1
        versioningRestoreReadinessTimer.start()
        return true
    }

    function clearVersioningEditablePreview() {
        versioningEditablePreviewTimeout.stop()
        secondaryWindow.versioningEditablePreviewItems = []
        secondaryWindow.versioningEditablePreviewOmitted = 0
        secondaryWindow.versioningEditablePreviewRequestId = ""
        secondaryWindow.versioningEditablePreviewTabId = ""
        versioningEditableRestoreReview.close()
    }

    function finishVersioningEditableReview(confirmed) {
        var requestId = secondaryWindow.versioningEditablePreviewRequestId
        var tabId = secondaryWindow.versioningEditablePreviewTabId
        secondaryWindow.clearVersioningEditablePreview()
        if (requestId.length === 0) {
            return
        }
        var refillConfirmed = !!confirmed
                && secondaryUi.confirm_versioned_restore_editable_preview(requestId)
        if (confirmed && !refillConfirmed) {
            secondaryUi.status_text = "Editable draft refill was not authorized; restoring page structure only"
        } else if (!confirmed) {
            secondaryUi.status_text = "Editable draft refill skipped"
        }
        secondaryWindow.queueVersioningRestoreGeometry(requestId, tabId)
    }

    function reviewVersioningEditableRestore(requestId, tabId) {
        if (!/^\d{1,20}$/.test(requestId) || requestId.length === 0
                || typeof tabId !== "string" || tabId.length > 64) {
            secondaryUi.status_text = "Editable draft review request was invalid"
            return false
        }
        if (!secondaryWindow.windowRestoreEditableReviewEnabled) {
            return secondaryWindow.queueVersioningRestoreGeometry(requestId, tabId)
        }
        secondaryWindow.windowRestoreEditableReviewEnabled = false
        var previewJson = secondaryUi.versioned_restore_editable_preview(requestId)
        if (typeof previewJson !== "string" || previewJson.length === 0
                || previewJson.length > 20 * 1024) {
            secondaryUi.status_text = "Editable draft preview unavailable; restoring page structure only"
            return secondaryWindow.queueVersioningRestoreGeometry(requestId, tabId)
        }
        var preview
        try {
            preview = JSON.parse(previewJson)
        } catch (error) {
            secondaryUi.status_text = "Editable draft preview unavailable; restoring page structure only"
            return secondaryWindow.queueVersioningRestoreGeometry(requestId, tabId)
        }
        if (!preview || !Array.isArray(preview.items)
                || preview.items.length > 64
                || !Number.isFinite(Number(preview.omitted || 0))) {
            secondaryUi.status_text = "Editable draft preview invalid; restoring page structure only"
            return secondaryWindow.queueVersioningRestoreGeometry(requestId, tabId)
        }
        var safeItems = []
        var usedBytes = 0
        for (var i = 0; i < preview.items.length; ++i) {
            var item = preview.items[i]
            if (!item || (item.kind !== "text" && item.kind !== "textarea"
                          && item.kind !== "checkbox")) {
                secondaryUi.status_text = "Editable draft preview invalid; restoring page structure only"
                return secondaryWindow.queueVersioningRestoreGeometry(requestId, tabId)
            }
            if (item.kind === "checkbox" && item.value === true) {
                safeItems.push({ kind: "checkbox", displayValue: "Checked" })
                continue
            }
            if ((item.kind !== "text" && item.kind !== "textarea")
                    || typeof item.value !== "string" || item.value.length === 0
                    || item.value.length > 4096) {
                secondaryUi.status_text = "Editable draft preview invalid; restoring page structure only"
                return secondaryWindow.queueVersioningRestoreGeometry(requestId, tabId)
            }
            usedBytes += item.value.length * 2
            if (usedBytes > 16 * 1024) {
                secondaryUi.status_text = "Editable draft preview exceeded its safe limit; restoring page structure only"
                return secondaryWindow.queueVersioningRestoreGeometry(requestId, tabId)
            }
            safeItems.push({ kind: item.kind, displayValue: item.value })
        }
        if (safeItems.length === 0) {
            if (Number(preview.omitted || 0) > 0) {
                secondaryUi.status_text = "Some editable drafts were not eligible for review"
            }
            return secondaryWindow.queueVersioningRestoreGeometry(requestId, tabId)
        }
        secondaryWindow.versioningEditablePreviewRequestId = requestId
        secondaryWindow.versioningEditablePreviewTabId = tabId
        secondaryWindow.versioningEditablePreviewItems = safeItems
        secondaryWindow.versioningEditablePreviewOmitted = Number(preview.omitted || 0)
        versioningEditableRestoreReview.open()
        versioningEditablePreviewTimeout.restart()
        return true
    }

    function versioningRestoreContextCurrent(requestId, generation,
                                             view, tabId, epoch, pageUrl) {
        return secondaryWindow.versioningRestoreSidecarRequestId === requestId
                && secondaryWindow.versioningRestoreSidecarGeneration === generation
                && secondaryWindow.activeView === view
                && secondaryUi.active_tab_index === 0
                && secondaryUi.tab_id_for_index(0) === tabId
                && view.versioningNavigationEpoch === epoch
                && view.url.toString() === pageUrl
                && secondaryUi.current_url === pageUrl
                && view.profile === secondaryWindow.windowWebEngineProfile
                && !secondaryWindow.windowTransientProfile
                && view.lifecycleState === WebEngineView.LifecycleState.Active
    }

    function advanceVersioningRestoreGeometry() {
        var requestId = secondaryWindow.versioningRestoreSidecarRequestId
        if (requestId.length === 0) {
            versioningRestoreReadinessTimer.stop()
            return
        }
        secondaryWindow.versioningRestoreSidecarAttempts += 1
        if (secondaryWindow.versioningRestoreSidecarAttempts > 25) {
            secondaryWindow.clearVersioningRestoreGeometry(requestId, true)
            secondaryUi.status_text = "Geometry restore skipped: page was not ready in time"
            return
        }
        if (secondaryWindow.versioningRestoreSidecarCaptureRunning) {
            return
        }
        var view = secondaryWindow.activeView
        var tabId = secondaryUi.tab_id_for_index(0)
        if (secondaryWindow.windowTransferMode
                || secondaryUi.active_tab_index !== 0
                || tabId !== secondaryWindow.versioningRestoreSidecarTabId
                || !view || view !== secondaryView
                || view.profile !== secondaryWindow.windowWebEngineProfile
                || secondaryWindow.windowTransientProfile
                || view.lifecycleState !== WebEngineView.LifecycleState.Active) {
            secondaryWindow.clearVersioningRestoreGeometry(requestId, true)
            secondaryUi.status_text = "Geometry restore skipped: destination view changed"
            return
        }
        if (view.loading || !view.visible || !view.enabled) {
            return
        }
        var pageUrl = view.url.toString()
        if (!/^https?:\/\//i.test(pageUrl) || secondaryUi.current_url !== pageUrl) {
            return
        }
        var epoch = view.versioningNavigationEpoch
        var requestJson = secondaryUi.versioned_restore_geometry_capture_request(
            requestId, tabId)
        if (requestJson === "ferric-media-fingerprint-key-pending") {
            return
        }
        if (!requestJson || requestJson.length === 0) {
            secondaryWindow.clearVersioningRestoreGeometry(requestId, true)
            secondaryUi.status_text = "Geometry restore skipped: page qualification failed"
            return
        }
        var request
        try {
            request = JSON.parse(requestJson)
        } catch (error) {
            secondaryWindow.clearVersioningRestoreGeometry(requestId, true)
            secondaryUi.status_text = "Geometry restore skipped: page qualification failed"
            return
        }
        var script = VersionedPageState.geometryScript(request)
        if (typeof script !== "string" || script.length === 0) {
            secondaryWindow.clearVersioningRestoreGeometry(requestId, true)
            secondaryUi.status_text = "Geometry restore skipped: capture request was invalid"
            return
        }
        secondaryWindow.versioningRestoreSidecarCaptureRunning = true
        secondaryWindow.versioningRestoreSidecarAttempts = 0
        var generation = secondaryWindow.versioningRestoreSidecarGeneration
        view.runJavaScript(script, rootWindow.browserScriptWorld, function(currentJson) {
            if (!secondaryWindow.versioningRestoreContextCurrent(
                        requestId, generation, view, tabId, epoch, pageUrl)) {
                secondaryWindow.clearVersioningRestoreGeometry(requestId, true)
                return
            }
            if (typeof currentJson !== "string" || currentJson.length === 0
                    || currentJson.length > 262144) {
                secondaryWindow.clearVersioningRestoreGeometry(requestId, true)
                secondaryUi.status_text = "Geometry restore skipped: live observation was unavailable"
                return
            }
            var geometryRequest = secondaryUi.versioned_restore_geometry_script(
                requestId, currentJson)
            if (!geometryRequest || geometryRequest.length === 0) {
                secondaryWindow.clearVersioningRestoreGeometry(requestId, true)
                secondaryUi.status_text = "Geometry restore skipped: live observation did not qualify"
                return
            }
            var restoreScript = VersionedPageState.restoreGeometryScript(geometryRequest)
            if (typeof restoreScript !== "string" || restoreScript.length === 0) {
                secondaryWindow.clearVersioningRestoreGeometry(requestId, true)
                secondaryUi.status_text = "Geometry restore skipped: plan was invalid"
                return
            }
            view.runJavaScript(restoreScript, rootWindow.browserScriptWorld, function(resultJson) {
                if (!secondaryWindow.versioningRestoreContextCurrent(
                            requestId, generation, view, tabId, epoch, pageUrl)) {
                    secondaryWindow.clearVersioningRestoreGeometry(requestId, true)
                    return
                }
                secondaryWindow.clearVersioningRestoreGeometry(requestId, true)
                var result = null
                try {
                    result = typeof resultJson === "string" ? JSON.parse(resultJson) : null
                } catch (error) {
                    result = null
                }
                if (!result || result.ok !== true
                        || !Number.isFinite(Number(result.applied))
                        || !Number.isFinite(Number(result.skipped))) {
                    secondaryUi.status_text = "Structural restore completed; geometry was not applied"
                    return
                }
                var applied = Number(result.applied)
                var skipped = Number(result.skipped)
                var mediaApplied = Number(result.media_applied || 0)
                var mediaSkipped = Number(result.media_skipped || 0)
                var editableApplied = Number(result.editable_applied || 0)
                var editableSkipped = Number(result.editable_skipped || 0)
                var editableUnaccepted = Number(result.editable_unaccepted || 0)
                var mediaStatus = mediaApplied > 0 || mediaSkipped > 0
                        ? "; media positions " + mediaApplied + " applied, "
                          + mediaSkipped + " skipped"
                        : ""
                var editableStatus = editableApplied > 0 || editableSkipped > 0
                        ? "; editable drafts " + editableApplied + " applied, "
                          + editableSkipped + " skipped (" + editableUnaccepted
                          + " not accepted by the page)"
                        : ""
                secondaryUi.status_text = skipped > 0 || mediaSkipped > 0
                        || editableSkipped > 0
                        ? "Restore applied partially (" + applied + " geometry, "
                          + skipped + " geometry skipped" + mediaStatus
                          + editableStatus + ")"
                        : "Restore applied (" + applied + " geometry positions"
                          + mediaStatus + editableStatus + ")"
            })
        })
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
        if (secondaryUi.has_pending_versioning_object_work()) {
            secondaryWindow.windowShutdownWaitingForDownloadFinishes = true
            secondaryWindow.windowShutdownPromptVisible = true
            secondaryWindow.scheduleRuntimeWork(50)
            return
        }
        secondaryWindow.windowShutdownWaitingForDownloadFinishes = false
        secondaryWindow.checkPageStateBeforeQuit()
    }

    function finalizeQuit() {
        if (secondaryWindow.hasActiveDownloads()
                || rootWindow.hasActiveShutdownRequestsFor(
                    secondaryUi, secondaryWindow)) {
            secondaryWindow.windowShutdownPromptVisible = true
            return
        }
        if (secondaryUi.has_pending_versioning_object_work()) {
            secondaryWindow.windowShutdownWaitingForDownloadFinishes = true
            secondaryWindow.windowShutdownPromptVisible = true
            secondaryWindow.scheduleRuntimeWork(50)
            return
        }
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
        secondaryWindow.windowShutdownWaitingForDownloadFinishes = true
        if (secondaryWindow.pendingDownloadId.length > 0) {
            secondaryProfile.cancelPendingDownload()
        }
        for (var pendingKey in secondaryWindow.pendingDownloadRequests) {
            var pendingDownload = secondaryWindow.pendingDownloadRequests[pendingKey]
            if (pendingDownload) {
                rootWindow.resolveQtRequest(secondaryUi, pendingDownload, "download", "cancel", [])
                secondaryUi.update_download(
                    String(pendingKey), "cancelled", pendingDownload.receivedBytes, true)
            }
        }
        for (var key in secondaryWindow.activeDownloads) {
            var download = secondaryWindow.activeDownloads[key]
            if (download && !download.isFinished) {
                rootWindow.resolveQtRequest(secondaryUi, download, "download", "cancel", [])
            }
        }
        rootWindow.cancelShutdownRequestsFor(secondaryUi, secondaryWindow)
        if (secondaryWindow.hasActiveDownloads()) {
            secondaryWindow.windowShutdownPromptVisible = true
        } else {
            secondaryWindow.windowShutdownWaitingForDownloadFinishes = false
            secondaryWindow.continueQuitRequest()
        }
    }

    FerricFileDialogSurfaces {
        id: secondaryDialogSurfaces
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
        onAccepted: secondaryProfile.acceptPendingDownload()
        onRejected: secondaryProfile.cancelPendingDownload()
    }

    Dialog {
        id: versioningEditableRestoreReview
        modal: true
        focus: true
        title: "Review recovered form drafts"
        anchors.centerIn: Overlay.overlay
        width: Math.max(320, Math.min(720, secondaryWindow.width - 48))
        height: Math.max(300, Math.min(560, secondaryWindow.height - 48))
        padding: 20
        closePolicy: Popup.CloseOnEscape
        standardButtons: Dialog.NoButton
        Accessible.role: Accessible.Dialog

        onRejected: secondaryWindow.finishVersioningEditableReview(false)

        ColumnLayout {
            anchors.fill: parent
            spacing: 12

            Label {
                Layout.fillWidth: true
                text: "These saved values can be sensitive. Review them before allowing a one-time refill into matching blank native fields. Ferric will not submit the page or dispatch input/change events. The site may still replace a value."
                wrapMode: Text.WordWrap
            }

            ScrollView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true

                ColumnLayout {
                    width: parent.width
                    spacing: 8

                    Repeater {
                        model: secondaryWindow.versioningEditablePreviewItems

                        delegate: Frame {
                            required property var modelData
                            Layout.fillWidth: true

                            ColumnLayout {
                                anchors.fill: parent
                                Label {
                                    Layout.fillWidth: true
                                    text: modelData.kind === "textarea" ? "Text area"
                                          : (modelData.kind === "checkbox" ? "Checkbox" : "Text field")
                                    font.bold: true
                                }
                                Text {
                                    Layout.fillWidth: true
                                    text: String(modelData.displayValue || "")
                                    textFormat: Text.PlainText
                                    wrapMode: Text.WrapAnywhere
                                    Accessible.name: "Recovered " + modelData.kind + " value"
                                }
                            }
                        }
                    }
                }
            }

            Label {
                Layout.fillWidth: true
                visible: secondaryWindow.versioningEditablePreviewOmitted > 0
                text: secondaryWindow.versioningEditablePreviewOmitted
                      + " additional value(s) were omitted because they were unsupported or exceeded the preview limit."
                wrapMode: Text.WordWrap
            }

            RowLayout {
                Layout.fillWidth: true
                Item { Layout.fillWidth: true }
                Button {
                    text: "Skip refilling"
                    onClicked: secondaryWindow.finishVersioningEditableReview(false)
                }
                Button {
                    text: "Fill matching blank fields once"
                    highlighted: true
                    onClicked: secondaryWindow.finishVersioningEditableReview(true)
                }
            }
        }
    }

    WebEngineProfile {
        id: secondaryProfile
        offTheRecord: secondaryWindow.windowTransientProfile
        storageName: secondaryWindow.windowTransientProfile ? "" : "ferric-browser-" + secondaryWindow.windowProfileName
        persistentStoragePath: secondaryWindow.windowTransientProfile
                || secondaryWindow.windowStorageBasePath.length === 0
                ? ""
                : secondaryWindow.windowStorageBasePath + "/webengine/"
                  + secondaryWindow.windowProfileName
        cachePath: secondaryWindow.windowTransientProfile
                || secondaryWindow.windowStorageBasePath.length === 0
                ? ""
                : secondaryWindow.windowStorageBasePath + "/webengine-cache/"
                  + secondaryWindow.windowProfileName
        persistentPermissionsPolicy: WebEngineProfile.AskEveryTime
        spellCheckEnabled: rootWindow.spellcheckEnabled(secondaryUi)
        spellCheckLanguages: rootWindow.spellcheckLanguages(secondaryUi)
        isPushServiceEnabled: rootWindow.pushServiceEnabled(
            secondaryUi, secondaryWindow.windowTransientProfile)

        function acceptPendingDownload() {
            var id = secondaryWindow.pendingDownloadId
            var download = secondaryWindow.pendingDownloadRequests[id]
            var selectedPath = secondaryDownloadChooser.selectedFile.toLocalFile()
            if (!download) {
                secondaryWindow.pendingDownloadId = ""
                rootWindow.restoreOverlayFocus()
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
                    rootWindow.restoreOverlayFocus()
                    return
                }
                download.downloadDirectory = stagingDirectory
                download.downloadFileName = finalName
                secondaryWindow.activeDownloads[id] = download
                delete secondaryWindow.pendingDownloadRequests[id]
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
            rootWindow.restoreOverlayFocus()
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
            rootWindow.restoreOverlayFocus()
        }

        function handleForwardedDownloadRequested(download) {
            var owner = secondaryWindow.downloadOwnerFor(download)
            if (!owner) {
                download.cancel()
                secondaryUi.status_text = "Download refused: window owner was ambiguous"
                return
            }
            if (owner !== secondaryWindow && owner.handleDownloadRequested) {
                owner.handleDownloadRequested(download)
                return
            }
            var id = String(download.id)
            if (ProfileDownloadRouting.retainAndCancelIfShutdownApproved(
                        secondaryWindow, download, function(request) {
                            return rootWindow.resolveQtRequest(
                                       secondaryUi, request, "download", "cancel", [])
                        }))
                return
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
                delete secondaryWindow.pendingDownloadRequests[id]
                download.stateChanged.connect(function() {
                    secondaryUi.update_download(id, secondaryWindow.secondaryDownloadStateName(download), download.receivedBytes, false)
                })
                rootWindow.resolveQtRequest(secondaryUi, download, "download", "accept", [])
                secondaryUi.update_download(id, "in-progress", 0, false)
                return
            }
            var directory = secondaryUi.default_download_directory()
            if (rootWindow.downloadsAskDestination()) {
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
                secondaryUi.update_download(id, "cancelled", download.receivedBytes, true)
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
            secondaryWindow.activeDownloads[id] = download
            delete secondaryWindow.pendingDownloadRequests[id]
            rootWindow.resolveQtRequest(secondaryUi, download, "download", "accept", [])
            secondaryUi.update_download(id, "in-progress", 0, false)
        }
        onDownloadRequested: function(download) {
            secondaryProfile.handleForwardedDownloadRequested(download)
        }
        function handleForwardedDownloadFinished(download) {
            var owner = secondaryWindow.downloadOwnerFor(download, false)
            if (!owner) {
                secondaryUi.status_text = "Download completion owner was unavailable"
                return
            }
            if (owner !== secondaryWindow && owner.handleDownloadFinished) {
                owner.handleDownloadFinished(download)
                return
            }
            var id = String(download.id)
            var state = secondaryWindow.secondaryDownloadStateName(download)
            if (state === "completed" && !secondaryUi.finalize_download(id)) {
                state = "interrupted"
            } else if (state !== "completed") {
                secondaryUi.discard_download_staging(id)
            }
            delete secondaryWindow.activeDownloads[id]
            delete secondaryWindow.pendingDownloadRequests[id]
            delete secondaryWindow.downloadCompletionRequests[id]
            if (secondaryWindow.pendingDownloadId === id) {
                secondaryWindow.pendingDownloadId = ""
                secondaryWindow.pendingDownloadSuggestedName = ""
            }
            secondaryUi.update_download(id, state, download.receivedBytes, true)
            if (secondaryWindow.windowShutdownWaitingForDownloadFinishes
                    && !secondaryWindow.hasActiveDownloads()) {
                secondaryWindow.windowShutdownWaitingForDownloadFinishes = false
                secondaryWindow.windowShutdownPromptVisible = false
                secondaryWindow.continueQuitRequest()
            }
            if (!secondaryWindow.hasActiveDownloads()
                    && secondaryWindow.windowShutdownApproved)
                secondaryWindow.requestApprovedWindowRelease()
        }
        onDownloadFinished: function(download) {
            secondaryProfile.handleForwardedDownloadFinished(download)
        }
        onPresentNotification: function(notification) {
            rootWindow.presentWebNotification(
                secondaryUi, notification, secondaryWindow.windowTransientProfile)
        }
    }

    BrowserUi {
        id: secondaryUi
        hint_chrome_available: false
        status_text: "Ready"
        onCore_window_idChanged: rootWindow.refreshBrowserWindowIdentity(secondaryUi)
        onWindow_tokenChanged: rootWindow.refreshBrowserWindowIdentity(secondaryUi)
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

    FerricShutdownDecisionDialog {
        hostWindow: secondaryWindow
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
        promptVisible: secondaryWindow.windowShutdownPromptVisible
        commandText: ":window-close"
        title: secondaryWindow.windowShutdownWaitingForDownloadFinishes
               ? "Waiting for active work to finish" : "Active browser work is still running"
        message: secondaryWindow.windowShutdownWaitingForDownloadFinishes
                 ? "Waiting for canceled downloads and archive storage work to retire. Keep the window open to stop closing."
                 : "Cancel active work before closing, or keep this window open."
        keepLabel: "Keep window open"
        proceedLabel: "Cancel and close"
        dialogHeight: 220 * rootWindow.chromeScale
        onKeepRequested: {
            secondaryWindow.windowShutdownPromptVisible = false
            secondaryWindow.windowShutdownWaitingForDownloadFinishes = false
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
            secondaryWindow.confirmShutdownPagePrompt()
        }
    }

    FerricShutdownDecisionDialog {
        hostWindow: secondaryWindow
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
                rootWindow.restoreOverlayFocus()
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

    RequestInterceptor {
        id: secondaryRequestInterceptor
        enabled: secondaryUi.blocking_enabled || securityDenyHosts.length > 0
        blockedHosts: secondaryUi.blocking_hosts
        exceptionHosts: secondaryUi.blocking_exceptions
        blockedRuleHosts: secondaryUi.blocking_rule_hosts
        blockedRuleListIds: secondaryUi.blocking_rule_list_ids
        exceptionRuleHosts: secondaryUi.blocking_exception_rule_hosts
        exceptionRuleListIds: secondaryUi.blocking_exception_rule_list_ids
        adblockEngineHandle: secondaryUi.blocking_adblock_handle
        bypassSites: secondaryUi.blocking_bypass_sites
        securityDenyHosts: secondaryUi.blocking_security_deny_hosts
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
            var path = urls[i].toLocalFile()
            if (!path || urls[i].scheme !== "file") {
                return []
            }
            paths.push(path)
        }
        return paths
    }

    function secondaryFolderDialogPaths() {
        var url = secondaryDialogSurfaces.engineFolderSelectedFolder
        var path = url && url.scheme === "file" ? url.toLocalFile() : ""
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
            rootWindow.restoreOverlayFocus()
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
                rootWindow.restoreOverlayFocus()
            }
            secondaryUi.status_text = "File selection returned no local paths"
            return
        }
        rootWindow.resolveQtRequest(secondaryUi, request, "file-dialog", "dialogAccept", [paths])
        rootWindow.restoreOverlayFocus()
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
                rootWindow.restoreOverlayFocus()
            }
            secondaryUi.status_text = "Folder selection returned no local path"
            return
        }
        rootWindow.resolveQtRequest(secondaryUi, request, "file-dialog", "dialogAccept", [paths])
        rootWindow.restoreOverlayFocus()
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
        rootWindow.restoreOverlayFocus()
        secondaryUi.status_text = "File selection cancelled"
    }

    function handleFileDialogRequested(request) {
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
        if (action === "context-save-request") {
            rootWindow.saveContextMembership(secondaryUi)
            return
        }
        if (action === "context-commit-request") {
            rootWindow.commitSavedContext(secondaryUi)
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
        if (action.indexOf("versioning-sidecar-ready\t") === 0) {
            var sidecarFields = action.split("\t")
            if (sidecarFields.length === 3) {
                secondaryWindow.reviewVersioningEditableRestore(
                    sidecarFields[1], sidecarFields[2])
            } else {
                secondaryUi.status_text = "Restore sidecar request was invalid"
            }
            return
        }
        if (action.indexOf("versioning-branch-switch\t") === 0) {
            if (!rootWindow.activateVersionedBranchTarget(
                        secondaryUi, action.slice("versioning-branch-switch\t".length)))
                secondaryUi.status_text = "Clean branch switch refused: target is stale or unavailable"
            return
        }
        if (action.indexOf("scroll-target\t") === 0) {
            var scrollTargetParts = action.split("\t")
            var scrollTargetIndex = scrollTargetParts.length === 3
                    ? secondaryUi.tab_index_for_id(scrollTargetParts[1]) : -1
            var scrollTargetView = scrollTargetIndex === 0
                    ? secondaryWindow.activeView : null
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
            var secondaryTabId = secondaryUi.tab_id_for_index(0)
            var secondaryMuted = secondaryMuteParts.length >= 3
                    && secondaryMuteParts[2] === "true"
            if (secondaryMuteParts.length < 2
                    || secondaryMuteParts[1] !== secondaryTabId) {
                secondaryUi.status_text = "Tab mute target is stale"
                return
            }
            secondaryWindow.activeView.audioMuted = secondaryMuted
            secondaryUi.status_text = secondaryMuted ? "Tab muted" : "Tab unmuted"
            return
        }
        if (action.indexOf("navigate\t") === 0) {
            var navigation = action.split("\t")
            if (navigation.length >= 3) {
                secondaryWindow.activeView.url = navigation.slice(2).join("\t")
                secondaryWindow.activeView.forceActiveFocus()
                return
            }
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
            var historyMoved = 0
            while (historyMoved < historyCount
                    && ((historyBack && secondaryWindow.activeView.canGoBack)
                        || (!historyBack && secondaryWindow.activeView.canGoForward))) {
                if (historyBack) {
                    secondaryWindow.activeView.goBack()
                } else {
                    secondaryWindow.activeView.goForward()
                }
                historyMoved += 1
            }
            secondaryUi.status_text = historyMoved === 0
                    ? "History boundary reached"
                    : historyMoved < historyCount
                      ? "History boundary reached after " + historyMoved + " step(s)"
                      : (historyBack ? "Back requested" : "Forward requested")
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
        if (action.indexOf("versioning-restore-window\t") === 0) {
            rootWindow.openVersionedRestoreWindow(secondaryUi, secondaryWindow, action)
            return
        }
        if (action.indexOf("versioning-clean-branch-ready\t") === 0) {
            rootWindow.openVersionedCleanBranch(secondaryUi, secondaryWindow, action)
            return
        }
        if (action.indexOf("zoom\t") === 0) {
            var secondaryZoomParts = action.split("\t")
            if (secondaryZoomParts.length >= 3) {
                var secondaryZoomFactor = Number(secondaryZoomParts[2])
                if (Number.isFinite(secondaryZoomFactor)
                        && secondaryZoomFactor >= 0.25
                        && secondaryZoomFactor <= 5.0) {
                    secondaryWindow.activeView.zoomFactor = secondaryZoomFactor
                }
            }
            secondaryWindow.activeView.forceActiveFocus()
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
            var secondaryEvalView = secondaryEvalIndex >= 0
                    && secondaryEvalIndex === secondaryUi.active_tab_index
                    ? secondaryWindow.activeView : null
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
                if (secondaryWindow.windowTransferMode) {
                    secondaryUi.status_text =
                            "Use devtools --detach for a transferred view"
                } else {
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
        secondaryUi.status_text = "Browser action is unavailable in this window"
        secondaryWindow.activeView.forceActiveFocus()
    }

    function scheduleRuntimeWork(delay) {
        var boundedDelay = Math.max(0, Math.min(60000, Number(delay) || 0))
        if (!secondaryRuntimeWorkTimer.running
                || boundedDelay < secondaryRuntimeWorkTimer.interval) {
            secondaryRuntimeWorkTimer.interval = Math.max(1, boundedDelay)
            secondaryRuntimeWorkTimer.restart()
        }
    }

    function processRuntimeWork() {
        secondaryWindow.applySwitcherEngineAction()
        secondaryUi.poll_config()
        secondaryWindow.maybeOpenPendingEngineFileDialog()
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
            if (editorError.length > 0) {
                secondaryUi.deliver_editor_apply(
                    editorToken, JSON.stringify({error: "external-editor-failed"}))
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
                rootWindow.runBrowserScript(
                    secondaryWindow.activeView, rootWindow.selectionScript(),
                    function(value) {
                        secondaryUi.deliver_selection(selectionToken,
                                                      JSON.stringify(value))
                    })
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
                secondaryUi.clipboard_request_primary)
        }

        secondaryWindow.applySwitcherEngineAction()
        if (secondaryWindow.windowShutdownWaitingForDownloadFinishes) {
            if (!secondaryWindow.hasActiveDownloads()
                    && !secondaryUi.has_pending_versioning_object_work()) {
                secondaryWindow.windowShutdownWaitingForDownloadFinishes = false
                secondaryWindow.windowShutdownPromptVisible = false
                secondaryWindow.continueQuitRequest()
            } else if (!secondaryWindow.hasActiveDownloads()) {
                secondaryWindow.scheduleRuntimeWork(50)
            }
        }
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
                    if (secondaryWindow.activeView.canGoBack) {
                        secondaryUi.back()
                        secondaryWindow.activeView.goBack()
                    } else {
                        secondaryUi.status_text = "History boundary reached"
                    }
                }
            }
            ToolButton {
                text: "›"
                Accessible.name: "Forward"
                onClicked: {
                    if (secondaryWindow.activeView.canGoForward) {
                        secondaryUi.forward()
                        secondaryWindow.activeView.goForward()
                    } else {
                        secondaryUi.status_text = "History boundary reached"
                    }
                }
            }
            TextField {
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
                    secondaryWindow.activeView.url = secondaryUi.initial_url
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

    Item {
        id: transferViewHost
        anchors.fill: parent
        anchors.bottomMargin: secondaryWindow.bottomChromeHeight
        visible: secondaryWindow.windowTransferMode
    }

    Item {
        id: fallbackViewHost
        anchors.fill: parent
        anchors.bottomMargin: secondaryWindow.bottomChromeHeight
        visible: !secondaryWindow.windowTransferMode
    }

    WebEngineView {
        id: secondaryView
        property var viewUi: secondaryUi
        property var viewHost: secondaryWindow
        property var viewProfile: secondaryWindow.windowWebEngineProfile
        property var viewInterceptor: secondaryWindow.windowRequestInterceptor
        property bool viewTransientProfile: secondaryWindow.windowTransientProfile
        property bool viewTransferred: false
        property bool viewIsSecondaryStatic: true
        property int tabIndex: -1
        property int versioningNavigationEpoch: 0
        property string versioningTypedGetCandidateUrl: ""
        property double versioningMethodSequence: -1
        property var viewTabs: null
        property var viewModel: ({})
        property string modelNavigationUrl:
            secondaryWindow.windowTransferMode
            || (secondaryWindow.windowCleanBranch
                && !secondaryWindow.windowCleanBranchNavigationAdmitted)
            ? "about:blank" : secondaryUi.initial_url
        property bool versioningUrlBindingSuspended: false
        property bool versioningFormNavigationPending: false
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
        devToolsView: secondaryWindow.devToolsVisible
                ? secondaryWindow.devToolsAttachedView : null
        // Navigation-scoped content settings are snapshotted per
        // document; live config changes apply to the next navigation.
        property var effectiveSiteSettings: ({ values: {}, matched_rules: [] })
        function refreshEffectiveSiteSettings() {
            effectiveSiteSettings = rootWindow.siteRuleSettingsFor(
                viewUi, secondaryView.url.toString())
        }
        onModelNavigationUrlChanged: {
            if (versioningUrlBindingSuspended
                    && !versioningFormNavigationPending
                    && modelNavigationUrl !== secondaryView.url.toString()) {
                versioningUrlBindingSuspended = false
            }
        }
        Connections {
            target: viewUi
            function onSite_experiment_kindChanged() {
                secondaryView.refreshEffectiveSiteSettings()
            }
        }
        property int blockedRequestCount: {
            var host = secondaryView.url && secondaryView.url.host
                    ? String(secondaryView.url.host) : ""
            var counts = viewInterceptor.blockedSiteCounts || ({})
            return host.length > 0 && counts[host] !== undefined ? Number(counts[host]) : 0
        }
        anchors.fill: parent
        anchors.bottomMargin: secondaryWindow.bottomChromeHeight
        visible: !secondaryWindow.windowTransferMode
        profile: viewProfile
        // A bound URL starts a GET. Let an in-view form own its POST result
        // until a distinct model navigation target is requested.
        Binding on url {
            when: !secondaryView.versioningUrlBindingSuspended
            value: secondaryView.modelNavigationUrl
            restoreMode: Binding.RestoreNone
        }
        settings.javascriptEnabled: !!rootWindow.siteRuleValue(
            effectiveSiteSettings, "content.javascript", true)
        settings.autoLoadImages: !!rootWindow.siteRuleValue(
            effectiveSiteSettings, "content.images", true)
        settings.forceDarkMode: !!rootWindow.siteRuleValue(
            effectiveSiteSettings, "content.force_dark", false)
        settings.playbackRequiresUserGesture:
            rootWindow.siteRuleValue(effectiveSiteSettings, "content.autoplay", "engine-default")
            === "require-gesture"
        Accessible.name: "Web content"
        Component.onCompleted: {
            refreshEffectiveSiteSettings()
        }
        onUrlChanged: {
            refreshEffectiveSiteSettings()
            if (viewTabs === tabs) {
                viewUi.navigation_url_changed_for(tabIndex, url.toString())
            } else {
                viewUi.navigation_url_changed(url.toString())
            }
            if (viewHost === secondaryWindow) {
                secondaryAddress.text = viewUi.display_url
            }
        }
        onTitleChanged: {
            if (viewTabs === tabs && tabIndex >= 0 && tabIndex < tabs.count) {
                tabs.setProperty(tabIndex, "title", title || "New tab")
            }
        }
        onRecentlyAudibleChanged: {
            if (viewUi === rootWindow.primaryBrowserUi
                    && tabIndex === viewUi.active_tab_index) {
                rootWindow.updateMprisForPrimaryView(webView)
            }
        }
        onAudioMutedChanged: {
            if (viewUi === rootWindow.primaryBrowserUi
                    && tabIndex === viewUi.active_tab_index) {
                rootWindow.updateMprisForPrimaryView(webView)
            }
        }
        onNavigationRequested: function(request) {
            if (!request.isMainFrame)
                return
            if (request.hasFormData) {
                secondaryView.versioningFormNavigationPending = true
                secondaryView.versioningUrlBindingSuspended = true
            }
            secondaryView.versioningMethodSequence = viewInterceptor.mainFrameMethodSequence()
            secondaryView.versioningTypedGetCandidateUrl =
                request.navigationType === WebEngineNavigationRequest.TypedNavigation
                && !request.hasFormData ? request.url.toString() : ""
        }
        onLoadingChanged: function(loadRequest) {
            if (loadRequest.status === WebEngineView.LoadStartedStatus) {
                secondaryView.versioningNavigationEpoch += 1
                if (secondaryView.versioningTypedGetCandidateUrl
                        !== loadRequest.url.toString()) {
                    secondaryView.versioningTypedGetCandidateUrl = ""
                }
                rootWindow.clearPageDialogForView(secondaryView)
                rootWindow.clearClientCertificateForView(secondaryView)
                rootWindow.clearCertificateErrorForView(secondaryView)
                rootWindow.clearWebAuthForView(secondaryView)
                rootWindow.clearContextMenuForView(secondaryView)
                rootWindow.clearDesktopMediaForView(secondaryView)
                rootWindow.clearSiteDataClearForView(secondaryView)
                rootWindow.noteCaptureNavigation(secondaryView)
                secondaryWindow.clearFileDialogForView(secondaryView)
                rootWindow.clearRendererFailureForView(secondaryView)
                rootWindow.resetPageDialogBudget(secondaryView)
                viewInterceptor.clearSiteEvidence(secondaryView.url.host)
                rootWindow.cancelPermissionForUi(viewUi)
                rootWindow.installPageUserscripts(
                    viewUi, secondaryView, secondaryView.url.toString(),
                    viewTransientProfile)
                rootWindow.injectPageUserscripts(
                    viewUi, secondaryView, secondaryView.url.toString(),
                    viewTransientProfile, "document_start")
                rootWindow.injectCosmeticRules(viewUi, secondaryView)
                if (viewTabs === tabs) {
                    viewUi.navigation_started_for(tabIndex, loadRequest.url.toString())
                } else {
                    viewUi.navigation_started(loadRequest.url.toString())
                }
            } else if (loadRequest.status === WebEngineView.LoadSucceededStatus) {
                secondaryView.versioningFormNavigationPending = false
                if (viewTabs === tabs) {
                    viewUi.navigation_committed_for(
                        tabIndex, secondaryView.url.toString(), secondaryView.title)
                    if (secondaryView.versioningTypedGetCandidateUrl
                            === secondaryView.url.toString()
                            && secondaryView.versioningMethodSequence >= 0
                            && viewInterceptor.uniqueMainFrameGetSince(
                                secondaryView.versioningMethodSequence,
                                secondaryView.url.toString())) {
                        viewUi.navigation_typed_safe_get_committed_for(
                            tabIndex, secondaryView.url.toString())
                    }
                } else {
                    viewUi.navigation_committed(secondaryView.url.toString(), secondaryView.title)
                    if (secondaryView.versioningTypedGetCandidateUrl
                            === secondaryView.url.toString()
                            && secondaryView.versioningMethodSequence >= 0
                            && viewInterceptor.uniqueMainFrameGetSince(
                                secondaryView.versioningMethodSequence,
                                secondaryView.url.toString())) {
                        viewUi.navigation_typed_safe_get_committed_for(
                            viewUi.active_tab_index, secondaryView.url.toString())
                    }
                }
                secondaryView.versioningTypedGetCandidateUrl = ""
                secondaryView.versioningMethodSequence = -1
                rootWindow.injectPageUserscripts(
                    viewUi, secondaryView, secondaryView.url.toString(),
                    viewTransientProfile, "document_end")
                rootWindow.injectCosmeticRules(viewUi, secondaryView)
                Qt.callLater(function() {
                    rootWindow.injectPageUserscripts(
                        viewUi, secondaryView, secondaryView.url.toString(),
                        viewTransientProfile, "document_idle")
                })
                if (viewTabs === tabs) {
                    viewUi.navigation_completed_for(tabIndex)
                } else {
                    viewUi.navigation_completed()
                }
            } else if (loadRequest.status === WebEngineView.LoadFailedStatus) {
                secondaryView.versioningFormNavigationPending = false
                secondaryView.versioningTypedGetCandidateUrl = ""
                secondaryView.versioningMethodSequence = -1
                if (viewTabs === tabs) {
                    viewUi.navigation_failed_with_details(
                        tabIndex,
                        loadRequest.url.toString(),
                        rootWindow.navigationFailureKind(loadRequest),
                        rootWindow.navigationFailureDetail(loadRequest))
                } else {
                    viewUi.navigation_failed_with_details(
                        -1,
                        loadRequest.url.toString(),
                        rootWindow.navigationFailureKind(loadRequest),
                        rootWindow.navigationFailureDetail(loadRequest))
                }
            }
        }
        onPermissionRequested: function(permissionRequest) {
            rootWindow.handleImmediatePermissionRequested(
                viewUi, permissionRequest, viewTransientProfile, viewHost)
        }
        onJavaScriptDialogRequested: function(request) {
            rootWindow.handleJavaScriptDialogRequested(
                viewUi, secondaryView, request, viewTransientProfile)
        }
        onAuthenticationDialogRequested: function(request) {
            rootWindow.handleAuthenticationDialogRequested(
                viewUi, secondaryView, request, viewTransientProfile)
        }
        onSelectClientCertificate: function(selection) {
            rootWindow.handleClientCertificateRequested(
                viewUi, secondaryView, selection, viewTransientProfile, viewHost)
        }
        onCertificateError: function(error) {
            rootWindow.handleCertificateError(
                viewUi, secondaryView, error, viewHost)
        }
        onWebAuthUxRequested: function(request) {
            rootWindow.handleWebAuthRequested(viewUi, secondaryView, request, viewHost)
        }
        onContextMenuRequested: function(request) {
            rootWindow.handleContextMenuRequested(viewUi, secondaryView, request, viewHost)
        }
        onFileDialogRequested: function(request) {
            if (viewHost === secondaryWindow) {
                secondaryWindow.handleFileDialogRequested(request)
            } else {
                rootWindow.handleFileDialogRequested(request, secondaryView, viewUi)
            }
        }
        onDesktopMediaRequested: function(request) {
            rootWindow.handleDesktopMediaRequested(
                viewUi, secondaryView, request, viewHost)
        }
        onFullScreenRequested: function(request) {
            request.accept()
            if (request.toggleOn) {
                viewHost.showFullScreen()
                viewUi.status_text = "Page fullscreen enabled"
            } else {
                viewHost.showNormal()
                viewUi.status_text = "Page fullscreen ended"
            }
        }
        onRenderProcessTerminated: function(terminationStatus, exitCode) {
            rootWindow.handleRendererProcessTerminated(
                viewUi, secondaryView, viewHost, -1,
                terminationStatus, exitCode)
        }
        onNewWindowRequested: function(request) {
            if (!viewUi.popup_allowed(request.requestedUrl.toString(), request.userInitiated)) {
                return
            }
            if (secondaryWindow.windowCleanBranch) {
                if (!CleanBranchPopupOwnership.sourceReady(
                            rootWindow, secondaryWindow, viewUi,
                            viewProfile, viewInterceptor, secondaryView)) {
                    viewUi.navigation_failed()
                    return
                }
            }
            if (!rootWindow.canRegisterPopupWindow()) {
                return
            }
            var popup = rootWindow.popupWindowFactory.createObject(null, {
                popupRequest: request,
                popupOpenerHost: secondaryWindow,
                popupJourneyToken: viewUi.take_popup_journey_token(
                    request.requestedUrl.toString()),
                popupProfile: viewProfile,
                popupRequestInterceptor: viewInterceptor,
                popupPermissionUi: viewUi,
                popupContextName: viewUi.context_name,
                popupContextLabel: viewUi.context_label,
                popupPrivateProfile: viewTransientProfile,
                popupEphemeralProfile: secondaryWindow.windowEphemeralProfile
                        || secondaryWindow.windowCleanBranch,
                popupBranchToken: secondaryWindow.windowCleanBranch
                        ? secondaryWindow.windowBoundBranchToken : "",
                popupProfileName: secondaryWindow.windowProfileName
            })
            if (!popup) {
                viewUi.navigation_failed()
            }
        }
    }

    Loader {
        id: secondaryDevToolsLoader
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: Math.min(280, parent.height * 0.55)
        active: secondaryWindow.devToolsVisible
                && !secondaryWindow.windowTransferMode
        visible: active
        z: 20
        sourceComponent: Component {
            WebEngineView {
                anchors.fill: parent
                profile: secondaryWindow.activeView
                        ? secondaryWindow.activeView.profile
                        : secondaryWindow.windowWebEngineProfile
                inspectedView: secondaryWindow.activeView
                Accessible.name: "Attached developer tools"
            }
        }
    }

    function detachActiveViewForTransfer() {
        var view = secondaryWindow.activeView
        if (!view) {
            return null
        }
        secondaryWindow.devToolsVisible = false
        if (secondaryWindow.devToolsWindow) {
            secondaryWindow.devToolsWindow.close()
            secondaryWindow.devToolsWindow = null
        }
        secondaryWindow.windowDetachedView = view
        secondaryWindow.windowDetachedWasTransfer = secondaryWindow.windowTransferMode
        if (view === secondaryWindow.windowFallbackView) {
            secondaryWindow.windowFallbackView = null
        }
        if (view === secondaryView) {
            view.parent = null
        } else {
            view.parent = null
        }
        view.visible = false
        return view
    }

    function restoreDetachedView(view) {
        if (!view) {
            return false
        }
        if (secondaryWindow.windowDetachedWasTransfer) {
            secondaryWindow.windowTransferView = view
            view.parent = transferViewHost
            view.anchors.fill = transferViewHost
        } else {
            if (view === secondaryView) {
                view.parent = secondaryWindow.contentItem
                view.anchors.fill = secondaryWindow.contentItem
                secondaryView.visible = true
            } else {
                secondaryWindow.windowFallbackView = view
                view.parent = fallbackViewHost
                view.anchors.fill = fallbackViewHost
            }
        }
        view.visible = true
        secondaryWindow.windowDetachedView = null
        return true
    }

    function prepareTransferFallback() {
        if (secondaryWindow.windowPreparedTransferFallback) {
            return true
        }
        var fallback = rootWindow.webViewFactory.createObject(fallbackViewHost, {
            tabIndex: -1,
            viewUi: secondaryUi,
            viewTabs: null,
            viewModel: {
                loaded: true,
                url: "about:blank",
                title: "New tab",
                muted: false,
                zoom: 1.0,
                scrollX: -1,
                scrollY: -1
            },
            viewProfile: secondaryWindow.windowWebEngineProfile,
            viewInterceptor: secondaryWindow.windowRequestInterceptor,
            viewTransientProfile: secondaryWindow.windowTransientProfile,
            viewTransferred: false
        })
        if (!fallback) {
            secondaryUi.status_text = "Transfer fallback view could not be created"
            return false
        }
        fallback.visible = false
        secondaryWindow.windowPreparedTransferFallback = fallback
        return true
    }

    function discardPreparedTransferFallback() {
        if (secondaryWindow.windowPreparedTransferFallback) {
            secondaryWindow.windowPreparedTransferFallback.destroy()
            secondaryWindow.windowPreparedTransferFallback = null
        }
    }

    function resetAfterTransferSource() {
        if (!prepareTransferFallback()) {
            return false
        }
        secondaryWindow.windowTransferView = null
        secondaryWindow.windowTransferSourceUi = null
        secondaryWindow.windowTransferSourceHost = null
        secondaryWindow.windowTransferOperationId = ""
        secondaryWindow.windowTransferPayload = ""
        secondaryView.visible = false
        if (secondaryWindow.windowFallbackView) {
            secondaryWindow.windowFallbackView.destroy()
            secondaryWindow.windowFallbackView = null
        }
        // complete_tab_transfer() creates the reducer's mandatory
        // blank fallback when this was the source's last tab.
        var fallback = secondaryWindow.windowPreparedTransferFallback
        secondaryWindow.windowPreparedTransferFallback = null
        fallback.viewHost = secondaryWindow
        fallback.parent = fallbackViewHost
        fallback.anchors.fill = fallbackViewHost
        fallback.visible = true
        secondaryWindow.windowFallbackView = fallback
        rootWindow.updateBrowserWindowView(secondaryUi, fallback)
        secondaryUi.status_text = "Live tab moved; blank fallback tab created"
        return true
    }

    function submitInteractiveCommand(text) {
        if (secondaryWindow.windowCleanBranch
                && /^commit(?:\s|$)/.test(String(text || "").trim())) {
            if (!secondaryWindow.windowCleanBranchPublished
                    || !secondaryWindow.windowBranchParentUi) {
                secondaryUi.status_text = "Branch export refused: owner unavailable"
                secondaryCommandBar.showFeedback(secondaryUi.status_text, true)
                return
            }
            var encoded = secondaryWindow.windowBranchParentUi
                    .preview_versioned_clean_branch_export(
                        secondaryUi, secondaryWindow.windowBoundBranchToken, text)
            var preview = null
            try {
                preview = JSON.parse(String(encoded || ""))
            } catch (error) {
                preview = null
            }
            if (!preview
                    || !/^operation:[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(String(preview.operation || ""))
                    || typeof preview.destination !== "string"
                    || preview.destination.length === 0
                    || preview.destination.length > 256
                    || !Number.isInteger(preview.pages)
                    || preview.pages < 1 || preview.pages > 128) {
                if (preview && typeof preview.operation === "string")
                    secondaryWindow.windowBranchParentUi
                        .cancel_versioned_clean_branch_export(preview.operation)
                secondaryUi.status_text = "Branch export refused: scope or owner unavailable"
                secondaryCommandBar.showFeedback(secondaryUi.status_text, true)
                return
            }
            secondaryWindow.branchExportOperation = preview.operation
            secondaryWindow.branchExportDestination = preview.destination
            secondaryWindow.branchExportPageCount = preview.pages
            secondaryWindow.pendingInteractiveCommand = ""
            secondaryCommandBar.clearInput()
            branchExportDialog.open()
            return
        }
        if (secondaryWindow.windowCleanBranch
                && secondaryWindow.windowBranchParentUi
                && /^switch(?:\s|$)/.test(String(text || "").trim())) {
            var switched = rootWindow.switchVersionedCleanBranch(
                    secondaryWindow.windowBranchParentUi, secondaryUi, text)
            if (!switched) {
                secondaryUi.status_text = "Clean branch switch refused: target is stale, ambiguous, or unavailable"
                secondaryCommandBar.showFeedback(secondaryUi.status_text, true)
                secondaryCommandBar.selectAllInput()
                return
            }
            secondaryWindow.pendingInteractiveCommand = ""
            secondaryCommandBar.clearInput()
            secondaryWindow.showCommandNotice(
                        "Live branch window activation requested", false)
            return
        }
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

    Dialog {
        id: branchExportDialog
        parent: secondaryWindow.contentItem
        anchors.centerIn: parent
        width: Math.min(520, secondaryWindow.width - 32)
        modal: true
        title: "Export clean branch structure?"
        standardButtons: Dialog.NoButton
        closePolicy: Popup.CloseOnEscape
        onClosed: {
            if (secondaryWindow.branchExportOperation.length > 0
                    && secondaryWindow.windowBranchParentUi) {
                secondaryWindow.windowBranchParentUi
                    .cancel_versioned_clean_branch_export(
                        secondaryWindow.branchExportOperation)
            }
            secondaryWindow.branchExportOperation = ""
            secondaryWindow.branchExportDestination = ""
            secondaryWindow.branchExportPageCount = 0
        }
        ColumnLayout {
            anchors.fill: parent
            spacing: 12
            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                text: "Save " + secondaryWindow.branchExportPageCount
                      + " branch page structure(s) to normal profile ‘"
                      + secondaryWindow.branchExportDestination + "’?"
            }
            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                text: "This durable capture may include browsing URLs, tab order and basic page settings. It does not copy cookies, page bodies, grants or the live authenticated branch. The branch remains transient."
            }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                Button {
                    text: "Cancel"
                    onClicked: branchExportDialog.close()
                }
                Button {
                    text: "Export structure"
                    enabled: secondaryWindow.branchExportOperation.length > 0
                    onClicked: {
                        var operation = secondaryWindow.branchExportOperation
                        var accepted = secondaryWindow.windowBranchParentUi
                                && secondaryWindow.windowBranchParentUi
                                    .confirm_versioned_clean_branch_export(
                                        secondaryUi, operation)
                        if (!accepted && secondaryWindow.windowBranchParentUi)
                            secondaryWindow.windowBranchParentUi
                                .cancel_versioned_clean_branch_export(operation)
                        secondaryWindow.branchExportOperation = ""
                        branchExportDialog.close()
                        secondaryWindow.showCommandNotice(
                            accepted
                            ? "Branch structure export queued for durable publication"
                            : "Branch export refused: owner or pages changed",
                            !accepted)
                    }
                }
            }
        }
    }

    FerricCommandLine {
        id: secondaryCommandBar
        browserWindow: rootWindow
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

    Rectangle {
        id: secondarySearchBar
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: rootWindow.inputBarHeight
        z: 20
        visible: secondaryUi.mode === "search"
        color: rootWindow.surfaceColor

        Label {
            id: secondarySearchPrefix
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            width: rootWindow.inputBarHeight
            text: secondaryUi.search_backward ? "?" : "/"
            color: rootWindow.warningColor
            font.bold: true
            horizontalAlignment: Text.AlignHCenter
            Accessible.ignored: true
        }

        TextField {
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
            color: rootWindow.primaryTextColor
            selectionColor: rootWindow.selectionColor
            selectedTextColor: rootWindow.selectionTextColor
            placeholderText: "search"
            placeholderTextColor: rootWindow.mutedTextColor
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
        browserWindow: window
        statusVisible: secondaryWindow.statusBarVisible
        mode: secondaryUi.mode
        displayUrl: secondaryUi.display_url
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

    function attachTransferredView(view, payload, sourceUi, sourceHost, operationId) {
        if (!view || typeof payload !== "string" || payload.length === 0) {
            return false
        }
        var sourceUrl = view.url ? view.url.toString() : "about:blank"
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
        secondaryWindow.windowTransferView = view
        secondaryWindow.windowTransferSourceUi = sourceUi
        secondaryWindow.windowTransferSourceHost = sourceHost
        secondaryWindow.windowTransferPayload = payload
        view.viewUi = secondaryUi
        view.viewTabs = null
        view.viewModel = {
            loaded: true,
            url: transferData.url || "about:blank",
            title: transferData.title || "New tab",
            muted: !!transferData.muted,
            zoom: Number(transferData.zoom_hundredths || 100) / 100.0,
            scrollX: -1,
            scrollY: -1
        }
        view.viewTransferred = true
        if (view.viewHost !== undefined) {
            view.viewHost = secondaryWindow
        }
        view.tabIndex = -1
        view.parent = transferViewHost
        view.anchors.fill = transferViewHost
        view.visible = true
        if (view.viewIsSecondaryStatic === true) {
            view.url = sourceUrl
        }
        rootWindow.updateBrowserWindowView(secondaryUi, view)
        if (!rootWindow.completeDetachedSource(sourceUi, sourceHost, payload)) {
            view.parent = null
            view.visible = false
            secondaryUi.rollback_tab_transfer(adoptedTabId)
            rootWindow.restoreSourceView(sourceUi, sourceHost, view)
            resetAfterTransferSource()
            return false
        }
        if (operationId && operationId.length > 0 && sourceUi) {
            sourceUi.complete_transfer_operation(operationId, true)
        }
        if (secondaryWindow.windowTransferOperationId.length > 0
                && secondaryWindow.windowTransferSourceUi) {
            secondaryWindow.windowTransferSourceUi.complete_transfer_operation(
                        secondaryWindow.windowTransferOperationId, true)
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
            if (secondaryUi.profile_bootstrap_pending) {
                return
            }
            stop()
            if (!secondaryUi.queue_bookmark_transfer(
                        secondaryWindow.windowBookmarkTransferUrl,
                        secondaryWindow.windowBookmarkTransferTitle)) {
                secondaryUi.status_text = "Bookmark transfer failed"
            }
            secondaryWindow.windowBookmarkTransferUrl = ""
            secondaryWindow.windowBookmarkTransferTitle = ""
        }
    }

    Timer {
        id: secondaryVersioningBindTimer
        interval: 25
        repeat: true
        onTriggered: {
            if (secondaryUi.profile_bootstrap_pending) {
                return
            }
            stop()
            rootWindow.refreshBrowserWindowIdentity(secondaryUi)
            if (!secondaryWindow.windowWebEngineProfile
                    || !secondaryUi.bind_native_versioning_profile()) {
                secondaryUi.status_text = "Versioning native profile binding unavailable"
                if (secondaryWindow.windowCleanBranch) {
                    secondaryWindow.visible = false
                    Qt.callLater(function() { secondaryWindow.destroy() })
                }
            } else if (secondaryWindow.windowCleanBranch) {
                secondaryWindow.windowCleanBranchNativeBound = true
            } else if (secondaryWindow.windowRestoreCommitId.length > 0) {
                secondaryUi.request_versioned_pristine_restore(
                                secondaryWindow.windowRestoreCommitId,
                                secondaryWindow.windowRestoreMediaPositionEnabled,
                                secondaryWindow.windowRestoreEditableDraftsEnabled)
                secondaryWindow.windowRestoreCommitId = ""
                secondaryWindow.windowRestoreMediaPositionEnabled = false
                secondaryWindow.windowRestoreEditableDraftsEnabled = false
            }
        }
    }

    Component.onCompleted: {
        secondaryWindow.windowBoundBranchToken = secondaryWindow.windowBranchToken
        secondaryWindow.windowBranchTokenLocked = true
        if (secondaryWindow.windowCleanBranch
                && (secondaryWindow.windowPrivateProfile
                    || secondaryWindow.windowEphemeralProfile
                    || secondaryWindow.windowSharedProfile
                    || secondaryWindow.windowTransferMode
                    || secondaryWindow.windowRestoreCommitId.length > 0
                    || secondaryWindow.windowStartupContext.length > 0
                    || secondaryWindow.windowStartupContextRestore
                    || secondaryWindow.windowBookmarkTransferUrl.length > 0
                    || secondaryWindow.windowStartupUrl !== "about:blank")) {
            secondaryWindow.visible = false
            Qt.callLater(function() { secondaryWindow.destroy() })
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
        rootWindow.installFocusObserver(secondaryWindow.activeView)
        secondaryUi.set_startup_configuration(
                    rootWindow.primaryBrowserUi.config_json, rootWindow.primaryBrowserUi.config_base_json,
                    rootWindow.primaryBrowserUi.cli_overrides_json,
                    secondaryUi.profile_overrides_json,
                    rootWindow.primaryBrowserUi.config_path, rootWindow.primaryBrowserUi.config_source)
        secondaryUi.set_contexts_configuration(rootWindow.primaryBrowserUi.contexts_json)
        secondaryUi.configure_profile(
                                      secondaryWindow.windowPrivateProfile,
                                      secondaryWindow.windowEphemeralProfile
                                      || secondaryWindow.windowCleanBranch,
                                      secondaryWindow.windowProfileLabel,
                                      secondaryWindow.windowProfileName,
                                      rootWindow.storageBasePath)
        secondaryVersioningBindTimer.start()
        if (secondaryWindow.windowBookmarkTransferUrl.length > 0) {
            bookmarkTransferTimer.start()
        }
        secondaryUi.set_context_entry_reuse(secondaryWindow.windowStartupContextRestore)
        if (secondaryWindow.windowCleanBranch
                && !rootWindow.retainCleanBranchOwner(
                        secondaryWindow.windowBoundBranchToken, secondaryWindow,
                        secondaryProfile, secondaryView)) {
            secondaryUi.status_text = "Clean branch profile isolation unavailable"
            secondaryWindow.visible = false
            Qt.callLater(function() { secondaryWindow.destroy() })
            return
        }
        if (!secondaryWindow.windowCleanBranch)
            rootWindow.registerBrowserWindow(
                        secondaryWindow, secondaryUi, secondaryWindow.activeView,
                        secondaryWindow.windowWebEngineProfile,
                        secondaryWindow.windowProfileName,
                        secondaryWindow.windowEphemeralInvocationToken,
                        secondaryWindow.windowEphemeralProfile)
        var interceptorAttached = secondaryWindow.windowSharedProfile
                || secondaryRequestInterceptor.attach(secondaryProfile)
        if (!interceptorAttached) {
            secondaryUi.status_text = "Request interceptor unavailable"
            if (secondaryWindow.windowCleanBranch) {
                secondaryWindow.visible = false
                Qt.callLater(function() { secondaryWindow.destroy() })
                return
            }
        }
        if (secondaryWindow.windowTransferMode) {
            if (!secondaryWindow.attachTransferredView(
                        secondaryWindow.windowTransferView,
                        secondaryWindow.windowTransferPayload,
                        secondaryWindow.windowTransferSourceUi,
                        secondaryWindow.windowTransferSourceHost,
                        secondaryWindow.windowTransferOperationId)) {
                secondaryUi.status_text = "Live tab adoption failed"
                if (secondaryWindow.windowTransferOperationId.length > 0
                        && secondaryWindow.windowTransferSourceUi) {
                    secondaryWindow.windowTransferSourceUi.complete_transfer_operation(
                                secondaryWindow.windowTransferOperationId, false)
                    secondaryWindow.windowTransferOperationId = ""
                }
                return
            }
        } else if (secondaryWindow.windowRestoreCommitId.length > 0) {
            // Keep the fresh destination inert until its own profile worker
            // validates the exact commit and admits the restore plan.
        } else if (secondaryWindow.windowStartupContext.length > 0) {
            secondaryUi.execute_command(
                        "context-enter " + secondaryWindow.windowStartupContext)
            rootWindow.routeContextWorkspace(secondaryUi)
        }
        if (secondaryWindow.windowCleanBranch) {
            // A prepared child must stay inert until Rust admits its fork and
            // exact owner. No caller-supplied startup URL may navigate here.
        } else if (secondaryWindow.windowStartupContextRestore) {
            secondaryWindow.applySwitcherEngineAction()
        } else if (secondaryWindow.windowStartupRoutePreflighted) {
            secondaryUi.navigate_without_context_route(
                        secondaryWindow.windowStartupUrl)
        } else {
            secondaryUi.navigate_initial(
                        secondaryWindow.windowStartupUrl, "external-open", false)
        }
    }
    Component.onDestruction: {
        if (secondaryWindow.branchExportOperation.length > 0
                && secondaryWindow.windowBranchParentUi) {
            secondaryWindow.windowBranchParentUi
                .cancel_versioned_clean_branch_export(
                    secondaryWindow.branchExportOperation)
            secondaryWindow.branchExportOperation = ""
        }
        if (secondaryWindow.versioningRestoreSidecarRequestId.length > 0) {
            secondaryUi.discard_versioned_restore_sidecar(
                secondaryWindow.versioningRestoreSidecarRequestId)
            secondaryWindow.versioningRestoreSidecarRequestId = ""
        }
        if (secondaryWindow.devToolsWindow) {
            secondaryWindow.devToolsWindow.close()
            secondaryWindow.devToolsWindow = null
        }
        rootWindow.unregisterBrowserWindow(secondaryUi)
            rootWindow.clearPageDialogForView(secondaryView)
            rootWindow.clearClientCertificateForView(secondaryView)
            rootWindow.clearCertificateErrorForView(secondaryView)
            rootWindow.clearWebAuthForView(secondaryView)
            rootWindow.clearContextMenuForView(secondaryView)
            rootWindow.clearDesktopMediaForView(secondaryView)
            rootWindow.clearCaptureSessionForView(secondaryView)
rootWindow.clearCaptureSessionsForHost(secondaryWindow)
        secondaryWindow.clearFileDialogForView(secondaryView)
        rootWindow.clearSiteDataClearForView(secondaryView)
        rootWindow.cancelPermissionForUi(secondaryUi)
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
        secondaryUi.view_closed()
        if (secondaryWindow.windowCleanBranch) {
            if (secondaryWindow.windowCleanBranchLineagePublished
                    && secondaryWindow.windowBranchParentUi) {
                secondaryWindow.windowBranchParentUi.close_versioned_clean_branch(
                            secondaryUi, secondaryWindow.windowBoundBranchToken)
            } else if (secondaryWindow.windowBranchParentUi) {
                secondaryWindow.windowBranchParentUi.abort_versioned_clean_branch(
                            secondaryWindow.windowBoundBranchToken)
            }
            rootWindow.releaseCleanBranchOwner(
                        secondaryWindow.windowBoundBranchToken, secondaryWindow,
                        secondaryProfile, secondaryView)
        }
        secondaryUi.release_transient_resources()
    }
}

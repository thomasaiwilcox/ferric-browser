import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import QtWebEngine
import io.github.ferricbrowser 1.0
import "../scripts/KeyboardPresentation.js" as KeyboardPresentation
import "../scripts/SpellcheckPresentation.js" as SpellcheckPresentation
import "../scripts/NativePageCaptureCallbacks.js" as NativePageCaptureCallbacks

FerricBrowserRuntimeChrome {
    id: window

    permissionPromptSurfaceComponentObject: permissionPromptSurfaceComponent
    captureIndicatorComponentObject: captureIndicatorComponent
    desktopMediaSurfaceComponentObject: desktopMediaSurfaceComponent
    rendererFailureSurfaceComponentObject: rendererFailureSurfaceComponent

    readonly property var commandSurface: window.commandSurfaceObject
    readonly property var searchSurface: window.searchSurfaceObject
    readonly property var attachedDevToolsLoader: window.attachedDevToolsLoaderObject
    readonly property alias versioningRestoreCoordinatorObject: versioningRestoreCoordinator
    property bool profileBootstrapComplete: false

    FerricVersioningRestoreHost {
        id: versioningRestoreCoordinator
        hostWindow: window
        browserUi: window.primaryBrowserUi
    }

    function configuredBlocklistIds() {
        return browserUi.feature_blocking_list_ids || []
    }

    function requestBlocklistUpdate() {
        if (window.temporaryProfile || window.storageBasePath.length === 0) {
            return false
        }
        if (blocklistUpdater.busy) {
            return false
        }
        var configuredLists = window.configuredBlocklistIds()
        if (configuredLists.length === 0) {
            browserUi.status_text = "No supported blocklists configured"
            return false
        }
        window.blocklistInstallFailed = false
        window.blocklistInstallError = ""
        if (!blocklistUpdater.update(configuredLists, window.storageBasePath)) {
            browserUi.status_text = blocklistUpdater.status
            return false
        }
        return true
    }

    function configuredLinkCleaningUpdate() {
        return {
            source: browserUi.feature_link_cleaning_update_source,
            checksum: browserUi.feature_link_cleaning_update_sha256
        }
    }

    function requestLinkCleaningUpdate(automatic) {
        automatic = automatic === true
        if (window.temporaryProfile || window.storageBasePath.length === 0) {
            if (!automatic) {
                browserUi.status_text = "Clean-link updates require a normal profile"
            }
            return false
        }
        if (linkRuleUpdater.busy) {
            return false
        }
        var intervalMs = window.linkCleaningUpdateIntervalHours * 60 * 60 * 1000
        var now = Date.now()
        if (automatic && window.linkCleaningLastUpdateAt > 0
                && now - window.linkCleaningLastUpdateAt < intervalMs) {
            linkCleaningUpdateTimer.restart()
            return false
        }
        var configured = window.configuredLinkCleaningUpdate()
        if (!configured.source || !configured.checksum) {
            if (!automatic) {
                browserUi.status_text = "No explicit clean-link update source configured"
            }
            return false
        }
        window.linkRuleInstallFailed = false
        window.linkRuleInstallError = ""
        if (!linkRuleUpdater.update(
                    configured.source, configured.checksum, window.storageBasePath)) {
            if (!automatic) {
                browserUi.status_text = linkRuleUpdater.status
            }
            return false
        }
        window.linkCleaningLastUpdateAt = now
        return true
    }

    function configureLinkCleaningUpdateTimer() {
        var configured = window.configuredLinkCleaningUpdate()
        window.linkCleaningUpdateConfigured = configured.source.length > 0
                && configured.checksum.length > 0
        if (window.linkCleaningUpdateConfigured
                && !window.temporaryProfile
                && window.storageBasePath.length > 0) {
            linkCleaningUpdateTimer.restart()
        } else {
            linkCleaningUpdateTimer.stop()
        }
    }

    function configureBlocklistUpdateTimer() {
        window.blocklistUpdateIntervalHours = browserUi.feature_blocking_update_interval_hours
        blocklistUpdateTimer.restart()
    }

    function closeTabAtIndex(index) {
        var view = tabViewAt(index)
        if (index < 0 || !view) {
            return false
        }
        if (window.windowCleanBranch && window.windowCleanBranchPublished
                && view === window.windowBranchInitialView) {
            browserUi.status_text = "The first clean-branch tab anchors its profile; close the branch window"
            return false
        }
        var capture = window.nativePageCaptureSave
        if (capture && capture.view === view) {
            // Qt's profile-level downloadRequested event contains a pointer to
            // the originating WebEngine page. Keep that page alive until Qt
            // has delivered the event and QML has retained the exact request;
            // otherwise destroying the tab can leave Qt with a stale page
            // pointer while it dispatches the queued signal.
            if (String(capture.downloadId || "").length === 0) {
                var pendingClose = window.pendingNativePageCaptureClose
                if (pendingClose && pendingClose.view !== view) {
                    browserUi.status_text = "Another tab close is waiting for native archive request registration"
                    return false
                }
                window.pendingNativePageCaptureClose = {
                    tabId: String(capture.tabId),
                    path: String(capture.path || ""),
                    view: view,
                    requestRegistered: false
                }
                browserUi.status_text = "Tab close is waiting for native archive request registration"
                window.scheduleRuntimeWork(0)
                return true
            }
            if (!window.retireNativePageCaptureForClosingView(view)) {
                browserUi.status_text = "Tab close is waiting for native archive callback cleanup"
                return false
            }
        }
        window.clearPageDialogForView(view)
        window.clearClientCertificateForView(view)
        window.clearCertificateErrorForView(view)
        window.clearWebAuthForView(view)
        window.clearContextMenuForView(view)
        window.cancelPermissionForTab(index)
        window.clearFileDialogForView(view)
        if (!browserUi.close_tab(index)) {
            return false
        }
        window.removeTabViewAt(index, null)
        tabs.remove(index)
        if (window.pendingNativePageCaptureClose
                && window.pendingNativePageCaptureClose.view === view) {
            window.pendingNativePageCaptureClose = null
        }
        if (browserUi.tab_count > tabs.count) {
            tabs.append({ url: browserUi.initial_url, title: "New tab",
                          pinned: false, muted: false, zoom: 1.0,
                          suspended: false, discarded: false })
        }
        window.syncTabModel()
        return true
    }

    function executeHistoryTraversal(view, backwards, requestedCount) {
        var count = Number(requestedCount)
        if (!Number.isFinite(count) || count < 1 || count > 100) {
            count = 1
        }
        var moved = 0
        for (var index = 0; index < count; ++index) {
            var canMove = backwards ? view.canGoBack : view.canGoForward
            if (!canMove) {
                break
            }
            if (backwards) {
                view.goBack()
            } else {
                view.goForward()
            }
            moved += 1
        }
        if (moved === 0) {
            browserUi.status_text = "History boundary reached"
        } else if (moved < count) {
            browserUi.status_text = "History boundary reached after " + moved + " step(s)"
        }
    }

    function executePendingEngineAction() {
        var action = browserUi.take_engine_action()
        if (action === "context-save-request") {
            window.saveContextMembership(browserUi)
            return
        }
        if (action === "context-commit-request") {
            window.commitSavedContext(browserUi)
            return
        }
        if (action.indexOf("command-prefill\t") === 0) {
            commandSurface.commandText = action.slice("command-prefill\t".length)
            commandSurface.cursorPosition = commandSurface.commandText.length
            commandSurface.focusInput()
            return
        }
        if (action === "quit-request") {
            window.beginQuitRequest(":quit")
            return
        }
        if (action === "window-close-request") {
            window.beginQuitRequest(":window-close")
            return
        }
        if (action.indexOf("window-focus\t") === 0) {
            var focusParts = action.split("\t")
            var focusTarget = focusParts.length >= 2 ? focusParts[1] : ""
            var focusOperationId = focusParts.length >= 3 ? focusParts[2] : ""
            var focusEntry = window.browserWindowEntryForTarget(focusTarget)
            var restoreFocus = false
            if (!focusEntry) {
                focusEntry = window.versioningWindowOwner.entryForOwnerToken(focusTarget)
                restoreFocus = !!focusEntry
            }
            if (!focusEntry || !focusEntry.host || !focusEntry.ui) {
                browserUi.status_text = "Window focus target is stale"
                if (focusOperationId.length > 0) {
                    browserUi.complete_window_focus(focusOperationId, "stale")
                }
                return
            }
            if (!window.activateBrowserWindow(
                        focusEntry.host, restoreFocus ? browserUi : focusEntry.ui,
                        focusOperationId)) {
                focusEntry.ui.status_text = "Window activation outcome unknown"
            } else {
                focusEntry.ui.status_text = "Window focused"
            }
            return
        }
        if (action.indexOf("window-move\t") === 0) {
            var moveParts = action.split("\t")
            var moveTarget = moveParts.length >= 2 ? moveParts[1] : ""
            var moveWorkspace = moveParts.length >= 3 ? moveParts[2] : ""
            var moveEntry = window.browserWindowEntryForTarget(moveTarget)
            if (!moveEntry || !moveEntry.host || !moveEntry.ui || !moveWorkspace) {
                browserUi.status_text = "Window move target is stale"
                return
            }
            if (!window.activateBrowserWindow(moveEntry.host, moveEntry.ui)) {
                moveEntry.ui.status_text = "Window activation outcome unknown; move was not attempted"
            } else if (!moveEntry.host.active) {
                moveEntry.ui.status_text = "Window activation was denied; move was not attempted"
            } else if (!moveEntry.ui.hyprland_move_active_window(moveWorkspace)) {
                moveEntry.ui.status_text = "Compositor denied moving the window"
            }
            return
        }
        if (action.indexOf("tab-suspend-request\t") === 0) {
            var suspendRequestParts = action.split("\t")
            if (suspendRequestParts.length >= 2) {
                window.requestTabSuspension(suspendRequestParts[1])
            }
            return
        }
        if (action.indexOf("tab-discard-request\t") === 0) {
            var discardRequestParts = action.split("\t")
            if (discardRequestParts.length >= 2) {
                window.requestTabDiscard(discardRequestParts[1])
            }
            return
        }
        if (action.indexOf("tab-suspend\t") === 0) {
            var suspendParts = action.split("\t")
            var suspendId = suspendParts.length >= 2 ? suspendParts[1] : ""
            var suspendIndex = browserUi.tab_index_for_id(suspendId)
            var suspendView = tabViewAt(suspendIndex)
            if (!suspendView || suspendIndex === browserUi.active_tab_index) {
                browserUi.status_text = "Tab suspension target is no longer hidden"
                return
            }
            suspendView.lifecycleState = WebEngineView.LifecycleState.Frozen
            tabs.setProperty(suspendIndex, "suspended", true)
            browserUi.status_text = "Tab suspended; engine lifecycle frozen"
            return
        }
        if (action.indexOf("tab-discard\t") === 0) {
            var discardParts = action.split("\t")
            var discardId = discardParts.length >= 2 ? discardParts[1] : ""
            var discardIndex = browserUi.tab_index_for_id(discardId)
            var discardView = tabViewAt(discardIndex)
            if (!discardView || discardIndex === browserUi.active_tab_index) {
                browserUi.status_text = "Tab discard target is no longer hidden"
                return
            }
            discardView.lifecycleState = WebEngineView.LifecycleState.Discarded
            tabs.setProperty(discardIndex, "suspended", false)
            tabs.setProperty(discardIndex, "discarded", true)
            browserUi.status_text = "Tab discarded; resume will reload the page"
            return
        }
        if (action.indexOf("tab-resume\t") === 0) {
            var resumeParts = action.split("\t")
            var resumeId = resumeParts.length >= 2 ? resumeParts[1] : ""
            var resumeIndex = browserUi.tab_index_for_id(resumeId)
            var resumeView = tabViewAt(resumeIndex)
            if (!resumeView) {
                browserUi.status_text = "Tab resume target is stale"
                return
            }
            var wasDiscarded = resumeIndex >= 0 && resumeIndex < tabs.count
                    && tabs.get(resumeIndex).discarded
            resumeView.lifecycleState = WebEngineView.LifecycleState.Active
            if (resumeIndex >= 0 && resumeIndex < tabs.count) {
                tabs.setProperty(resumeIndex, "suspended", false)
                tabs.setProperty(resumeIndex, "discarded", false)
            }
            if (wasDiscarded) {
                resumeView.reload()
            }
            browserUi.status_text = wasDiscarded
                    ? "Tab resumed; discarded page reload requested"
                    : "Tab resumed; page state is live"
            return
        }
        if (action.indexOf("tab-mute\t") === 0) {
            var muteParts = action.split("\t")
            var muteId = muteParts.length >= 2 ? muteParts[1] : ""
            var muteIndex = browserUi.tab_index_for_id(muteId)
            var muted = muteParts.length >= 3 && muteParts[2] === "true"
            if (muteIndex < 0 || muteIndex >= tabs.count) {
                browserUi.status_text = "Tab mute target is stale"
                return
            }
            tabs.setProperty(muteIndex, "muted", muted)
            browserUi.status_text = muted ? "Tab muted" : "Tab unmuted"
            return
        }
        if (action.indexOf("permission-reset\t") === 0) {
            var permissionParts = action.split("\t")
            if (permissionParts.length >= 3) {
                var permissionOrigin = permissionParts[1]
                if (permissionParts[2] === "notifications") {
                    window.closeWebNotificationsForOrigin(permissionOrigin)
                }
                var reloaded = window.reloadViewsForPermission(permissionOrigin)
                browserUi.status_text = reloaded > 0
                        ? "Permission revoked; reloaded " + reloaded + " matching view(s)"
                        : "Permission revoked; no matching active view"
            }
            return
        }
        if (action.indexOf("print-pdf\t") === 0) {
            var pdfParts = action.split("\t")
            if (pdfParts.length >= 3) {
                var pdfIndex = Number(pdfParts[1])
                var pdfView = tabViewAt(pdfIndex)
                var pdfPath = browserUi.prepare_print_pdf(pdfParts.slice(2).join("\t"))
                if (pdfView && pdfPath.length > 0) {
                    pdfView.printToPdf(pdfPath)
                } else {
                    browserUi.finish_print_pdf(pdfParts.slice(2).join("\t"), false)
                }
            }
            return
        }
        if (action.indexOf("save-page\t") === 0) {
            var saveParts = action.split("\t")
            if (saveParts.length >= 3) {
                var saveIndex = Number(saveParts[1])
                var saveView = tabViewAt(saveIndex)
                var savePath = browserUi.prepare_save_page(saveParts.slice(2).join("\t"))
                if (saveView && savePath.length > 0) {
                    saveView.save(savePath, WebEngineDownloadRequest.MimeHtmlSaveFormat)
                    browserUi.status_text = "Page save requested; completion is tracked in Downloads"
                } else {
                    browserUi.take_save_page_path()
                    browserUi.status_text = "Page save target is stale"
                }
            }
            return
        }
        if (action.indexOf("view-source\t") === 0) {
            var sourceParts = action.split("\t")
            if (sourceParts.length >= 3) {
                var sourceIndex = Number(sourceParts[1])
                var sourceView = tabViewAt(sourceIndex)
                if (sourceView) {
                    sourceView.url = "view-source:" + sourceParts.slice(2).join("\t")
                    browserUi.status_text = "Viewing page source"
                } else {
                    browserUi.status_text = "Source view target is stale"
                }
            }
            return
        }
        if (action === "jseval") {
            var evalIndex = browserUi.tab_index_for_id(browserUi.jseval_tab_id)
            var evalView = tabViewAt(evalIndex)
            var evalScript = browserUi.jseval_script
            var evalWorld = browserUi.jseval_world === "page"
                    ? WebEngineScript.MainWorld : window.browserScriptWorld
            if (!evalView || evalScript.length === 0 || evalScript.length > 65536) {
                browserUi.status_text = "JavaScript evaluation target is stale or invalid"
                return
            }
            window.runBrowserScript(evalView, evalScript, function() {
                browserUi.status_text = "JavaScript evaluation completed"
            }, evalWorld)
            return
        }
        if (action.indexOf("devtools\t") === 0) {
            var devtoolsParts = action.split("\t")
            window.toggleDevTools(devtoolsParts.length >= 2 && devtoolsParts[1] === "true")
            return
        }
        if (action.indexOf("print\t") === 0) {
            var printParts = action.split("\t")
            var printView = printParts.length >= 2
                    ? tabViewAt(Number(printParts[1])) : null
            var printPath = browserUi.prepare_print_job()
            if (printPath.length > 0 && printView) {
                printView.printToPdf(function(success) {
                    browserUi.finish_print_job(printPath, success)
                })
            } else {
                browserUi.finish_print_job(printPath, false)
            }
            return
        }
        if (action === "show-downloads") {
            window.openInternalSurface()
            window.downloadManagerVisible = true
            window.refreshDownloads()
            return
        }
        if (action === "show-switcher") {
            window.showSwitcherWith(
                String(browserUi.switcher_request_scope || "all"),
                String(browserUi.switcher_request_query || ""))
            return
        }
        if (action.indexOf("tab-close\t") === 0) {
            var tabCloseParts = action.split("\t")
            var tabCloseId = tabCloseParts.length >= 2 ? tabCloseParts[1] : ""
            var tabCloseIndex = browserUi.tab_index_for_id(tabCloseId)
            if (tabCloseIndex < 0) {
                browserUi.status_text = "Tab close target is stale"
                return
            }
            if (!window.closeTabAtIndex(tabCloseIndex)) {
                browserUi.status_text = "Tab close was rejected"
                return
            }
            return
        }
        if (action.indexOf("tab-close-active\t") === 0) {
            var tabCloseActiveParts = action.split("\t")
            var tabCloseCount = Number(tabCloseActiveParts[1])
            if (!Number.isFinite(tabCloseCount) || tabCloseCount < 1 || tabCloseCount > 100) {
                browserUi.status_text = "Tab close count is invalid"
                return
            }
            var closedCount = 0
            for (var tabCloseAttempt = 0; tabCloseAttempt < tabCloseCount; ++tabCloseAttempt) {
                var candidateIndex = -1
                var activeTabIndex = browserUi.active_tab_index
                if (activeTabIndex >= 0 && activeTabIndex < tabs.count
                        && !tabs.get(activeTabIndex).pinned) {
                    candidateIndex = activeTabIndex
                } else {
                    for (var tabCloseCandidate = 0;
                         tabCloseCandidate < tabs.count;
                         ++tabCloseCandidate) {
                        if (!tabs.get(tabCloseCandidate).pinned) {
                            candidateIndex = tabCloseCandidate
                            break
                        }
                    }
                }
                if (candidateIndex < 0 || !window.closeTabAtIndex(candidateIndex)) {
                    break
                }
                closedCount += 1
            }
            browserUi.status_text = closedCount > 0
                    ? "Closed " + closedCount + " tab" + (closedCount === 1 ? "" : "s")
                    : "No unpinned tabs available to close"
            return
        }
        if (action.indexOf("tab-give\t") === 0
                || action.indexOf("tab-move-context\t") === 0) {
            var giveSource = window.focusedBrowserWindowEntry()
            window.executeBrowserTransferAction(
                        giveSource ? giveSource.ui : browserUi,
                        giveSource ? giveSource.host : window,
                        action)
            return
        }
        if (action === "tab-detach" || action.indexOf("tab-detach\t") === 0) {
            var detachSource = window.focusedBrowserWindowEntry()
            window.executeBrowserTransferAction(
                        detachSource ? detachSource.ui : browserUi,
                        detachSource ? detachSource.host : window,
                        action)
            return
        }
        if (action.indexOf("reopen-window\t") === 0) {
            window.openReopenedWindow(browserUi, window, action)
            return
        }
        if (action.indexOf("versioning-restore-window\t") === 0) {
            window.openVersionedRestoreWindow(browserUi, window, action)
            return
        }
        if (action.indexOf("versioning-clean-branch-ready\t") === 0) {
            window.openVersionedCleanBranch(browserUi, window, action)
            return
        }
        if (action.indexOf("versioning-branch-switch\t") === 0) {
            if (!window.activateVersionedBranchTarget(
                        browserUi, action.slice("versioning-branch-switch\t".length)))
                browserUi.status_text = "Clean branch switch refused: target is stale or unavailable"
            return
        }
        if (action.indexOf("versioning-sidecar-ready\t") === 0) {
            var allowEditable = window.versioningRestoreEditableReviewEnabled
            window.versioningRestoreEditableReviewEnabled = false
            versioningRestoreCoordinator.handleSidecarReady(action, allowEditable)
            return
        }
        if (action.indexOf("reopen-window-confirm\t") === 0) {
            window.showReopenWindowConfirmation(browserUi, window, action)
            return
        }
        if (action.indexOf("zoom\t") === 0) {
            var zoomParts = action.split("\t")
            if (zoomParts.length >= 3) {
                var zoomTabIndex = browserUi.tab_index_for_id(zoomParts[1])
                var zoomFactor = Number(zoomParts[2])
                if (zoomTabIndex >= 0 && Number.isFinite(zoomFactor)
                        && zoomFactor >= 0.25 && zoomFactor <= 5.0) {
                    tabs.setProperty(zoomTabIndex, "zoom", zoomFactor)
                    var zoomView = tabViewAt(zoomTabIndex)
                    if (zoomView) {
                        zoomView.zoomFactor = zoomFactor
                    }
                }
            }
            return
        }
        if (action === "blocklist-update") {
            window.requestBlocklistUpdate()
            return
        }
        if (action === "link-cleaning-update") {
            window.requestLinkCleaningUpdate(false)
            return
        }
        if (action === "show-site-ledger") {
            window.showSiteLedger()
            return
        }
        if (action.indexOf("site-data-clear\t") === 0) {
            var siteDataParts = action.split("\t")
            var siteDataOrigin = siteDataParts.length >= 2 ? siteDataParts[1] : ""
            var siteDataView = window.activeWebView()
            if (!siteDataView || !siteDataOrigin) {
                browserUi.site_data_clear_finished(
                            siteDataOrigin,
                            JSON.stringify({"error": "active view unavailable"}))
                return
            }
            window.siteDataClearOrigin = siteDataOrigin
            window.siteDataClearView = siteDataView
            window.siteDataClearUi = browserUi
            window.siteDataClearPending = true
            siteDataView.runJavaScript(window.siteDataClearScript(), window.browserScriptWorld)
            return
        }
        if (action === "show-diagnostics") {
            window.showDiagnostics()
            return
        }
        if (action === "show-binding-help") {
            window.showBindingHelp()
            return
        }
        if (action.indexOf("show-binding-help\t") === 0) {
            window.showBindingHelp(action.split("\t").slice(1).join("\t"))
            return
        }
        if (action.indexOf("show-session-preview\t") === 0) {
            window.showSessionPreview(action.split("\t").slice(1).join("\t"), false)
            return
        }
        if (action === "show-settings") {
            window.showSettings()
            return
        }
        if (action.indexOf("show-settings\t") === 0) {
            window.showSettings(action.split("\t").slice(1).join("\t"))
            return
        }
        if (action.indexOf("site-doctor-reload\t") === 0) {
            var doctorParts = action.split("\t")
            var doctorIndex = doctorParts.length >= 2 ? Number(doctorParts[1]) : -1
            var doctorView = tabViewAt(doctorIndex)
            if (!doctorView) {
                browserUi.finish_site_doctor_experiment(
                    doctorParts.length >= 3 ? doctorParts[2] : "", false)
                browserUi.status_text = "Site Doctor target tab is unavailable"
                return
            }
            if (window.siteDoctorUserscriptsDisabled(browserUi)) {
                window.clearInstalledPageUserscripts(doctorView)
            }
            doctorView.reload()
            browserUi.status_text = "Site Doctor experiment running"
            return
        }
        if (action.indexOf("site-doctor-fresh-view\t") === 0) {
            var freshParts = action.split("\t")
            var freshIndex = freshParts.length >= 2 ? Number(freshParts[1]) : -1
            window.syncTabModel()
            var freshView = tabViewAt(freshIndex)
            if (!freshView || freshParts.length < 4) {
                browserUi.finish_site_doctor_experiment(
                    freshParts.length >= 3 ? freshParts[2] : "", false)
                Qt.callLater(function() { window.executePendingEngineAction() })
                browserUi.status_text = "Site Doctor fresh-view target is unavailable"
                return
            }
            tabs.setProperty(freshIndex, "loaded", false)
            tabs.setProperty(freshIndex, "url", freshParts.slice(3).join("\t"))
            tabs.setProperty(freshIndex, "loaded", true)
            browserUi.status_text = "Site Doctor fresh view running"
            return
        }
        if (action.indexOf("site-doctor-close\t") === 0) {
            var closeParts = action.split("\t")
            var closeIndex = closeParts.length >= 2 ? Number(closeParts[1]) : -1
            if (!window.closeTabAtIndex(closeIndex)) {
                browserUi.status_text = "Site Doctor could not close its temporary view"
                return
            }
            browserUi.status_text = "Site Doctor temporary view closed"
            return
        }
        if (action === "blocking-toggle-site") {
            browserUi.toggle_blocking_site()
            return
        }
        if (action.indexOf("download-open\t") === 0 || action.indexOf("download-show\t") === 0) {
            var desktopAction = action.split("\t")
            if (desktopAction.length >= 2) {
                window.openExternalUri(browserUi, desktopAction.slice(1).join("\t"))
            }
            return
        }
        if (action.indexOf("external-open\t") === 0) {
            var externalAction = action.split("\t")
            if (externalAction.length >= 2) {
                window.openExternalUri(browserUi, externalAction.slice(1).join("\t"))
            }
            return
        }
        if (action.indexOf("download-cancel\t") === 0) {
            var cancelParts = action.split("\t")
            var pendingDownload = window.activeDownloads[cancelParts[1]]
            if (pendingDownload) {
                window.resolveQtRequest(browserUi, pendingDownload, "download", "cancel", [])
            }
            return
        }
        if (action.indexOf("download-pause\t") === 0 || action.indexOf("download-resume\t") === 0) {
            var pauseParts = action.split("\t")
            var pauseDownload = window.activeDownloads[pauseParts[1]]
            if (pauseDownload) {
                if (action.indexOf("download-pause\t") === 0) {
                    pauseDownload.pause()
                } else {
                    pauseDownload.resume()
                }
            }
            return
        }
        if (action.indexOf("download-retry\t") === 0) {
            var retryParts = action.split("\t")
            var retryView = window.activeWebView()
            if (retryParts.length >= 2 && retryView) {
                window.runBrowserScript(retryView, window.downloadLinkScript(retryParts.slice(1).join("\t")))
            }
            return
        }
        if (action.indexOf("fullscreen\t") === 0) {
            var fullscreenState = action.split("\t")[1] || "toggle"
            var currentlyFullscreen = window.visibility === Window.FullScreen
            var enterFullscreen = fullscreenState === "on"
                    || (fullscreenState === "toggle" && !currentlyFullscreen)
            if (enterFullscreen) {
                window.showFullScreen()
                browserUi.status_text = "Fullscreen enabled"
            } else {
                window.showNormal()
                browserUi.status_text = "Fullscreen ended"
            }
            return
        }
        if (action.indexOf("new-window\t") === 0) {
            var windowAction = action.split("\t")
            if (windowAction.length >= 5) {
                window.browserWindowFactory.createObject(null, {
                    windowStartupUrl: windowAction.slice(4).join("\t"),
                    windowProfileName: windowAction[2],
                    windowProfileLabel: windowAction[2],
                    windowStartupContext: windowAction[3],
                    windowPrivateProfile: windowAction[1] === "true"
                })
            } else if (windowAction.length >= 4) {
                window.browserWindowFactory.createObject(null, {
                    windowStartupUrl: windowAction.slice(3).join("\t"),
                    windowProfileName: windowAction[2],
                    windowProfileLabel: windowAction[2],
                    windowPrivateProfile: windowAction[1] === "true"
                })
            }
            return
        }
        if (action.indexOf("profile-delete\t") === 0) {
            var profileDeleteAction = action.split("\t")
            if (profileDeleteAction.length >= 2) {
                window.showProfileDeletePreview(profileDeleteAction.slice(1).join("\t"))
            }
            return
        }
        if (action.indexOf("profile-window\t") === 0) {
            var profileWindowAction = action.split("\t")
            if (profileWindowAction.length >= 4) {
                window.openProfile(
                            profileWindowAction[1],
                            profileWindowAction[2],
                            profileWindowAction.slice(3).join("\t"))
            }
            return
        }
        if (action.indexOf("ephemeral-window\t") === 0) {
            window.openEphemeralWindow(action.split("\t").slice(1).join("\t"))
            return
        }
        if (action.indexOf("context-window\t") === 0) {
            var contextWindowAction = action.split("\t")
            if (contextWindowAction.length >= 5) {
                window.browserWindowFactory.createObject(null, {
                    windowStartupUrl: contextWindowAction.slice(4).join("\t"),
                    windowProfileName: contextWindowAction[2],
                    windowProfileLabel: contextWindowAction[2],
                    windowStartupContext: contextWindowAction[3],
                    windowStartupContextRestore: true,
                    windowPrivateProfile: contextWindowAction[1] === "true"
                })
            }
            return
        }
        if (action.indexOf("navigate\t") === 0) {
            var navigation = action.split("\t")
            if (navigation.length >= 3) {
                var navigationIndex = Number(navigation[1])
                var navigationView = tabViewAt(navigationIndex)
                if (navigationView) {
                    tabs.setProperty(navigationIndex, "loaded", true)
                    navigationView.url = navigation.slice(2).join("\t")
                }
            }
            return
        }
        if (action.indexOf("scroll-target\t") === 0) {
            var scrollTargetParts = action.split("\t")
            var scrollTargetIndex = scrollTargetParts.length === 3
                    ? browserUi.tab_index_for_id(scrollTargetParts[1]) : -1
            var scrollTargetView = tabViewAt(scrollTargetIndex)
            window.applyScrollTargetAction(
                browserUi, scrollTargetView,
                scrollTargetParts.length === 3 ? scrollTargetParts[2] : "",
                window)
            return
        }
        var webView = window.activeWebView()
        if (!webView) {
            return
        }
        if (action === "back" || action.indexOf("back\t") === 0) {
            var backParts = action.split("\t")
            window.executeHistoryTraversal(webView, true,
                                           backParts.length >= 2 ? backParts[1] : 1)
        } else if (action === "forward" || action.indexOf("forward\t") === 0) {
            var forwardParts = action.split("\t")
            window.executeHistoryTraversal(webView, false,
                                           forwardParts.length >= 2 ? forwardParts[1] : 1)
        } else if (action === "reload" || action.indexOf("reload\t") === 0) {
            var reloadParts = action.split("\t")
            if (reloadParts[1] === "true") {
                webView.reloadAndBypassCache()
            } else {
                webView.reload()
            }
        } else if (action === "stop") {
            webView.stop()
        } else if (action.indexOf("find\t") === 0) {
            var findRequest = action.split("\t")
            var flags = window.searchFindFlags(
                browserUi.search_text, findRequest[2], findRequest[1] === "true")
            webView.findText(browserUi.search_text, flags)
        } else if (action.indexOf("find-next\t") === 0) {
            var findParts = action.split("\t")
            var findCount = Number(findParts[1])
            var findBackward = findParts[2] === "true"
            if (Number.isFinite(findCount) && findCount >= 1 && findCount <= 100) {
                var findFlags = window.searchFindFlags(
                    browserUi.search_text, findParts[3], findBackward)
                for (var findIndex = 0; findIndex < findCount; ++findIndex) {
                    webView.findText(browserUi.search_text, findFlags)
                }
            }
        } else if (action.indexOf("scroll\t") === 0
                   || action.indexOf("scroll-page\t") === 0
                   || action.indexOf("scroll-to\t") === 0) {
            var scrollParts = action.split("\t")
            if (scrollParts[0] === "scroll" && scrollParts.length >= 3) {
                window.runBrowserScript(webView, window.scrollScript(
                    "scroll", scrollParts[1], false, Number(scrollParts[2])))
            } else if (scrollParts[0] === "scroll-page" && scrollParts.length >= 4) {
                window.runBrowserScript(webView, window.scrollScript(
                    "scroll-page", scrollParts[1], scrollParts[2] === "true",
                    Number(scrollParts[3])))
            } else if (scrollParts[0] === "scroll-to" && scrollParts.length >= 2) {
                window.runBrowserScript(webView, window.scrollScript(
                    "scroll-to", scrollParts[1], false, 1))
            }
            window.scheduleScrollCapture()
        } else if (action === "clear-find") {
            webView.findText("")
        }
    }

    function activeWebView() {
        return tabViewAt(browserUi.active_tab_index)
    }

    function nativePageCaptureViewForTab(tabId) {
        var expected = String(tabId || "")
        if (expected.length === 0)
            return null
        for (var index = 0; index < browserUi.tab_count; ++index) {
            if (String(browserUi.tab_id_for_index(index)) !== expected)
                continue
            var view = tabViewAt(index)
            try {
                return view && String(view.stableTabId) === expected ? view : null
            } catch (error) {
                return null
            }
        }
        return null
    }

    function nativePageCaptureTabIdForView(candidateView) {
        if (!candidateView)
            return ""
        for (var index = 0; index < browserUi.tab_count; ++index) {
            var view = tabViewAt(index)
            if (view !== candidateView)
                continue
            var expected = String(browserUi.tab_id_for_index(index) || "")
            try {
                return expected.length > 0 && String(view.stableTabId) === expected
                        ? expected : ""
            } catch (error) {
                return ""
            }
        }
        return ""
    }

    function updateActiveTabUrl() {
        var active = browserUi.active_tab_index
        if (active >= 0 && active < tabs.count) {
            tabs.setProperty(active, "url", browserUi.initial_url)
        }
    }

    function applyRestorePayload(payload, hasModeLine) {
        var lines = payload.split("\n")
        var append = hasModeLine && lines.shift() === "append"
        var previousCount = tabs.count
        if (!append) {
            tabs.clear()
            previousCount = 0
        }
        for (var i = 0; i < lines.length; ++i) {
            if (lines[i].length === 0) {
                continue
            }
            var fields = lines[i].split("\t")
            var tabIndex = previousCount + i
            tabs.append({
                url: fields[0],
                title: fields.length > 4 && fields[4].length > 0 && !hasModeLine
                       ? fields[4]
                       : (i === 0 ? "Recovered tab" : "Recovered tab " + (i + 1)),
                loaded: tabIndex === browserUi.active_tab_index,
                pinned: fields.length > 1 && fields[1] === "true",
                muted: fields.length > 2 && fields[2] === "true",
                zoom: fields.length > 3 ? Number(fields[3]) : 1.0,
                scrollX: hasModeLine && fields.length > 4 ? Number(fields[4]) : -1,
                scrollY: hasModeLine && fields.length > 5 ? Number(fields[5]) : -1,
                suspended: false,
                discarded: false
            })
        }
    }

    function syncTabModel() {
        Qt.callLater(window.tabStripObject.refreshModelPresentation)
        var pending = browserUi.take_session_restore_values()
        if (pending.length > 0) {
            window.applyRestorePayload(pending, true)
        }
        var coreIds = []
        for (var coreIndex = 0; coreIndex < browserUi.tab_count; ++coreIndex) {
            coreIds.push(browserUi.tab_id_for_index(coreIndex))
        }
        for (var retainedIndex = 0; retainedIndex < coreIds.length; ++retainedIndex) {
            if (window.versioningViewHostObject.hasRetainedTab(coreIds[retainedIndex])) {
                // The core must remove retained membership from the visible
                // tab projection before reconciliation may create a view.
                // Never substitute a fresh page for a retained handle.
                browserUi.status_text = "Retained tab is still in the visible projection"
                return
            }
        }
        // Logical cleanup may close a blank fallback before the native host
        // has acknowledged restoration. Do not let generic reconciliation
        // destroy that protected QObject while it still owns the hold.
        for (var protectedIndex = 0; protectedIndex < tabViews.length; ++protectedIndex) {
            var protectedView = tabViews[protectedIndex]
            if (coreIds.indexOf(protectedView.stableTabId) < 0
                    && window.versioningViewHostObject.isPreparedView(protectedView)) {
                browserUi.status_text = "Live stash view cleanup remains pending"
                return
            }
        }
        var aligned = pending.length === 0
                && tabs.count === coreIds.length
                && tabViews.length === coreIds.length
        if (aligned) {
            for (var alignedIndex = 0; alignedIndex < coreIds.length; ++alignedIndex) {
                if (tabs.get(alignedIndex).tabId !== coreIds[alignedIndex]
                        || tabViews[alignedIndex].stableTabId !== coreIds[alignedIndex]) {
                    aligned = false
                    break
                }
            }
        }
        var activeBeforeSync = browserUi.active_tab_index
        if (aligned && activeBeforeSync >= 0 && activeBeforeSync < tabs.count
                && !tabs.get(activeBeforeSync).loaded) {
            aligned = false
        }
        if (aligned) {
            window.syncLazyRestoreLabels()
            return
        }
        var identitySignature = coreIds.join("\n")
        if (window.tabIdentitySignature.length > 0
                && window.tabIdentitySignature !== identitySignature) {
            window.noteTabActivity()
        }
        window.tabIdentitySignature = identitySignature
        for (var modelIndex = 0; modelIndex < coreIds.length; ++modelIndex) {
            var wantedId = coreIds[modelIndex]
            var foundIndex = -1
            for (var searchIndex = modelIndex; searchIndex < tabs.count; ++searchIndex) {
                if (tabs.get(searchIndex).tabId === wantedId) {
                    foundIndex = searchIndex
                    break
                }
            }
            if (foundIndex >= 0) {
                if (foundIndex !== modelIndex) {
                    tabs.move(foundIndex, modelIndex, 1)
                }
            } else if (modelIndex < tabs.count
                       && String(tabs.get(modelIndex).tabId || "").length === 0) {
                tabs.setProperty(modelIndex, "tabId", wantedId)
            } else {
                tabs.insert(modelIndex, {
                    tabId: wantedId,
                    url: "about:blank",
                    title: "New tab",
                    loaded: true,
                    pinned: false,
                    muted: false,
                    zoom: 1.0,
                    suspended: false,
                    discarded: false
                })
            }
        }
        while (tabs.count > coreIds.length) {
            tabs.remove(tabs.count - 1)
        }
        var active = browserUi.active_tab_index
        if (active >= 0 && active < tabs.count && !tabs.get(active).loaded) {
            tabs.setProperty(active, "loaded", true)
        }
        var remainingViews = window.versioningViewHostObject.visibleViewsOnly(tabViews)
        var orderedViews = []
        for (var index = 0; index < tabs.count; ++index) {
            var tabId = coreIds[index]
            var view = null
            for (var viewSearch = 0; viewSearch < remainingViews.length; ++viewSearch) {
                if (remainingViews[viewSearch].stableTabId === tabId) {
                    view = remainingViews.splice(viewSearch, 1)[0]
                    break
                }
            }
            if (!view && remainingViews.length > 0
                    && String(remainingViews[0].stableTabId || "").length === 0) {
                view = remainingViews.shift()
            }
            if (!view) {
                view = window.versioningViewHostObject.createOwnedView(
                            window.webViewFactory, window.webViewsObject, {
                                tabIndex: index,
                                stableTabId: tabId,
                                viewUi: browserUi,
                                viewTabs: tabs,
                                viewModel: tabs.get(index),
                                viewProfile: browserProfile,
                                viewInterceptor: requestInterceptor,
                                viewTransientProfile: window.temporaryProfile,
                                viewTransferred: false
                            })
            }
            if (!view) {
                browserUi.status_text = "Could not create tab view"
                break
            }
            view.stableTabId = tabId
            orderedViews.push(view)
        }
        for (var staleIndex = 0; staleIndex < remainingViews.length; ++staleIndex) {
            remainingViews[staleIndex].destroy()
        }
        tabViews = orderedViews
        for (var viewIndex = 0; viewIndex < tabViews.length; ++viewIndex) {
            tabViews[viewIndex].tabIndex = viewIndex
            tabViews[viewIndex].viewModel = tabs.get(viewIndex)
        }
        if (tabViews.length !== tabs.count || tabs.count !== browserUi.tab_count) {
            browserUi.status_text = "Tab model/view invariant failed"
            console.error("tab count invariant failed", tabViews.length,
                          tabs.count, browserUi.tab_count)
            return
        }
        for (var verifyIndex = 0; verifyIndex < tabs.count; ++verifyIndex) {
            if (tabs.get(verifyIndex).tabId !== coreIds[verifyIndex]
                    || tabViews[verifyIndex].stableTabId !== coreIds[verifyIndex]) {
                browserUi.status_text = "Tab identity invariant failed"
                console.error("tab identity invariant failed", verifyIndex)
                return
            }
        }
        if (window.windowCleanBranch && window.windowCleanBranchPublished) {
            if (tabViews.indexOf(window.windowBranchInitialView) < 0) {
                window.windowBranchGroupViews = []
                browserUi.status_text = "Clean branch first-view lease is no longer current"
                return
            }
            window.windowBranchGroupViews = tabViews.filter(function(view) {
                return view !== window.windowBranchInitialView
            })
        }
        window.syncLazyRestoreLabels()
    }

    function syncLazyRestoreLabels() {
        for (var index = 0; index < tabs.count; ++index) {
            if (browserUi.has_lazy_restore_page(index)
                    && tabs.get(index).title !== "Restored page — select to load") {
                tabs.setProperty(index, "title", "Restored page — select to load")
            }
        }
    }

    function reconciledStashProjection() {
        window.syncTabModel()
        var projectedIds = []
        for (var i = 0; i < browserUi.tab_count; ++i) {
            projectedIds.push(browserUi.tab_id_for_index(i))
        }
        var activeIndex = browserUi.active_tab_index
        if (activeIndex < 0 || activeIndex >= projectedIds.length
                || tabViews.length !== projectedIds.length
                || tabs.count !== projectedIds.length) {
            return null
        }
        return { ids: projectedIds, activeId: projectedIds[activeIndex] }
    }

    // Materialization acknowledges only native identity, profile and blank
    // fallback presentation; it does not hide or freeze the source.
    function materializeReservedNativeFallback(sourceTabId, fallbackTabId) {
        var projection = window.reconciledStashProjection()
        return projection !== null
                && window.versioningViewHostObject.hasMaterializedReservedFallback(
                    sourceTabId, fallbackTabId,
                    projection.ids, tabViews, projection.activeId)
    }

    // The admitted ID selects the source. Current focus is never substituted
    // for it, and the host check does not hide or freeze the view.
    function acknowledgeAdmittedNativeStashStart(operationId, sourceTabId) {
        var projection = window.reconciledStashProjection()
        if (projection === null
                || !window.versioningViewHostObject.hasAdmittedVisibleSource(
                    sourceTabId, projection.ids, tabViews)) {
            return browserUi.refuse_native_stash_start(operationId)
        }
        if (browserUi.acknowledge_native_stash_start(operationId, sourceTabId)) {
            return true
        }
        return browserUi.refuse_native_stash_start(operationId)
    }

    // Capture the exact QObject while the logical fallback is still merely
    // materialized. A later Rust ticket may only hide this held source.
    function holdReservedNativeStash(viewId, sourceTabId, fallbackTabId) {
        var projection = window.reconciledStashProjection()
        return projection !== null
                && window.versioningViewHostObject.holdReservedStash(
                    viewId, sourceTabId, fallbackTabId,
                    projection.ids, tabViews, projection.activeId)
    }

    // Invoked only after an application-owned stage ticket is installed.
    function prepareReservedNativeStash(viewId, sourceTabId, fallbackTabId) {
        var projection = window.reconciledStashProjection()
        if (projection === null) {
            return false
        }
        return window.versioningViewHostObject.prepareHeldStash(
                    viewId, sourceTabId, fallbackTabId,
                    projection.ids, tabViews, projection.activeId)
    }

    function scheduleRuntimeWork(delay) {
        var boundedDelay = Math.max(0, Math.min(60000, Number(delay) || 0))
        if (!runtimeWorkTimer.running || boundedDelay < runtimeWorkTimer.interval) {
            runtimeWorkTimer.interval = Math.max(1, boundedDelay)
            runtimeWorkTimer.restart()
        }
    }

    function processPendingNativePageCaptureClose() {
        var pending = window.pendingNativePageCaptureClose
        if (!pending || !pending.view)
            return false
        var index = browserUi.tab_index_for_id(pending.tabId)
        if (index < 0 || tabViewAt(index) !== pending.view) {
            window.pendingNativePageCaptureClose = null
            return false
        }
        var capture = window.nativePageCaptureSave
        if (capture && capture.view === pending.view
                && String(capture.downloadId || "").length > 0) {
            pending.requestRegistered = true
        }
        if (pending.requestRegistered !== true)
            return false
        if (capture && capture.view === pending.view
                && capture.downloadFinished === true) {
            return false
        }
        window.pendingNativePageCaptureClose = null
        if (!window.closeTabAtIndex(index)) {
            if (!window.pendingNativePageCaptureClose)
                window.pendingNativePageCaptureClose = pending
            return false
        }
        if (window.shutdownWaitingForDownloadFinishes
                && !window.hasActiveDownloads()) {
            window.shutdownWaitingForDownloadFinishes = false
            window.shutdownPromptVisible = false
            window.continueQuitRequest()
        }
        return true
    }

    function processRuntimeWork() {
            if (window.instanceRequestOwner) {
                window.dispatchRoutedIpc(browserUi.poll_ipc())
            }
            window.syncTabModel()
            window.processPendingNativePageCaptureClose()
            var admittedStashStart = browserUi.pending_native_stash_start()
            if (admittedStashStart.length === 2) {
                if (window.acknowledgeAdmittedNativeStashStart(
                        admittedStashStart[0], admittedStashStart[1])) {
                    window.scheduleRuntimeWork(0)
                }
            }
            window.nativeStashDisclosure = browserUi.pending_native_stash_disclosure()
            window.nativePageCaptureDisclosure =
                    browserUi.pending_native_page_capture_disclosure()
            var closeAwaitingProfileRequest = window.pendingNativePageCaptureClose
                    && window.pendingNativePageCaptureClose.requestRegistered !== true
            if (!closeAwaitingProfileRequest) {
                var pageCaptureSave = browserUi.pending_native_page_capture_save()
                if (pageCaptureSave.length === 2 && window.nativePageCaptureSave
                        && String(pageCaptureSave[0])
                        === String(window.nativePageCaptureSave.operationId)
                        && String(pageCaptureSave[1])
                        !== String(window.nativePageCaptureSave.tabId)
                        && window.nativePageCaptureSave.downloadFinished === true) {
                    var completedPage = window.nativePageCaptureSave
                    if (NativePageCaptureCallbacks.retireCaptureIdentities(
                                window, completedPage, [])) {
                        pageCaptureMediaKeyTimer.stop()
                        window.nativePageCaptureSave = null
                        window.discardNativePageCaptureMutationProbe(completedPage)
                    } else {
                        NativePageCaptureCallbacks.failClosedRetirement(
                                    window, browserUi, completedPage, [],
                                    "Native page-save callback capacity is exhausted; window capture was stopped",
                                    false)
                    }
                }
                if (pageCaptureSave.length === 2 && window.nativePageCaptureSave
                        && String(pageCaptureSave[0])
                        !== String(window.nativePageCaptureSave.operationId)) {
                    if (window.retireSupersededNativePageCapture(
                            window.nativePageCaptureSave, String(pageCaptureSave[0]))) {
                        pageCaptureCollectionTimer.stop()
                        pageCaptureMediaKeyTimer.stop()
                    }
                }
                if (pageCaptureSave.length === 2 && !window.nativePageCaptureSave) {
                    window.startNativePageCaptureSave(pageCaptureSave[0], pageCaptureSave[1])
                }
                if (window.nativePageCaptureSave
                        && !browserUi.has_native_page_capture_request()) {
                    pageCaptureCollectionTimer.stop()
                    pageCaptureMediaKeyTimer.stop()
                    var retiredCapture = window.nativePageCaptureSave
                    if (retiredCapture.downloadFinished !== true
                            && !NativePageCaptureCallbacks.retireCaptureIdentities(
                                window, retiredCapture, [])) {
                        NativePageCaptureCallbacks.failClosedRetirement(
                                    window, browserUi, retiredCapture,
                                    retiredCapture.download ? [retiredCapture.download] : [],
                                    "Native page-save callback capacity is exhausted; window capture was stopped")
                    } else {
                        window.nativePageCaptureSave = null
                        window.discardNativePageCaptureMutationProbe(retiredCapture)
                        if (retiredCapture.downloadFinished !== true
                                && retiredCapture.download) {
                            try { retiredCapture.download.cancel() } catch (error) {}
                        }
                    }
                }
            }
            var pendingStash = browserUi.pending_native_stash_preparation()
            if (pendingStash.length === 3) {
                if (!window.materializeReservedNativeFallback(
                            pendingStash[1], pendingStash[2])
                        || !browserUi.acknowledge_native_fallback_materialized(
                            pendingStash[0])
                        || !window.holdReservedNativeStash(
                            pendingStash[0], pendingStash[1], pendingStash[2])) {
                    browserUi.abort_native_stash_preparation(pendingStash[0])
                } else if (!browserUi.acknowledge_native_stash_hold_installed(
                               pendingStash[0])
                           && !browserUi.abort_native_stash_held_preparation(
                               pendingStash[0])) {
                    browserUi.status_text = "Live stash hold cleanup remains pending"
                }
                window.syncTabModel()
            }
            window.nativeStashReservationPending = browserUi.has_native_stash_preparation()
            window.nativeVersioningOperationPending = browserUi.has_active_versioning_operations()
            window.executePendingEngineAction()
            browserUi.poll_config()
            window.maybeOpenPendingEngineFileDialog()
            window.maybeOpenPendingDesktopMediaRequest()
            browserUi.refresh_operations()
            browserUi.tick_bindings()
            browserUi.tick_site_doctor_experiment()
            browserUi.checkpoint_session()
            if (browserUi.take_caret_request()) {
                var caretToken = browserUi.caret_request_token
                var caretOperation = browserUi.caret_request_operation
                var caretView = window.activeWebView()
                if (caretView) {
                    window.runBrowserScript(caretView, window.caretScript(caretOperation, window.caretSelecting), function(value) {
                        var response = value || {error: "caret script returned no result"}
                        if (response.selecting !== undefined) {
                            window.caretSelecting = response.selecting
                        }
                        browserUi.deliver_caret(caretToken, JSON.stringify(response))
                    })
                } else {
                    browserUi.deliver_caret(caretToken, JSON.stringify({error: "document view unavailable"}))
                }
            }
            var editorRequest = browserUi.take_editor_request()
            if (editorRequest.length > 0) {
                var editorView = window.activeWebView()
                if (editorView) {
                    window.runBrowserScript(editorView, window.editorScript(), function(value) {
                        browserUi.deliver_editor(editorRequest, JSON.stringify(value || {error: "editor script returned no result"}))
                    })
                } else {
                    browserUi.deliver_editor(editorRequest, JSON.stringify({error: "document view unavailable"}))
                }
            }
            if (browserUi.take_editor_completion()) {
                var editorToken = browserUi.editor_completion_token
                var editorOriginal = browserUi.editor_completion_original
                var editorUpdated = browserUi.editor_completion_updated
                var editorError = browserUi.editor_completion_error
                var applyView = window.activeWebView()
                if (editorError.length > 0) {
                    browserUi.deliver_editor_apply(editorToken, JSON.stringify({error: "external-editor-failed"}))
                } else if (applyView) {
                    window.runBrowserScript(applyView, window.editorApplyScript(editorOriginal, editorUpdated), function(value) {
                        browserUi.deliver_editor_apply(editorToken, JSON.stringify(value || {error: "editor apply returned no result"}))
                    })
                } else {
                    browserUi.deliver_editor_apply(editorToken, JSON.stringify({error: "document view unavailable"}))
                }
            }
            var selectionToken = browserUi.take_selection_request()
            if (selectionToken.length > 0) {
                var selectionView = window.activeWebView()
                if (selectionView) {
                    window.runBrowserScript(selectionView, window.selectionScript(), function(value) {
                        browserUi.deliver_selection(selectionToken, JSON.stringify(value))
                    })
                } else {
                    browserUi.deliver_selection(selectionToken, JSON.stringify({error: "document view unavailable"}))
                }
            }
            if (browserUi.take_download_request()) {
                var downloadView = window.activeWebView()
                if (downloadView) {
                    window.runBrowserScript(downloadView, window.downloadLinkScript(browserUi.download_request_url), function() {
                        browserUi.complete_download_request(browserUi.download_request_token, true)
                    })
                } else {
                    browserUi.complete_download_request(browserUi.download_request_token, false)
                }
            }
            var clipboardRequest = browserUi.take_clipboard_request()
            if (clipboardRequest.length > 0) {
                window.copyToClipboard(
                    clipboardRequest,
                    !browserUi.clipboard_request_sensitive,
                    browserUi.clipboard_request_primary)
            }
            window.syncTabModel()
            window.executePendingEngineAction()
            if (window.shutdownWaitingForDownloadFinishes) {
                if (!window.hasActiveDownloads()
                        && !browserUi.has_pending_versioning_object_work()) {
                    window.shutdownWaitingForDownloadFinishes = false
                    window.shutdownPromptVisible = false
                    window.continueQuitRequest()
                } else if (!window.hasActiveDownloads()) {
                    window.scheduleRuntimeWork(50)
                }
            }
            var nextDelay = browserUi.maintenance_delay_ms()
            if (nextDelay >= 0) {
                window.scheduleRuntimeWork(nextDelay)
            }
    }

    function startNativePageCaptureSave(operationId, tabId) {
        var expectedTab = String(tabId || "")
        var view = window.nativePageCaptureViewForTab(expectedTab)
        var maxArchiveBytes = Number(browserUi.native_page_capture_archive_limit_bytes())
        if (NativePageCaptureCallbacks.captureTabIsPoisoned(window, expectedTab)) {
            browserUi.cancel_native_page_capture_request(String(operationId || ""))
            browserUi.status_text = "Page archiving is unavailable for this tab after an interrupted native save"
            return false
        }
        if (!window.nativePageCaptureRetirementCapacityAvailable()) {
            browserUi.cancel_native_page_capture_request(String(operationId || ""))
            browserUi.status_text = window.nativePageCaptureRetirementExhausted
                    ? "Page saves are blocked in this window because callback capacity was exhausted"
                    : "Page archive capture is unavailable until pending native save callbacks retire"
            return false
        }
        if (!Number.isSafeInteger(maxArchiveBytes) || maxArchiveBytes <= 0) {
            browserUi.cancel_native_page_capture_request(String(operationId || ""))
            browserUi.status_text = "Page archive capture is unavailable because its size limit could not be read"
            return false
        }
        if (expectedTab.length === 0 || !view
                || window.temporaryProfile) {
            var pageFailed = expectedTab.length > 0 && !window.temporaryProfile
                    && browserUi.fail_native_window_capture_page(
                           String(operationId || ""), expectedTab,
                           "native-view-lost", "", "")
            if (!pageFailed) {
                browserUi.cancel_native_page_capture_request(String(operationId || ""))
            }
            window.scheduleRuntimeWork(0)
            return false
        }
        var capturedUrl = ""
        try { capturedUrl = String(view.url || "") } catch (error) {}
        if (capturedUrl.length === 0) {
            var missingUrlFailed = browserUi.fail_native_window_capture_page(
                        String(operationId || ""), expectedTab,
                        "native-view-lost", "", "")
            if (!missingUrlFailed)
                browserUi.cancel_native_page_capture_request(String(operationId || ""))
            window.scheduleRuntimeWork(0)
            return false
        }
        if (!window.versioningWindowOwner
                || !window.versioningWindowOwner.profilePageCaptureAvailable(
                       window.browserProfile, window)) {
            browserUi.cancel_native_page_capture_request(String(operationId || ""))
            browserUi.status_text = "Another page archive save is active for this profile"
            window.scheduleRuntimeWork(0)
            return false
        }
        var capture = {
            operationId: String(operationId),
            tabId: expectedTab,
            maxArchiveBytes: maxArchiveBytes,
            profile: window.browserProfile,
            path: "",
            view: view,
            capturedUrl: capturedUrl,
            downloadId: "",
            collectionFinished: false,
            collectionTimedOut: false,
            sidecarAccepted: false,
            mediaKeyStatus: "not-requested",
            mediaKeyWaitAttempts: 0,
            collectorStarted: false,
            collectorStartFailed: false,
            collectorEntryPointAvailable: false,
            collectorStartErrorName: "none",
            collectorStartErrorLine: -1,
            collectorReferenceErrorSymbol: "none",
            collectorStartDurationMs: 0,
            collectorCallbackReceived: false,
            collectorResultType: "not-received",
            collectorResultLength: 0,
            mutationProbe: null,
            mutationEvidenceSubmitted: false
        }
        window.nativePageCaptureSave = capture
        window.collectNativePageCaptureSidecar(capture)
        return true
    }

    function collectNativePageCaptureSidecar(capture) {
        if (!capture || capture !== window.nativePageCaptureSave
                || capture.collectionFinished) {
            return false
        }
        var view = capture.view
        var currentView = window.nativePageCaptureViewForTab(capture.tabId)
        if (!view || currentView !== view) {
            window.finishNativePageCaptureCollection(capture, "")
            return false
        }
        var challenge = browserUi.native_page_capture_sidecar_request(
                    capture.operationId, capture.tabId)
        if (challenge && challenge.length === 1
                && String(challenge[0]) === "pending-media-key") {
            capture.mediaKeyWaitAttempts = 0
            capture.mediaKeyStatus = "pending"
            pageCaptureMediaKeyTimer.restart()
            return true
        }
        if (!challenge || challenge.length !== 8) {
            try {
                capture.mediaKeyStatus = String(
                            browserUi.native_page_capture_media_key_status())
            } catch (error) {}
            window.finishNativePageCaptureCollection(capture, "")
            return false
        }
        capture.mediaRequested = String(challenge[7]) === "true"
        capture.mediaKeyStatus = capture.mediaRequested ? "ready" : "not-requested"
        var request = {
            expected_origin: String(challenge[0]),
            expected_route: String(challenge[1]),
            route_fingerprint: String(challenge[2]),
            capture_fingerprint: String(challenge[3]),
            nonce: String(challenge[4]),
            max_ms: 50,
            include_controls: true,
            include_editable: String(challenge[5]) === "true",
            include_selection: String(challenge[6]) === "true",
            include_media: String(challenge[7]) === "true"
        }
        pageCaptureCollectionTimer.restart()
        var collectorStart = Date.now()
        var collectorStarted = false
        capture.collectorEntryPointAvailable =
                typeof window.collectVersionedFrameTree === "function"
        try {
            collectorStarted = window.collectVersionedFrameTree(view, request, function(rawResult, mutationProbe) {
                capture.collectorCallbackReceived = true
                capture.collectorResultType = typeof rawResult
                capture.collectorResultLength = typeof rawResult === "string"
                        ? Math.min(rawResult.length, 262145) : 0
                if (window.nativePageCaptureSave !== capture
                        || capture.collectionFinished === true
                        || capture.mutationEvidenceSubmitted === true) {
                    if (typeof mutationProbe === "function")
                        mutationProbe(function() {})
                    return
                }
                capture.mutationProbe = typeof mutationProbe === "function"
                        ? mutationProbe : null
                window.finishNativePageCaptureCollection(capture, String(rawResult || ""))
            })
        } catch (error) {
            capture.collectorStartFailed = true
            var errorName = error && typeof error.name === "string" ? error.name : ""
            capture.collectorStartErrorName = ["Error", "TypeError", "ReferenceError",
                                               "RangeError", "SyntaxError"].indexOf(errorName) >= 0
                    ? errorName : "other"
            var errorLine = Number(error && error.lineNumber)
            capture.collectorStartErrorLine = Number.isInteger(errorLine)
                    && errorLine >= 0 && errorLine <= 100000 ? errorLine : -1
            if (errorName === "ReferenceError") {
                var undefinedSymbol = /^([A-Za-z_$][A-Za-z0-9_$]{0,63}) is not defined$/.exec(
                            String(error.message || ""))
                if (undefinedSymbol)
                    capture.collectorReferenceErrorSymbol = undefinedSymbol[1]
            }
        }
        capture.collectorStartDurationMs = Math.min(60000, Date.now() - collectorStart)
        capture.collectorStarted = collectorStarted
        if (!collectorStarted) {
            window.finishNativePageCaptureCollection(capture, "")
        }
        return true
    }

    function finishNativePageCaptureCollection(capture, sidecarJson) {
        if (!capture || capture !== window.nativePageCaptureSave
                || capture.collectionFinished) {
            return false
        }
        capture.collectionFinished = true
        pageCaptureCollectionTimer.stop()
        pageCaptureMediaKeyTimer.stop()
        var currentView = null
        try { currentView = window.nativePageCaptureViewForTab(capture.tabId) } catch (error) {}
        var captureStatus = NativePageCaptureCallbacks.preSaveCaptureStatus(
                    capture, currentView)
        if (captureStatus !== "current") {
            window.discardNativePageCaptureMutationProbe(capture)
            window.nativePageCaptureSave = null
            var continued = browserUi.fail_native_window_capture_page(
                        capture.operationId, capture.tabId, captureStatus, "", "")
            if (!continued)
                browserUi.cancel_native_page_capture_request(capture.operationId)
            browserUi.status_text = captureStatus === "document-changed"
                    ? "Page changed while its archive was being prepared; the stale capture was discarded"
                    : "The page view was lost while its archive was being prepared"
            window.scheduleRuntimeWork(0)
            return false
        }
        var boundedSidecar = String(sidecarJson || "")
        if (boundedSidecar.length > 0 && boundedSidecar.length <= 256 * 1024) {
            capture.sidecarAccepted = browserUi.complete_native_page_capture_sidecar(
                        capture.operationId, boundedSidecar)
        }
        var path = String(browserUi.claim_native_page_capture_save(
                              capture.operationId, capture.tabId,
                              capture.capturedUrl) || "")
        if (path.length === 0) {
            window.discardNativePageCaptureMutationProbe(capture)
            window.nativePageCaptureSave = null
            browserUi.cancel_native_page_capture_request(capture.operationId)
            window.scheduleRuntimeWork(0)
            return false
        }
        capture.path = path
        try {
            capture.view.save(path, WebEngineDownloadRequest.MimeHtmlSaveFormat)
        } catch (error) {
            var saveFailureView = null
            try {
                saveFailureView = window.nativePageCaptureViewForTab(capture.tabId)
            } catch (viewError) {}
            var saveFailureStatus = NativePageCaptureCallbacks.preSaveCaptureStatus(
                        capture, saveFailureView)
            var saveFailureReason = saveFailureStatus === "current"
                    ? "invalid-archive" : saveFailureStatus
            if (!NativePageCaptureCallbacks.retireCaptureIdentities(
                        window, capture, [])) {
                NativePageCaptureCallbacks.failClosedRetirement(
                            window, browserUi, capture, [],
                            "Native page-save callback capacity is exhausted; window capture was stopped",
                            false)
                return false
            }
            window.discardNativePageCaptureMutationProbe(capture)
            window.nativePageCaptureSave = null
            if (!browserUi.fail_native_window_capture_page(
                           capture.operationId, capture.tabId,
                           saveFailureReason, path, "")) {
                browserUi.cancel_native_page_capture_request(capture.operationId)
            }
            browserUi.status_text = saveFailureReason === "document-changed"
                    ? "Page changed before its archive save could start; the stale capture was discarded"
                    : saveFailureReason === "native-view-lost"
                        ? "The page view was lost before its archive save could start"
                        : "Page archive save could not be started"
            window.scheduleRuntimeWork(0)
            return false
        }
        return true
    }

    Timer {
        id: runtimeWorkTimer
        interval: 1
        repeat: false
        running: false
        onTriggered: window.processRuntimeWork()
    }

    Timer {
        id: pageCaptureCollectionTimer
        interval: 2000
        repeat: false
        onTriggered: {
            var capture = window.nativePageCaptureSave
            if (capture)
                capture.collectionTimedOut = true
            window.finishNativePageCaptureCollection(capture, "")
        }
    }

    Timer {
        id: pageCaptureMediaKeyTimer
        interval: 50
        repeat: false
        onTriggered: {
            var capture = window.nativePageCaptureSave
            if (!capture || capture.collectionFinished)
                return
            var keyStatus = "unavailable"
            try {
                keyStatus = String(browserUi.native_page_capture_media_key_status())
            } catch (error) {}
            capture.mediaKeyStatus = keyStatus
            if (keyStatus === "ready") {
                window.collectNativePageCaptureSidecar(capture)
                return
            }
            if (keyStatus === "pending") {
                capture.mediaKeyWaitAttempts =
                        Math.max(0, Number(capture.mediaKeyWaitAttempts) || 0) + 1
                if (capture.mediaKeyWaitAttempts < 100) {
                    restart()
                    return
                }
            }
            // Missing or unavailable profile keys omit only the optional
            // media sidecar. Never collect an unkeyed media URL.
            window.finishNativePageCaptureCollection(capture, "")
        }
    }

    property int focusProbeRetries: 0

    function scheduleFocusProbe(retries) {
        window.focusProbeRetries = Math.max(window.focusProbeRetries,
                                            Math.max(1, Number(retries) || 1))
        focusProbeTimer.restart()
    }

    Timer {
        id: focusProbeTimer
        interval: 40
        repeat: false
        onTriggered: {
            var focusView = window.activeWebView()
            if (focusView && browserUi.active_tab_index >= 0) {
                window.probeActiveFocus(
                    browserUi, browserUi.active_tab_index, focusView)
            }
            window.focusProbeRetries = Math.max(0, window.focusProbeRetries - 1)
            if (window.focusProbeRetries > 0) {
                restart()
            }
        }
    }

    function scheduleScrollCapture() {
        scrollCaptureTimer.restart()
    }

    Timer {
        id: scrollCaptureTimer
        interval: 180
        repeat: false
        onTriggered: {
            var index = browserUi.active_tab_index
            var scrollView = tabViewAt(index)
            if (index >= 0 && scrollView && tabs.get(index).loaded) {
                window.captureScrollPosition(browserUi, index, scrollView)
            }
        }
    }

    Shortcut {
        sequence: "Ctrl+Shift+Escape"
        onActivated: {
            if (window.visibility === Window.FullScreen) {
                window.showNormal()
                browserUi.status_text = "Fullscreen ended"
            } else {
                browserUi.escape()
            }
        }
    }

    function logicalNormalKeyText(event) {
        if (!event) {
            return ""
        }
        var control = !!(event.modifiers & Qt.ControlModifier)
        var shift = !!(event.modifiers & Qt.ShiftModifier)
        var alt = !!(event.modifiers & Qt.AltModifier)
        var meta = !!(event.modifiers & Qt.MetaModifier)
        if (alt && !control && !shift && !meta) {
            if (event.key >= Qt.Key_1 && event.key <= Qt.Key_9) {
                return "Alt+" + String.fromCharCode(event.key)
            }
            if (event.key === Qt.Key_M) {
                return "Alt+m"
            }
            return ""
        }
        if (control && alt && !shift && !meta && event.key === Qt.Key_P) {
            return "Ctrl+Alt+p"
        }
        if (control && !alt && !meta) {
            if (event.key === Qt.Key_6 || event.key === Qt.Key_AsciiCircum) {
                return "Ctrl+^"
            }
            if (shift) {
                if (event.key === Qt.Key_N) {
                    return "Ctrl+Shift+n"
                }
                if (event.key === Qt.Key_T) {
                    return "Ctrl+Shift+t"
                }
                if (event.key === Qt.Key_W) {
                    return "Ctrl+Shift+w"
                }
                return ""
            }
            if (event.key === Qt.Key_B) {
                return "Ctrl+b"
            }
            if (event.key === Qt.Key_D) {
                return "Ctrl+d"
            }
            if (event.key === Qt.Key_F) {
                return "Ctrl+f"
            }
            if (event.key === Qt.Key_F5) {
                return "Ctrl+F5"
            }
            if (event.key === Qt.Key_N) {
                return "Ctrl+n"
            }
            if (event.key === Qt.Key_U) {
                return "Ctrl+u"
            }
            if (event.key === Qt.Key_P) {
                return "Ctrl+p"
            }
            if (event.key === Qt.Key_Q) {
                return "Ctrl+q"
            }
            if (event.key === Qt.Key_S) {
                return "Ctrl+s"
            }
            if (event.key === Qt.Key_Space) {
                return "Ctrl+Space"
            }
            if (event.key === Qt.Key_Tab) {
                return "Ctrl+Tab"
            }
            if (event.key === Qt.Key_T) {
                return "Ctrl+t"
            }
            if (event.key === Qt.Key_V) {
                return "Ctrl+v"
            }
            if (event.key === Qt.Key_W) {
                return "Ctrl+w"
            }
            if (event.key === Qt.Key_PageDown) {
                return "Ctrl+PgDown"
            }
            if (event.key === Qt.Key_PageUp) {
                return "Ctrl+PgUp"
            }
            return ""
        }
        if (!control && !alt && !meta) {
            if (event.key === Qt.Key_F5) {
                return "F5"
            }
            if (event.key === Qt.Key_F11) {
                return "F11"
            }
            if (event.key === Qt.Key_Back) {
                return "Back"
            }
            if (event.key === Qt.Key_Forward) {
                return "Forward"
            }
        }
        if (alt || meta) {
            return ""
        }
        return KeyboardPresentation.printableKey(
                    event.text, event.key, shift, Qt.Key_A, Qt.Key_Z)
    }

    function completeBrowserKeyAction(ui, host) {
        if (ui === browserUi) {
            window.syncTabModel()
            window.executePendingEngineAction()
        } else if (host && host.applySwitcherEngineAction) {
            host.applySwitcherEngineAction()
        }
    }

    function logicalGridKeyText(event) {
        if (!event) {
            return ""
        }
        var control = !!(event.modifiers & Qt.ControlModifier)
        var shift = !!(event.modifiers & Qt.ShiftModifier)
        var alt = !!(event.modifiers & Qt.AltModifier)
        var meta = !!(event.modifiers & Qt.MetaModifier)
        if (alt || meta) {
            return ""
        }
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            if (control && !shift) {
                return "Ctrl+Enter"
            }
            if (shift && !control) {
                return "Shift+Enter"
            }
            return !control && !shift ? "Enter" : ""
        }
        if (control || shift) {
            if (shift && event.key === Qt.Key_Slash && event.text === "?") {
                return "?"
            }
            return ""
        }
        if (event.key >= Qt.Key_0 && event.key <= Qt.Key_9) {
            return String.fromCharCode(event.key)
        }
        if (event.key === Qt.Key_Space) {
            return "Space"
        }
        if (event.key === Qt.Key_Backspace) {
            return "Backspace"
        }
        if (event.key === Qt.Key_Escape) {
            return "Escape"
        }
        return event.text === "?" ? "?" : ""
    }

    function handleBrowserKey(ui, host, event) {
        if (!ui || !event) {
            return false
        }
        var logicalText = window.logicalNormalKeyText(event)
        var handled = false
        if (window.handleMediaKey(event)) {
            handled = true
        } else if (event.key === Qt.Key_Escape) {
            if (ui.mode === "grid" && event.modifiers !== Qt.NoModifier) {
                // Unsupported modified Escape is consumed while Grid owns the
                // keyboard, but does not mutate the spatial session.
            } else if (ui === browserUi && ui.mode === "hint") {
                window.closeHints()
            } else {
                ui.escape()
            }
            handled = true
        } else if (ui.mode === "grid") {
            // Grid owns the whole triggering gesture. Auto-repeat is consumed
            // without entering the Rust binding resolver, preventing a held
            // digit or Enter key from refining/clicking repeatedly.
            if (event.isAutoRepeat) {
                handled = true
            } else {
                var gridText = window.logicalGridKeyText(event)
                if (gridText.length > 0) {
                    ui.handle_key(gridText)
                }
                handled = true
            }
        } else if (ui.mode === "normal" && logicalText === ":") {
            ui.enter_command()
            handled = true
        } else if (ui.mode === "normal"
                   && (logicalText === "/" || logicalText === "?")) {
            if (logicalText === "?" && ui.binding_overlay.length > 0) {
                ui.handle_key(logicalText)
                window.completeBrowserKeyAction(ui, host)
            } else {
                ui.enter_search(logicalText === "?")
            }
            handled = true
        } else if (ui === browserUi && ui.mode === "hint") {
            var control = (event.modifiers & Qt.ControlModifier) !== 0
            var shift = (event.modifiers & Qt.ShiftModifier) !== 0
            if (control && event.key === Qt.Key_U) {
                window.applyHintInteraction("clear", "")
            } else if (event.key === Qt.Key_Backspace) {
                window.applyHintInteraction("backspace", "")
            } else if (event.key === Qt.Key_Tab || event.key === Qt.Key_Right
                       || event.key === Qt.Key_Down) {
                window.applyHintInteraction(shift ? "previous" : "next", "")
            } else if (event.key === Qt.Key_Backtab || event.key === Qt.Key_Left
                       || event.key === Qt.Key_Up) {
                window.applyHintInteraction("previous", "")
            } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                window.applyHintInteraction("activate", "")
            } else if (control && event.key === Qt.Key_Space) {
                window.applyHintInteraction("rotate", "")
            } else if (logicalText === "/" && window.hintState.mode === "label") {
                window.applyHintInteraction("text-mode", "")
            } else if (logicalText === " " && window.hintState.mode === "label") {
                window.applyHintInteraction("rotate", "")
            } else if (logicalText.length === 1) {
                window.applyHintInteraction("character", logicalText)
            }
            handled = true
        } else if (ui.mode === "caret" && logicalText.length === 1) {
            var caretDirection = {
                h: "left",
                j: "down",
                k: "up",
                l: "right",
                w: "word-next",
                b: "word-prev",
                "0": "line-start",
                "$": "line-end"
            }[logicalText]
            if (caretDirection) {
                ui.execute_command("caret-move " + caretDirection)
                handled = true
            } else if (logicalText === "v") {
                ui.execute_command("caret-select toggle")
                handled = true
            } else if (logicalText === "y") {
                ui.execute_command("caret-yank")
                handled = true
            }
        } else if (ui.mode === "normal" && ui.search_text.length > 0
                   && (logicalText === "n" || logicalText === "N")) {
            ui.search_next(logicalText === "N")
            window.completeBrowserKeyAction(ui, host)
            handled = true
        } else if (ui.mode === "normal" && logicalText.length > 0
                   && ui.handle_key(logicalText)) {
            window.completeBrowserKeyAction(ui, host)
            handled = true
        }
        if (handled) {
            event.accepted = true
            if (ui === browserUi) {
                window.scheduleRuntimeWork(0)
                window.scheduleFocusProbe(2)
                window.scheduleScrollCapture()
            }
        }
        return handled
    }

    FerricPermissionPrompt {
        browserWindow: window
        hostWindow: window
        onDecisionRequested: function(allow, lifetime) {
            window.decidePermission(allow, lifetime)
        }
    }

    FerricShutdownDecisionDialog {
        hostWindow: window
        promptVisible: window.shutdownConfirmationVisible
        commandText: window.shutdownConfirmationCommand
        title: window.shutdownConfirmationCommand === ":quit"
               ? "Quit Ferric Browser?" : "Close this browser window?"
        message: {
            if (window.shutdownConfirmationCommand === ":quit") {
                return "This will close every Ferric Browser window and tab."
            }
            if (window.shutdownParticipantCount() > 0) {
                return "This is the primary browser window, so closing it will close every Ferric Browser window and tab."
            }
            var count = browserUi.tab_count
            return "This will close this window and its " + count
                    + (count === 1 ? " open tab." : " open tabs.")
        }
        keepLabel: "Keep browser open"
        proceedLabel: window.shutdownConfirmationCommand === ":quit"
                      ? "Quit browser" : "Close window"
        proceedAccessibleName: window.shutdownConfirmationCommand === ":quit"
                               ? "Quit Ferric Browser" : "Close browser window"
        dialogHeight: 220 * window.chromeScale
        onKeepRequested: {
            window.shutdownConfirmationVisible = false
            browserUi.status_text = "Shutdown cancelled"
            if (window.versioningRestoreParentOwner)
                window.versioningRestoreParentOwner.cancelRootShutdown()
        }
        onProceedRequested: {
            window.shutdownConfirmationVisible = false
            window.continueQuitRequest()
        }
    }

    FerricShutdownDecisionDialog {
        hostWindow: window
        promptVisible: window.shutdownPromptVisible
        commandText: window.shutdownConfirmationCommand
        title: window.shutdownWaitingForDownloadFinishes
               ? "Waiting for active work to finish" : "Active browser work is still running"
        message: window.shutdownWaitingForDownloadFinishes
                 ? "Waiting for canceled downloads and archive storage work to retire. Keep the browser open to stop shutdown."
                 : "Cancel active browser work before quitting, or keep the browser open."
        keepLabel: "Keep browser open"
        proceedLabel: "Cancel active work and quit"
        dialogHeight: 220 * window.chromeScale
        onKeepRequested: {
            window.shutdownPromptVisible = false
            window.shutdownWaitingForDownloadFinishes = false
            browserUi.status_text = "Shutdown cancelled"
            if (window.versioningRestoreParentOwner)
                window.versioningRestoreParentOwner.cancelRootShutdown()
        }
        onProceedRequested: {
            window.shutdownPromptVisible = false
            window.cancelDownloadsAndQuit()
        }
    }

    FerricShutdownDecisionDialog {
        hostWindow: window
        promptVisible: window.shutdownPagePromptVisible
        commandText: window.shutdownConfirmationCommand
        title: "Page state may be lost"
        message: window.shutdownPagePromptReason + " Close anyway may lose that state."
        keepLabel: "Keep browser open"
        proceedLabel: "Close anyway"
        proceedAccessibleName: "Close anyway despite page state"
        dialogHeight: 220 * window.chromeScale
        onKeepRequested: {
            window.shutdownPageProbeGeneration += 1
            window.shutdownPagePromptVisible = false
            window.shutdownPagePromptSnapshot = null
            browserUi.status_text = "Shutdown cancelled"
            if (window.versioningRestoreParentOwner)
                window.versioningRestoreParentOwner.cancelRootShutdown()
        }
        onProceedRequested: {
            window.confirmShutdownPagePrompt()
        }
    }

    FerricShutdownDecisionDialog {
        hostWindow: window
        promptVisible: window.applicationShutdownForcePromptVisible
        commandText: ":quit"
        title: "Browser shutdown is taking longer than expected"
        message: window.applicationShutdownStage
                 + ". Continue waiting, or force quit? Force quit leaves the "
                 + "unclean-exit marker for recovery on the next launch."
        keepLabel: "Continue waiting"
        proceedLabel: "Force quit (unclean)"
        keepAccessibleName: "Continue waiting for shutdown"
        proceedAccessibleName: "Force quit and preserve the unclean marker"
        dialogBorderColor: window.errorColor
        dialogHeight: 250 * window.chromeScale
        stackingOrder: 110
        onKeepRequested: {
            window.applicationShutdownForcePromptVisible = false
            applicationShutdownTimer.restart()
        }
        onProceedRequested: window.forceApplicationShutdown()
    }

    Component {
        id: permissionPromptSurfaceComponent

        FerricPermissionPrompt {
            browserWindow: window
            onDecisionRequested: function(allow, lifetime) {
                window.decidePermission(allow, lifetime)
            }
        }
    }

    Component {
        id: captureIndicatorComponent

        FerricCaptureIndicator {
            browserWindow: window
            hostWindow: window
            onDismissRequested: function(sessionId) {
                window.dismissCaptureSession(sessionId)
            }
            onStopRequested: function(sessionId) {
                window.stopCaptureSession(sessionId)
            }
        }
    }

    Component {
        id: desktopMediaSurfaceComponent

        FerricDesktopMediaPrompt {
            browserWindow: window
            onScreenRequested: function(index) {
                window.selectDesktopScreen(index)
            }
            onWindowRequested: function(index) {
                window.selectDesktopWindow(index)
            }
            onCancellationRequested: window.clearDesktopMediaRequest(true)
        }
    }

    Component {
        id: rendererFailureSurfaceComponent

        FerricRendererFailurePrompt {
            browserWindow: window
            onReloadRequested: window.reloadRendererFailure()
            onCloseRequested: window.closeRendererFailure()
            onCopyUrlRequested: function(url) {
                window.copyToClipboard(url, true)
            }
            onDiagnosticsRequested: window.showRendererFailureDiagnostics()
            onSoftwareRestartRequested: window.restartSoftwareRendering()
            onDismissalRequested: {
                window.rendererFailureVisible = false
                window.restoreOverlayFocus()
            }
        }
    }

    Loader {
        id: desktopMediaSurfaceLoader
        anchors.fill: parent
        active: window.desktopMediaPromptVisible
                && window.pendingDesktopMediaHost === window
        sourceComponent: desktopMediaSurfaceComponent
        onLoaded: window.desktopMediaSurface = item
        onActiveChanged: if (!active) window.desktopMediaSurface = null
    }

    Loader {
        id: captureIndicatorSurfaceLoader
        anchors.fill: parent
        active: (window.captureSessions || []).some(function(session) {
            return session && session.host === window
        })
        sourceComponent: captureIndicatorComponent
        onLoaded: window.captureIndicatorSurface = item
        onActiveChanged: if (!active) window.captureIndicatorSurface = null
    }

    Loader {
        id: rendererFailureSurfaceLoader
        anchors.fill: parent
        active: window.rendererFailureVisible
                && window.rendererFailureHost === window
        sourceComponent: rendererFailureSurfaceComponent
        onLoaded: window.rendererFailureSurface = item
        onActiveChanged: if (!active) window.rendererFailureSurface = null
    }

    function completeProfileBootstrap() {
        var nativeBound = window.versioningViewHostObject.ownerProfile === browserProfile
                && browserUi.bind_native_versioning_profile()
        if (!nativeBound) {
            browserUi.status_text = "Versioning native profile binding unavailable"
        }
        if (window.windowCleanBranch) {
            window.syncTabModel()
            window.windowBranchFirstView = window.tabViewAt(0)
            var root = window.windowBranchRootWindow
            var firstView = window.windowBranchInitialView
            var isolated = nativeBound && root && firstView
                    && window.windowBranchParentUi
                    && window.versioningRestoreParentOwner
                    && window.versioningRestoreSharedProfile === null
                    && window.temporaryProfile && window.ephemeralProfile
                    && window.startupUrl === "about:blank"
                    && (!window.startupAdditionalUrls
                        || window.startupAdditionalUrls.length === 0)
                    && requestInterceptor.attach(browserProfile)
                    && root.retainCleanBranchOwner(
                        window.windowBoundBranchToken, window, browserProfile, firstView)
            if (!isolated) {
                browserUi.status_text = "Clean branch profile isolation unavailable"
                window.shutdownApproved = true
                window.finishOwnedWindowClose()
                return
            }
            window.windowCleanBranchNativeBound = true
            return
        }
        engineUpdateNoticeTimer.start()
        if (window.instanceRequestOwner
                && !window.temporaryProfile && !window.recoveryAvailable) {
            browserUi.clear_current_session_checkpoints()
        }
        if (window.ephemeralProfile && window.ephemeralInvocationToken.length === 0) {
            window.ephemeralInvocationToken = window.profileName
        }
        window.registerBrowserWindow(
                    window, browserUi, window.activeWebView(), browserProfile,
                    window.profileName,
                    window.ephemeralInvocationToken, window.ephemeralProfile)
        window.refreshChromeAppearance()
        if (window.startupContext.length > 0) {
            browserUi.set_context_entry_reuse(true)
            browserUi.execute_command("context-enter " + window.startupContext)
            window.routeContextWorkspace(browserUi)
        }
        if (!window.versioningRestoreSharedInterceptor
                && !requestInterceptor.attach(browserProfile)) {
            browserUi.status_text = "Request interceptor unavailable"
        }
        if (window.versioningRestoreStartupCommitId.length > 0) {
            // The startup about:blank load can finish after profile bootstrap.
            // Requesting the plan before that callback settles lets its late
            // completion invalidate the selected page's one-use sidecar intent.
            versioningStartupRestoreTimer.attempts = 0
            versioningStartupRestoreTimer.start()
            return
        }
        if (window.startupBackground) {
            browserUi.status_text = "Opening startup URL in a background tab"
        } else if (window.startupCleanLink) {
            if (browserUi.execute_command("open --clean-link " + window.startupUrl)) {
                if (browserUi.link_preview_visible) {
                    window.showLinkPreview()
                } else {
                    window.executePendingEngineAction()
                }
            }
        } else {
            browserUi.navigate_initial(
                        window.startupUrl, window.startupEntryPoint,
                        window.startupTrustedLocalInput)
        }
        window.updateActiveTabUrl()
        window.profileBootstrapComplete = true
        Qt.callLater(window.openAdditionalStartupUrls)
    }

    function openAdditionalStartupUrls() {
        var urls = window.startupAdditionalUrls || []
        for (var index = 0; index < urls.length && index < 31; ++index) {
            var url = String(urls[index] || "")
            if (url.length === 0 || url.length > 65536) {
                continue
            }
            var tabIndex = browserUi.new_tab()
            if (tabIndex < 0) {
                browserUi.status_text = "Could not create a startup tab"
                break
            }
            window.syncTabModel()
            browserUi.navigate_initial(url, "external-open", true)
            window.executePendingEngineAction()
            window.updateActiveTabUrl()
        }
    }

    Timer {
        id: profileBootstrapTimer
        interval: 25
        repeat: true
        onTriggered: {
            if (!browserUi.profile_bootstrap_pending) {
                stop()
                window.completeProfileBootstrap()
            }
        }
    }

    Timer {
        id: versioningStartupRestoreTimer
        interval: 50
        repeat: true
        property int attempts: 0
        onTriggered: {
            attempts += 1
            var view = window.activeWebView()
            if (attempts > 100 || browserUi.tab_count !== 1
                    || browserUi.active_tab_index !== 0
                    || browserUi.current_url !== "about:blank"
                    || (view && view.url.toString() !== "about:blank")) {
                stop()
                window.versioningRestoreStartupCommitId = ""
                window.versioningRestoreStartupMediaPositions = false
                window.versioningRestoreStartupEditableDrafts = false
                browserUi.status_text = "Versioned restore destination did not remain blank"
                return
            }
            if (!view || view.loading || browserUi.load_state !== "complete") {
                return
            }
            stop()
            var restoreCommitId = window.versioningRestoreStartupCommitId
            var restoreMediaPositions = window.versioningRestoreStartupMediaPositions
            var restoreEditableDrafts = window.versioningRestoreStartupEditableDrafts
            window.versioningRestoreStartupCommitId = ""
            window.versioningRestoreStartupMediaPositions = false
            window.versioningRestoreStartupEditableDrafts = false
            window.versioningRestoreEditableReviewEnabled = restoreEditableDrafts
            if (!browserUi.request_versioned_pristine_tab_restore(
                        restoreCommitId, restoreMediaPositions, restoreEditableDrafts)) {
                window.versioningRestoreEditableReviewEnabled = false
                browserUi.status_text = "Versioned restore could not start; this new window remains blank"
            }
            window.scheduleRuntimeWork(0)
        }
    }

    Component.onCompleted: {
        window.windowBoundBranchToken = window.windowBranchToken
        window.windowBranchTokenLocked = true
        if (window.windowCleanBranch
                && (!window.windowBranchRootWindow || !window.windowBranchParentUi
                    || window.instanceRequestOwner || !window.temporaryProfile
                    || !window.ephemeralProfile
                    || window.versioningRestoreSharedProfile
                    || window.versioningRestoreStartupCommitId.length > 0)) {
            window.shutdownApproved = true
            Qt.callLater(function() { window.destroy() })
            return
        }
        // Install the IPC wake-up before entering the event loop. The rest of
        // profile and QML setup is deferred until the object tree is fully
        // finalized, avoiding reentrant property updates during creation.
        if (window.instanceRequestOwner) {
            browserUi.activate_runtime_wake()
        }
        Qt.callLater(function() {
            window.browserProfile = window.browserProfilePrototypeObject.instance()
            if (!window.browserProfile) {
                console.error("WebEngine profile instance was unavailable")
                if (window.versioningRestoreSourceUi) {
                    window.versioningRestoreSourceUi.status_text =
                            "Restore window profile could not initialize; current work is intact"
                }
                window.shutdownApproved = true
                window.finishOwnedWindowClose()
                return
            }
            window.scheduleRuntimeWork(0)
            if (!browserUi.set_startup_configuration(
                        window.startupConfigJson, window.startupConfigBaseJson,
                        window.startupCliOverridesJson, window.startupProfileOverridesJson,
                        window.startupConfigPath, window.startupConfigSource)) {
                if (window.versioningRestoreSourceUi) {
                    window.versioningRestoreSourceUi.status_text =
                            "Restore window configuration was rejected; current work is intact"
                }
                window.shutdownApproved = true
                window.finishOwnedWindowClose()
                return
            }
            browserUi.set_contexts_configuration(window.startupContextsJson)
            window.refreshSpellcheckInventory(browserUi)
            browserProfile.spellCheckEnabled = window.spellcheckEnabled(browserUi)
            browserProfile.isPushServiceEnabled = window.pushServiceEnabled(
                        browserUi, window.temporaryProfile)
            browserUi.probe_desktop_portals()
            window.configureBlocklistUpdateTimer()
            window.configureLinkCleaningUpdateTimer()
            browserUi.configure_profile(window.temporaryProfile && !window.ephemeralProfile,
                                        window.ephemeralProfile,
                                        window.profileLabel,
                                        window.profileName,
                                        window.storageBasePath)
            browserProfile.spellCheckLanguages = window.spellcheckLanguages(browserUi)
            profileBootstrapTimer.start()
            window.scheduleFocusProbe(3)
        })
    }
    Component.onDestruction: {
        if (window.versioningRestoreParentOwner)
            window.versioningRestoreParentOwner.forgetDestroyed(window)
        notificationPresenter.closeAll()
        window.closeWebNotifications()
        window.cancelPendingExternalUris("External action cancelled with the window")
        window.closeDevTools()
        window.unregisterBrowserWindow(browserUi)
        window.clearClientCertificateRequest(true)
        window.clearCertificateErrorRequest(true)
        window.clearWebAuthRequest(true)
        window.clearContextMenuRequest()
        window.clearDesktopMediaRequest(true)
        window.clearEngineFileDialog(true, "File selection cancelled with the window")
        window.captureSessions = []
        browserUi.view_closed()
        if (window.windowCleanBranch) {
            if (window.windowBranchParentUi) {
                if (window.windowCleanBranchLineagePublished)
                    window.windowBranchParentUi.close_versioned_clean_branch(
                                browserUi, window.windowBoundBranchToken)
                else
                    window.windowBranchParentUi.abort_versioned_clean_branch(
                                window.windowBoundBranchToken)
            }
            if (window.windowBranchRootWindow)
                window.windowBranchRootWindow.releaseCleanBranchOwner(
                            window.windowBoundBranchToken, window,
                            browserProfile, window.windowBranchInitialView)
        }
        browserUi.release_transient_resources()
    }
}

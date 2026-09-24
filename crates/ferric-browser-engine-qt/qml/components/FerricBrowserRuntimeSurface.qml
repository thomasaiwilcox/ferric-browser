import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import QtWebEngine
import io.github.ferricbrowser 1.0
import "../scripts/BrowserScripts.js" as BrowserScripts
import "../scripts/SpellcheckPresentation.js" as SpellcheckPresentation

FerricBrowserRuntimeChrome {
    id: window

    permissionPromptSurfaceComponentObject: permissionPromptSurfaceComponent
    captureIndicatorComponentObject: captureIndicatorComponent
    desktopMediaSurfaceComponentObject: desktopMediaSurfaceComponent
    rendererFailureSurfaceComponentObject: rendererFailureSurfaceComponent

    readonly property var commandSurface: window.commandSurfaceObject
    readonly property var searchSurface: window.searchSurfaceObject
    readonly property var attachedDevToolsLoader: window.attachedDevToolsLoaderObject

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
        if (action.indexOf("command-prefill\t") === 0) {
            commandSurface.commandText = action.slice("command-prefill\t".length)
            commandSurface.cursorPosition = commandSurface.commandText.length
            commandSurface.focusInput()
            return
        }
        if (action === "quit-request") {
            window.beginQuitRequest()
            return
        }
        if (action === "window-close-request") {
            window.beginQuitRequest()
            return
        }
        if (action.indexOf("window-focus\t") === 0) {
            var focusParts = action.split("\t")
            var focusTarget = focusParts.length >= 2 ? focusParts[1] : ""
            var focusOperationId = focusParts.length >= 3 ? focusParts[2] : ""
            var focusEntry = window.browserWindowEntryForTarget(focusTarget)
            if (!focusEntry || !focusEntry.host || !focusEntry.ui) {
                browserUi.status_text = "Window focus target is stale"
                if (focusOperationId.length > 0) {
                    browserUi.complete_window_focus(focusOperationId, "stale")
                }
                return
            }
            if (!window.activateBrowserWindow(
                        focusEntry.host, focusEntry.ui, focusOperationId)) {
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
        var pending = browserUi.take_session_restore_values()
        if (pending.length > 0) {
            window.applyRestorePayload(pending, true)
        }
        var coreIds = []
        for (var coreIndex = 0; coreIndex < browserUi.tab_count; ++coreIndex) {
            coreIds.push(browserUi.tab_id_for_index(coreIndex))
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
        var remainingViews = tabViews.slice(0)
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
                view = window.webViewFactory.createObject(window.webViewsObject, {
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
    }

    function scheduleRuntimeWork(delay) {
        var boundedDelay = Math.max(0, Math.min(60000, Number(delay) || 0))
        if (!runtimeWorkTimer.running || boundedDelay < runtimeWorkTimer.interval) {
            runtimeWorkTimer.interval = Math.max(1, boundedDelay)
            runtimeWorkTimer.restart()
        }
    }

    function processRuntimeWork() {
            browserUi.poll_ipc()
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
                var editorStderr = browserUi.editor_completion_stderr
                var applyView = window.activeWebView()
                if (editorError.length > 0) {
                    if (editorStderr.length > 0) {
                        editorError += " (stderr: " + editorStderr + ")"
                    }
                    browserUi.deliver_editor_apply(editorToken, JSON.stringify({error: editorError}))
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
            var nextDelay = browserUi.maintenance_delay_ms()
            if (nextDelay >= 0) {
                window.scheduleRuntimeWork(nextDelay)
            }
    }

    Timer {
        id: runtimeWorkTimer
        interval: 1
        repeat: false
        running: false
        onTriggered: window.processRuntimeWork()
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
        var text = event.text || ""
        if (text.length === 0 || text.length > 4) {
            return ""
        }
        for (var i = 0; i < text.length; ++i) {
            if (text.charCodeAt(i) < 0x20 || text.charCodeAt(i) === 0x7f) {
                return ""
            }
        }
        return text
    }

    function completeBrowserKeyAction(ui, host) {
        if (ui === browserUi) {
            window.syncTabModel()
            window.executePendingEngineAction()
        } else if (host && host.applySwitcherEngineAction) {
            host.applySwitcherEngineAction()
        }
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
            if (ui === browserUi && ui.mode === "hint") {
                window.closeHints()
            } else {
                ui.escape()
            }
            handled = true
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
        } else if (ui === browserUi && ui.mode === "hint"
                   && logicalText.length === 1) {
            window.hintInput += logicalText.toLowerCase()
            var matches = window.hintResults.filter(function(candidate) {
                return candidate.label.indexOf(window.hintInput) === 0
            })
            if (matches.length === 1 && matches[0].label === window.hintInput) {
                window.activateHint(matches[0].label)
            } else if (matches.length === 0) {
                window.hintInput = ""
                ui.status_text = "Unknown hint label"
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
        promptVisible: window.shutdownPromptVisible
        title: "Active downloads are still running"
        message: "Cancel active browser work before quitting, or keep the browser open."
        keepLabel: "Keep browser open"
        proceedLabel: "Cancel active work and quit"
        dialogHeight: 190 * window.chromeScale
        onKeepRequested: {
            window.shutdownPromptVisible = false
            browserUi.status_text = "Shutdown cancelled"
        }
        onProceedRequested: window.cancelDownloadsAndQuit()
    }

    FerricShutdownDecisionDialog {
        hostWindow: window
        promptVisible: window.shutdownPagePromptVisible
        title: "Page state may be lost"
        message: window.shutdownPagePromptReason + " Close anyway may lose that state."
        keepLabel: "Keep browser open"
        proceedLabel: "Close anyway"
        proceedAccessibleName: "Close anyway despite page state"
        dialogHeight: 220 * window.chromeScale
        onKeepRequested: {
            window.shutdownPageProbeGeneration += 1
            window.shutdownPagePromptVisible = false
            browserUi.status_text = "Shutdown cancelled"
        }
        onProceedRequested: window.finalizeQuit()
    }

    FerricShutdownDecisionDialog {
        hostWindow: window
        promptVisible: window.applicationShutdownForcePromptVisible
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

    function completeProfileBootstrap() {
        engineUpdateNoticeTimer.start()
        if (!window.temporaryProfile && !window.recoveryAvailable) {
            browserUi.clear_current_session_checkpoints()
        }
        if (window.ephemeralProfile && window.ephemeralInvocationToken.length === 0) {
            window.ephemeralInvocationToken = window.profileName
        }
        window.registerBrowserWindow(
                    window, browserUi, window.activeWebView(), browserProfile,
                    window.profileName,
                    window.ephemeralInvocationToken, window.ephemeralProfile)
        if (window.startupContext.length > 0) {
            browserUi.set_context_entry_reuse(true)
            browserUi.execute_command("context-enter " + window.startupContext)
            window.routeContextWorkspace(browserUi)
        }
        if (!requestInterceptor.attach(browserProfile)) {
            browserUi.status_text = "Request interceptor unavailable"
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

    Component.onCompleted: {
        window.browserProfile = window.browserProfilePrototypeObject.instance()
        if (!window.browserProfile) {
            console.error("WebEngine profile instance was unavailable")
            Qt.quit()
            return
        }
        browserUi.activate_runtime_wake()
        window.scheduleRuntimeWork(0)
        window.desktopMediaSurface = desktopMediaSurfaceComponent.createObject(
                    window.contentItem, { hostWindow: window })
        window.captureIndicatorSurface = captureIndicatorComponent.createObject(
                    window.contentItem, { hostWindow: window })
        window.rendererFailureSurface = rendererFailureSurfaceComponent.createObject(
                    window.contentItem, { hostWindow: window })
        if (!browserUi.set_startup_configuration(
                    window.startupConfigJson, window.startupConfigBaseJson,
                    window.startupCliOverridesJson, window.startupProfileOverridesJson,
                    window.startupConfigPath, window.startupConfigSource)) {
            Qt.quit()
            return
        }
        browserUi.set_contexts_configuration(window.startupContextsJson)
        window.refreshSpellcheckInventory(browserUi)
        browserProfile.spellCheckEnabled = window.spellcheckEnabled(browserUi)
        browserProfile.isPushServiceEnabled = window.pushServiceEnabled(
                    browserUi, window.temporaryProfile)
        browserUi.probe_desktop_portals()
        window.refreshChromeAppearance()
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
    }
    Component.onDestruction: {
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
        if (window.desktopMediaSurface) {
            window.desktopMediaSurface.destroy()
            window.desktopMediaSurface = null
        }
        if (window.captureIndicatorSurface) {
            window.captureIndicatorSurface.destroy()
            window.captureIndicatorSurface = null
        }
        if (window.rendererFailureSurface) {
            window.rendererFailureSurface.destroy()
            window.rendererFailureSurface = null
        }
        browserUi.view_closed()
        browserUi.release_transient_resources()
    }
}

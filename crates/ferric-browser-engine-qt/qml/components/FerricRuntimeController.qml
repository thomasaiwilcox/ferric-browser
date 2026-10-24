import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import QtWebEngine
import io.github.ferricbrowser 1.0
import "../scripts/BrowserScripts.js" as BrowserScripts
import "../scripts/ChromePresentation.js" as ChromePresentation
import "../scripts/SpellcheckPresentation.js" as SpellcheckPresentation
import "../scripts/SwitcherSources.js" as SwitcherSources
import "../scripts/SwitcherResults.js" as SwitcherResults
import "../scripts/CloseRequests.js" as CloseRequests
import "../scripts/TargetSearchPresentation.js" as TargetSearchPresentation
import "../scripts/EngineRequests.js" as EngineRequests
import "../scripts/KeyboardPresentation.js" as KeyboardPresentation

// Owns window behavior through an explicit host; no inherited creation scope.
QtObject {
    required property var host
    readonly property var window: host

    function configuredBlocklistIds() {
        return window.browserUi.feature_blocking_list_ids || []
    }

    function requestBlocklistUpdate() {
        if (window.temporaryProfile || window.storageBasePath.length === 0) {
            return false
        }
        if (window.blocklistUpdater.busy) {
            return false
        }
        var configuredLists = window.configuredBlocklistIds()
        if (configuredLists.length === 0) {
            window.browserUi.status_text = "No supported blocklists configured"
            return false
        }
        window.blocklistInstallFailed = false
        window.blocklistInstallError = ""
        if (!window.blocklistUpdater.update(configuredLists, window.storageBasePath)) {
            window.browserUi.status_text = window.blocklistUpdater.status
            return false
        }
        return true
    }

    function configuredLinkCleaningUpdate() {
        return {
            source: window.browserUi.feature_link_cleaning_update_source,
            checksum: window.browserUi.feature_link_cleaning_update_sha256
        }
    }

    function requestLinkCleaningUpdate(automatic) {
        automatic = automatic === true
        if (window.temporaryProfile || window.storageBasePath.length === 0) {
            if (!automatic) {
                window.browserUi.status_text = "Clean-link updates require a normal profile"
            }
            return false
        }
        if (window.linkRuleUpdater.busy) {
            return false
        }
        var intervalMs = window.linkCleaningUpdateIntervalHours * 60 * 60 * 1000
        var now = Date.now()
        if (automatic && window.linkCleaningLastUpdateAt > 0
                && now - window.linkCleaningLastUpdateAt < intervalMs) {
            window.linkCleaningUpdateTimer.restart()
            return false
        }
        var configured = window.configuredLinkCleaningUpdate()
        if (!configured.source || !configured.checksum) {
            if (!automatic) {
                window.browserUi.status_text = "No explicit clean-link update source configured"
            }
            return false
        }
        window.linkRuleInstallFailed = false
        window.linkRuleInstallError = ""
        if (!window.linkRuleUpdater.update(
                    configured.source, configured.checksum, window.storageBasePath)) {
            if (!automatic) {
                window.browserUi.status_text = window.linkRuleUpdater.status
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
            window.linkCleaningUpdateTimer.restart()
        } else {
            window.linkCleaningUpdateTimer.stop()
        }
    }

    function configureBlocklistUpdateTimer() {
        window.blocklistUpdateIntervalHours = window.browserUi.feature_blocking_update_interval_hours
        window.blocklistUpdateTimer.restart()
    }

    function closeTabAtIndex(index) {
        var view = window.tabViewAt(index)
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
        if (!window.browserUi.close_tab(index)) {
            return false
        }
        window.removeTabViewAt(index, null)
        window.tabs.remove(index)
        if (window.browserUi.tab_count > window.tabs.count) {
            window.tabs.append({ url: window.browserUi.initial_url, title: "New tab",
                          pinned: false, muted: false, zoom: 1.0,
                          suspended: false, discarded: false })
        }
        window.syncTabModel()
        return true
    }

    function executeHistoryTraversal(view, backwards, requestedCount) {
        view.requestContentHistoryTraversal(backwards, requestedCount, function(direction, count) {
            performHistoryTraversal(view, direction, count)
        })
    }

    function performHistoryTraversal(view, backwards, requestedCount) {
        var originalHistory = view.viewUi.content_history_export(view.tabIndex >= 0 ? view.tabIndex : view.viewUi.active_tab_index)
        var count = Math.max(1, Math.min(100, Number(requestedCount) || 1))
        var destination = ""
        for (var step = count; step >= 1; --step) {
            destination = view.viewUi.content_traverse(view.tabIndex >= 0 ? view.tabIndex : view.viewUi.active_tab_index, backwards ? -step : step, view.engineHistoryIndex)
            if (destination.length > 0) break
        }
        if (destination.length === 0) { view.viewUi.status_text = backwards ? "No earlier history" : "No later history"; return }
        var fields = destination.split("\t")
        var offset = Number(fields[0])
        var url = fields.slice(1).join("\t")
        if (url !== "rb://start" && url !== "rb://blank") offset = view.contentEngineOffset(url, offset, backwards)
        if (offset === null) {
            view.viewUi.content_history_import(view.tabIndex >= 0 ? view.tabIndex : view.viewUi.active_tab_index, originalHistory)
            view.viewUi.status_text = "That history entry is no longer available."
            return
        }
        view.setContentUrl(url, true, offset !== 0, false, view.engineHistoryIndex + offset)
        if (offset !== 0 && url !== "rb://start" && url !== "rb://blank") view.goBackOrForward(offset)
    }

    function executePendingEngineAction() {
        if (EngineRequests.execute(window.browserUi.take_engine_request(), window.browserUi, window, window)) return
        var action = window.browserUi.take_engine_action()
        if (action.indexOf("command-prefill\t") === 0) {
            window.commandSurface.commandText = action.slice("command-prefill\t".length)
            window.commandSurface.cursorPosition = window.commandSurface.commandText.length
            window.commandSurface.focusInput()
            return
        }
        if (action === "start-page-cycle") {
            var startView = window.activeWebView()
            if (startView && startView.nativeUrl === "rb://start") startView.startPage.cycleView()
            return
        }
        if (action.indexOf("start-page-view\t") === 0 || action.indexOf("start-page-add\t") === 0 || action.indexOf("start-page-bookmark\t") === 0) {
            var startParts = action.split("\t")
            var startView = window.activeWebView()
            if (startView && startView.nativeUrl === "rb://start") {
                if (startParts[0] === "start-page-view") startView.startPage.chooseView(startParts[1])
                else if (startParts[0] === "start-page-bookmark") {
                    startView.startPage.addCaptured(startParts[2], startParts.slice(3).join("\t"), startParts[1])
                } else startView.startPage.addCaptured(startParts[1], startParts.slice(2).join("\t"))
            }
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
            window.executeWindowFocusAction(window.browserUi, action)
            return
        }
        if (action.indexOf("window-move\t") === 0) {
            var moveParts = action.split("\t")
            var moveTarget = moveParts.length >= 2 ? moveParts[1] : ""
            var moveWorkspace = moveParts.length >= 3 ? moveParts[2] : ""
            var moveEntry = window.browserWindowEntryForTarget(moveTarget)
            if (!moveEntry || !moveEntry.host || !moveEntry.ui || !moveWorkspace) {
                window.browserUi.status_text = "This window is no longer available. Choose an open window and try again."
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
            var suspendIndex = window.browserUi.tab_index_for_id(suspendId)
            var suspendView = window.tabViewAt(suspendIndex)
            if (!suspendView || suspendIndex === window.browserUi.active_tab_index) {
                window.browserUi.status_text = "Tab suspension target is no longer hidden"
                return
            }
            suspendView.lifecycleState = WebEngineView.LifecycleState.Frozen
            window.tabs.setProperty(suspendIndex, "suspended", true)
            window.browserUi.status_text = "Tab suspended; background activity paused."
            return
        }
        if (action.indexOf("tab-discard\t") === 0) {
            var discardParts = action.split("\t")
            var discardId = discardParts.length >= 2 ? discardParts[1] : ""
            var discardIndex = window.browserUi.tab_index_for_id(discardId)
            var discardView = window.tabViewAt(discardIndex)
            if (!discardView || discardIndex === window.browserUi.active_tab_index) {
                window.browserUi.status_text = "Tab discard target is no longer hidden"
                return
            }
            discardView.lifecycleState = WebEngineView.LifecycleState.Discarded
            window.tabs.setProperty(discardIndex, "suspended", false)
            window.tabs.setProperty(discardIndex, "discarded", true)
            window.browserUi.status_text = "Tab discarded; resume will reload the page"
            return
        }
        if (action.indexOf("tab-resume\t") === 0) {
            var resumeParts = action.split("\t")
            var resumeId = resumeParts.length >= 2 ? resumeParts[1] : ""
            var resumeIndex = window.browserUi.tab_index_for_id(resumeId)
            var resumeView = window.tabViewAt(resumeIndex)
            if (!resumeView) {
                window.browserUi.status_text = "This tab is no longer available. Select an open tab and try again."
                return
            }
            var wasDiscarded = resumeIndex >= 0 && resumeIndex < window.tabs.count
                    && window.tabs.get(resumeIndex).discarded
            resumeView.lifecycleState = WebEngineView.LifecycleState.Active
            if (resumeIndex >= 0 && resumeIndex < window.tabs.count) {
                window.tabs.setProperty(resumeIndex, "suspended", false)
                window.tabs.setProperty(resumeIndex, "discarded", false)
            }
            if (wasDiscarded) {
                resumeView.reload()
            }
            window.browserUi.status_text = wasDiscarded
                    ? "Tab resumed; discarded page reload requested"
                    : "Tab resumed; page state is live"
            return
        }
        if (action.indexOf("tab-mute\t") === 0) {
            var muteParts = action.split("\t")
            var muteId = muteParts.length >= 2 ? muteParts[1] : ""
            var muteIndex = window.browserUi.tab_index_for_id(muteId)
            var muted = muteParts.length >= 3 && muteParts[2] === "true"
            if (muteIndex < 0 || muteIndex >= window.tabs.count) {
                window.browserUi.status_text = "This tab is no longer available. Select an open tab and try again."
                return
            }
            window.tabs.setProperty(muteIndex, "muted", muted)
            window.browserUi.status_text = muted ? "Tab muted" : "Tab unmuted"
            return
        }
        if (action.indexOf("permission-reset\t") === 0) {
            var permissionParts = action.split("\t")
            if (permissionParts.length === 3)
                window.applyPermissionReset(window.browserUi, permissionParts[1], permissionParts[2])
            return
        }

        if (action === "show-downloads") {
            window.downloadWorkspaceObject.show()
            return
        }
        if (action === "show-switcher") {
            window.showSwitcherWith(
                String(window.browserUi.switcher_request_scope || "all"),
                String(window.browserUi.switcher_request_query || ""))
            return
        }
        if (action.indexOf("tab-close\t") === 0) {
            var tabCloseParts = action.split("\t")
            var tabCloseId = tabCloseParts.length >= 2 ? tabCloseParts[1] : ""
            var tabCloseIndex = window.browserUi.tab_index_for_id(tabCloseId)
            if (tabCloseIndex < 0) {
                window.browserUi.status_text = "This tab is no longer available. Select an open tab and try again."
                return
            }
            if (!window.closeTabAtIndex(tabCloseIndex)) {
                window.browserUi.status_text = "Tab close was rejected"
                return
            }
            return
        }
        if (action.indexOf("tab-close-active\t") === 0) {
            var tabCloseActiveParts = action.split("\t")
            var tabCloseCount = Number(tabCloseActiveParts[1])
            if (!Number.isFinite(tabCloseCount) || tabCloseCount < 1 || tabCloseCount > 100) {
                window.browserUi.status_text = "Tab close count is invalid"
                return
            }
            var closedCount = 0
            for (var tabCloseAttempt = 0; tabCloseAttempt < tabCloseCount; ++tabCloseAttempt) {
                var candidateIndex = -1
                var activeTabIndex = window.browserUi.active_tab_index
                if (activeTabIndex >= 0 && activeTabIndex < window.tabs.count
                        && !window.tabs.get(activeTabIndex).pinned) {
                    candidateIndex = activeTabIndex
                } else {
                    for (var tabCloseCandidate = 0;
                         tabCloseCandidate < window.tabs.count;
                         ++tabCloseCandidate) {
                        if (!window.tabs.get(tabCloseCandidate).pinned) {
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
            window.browserUi.status_text = closedCount > 0
                    ? "Closed " + closedCount + " tab" + (closedCount === 1 ? "" : "s")
                    : "No unpinned tabs available to close"
            return
        }
        if (action.indexOf("tab-give\t") === 0
                || action.indexOf("tab-move-context\t") === 0) {
            var giveSource = window.focusedBrowserWindowEntry()
            window.executeBrowserTransferAction(
                        giveSource ? giveSource.ui : window.browserUi,
                        giveSource ? giveSource.host : window,
                        action)
            return
        }
        if (action === "tab-detach" || action.indexOf("tab-detach\t") === 0) {
            var detachSource = window.focusedBrowserWindowEntry()
            window.executeBrowserTransferAction(
                        detachSource ? detachSource.ui : window.browserUi,
                        detachSource ? detachSource.host : window,
                        action)
            return
        }
        if (action.indexOf("reopen-window\t") === 0) {
            window.openReopenedWindow(window.browserUi, window, action)
            return
        }
        if (action.indexOf("reopen-window-confirm\t") === 0) {
            window.showReopenWindowConfirmation(window.browserUi, window, action)
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
                window.browserUi.site_data_clear_finished(
                            siteDataOrigin,
                            JSON.stringify({"error": "active view unavailable"}))
                return
            }
            window.siteDataClearOrigin = siteDataOrigin
            window.siteDataClearView = siteDataView
            window.siteDataClearUi = window.browserUi
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
        if (action === "show-profiles") {
            window.refreshProfiles()
            window.openInternalSurface()
            window.profileManagerVisible = true
            return
        }
        if (action === "show-sessions") {
            window.refreshSessions()
            window.openInternalSurface()
            window.sessionManagerVisible = true
            return
        }
        if (action.indexOf("show-settings\t") === 0) {
            window.showSettings(action.split("\t").slice(1).join("\t"))
            return
        }
        if (action.indexOf("site-doctor-close\t") === 0) {
            var closeParts = action.split("\t")
            var closeIndex = closeParts.length >= 2 ? window.browserUi.tab_index_for_id(closeParts[1]) : -1
            if (!window.closeTabAtIndex(closeIndex)) {
                window.browserUi.status_text = "Site Doctor could not close its temporary view"
                return
            }
            window.browserUi.status_text = "Site Doctor temporary view closed"
            return
        }
        if (action === "blocking-toggle-site") {
            window.browserUi.toggle_blocking_site()
            return
        }
        if (action.indexOf("download-open\t") === 0 || action.indexOf("download-show\t") === 0) {
            var desktopAction = action.split("\t")
            if (desktopAction.length >= 2) {
                window.openExternalUri(window.browserUi, desktopAction.slice(1).join("\t"))
            }
            return
        }
        if (action.indexOf("external-open\t") === 0) {
            var externalAction = action.split("\t")
            if (externalAction.length >= 2) {
                window.openExternalUri(window.browserUi, externalAction.slice(1).join("\t"))
            }
            return
        }
        if (action.indexOf("download-cancel\t") === 0 || action.indexOf("download-pause\t") === 0
                || action.indexOf("download-resume\t") === 0) {
            var control = action.split("\t")
            window.profileResourceRegistry.controlDownload(window.browserUi, control[1], control[0].slice("download-".length))
            return
        }
        if (action.indexOf("fullscreen\t") === 0) {
            var fullscreenState = action.split("\t")[1] || "toggle"
            var currentlyFullscreen = window.visibility === Window.FullScreen
            var enterFullscreen = fullscreenState === "on"
                    || (fullscreenState === "toggle" && !currentlyFullscreen)
            if (enterFullscreen) {
                window.showFullScreen()
                window.browserUi.status_text = "Fullscreen enabled"
            } else {
                window.showNormal()
                window.browserUi.status_text = "Fullscreen ended"
            }
            return
        }
        if (action.indexOf("new-window\t") === 0 || action.indexOf("context-window\t") === 0) {
            window.executeNewWindowAction(window.browserUi, action)
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


    }

    function activeWebView() {
        return window.tabViewAt(window.browserUi.active_tab_index)
    }

    function updateActiveTabUrl() {
        var active = window.browserUi.active_tab_index
        if (active >= 0 && active < window.tabs.count) {
            window.tabs.setProperty(active, "url", window.browserUi.initial_url)
            window.executePendingEngineAction()
        }
    }

    function applyRestorePayload(payload, hasModeLine) {
        window.tabViewControllerObject.applyRestorePayload(payload, hasModeLine)
    }

    function syncTabModel() { window.tabViewControllerObject.syncTabModel() }

    function scheduleRuntimeWork(delay) {
        var boundedDelay = Math.max(0, Math.min(60000, Number(delay) || 0))
        if (!window.runtime_runtimeWorkTimer.running || boundedDelay < window.runtime_runtimeWorkTimer.interval) {
            window.runtime_runtimeWorkTimer.interval = Math.max(1, boundedDelay)
            window.runtime_runtimeWorkTimer.restart()
        }
    }

    function processRuntimeWork() {
            window.browserUi.poll_ipc()
            window.syncTabModel()
            window.executePendingEngineAction()
            window.browserUi.poll_config()
            window.profileSessionWorkspaceObject.presentCommandRequest()
            window.maybeOpenPendingDesktopMediaRequest()
            window.browserUi.refresh_operations()
            window.browserUi.tick_bindings()
            window.browserUi.tick_site_doctor_experiment()
            window.browserUi.checkpoint_session()
            if (window.browserUi.take_caret_request()) {
                var caretToken = window.browserUi.caret_request_token
                var caretOperation = window.browserUi.caret_request_operation
                var caretView = window.activeWebView()
                if (caretView) {
                    window.runBrowserScript(caretView, window.caretScript(caretOperation, window.caretSelecting), function(value) {
                        var response = value || {error: "caret script returned no result"}
                        if (response.selecting !== undefined) {
                            window.caretSelecting = response.selecting
                        }
                        window.browserUi.deliver_caret(caretToken, JSON.stringify(response))
                    })
                } else {
                    window.browserUi.deliver_caret(caretToken, JSON.stringify({error: "document view unavailable"}))
                }
            }
            var editorRequest = window.browserUi.take_editor_request()
            if (editorRequest.length > 0) {
                var editorView = window.activeWebView()
                if (editorView) {
                    window.runBrowserScript(editorView, window.editorScript(), function(value) {
                        window.browserUi.deliver_editor(editorRequest, JSON.stringify(value || {error: "editor script returned no result"}))
                    })
                } else {
                    window.browserUi.deliver_editor(editorRequest, JSON.stringify({error: "document view unavailable"}))
                }
            }
            if (window.browserUi.take_editor_completion()) {
                var editorToken = window.browserUi.editor_completion_token
                var editorOriginal = window.browserUi.editor_completion_original
                var editorUpdated = window.browserUi.editor_completion_updated
                var editorError = window.browserUi.editor_completion_error
                var editorStderr = window.browserUi.editor_completion_stderr
                var applyView = window.activeWebView()
                if (editorError.length > 0) {
                    if (editorStderr.length > 0) {
                        editorError += " (stderr: " + editorStderr + ")"
                    }
                    window.browserUi.deliver_editor_apply(editorToken, JSON.stringify({error: editorError}))
                } else if (applyView) {
                    window.runBrowserScript(applyView, window.editorApplyScript(editorOriginal, editorUpdated), function(value) {
                        window.browserUi.deliver_editor_apply(editorToken, JSON.stringify(value || {error: "editor apply returned no result"}))
                    })
                } else {
                    window.browserUi.deliver_editor_apply(editorToken, JSON.stringify({error: "document view unavailable"}))
                }
            }
            var selectionToken = window.browserUi.take_selection_request()
            if (selectionToken.length > 0) {
                var selectionView = window.activeWebView()
                if (selectionView) {
                    window.readSelection(window.browserUi, selectionView, selectionToken)
                } else {
                    window.browserUi.deliver_selection(selectionToken, JSON.stringify({error: "document view unavailable"}))
                }
            }
            window.downloadWorkspaceObject.dispatchRequest()
            var clipboardRequest = window.browserUi.take_clipboard_request()
            if (clipboardRequest.length > 0) {
                window.copyToClipboard(
                    clipboardRequest,
                    !window.browserUi.clipboard_request_sensitive,
                    window.browserUi.clipboard_request_primary)
            }
            window.syncTabModel()
            window.executePendingEngineAction()
            var nextDelay = window.browserUi.maintenance_delay_ms()
            if (nextDelay >= 0) {
                window.scheduleRuntimeWork(nextDelay)
            }
    }

    function scheduleFocusProbe(retries) {
        window.focusProbeRetries = Math.max(window.focusProbeRetries,
                                            Math.max(1, Number(retries) || 1))
        window.runtime_focusProbeTimer.restart()
    }

    function cancelFocusProbes() {
        window.focusProbeRetries = 0
        window.runtime_focusProbeTimer.stop()
    }

    function scheduleScrollCapture() {
        window.runtime_scrollCaptureTimer.restart()
    }

    function logicalNormalKeyText(event) {
        return KeyboardPresentation.logicalNormalKeyText(event)
    }

    function completeBrowserKeyAction(ui, host) {
        if (ui === window.browserUi) {
            window.syncTabModel()
            window.executePendingEngineAction()
        } else if (host && host.applySwitcherEngineAction) {
            host.applySwitcherEngineAction()
        }
    }

    function logicalGridKeyText(event) {
        return KeyboardPresentation.logicalGridKeyText(event)
    }

    function handleBrowserKey(ui, host, event) {
        if (!ui || !event) {
            return false
        }
        // Observable native prompts and Qt-owned menus receive the original
        // event before media, Mixed policy, or browser bindings inspect it.
        if (window.nativePromptActiveFor(ui)) {
            return false
        }
        var nativeView = host && host.activeView !== undefined ? host.activeView : (ui === window.browserUi ? window.activeWebView() : null)
        var hints = host && host.hintWorkspace && host.hintWorkspace.browserUi === ui
                ? host.hintWorkspace : null
        var pageSearch = host && host.searchWorkspace ? host.searchWorkspace : null
        var logicalText = window.logicalNormalKeyText(event)
        var handled = false
        var passing = ui.mode === "pass-through" || ui.mode === "pass-through-once"
        var mixedRoute = ui.mode === "normal"
                ? ui.mixed_key_route(logicalText) : "browser"
        if (passing && event.key === Qt.Key_Escape
                && event.modifiers === Qt.ControlModifier) {
            ui.escape()
            handled = true
        } else if (window.handleMediaKey(event)) {
            handled = true
        } else if (passing) {
            return false
        } else if (ui.mode === "normal"
                   && !event.browserOwnedRepeat
                   && mixedRoute === "page") {
            var pageRouter = host && host.browserKeyRouterObject
            if (pageRouter && pageRouter.pageOwnsCurrentEvent) {
                pageRouter.pageOwnsCurrentEvent()
            }
            return false
        } else if (ui.mode === "normal"
                   && (mixedRoute === "active-count"
                       || mixedRoute === "active-keychain")) {
            ui.handle_key(logicalText)
            window.completeBrowserKeyAction(ui, host)
            handled = true
        } else if (event.browserOwnedRepeat && ui.mode !== "normal") {
            // A browser-owned press may change modes. Its held repeat cannot
            // become a new page gesture in the new mode.
            handled = true
        } else if (event.key === Qt.Key_Escape) {
            if (ui.mode === "grid" && event.modifiers !== Qt.NoModifier) {
                // Unsupported modified Escape is consumed while Grid owns the
                // keyboard, but does not mutate the spatial session.
            } else if (hints && ui.mode === "hint") {
                hints.closeHints()
            } else {
                ui.escape()
            }
            handled = true
        } else if (ui.mode === "normal" && nativeView
                   && nativeView.nativeUrl === "rb://start"
                   && nativeView.startPage
                   && nativeView.startPage.handleNavigation(event)) {
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
        } else if (hints && ui.mode === "hint"
                   && ui.hint_pointer_mode
                   && ui.hint_pointer_state !== "selecting") {
            // Enter release owns preflight. While it is pending, Escape above
            // cancels and all other target input is consumed.
            handled = true
        } else if (hints && ui.mode === "hint") {
            var control = (event.modifiers & Qt.ControlModifier) !== 0
            var shift = (event.modifiers & Qt.ShiftModifier) !== 0
            if (control && event.key === Qt.Key_U) {
                hints.applyHintInteraction("clear", "")
            } else if (event.key === Qt.Key_Backspace) {
                hints.applyHintInteraction("backspace", "")
            } else if (event.key === Qt.Key_Tab || event.key === Qt.Key_Right
                       || event.key === Qt.Key_Down) {
                hints.applyHintInteraction(shift ? "previous" : "next", "")
            } else if (event.key === Qt.Key_Backtab || event.key === Qt.Key_Left
                       || event.key === Qt.Key_Up) {
                hints.applyHintInteraction("previous", "")
            } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                if (ui.hint_pointer_mode) {
                    if (!event.isAutoRepeat) hints.beginHintPointerConfirmation()
                } else {
                    hints.applyHintInteraction("activate", "")
                }
            } else if (control && event.key === Qt.Key_Space) {
                hints.applyHintInteraction("rotate", "")
            } else if (logicalText === "/" && hints.hintState.mode === "label") {
                hints.applyHintInteraction("text-mode", "")
            } else if (logicalText === " " && hints.hintState.mode === "label") {
                hints.applyHintInteraction("rotate", "")
            } else if (logicalText.length === 1) {
                hints.applyHintInteraction("character", logicalText)
            }
            handled = true
        } else if (ui.mode === "caret" && (logicalText === "Ctrl+x" || logicalText === "Ctrl+v" || logicalText === "Ctrl+Shift+v")) {
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
            ui.search_next(ui.search_backward !== (logicalText === "N"))
            window.completeBrowserKeyAction(ui, host)
            handled = true
        } else if (ui.mode === "normal" && pageSearch && pageSearch.retained && pageSearch.accepted
                   && (logicalText === "Enter" || logicalText === "Shift+Enter")
                   && !ui.key_has_binding(logicalText)) {
            if (!event.isAutoRepeat) pageSearch.follow(logicalText === "Shift+Enter")
            handled = true
        } else if (ui.mode === "normal" && logicalText.length > 0
                   && ((logicalText !== "Enter" && logicalText !== "Shift+Enter") || ui.key_has_binding(logicalText))
                   && ui.handle_key(logicalText)) {
            window.completeBrowserKeyAction(ui, host)
            handled = true
        }
        if (handled) {
            event.accepted = true
            if (ui === window.browserUi) {
                window.scheduleRuntimeWork(0)
                if (event.key !== Qt.Key_Escape) {
                    window.scheduleFocusProbe(2)
                } else {
                    window.cancelFocusProbes()
                }
                window.scheduleScrollCapture()
            }
        }
        return handled
    }

    function handleBrowserKeyRelease(ui, host, key, modifiers) {
        var hints = host && host.hintWorkspace && host.hintWorkspace.browserUi === ui
                ? host.hintWorkspace : null
        if (hints && ui.mode === "hint" && ui.hint_pointer_mode
                && ui.hint_pointer_state === "awaiting-confirmation-release"
                && (key === Qt.Key_Return || key === Qt.Key_Enter)) {
            hints.releaseHintPointerConfirmation()
            return true
        }
        return false
    }

    function completeProfileBootstrap() {
        if (window.runtimeDisposing || window.profileBootstrapComplete
                || window.browserUi.profile_bootstrap_pending || !window.browserUi.profile_available()) return
        window.refreshSpellcheckInventory(window.browserUi)
        window.browserProfile.spellCheckEnabled = window.spellcheckEnabled(window.browserUi)
        window.browserProfile.spellCheckLanguages = window.spellcheckLanguages(window.browserUi)
        window.browserProfile.isPushServiceEnabled = window.pushServiceEnabled(
                    window.browserUi, window.temporaryProfile)
        window.refreshChromeAppearance()
        window.engineUpdateNoticeTimer.start()
        if (!window.temporaryProfile && !window.recoveryAvailable) {
            window.browserUi.clear_current_session_checkpoints()
        }
        if (window.ephemeralProfile && window.ephemeralInvocationToken.length === 0) {
            window.ephemeralInvocationToken = window.profileName
        }
        if (!window.requestInterceptor.attach(window.browserProfile)) {
            window.browserUi.status_text = "Request interceptor unavailable"
        }
        window.profileResourceRegistry.activateWindow(window)
        window.profileBootstrapComplete = true
        window.syncTabModel()
        window.registerBrowserWindow(
                    window, window.browserUi, window.activeWebView(), window.browserProfile,
                    window.profileName,
                    window.ephemeralInvocationToken, window.ephemeralProfile)
        if (window.startupContext.length > 0) {
            window.browserUi.set_context_entry_reuse(true)
            window.browserUi.execute_command("context-enter " + window.startupContext)
            window.routeContextWorkspace(window.browserUi)
        }
        if (window.startupBackground) {
            window.browserUi.status_text = "Opening startup URL in a background tab"
        } else if (window.startupCleanLink) {
            // Hold startup previews over native blank content. The engine's
            // initial about:blank load is not the requested navigation.
            var previewView = window.activeWebView()
            if (previewView) previewView.setContentUrl("rb://blank", false)
            if (window.browserUi.execute_command("open --clean-link " + window.startupUrl)) {
                if (!window.browserUi.link_preview_visible) {
                    window.executePendingEngineAction()
                }
            }
        } else {
            window.browserUi.navigate_initial(
                        window.startupUrl, window.startupEntryPoint,
                        window.startupTrustedLocalInput)
        }
        window.updateActiveTabUrl()
        if (window.recoveryAvailable && window.startupContext.length === 0
                && !window.startupCleanLink && !window.startupBackground
                && (window.startupAdditionalUrls || []).length === 0
                && ["rb://start", "rb://blank", "about:blank"].indexOf(window.startupUrl) >= 0) {
            window.browserUi.remember_recovery_placeholder()
        }
        Qt.callLater(window.openAdditionalStartupUrls)
    }

    function openAdditionalStartupUrls() {
        var urls = window.startupAdditionalUrls || []
        for (var index = 0; index < urls.length && index < 31; ++index) {
            var url = String(urls[index] || "")
            if (url.length === 0 || url.length > 65536) {
                continue
            }
            var tabIndex = window.browserUi.new_tab()
            if (tabIndex < 0) {
                window.browserUi.status_text = "Could not create a startup tab"
                break
            }
            window.syncTabModel()
            window.browserUi.navigate_initial(url, "external-open", true)
            window.executePendingEngineAction()
            window.updateActiveTabUrl()
        }
    }
}

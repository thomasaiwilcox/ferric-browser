import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import QtWebEngine
import io.github.ferricbrowser 1.0
import "../scripts/BrowserScripts.js" as BrowserScripts
import "../scripts/SpellcheckPresentation.js" as SpellcheckPresentation

FerricBrowserRuntimeBase {
    id: window

    readonly property var browserUi: window.primaryBrowserUi
    readonly property var requestInterceptor: window.primaryRequestInterceptor
    readonly property var blocklistUpdater: window.blocklistUpdaterObject
    readonly property var linkRuleUpdater: window.linkRuleUpdaterObject
    readonly property var mprisController: window.mprisControllerObject
    readonly property var browserKeyRouter: window.browserKeyRouterObject
    readonly property var applicationShutdownTimer: window.applicationShutdownTimerObject
    readonly property var shutdownPageProbeTimer: window.shutdownPageProbeTimerObject
    readonly property var blocklistUpdateTimer: window.blocklistUpdateTimerObject
    readonly property var linkCleaningUpdateTimer: window.linkCleaningUpdateTimerObject
    readonly property var bindingOverlayTimer: window.bindingOverlayTimerObject
    readonly property var engineUpdateNoticeTimer: window.engineUpdateNoticeTimerObject
    readonly property var externalOpenPortalTimer: window.externalOpenPortalTimerObject
    readonly property var notificationPresenter: window.notificationPresenterObject
    readonly property var siteDataClearPollTimer: window.siteDataClearPollTimerObject

    readonly property alias clipboardBridgeObject: clipboardBridge
    readonly property alias activationOutcomeTimerObject: activationOutcomeTimer
    readonly property alias fileDialogSurfacesObject: fileDialogSurfaces
    readonly property alias certificatePromptsObject: certificatePrompts
    readonly property alias webAuthPromptObject: webAuthPrompt
    readonly property alias contextMenuObject: contextMenu
    readonly property alias tabsModel: tabs
    readonly property alias namedSessionsModel: namedSessions
    readonly property alias profilesModel: profiles
    readonly property alias downloadsModel: downloads
    readonly property alias libraryEntriesModel: libraryEntries
    readonly property alias libraryGraphEntriesModel: libraryGraphEntries
    readonly property alias libraryGraphNodesModel: libraryGraphNodes

    FerricBindingOverlay {
        browserWindow: window
        browserUi: window.browserUi
    }

    function runBrowserScript(view, script, callback, world) {
        if (!view || !script) {
            return false
        }
        var executionWorld = world === undefined ? window.browserScriptWorld : world
        if (callback) {
            view.runJavaScript(script, executionWorld, callback)
        } else {
            view.runJavaScript(script, executionWorld)
        }
        return true
    }

    function closeDevTools() {
        var detached = window.devToolsDetachedWindow
        window.devToolsDetachedWindow = null
        window.devToolsExternalView = null
        window.devToolsDetached = false
        window.devToolsVisible = false
        if (detached) {
            detached.close()
        }
    }

    function toggleDevTools(detach) {
        var inspected = window.activeWebView()
        if (!inspected) {
            browserUi.status_text = "DevTools target is unavailable"
            return false
        }
        if (window.devToolsVisible) {
            if (detach === window.devToolsDetached) {
                window.closeDevTools()
                browserUi.status_text = "DevTools closed"
                return true
            }
            window.closeDevTools()
        }
        if (detach) {
            var detached = window.devToolsWindowFactory.createObject(null, {
                inspectView: inspected,
                ownerWindow: window
            })
            if (!detached) {
                browserUi.status_text = "DevTools window could not be created"
                return false
            }
            window.devToolsDetachedWindow = detached
            window.devToolsExternalView = detached.inspectorView
            window.devToolsDetached = true
            window.devToolsVisible = true
            detached.show()
            browserUi.status_text = "DevTools detached"
        } else {
            window.devToolsDetached = false
            window.devToolsVisible = true
            browserUi.status_text = "DevTools attached"
        }
        inspected.forceActiveFocus()
        return true
    }

    function scrollScript(kind, direction, half, count) {
        return BrowserScripts.scroll(kind, direction, half, count)
    }

    function searchFindFlags(query, caseMode, backward) {
        var flags = backward ? WebEngineView.FindBackward : 0
        var sensitive = caseMode === "sensitive"
        if (caseMode === "smart") {
            var text = String(query || "")
            var characters = Array.from(text)
            for (var index = 0; index < characters.length; ++index) {
                var character = characters[index]
                var upper = character.toUpperCase()
                var lower = character.toLowerCase()
                if (character === upper && character !== lower) {
                    sensitive = true
                    break
                }
            }
        }
        if (sensitive) {
            flags |= WebEngineView.FindCaseSensitively
        }
        return flags
    }

    function captureScrollPosition(ui, index, view) {
        if (!ui || !view || index < 0 || !view.url || view.url.toString() === "about:blank") {
            return
        }
        var capturedUrl = view.url.toString()
        window.runBrowserScript(view,
            BrowserScripts.scrollPosition(),
            function(value) {
                if (view.tabIndex !== index || view.url.toString() !== capturedUrl || !value) {
                    return
                }
                var x = Number(value.x)
                var y = Number(value.y)
                if (isFinite(x) && isFinite(y) && x >= 0 && y >= 0) {
                    ui.set_scroll_position(index, x, y)
                }
            })
    }

    function restoreScrollPosition(view, x, y) {
        x = Number(x)
        y = Number(y)
        if (!view || !isFinite(x) || !isFinite(y) || x < 0 || y < 0) {
            return
        }
        window.runBrowserScript(view,
            BrowserScripts.restoreScrollPosition(x, y))
    }

    function siteDataClearScript() {
        return BrowserScripts.clearSiteData()
    }

    function clearSiteDataClearForView(view) {
        if (!window.siteDataClearPending || window.siteDataClearView !== view) {
            return
        }
        siteDataClearPollTimer.stop()
        window.siteDataClearPending = false
        window.siteDataClearView = null
        var ui = window.siteDataClearUi || browserUi
        window.siteDataClearUi = null
        ui.site_data_clear_finished(
                    window.siteDataClearOrigin,
                    JSON.stringify({"error": "site-data clear cancelled by navigation"}))
        window.siteDataClearOrigin = ""
    }

    function siteRuleSettingsFor(ui, url) {
        if (!ui || !ui.select_site_rule_settings || !url) {
            return { values: {}, matched_rules: [] }
        }
        var experimentDocument = String(ui.site_experiment_url || "").split(/[?#]/)[0]
        var currentDocument = String(url).split(/[?#]/)[0]
        if (ui.site_experiment_active
                && ui.site_experiment_kind === "compiled-defaults"
                && experimentDocument === currentDocument) {
            return { values: {}, matched_rules: [] }
        }
        if (!ui.select_site_rule_settings(url)) {
            return { values: {}, matched_rules: [] }
        }
        var values = ({})
        if (ui.site_rule_javascript_set) {
            values["content.javascript"] = ui.site_rule_javascript_enabled
        }
        if (ui.site_rule_images_set) {
            values["content.images"] = ui.site_rule_images_enabled
        }
        if (ui.site_rule_force_dark_set) {
            values["content.force_dark"] = ui.site_rule_force_dark_enabled
        }
        if (ui.site_rule_autoplay_set) {
            values["content.autoplay"] = String(ui.site_rule_autoplay)
        }
        if (ui.site_rule_zoom_set) {
            values["content.zoom"] = Number(ui.site_rule_zoom)
        }
        return { values: values, matched_rules: [] }
    }

    function siteRuleValue(settings, key, fallback) {
        if (settings && settings.values
                && settings.values[key] !== undefined) {
            return settings.values[key]
        }
        return fallback
    }

    function focusObserverSource() {
        return BrowserScripts.focusObserverSource()
    }

    function installFocusObserver(view) {
        if (!view || !view.userScripts) {
            return false
        }
        var existing = view.userScripts.find("ferric-browser-focus-observer")
        if (existing && existing.length > 0) {
            return false
        }
        var script = WebEngine.script()
        script.name = "ferric-browser-focus-observer"
        script.sourceCode = window.focusObserverSource()
        script.injectionPoint = WebEngineScript.DocumentCreation
        script.worldId = window.browserScriptWorld
        script.runsOnSubFrames = true
        view.userScripts.insert(script)
        return true
    }

    function focusProbeScript() {
        return BrowserScripts.focusProbe()
    }

    function probeFocusFrame(ui, index, frame, framePath, url) {
        if (!ui || !frame || !frame.isValid) {
            return
        }
        frame.runJavaScript(window.focusProbeScript(), window.browserScriptWorld, function(value) {
            var state = value || {sequence: 0, editable: false,
                                  user_activated: false, kind: "unknown"}
            ui.page_focus_observed_for(index, url, framePath,
                Number(state.sequence) || 0, !!state.editable, !!state.user_activated)
        })
        var children = frame.children || []
        for (var i = 0; i < children.length; ++i) {
            window.probeFocusFrame(ui, index, children[i], framePath + "." + i, url)
        }
    }

    function probeActiveFocus(ui, index, view) {
        if (!ui || !view || !view.mainFrame || !view.mainFrame.isValid) {
            return
        }
        window.probeFocusFrame(ui, index, view.mainFrame, "0", view.url.toString())
    }

    function cosmeticHostMatches(host, pattern) {
        host = String(host || "").toLowerCase().replace(/\.+$/, "")
        pattern = String(pattern || "").toLowerCase().replace(/\.+$/, "")
        if (!host || !pattern) {
            return false
        }
        if (pattern.indexOf("*.") === 0) {
            var suffix = pattern.slice(2)
            return host !== suffix && host.slice(-(suffix.length + 1)) === "." + suffix
        }
        return host === pattern
    }

    function injectCosmeticRules(ui, view) {
        if (window.safeMode || !ui || !view) {
            return
        }
        var ruleHosts = ui.blocking_cosmetic_rule_hosts
        var ruleSelectors = ui.blocking_cosmetic_rule_selectors
        var exceptionHosts = ui.blocking_cosmetic_exception_hosts
        var exceptionSelectors = ui.blocking_cosmetic_exception_selectors
        if (ruleHosts.length !== ruleSelectors.length
                || exceptionHosts.length !== exceptionSelectors.length) {
            ui.status_text = "Cosmetic filter metadata was incomplete"
            return
        }
        var host = view.url && view.url.host ? String(view.url.host) : ""
        var selectors = []
        for (var i = 0; i < ruleHosts.length && selectors.length < 256; ++i) {
            var ruleHost = String(ruleHosts[i])
            var ruleSelector = String(ruleSelectors[i])
            if (!cosmeticHostMatches(host, ruleHost)) {
                continue
            }
            var excluded = false
            for (var j = 0; j < exceptionHosts.length; ++j) {
                if (String(exceptionSelectors[j]) === ruleSelector
                        && cosmeticHostMatches(host, String(exceptionHosts[j]))) {
                    excluded = true
                    break
                }
            }
            if (!excluded && ruleSelector.length > 0 && selectors.indexOf(ruleSelector) < 0) {
                selectors.push(ruleSelector)
            }
        }
        if (selectors.length === 0) {
            return
        }
        var css = selectors.join(" { display: none !important; }\n")
                + " { display: none !important; }"
        var source = BrowserScripts.cosmeticFilter(css)
        window.runBrowserScript(view, source, function(ok) {
            if (ok === false) {
                ui.status_text = "Cosmetic filter injection failed"
            }
        }, WebEngineScript.MainWorld)
    }

    function matchingPageUserscriptsFor(ui, url, privateProfile) {
        if (!ui.select_matching_page_scripts(url, privateProfile)) {
            return []
        }
        var names = ui.page_userscript_names
        var sources = ui.page_userscript_sources
        var runAt = ui.page_userscript_run_at
        var runsOnSubFrames = ui.page_userscript_runs_on_sub_frames
        if (names.length !== sources.length || names.length !== runAt.length
                || names.length !== runsOnSubFrames.length) {
            ui.status_text = "Page userscript metadata was incomplete"
            return []
        }
        var scripts = []
        for (var i = 0; i < names.length; ++i) {
            scripts.push({
                name: String(names[i]),
                source: String(sources[i]),
                run_at: String(runAt[i]),
                runs_on_sub_frames: String(runsOnSubFrames[i]) === "true"
            })
        }
        return scripts
    }

    function injectPageUserscripts(ui, view, url, privateProfile, phase) {
        if (window.safeMode || window.userscriptsOff
                || !ui || !view || !url || !ui.select_matching_page_scripts) {
            return
        }
        if (window.siteDoctorUserscriptsDisabled(ui)) {
            return
        }
        var scripts = window.matchingPageUserscriptsFor(ui, url, privateProfile)
        for (var i = 0; i < scripts.length; ++i) {
            var script = scripts[i]
            if (script.run_at !== phase) {
                continue
            }
            if (script.runs_on_sub_frames) {
                continue
            }
            (function(pageUi, scriptName, scriptSource) {
                var source = BrowserScripts.pageUserscriptRun(scriptSource)
                window.runBrowserScript(view, source, function(ok) {
                    if (ok === false) {
                        pageUi.status_text = "Page userscript failed: " + scriptName
                    }
                }, WebEngineScript.MainWorld)
            })(ui, script.name, script.source)
        }
    }

    function clearInstalledPageUserscripts(view) {
        if (!view || !view.userScripts) {
            return
        }
        var names = view.pageUserScriptNames || []
        for (var i = 0; i < names.length; ++i) {
            var installed = view.userScripts.find(names[i])
            for (var j = 0; j < installed.length; ++j) {
                view.userScripts.remove(installed[j])
            }
        }
        view.pageUserScriptNames = []
    }

    function installPageUserscripts(ui, view, url, privateProfile) {
        if (window.safeMode || window.userscriptsOff
                || !ui || !view || !url || !ui.select_matching_page_scripts || !view.userScripts) {
            return
        }
        if (window.siteDoctorUserscriptsDisabled(ui)) {
            return
        }
        window.clearInstalledPageUserscripts(view)
        var scripts = window.matchingPageUserscriptsFor(ui, url, privateProfile)
        var names = []
        for (var i = 0; i < scripts.length; ++i) {
            var script = scripts[i]
            if (!script.runs_on_sub_frames) {
                continue
            }
            var injectionPoint = WebEngineScript.Deferred
            if (script.run_at === "document_start") {
                injectionPoint = WebEngineScript.DocumentCreation
            } else if (script.run_at === "document_end") {
                injectionPoint = WebEngineScript.DocumentReady
            }
            var installedName = "ferric-browser-page-userscript-" + script.name
            var installedScript = WebEngine.script()
            installedScript.name = installedName
            installedScript.sourceCode = BrowserScripts.pageUserscriptInstall(script.source)
            installedScript.injectionPoint = injectionPoint
            installedScript.worldId = WebEngineScript.MainWorld
            installedScript.runsOnSubFrames = true
            view.userScripts.insert(installedScript)
            names.push(installedName)
        }
        view.pageUserScriptNames = names
        if (names.length === 0) {
            view.pageUserScriptReloadUrl = ""
            return
        }
        if (view.pageUserScriptReloadUrl === url) {
            return
        }
        view.pageUserScriptReloadUrl = url
        Qt.callLater(function() {
            if (view.url && view.url.toString() === url) {
                view.reload()
            }
        })
    }

    TextField {
        id: clipboardBridge
        x: -1000
        y: -1000
        width: 1
        height: 1
        opacity: 0
        readOnly: true
        visible: true
    }

    Timer {
        id: copyNoticeTimer
        interval: 6000
        repeat: false
        onTriggered: window.copyNoticeVisible = false
    }

    Timer {
        id: activationOutcomeTimer
        interval: 250
        repeat: false
        onTriggered: {
            var host = window.pendingActivationHost
            var ui = window.pendingActivationUi || browserUi
            var operationId = window.pendingActivationOperationId
            if (!host || !ui) {
                window.activationStatus = "unknown"
                return
            }
            if (host.active) {
                window.activationStatus = "activated"
                host.forceActiveFocus()
                if (operationId.length > 0) {
                    ui.complete_window_focus(operationId, "activated")
                }
                ui.status_text = "Window activated"
                var workspaceHost = window.pendingWorkspaceRouteHost
                var workspaceUi = window.pendingWorkspaceRouteUi
                var workspace = window.pendingWorkspaceRoute
                if (workspaceHost === host && workspaceUi === ui && workspace.length > 0) {
                    window.pendingWorkspaceRouteHost = null
                    window.pendingWorkspaceRouteUi = null
                    window.pendingWorkspaceRoute = ""
                    if (!ui.hyprland_route_workspace(workspace)) {
                        ui.status_text = "Workspace routing was denied by the compositor"
                    }
                }
            } else {
                window.activationStatus = "unknown"
                ui.status_text = "Window activation outcome unknown; compositor may deny focus"
                if (operationId.length > 0) {
                    ui.complete_window_focus(operationId, "unknown")
                }
                if (window.pendingWorkspaceRouteHost === host) {
                    window.pendingWorkspaceRouteHost = null
                    window.pendingWorkspaceRouteUi = null
                    window.pendingWorkspaceRoute = ""
                    ui.status_text = "Workspace routing denied because window activation was denied"
                }
            }
            window.pendingActivationHost = null
            window.pendingActivationUi = null
            window.pendingActivationOperationId = ""
        }
    }

    FerricFileDialogSurfaces {
        id: fileDialogSurfaces
        onDownloadAccepted: window.acceptPendingDownload()
        onDownloadRejected: window.cancelPendingDownload()
        onJourneyExportAccepted: window.finishJourneyExport()
        onJourneyExportRejected: window.journeyExportPreviewVisible = true
        onDiagnosticsExportAccepted: window.finishDiagnosticsExport()
        onDiagnosticsExportRejected: window.diagnosticsVisible = true
        onEngineFileAccepted: window.acceptEngineFileDialog()
        onEngineFileRejected: window.rejectEngineFileDialog()
        onEngineFolderAccepted: window.acceptEngineFolderDialog()
        onEngineFolderRejected: window.rejectEngineFileDialog()
        onUserscriptManifestAccepted: window.installUserscriptManifest()
    }

    FerricCertificatePrompts {
        id: certificatePrompts
        browserWindow: window
        onClientCertificateAccepted: function(index) {
            window.acceptClientCertificate(index)
        }
        onClientCertificateRejected: window.rejectClientCertificate()
        onCertificateAccepted: window.acceptCertificateError()
        onCertificateRejected: window.rejectCertificateError()
    }

    FerricWebAuthPrompt {
        id: webAuthPrompt
        browserWindow: window
        onAccountSelected: function(account) {
            window.selectWebAuthAccount(account)
        }
        onPinSubmitted: window.submitWebAuthPin()
        onCancelled: window.cancelWebAuth()
        onRetryRequested: window.retryWebAuth()
    }

    FerricContextMenu {
        id: contextMenu
        browserWindow: window
        onItemActivated: function(item) {
            window.activateContextMenuItem(item)
        }
        onDismissed: window.clearContextMenuRequest()
    }

    FerricUserscriptRemovalDialog {
        id: userscriptRemovalDialog
        browserWindow: window
        onConfirmed: {
            var name = window.pendingUserscriptRemoval
            window.pendingUserscriptRemoval = ""
            if (browserUi.remove_userscript(name)) {
                window.settingsNotice = "Removing userscript…"
            }
        }
        onCancelled: window.pendingUserscriptRemoval = ""
    }

    function copyToClipboard(value, exposeValue, primary) {
        if (!value || value.length === 0) {
            return
        }
        if (exposeValue === undefined) {
            exposeValue = true
        }
        if (!browserUi.write_clipboard(value, primary === true)) {
            return
        }
        window.copiedValue = value
        window.copyNoticeSensitive = !exposeValue
        window.copiedText = exposeValue ? value : "Selected document text (content hidden)"
        window.copyNoticeVisible = true
        copyNoticeTimer.restart()
    }

    function fileDialogNameFilters(request) {
        var filters = []
        var patternsByMime = {
            "text/plain": ["*.txt", "*.text"],
            "text/html": ["*.html", "*.htm"],
            "application/pdf": ["*.pdf"],
            "application/json": ["*.json"],
            "application/zip": ["*.zip"],
            "image/*": ["*.bmp", "*.gif", "*.jpeg", "*.jpg", "*.png", "*.webp"],
            "audio/*": ["*.aac", "*.flac", "*.mp3", "*.ogg", "*.wav"],
            "video/*": ["*.avi", "*.mkv", "*.mov", "*.mp4", "*.webm"]
        }
        for (var i = 0; i < request.acceptedMimeTypes.length; ++i) {
            var mime = request.acceptedMimeTypes[i]
            var patterns = patternsByMime[mime]
            if (patterns) {
                filters.push(mime + " (" + patterns.join(" ") + ")")
            }
        }
        if (filters.length === 0) {
            return ["All files (*)"]
        }
        filters.push("All files (*)")
        return filters
    }

    function fileDialogPaths() {
        var paths = []
        var urls = fileDialogSurfaces.engineFileMode === FileDialog.OpenFiles
                ? fileDialogSurfaces.engineFileSelectedFiles
                : [fileDialogSurfaces.engineFileSelectedFile]
        for (var i = 0; i < urls.length; ++i) {
            var path = urls[i].toLocalFile()
            if (!path || urls[i].scheme !== "file") {
                return []
            }
            paths.push(path)
        }
        return paths
    }

    function fileUrlForPath(path) {
        var segments = String(path || "").split("/")
        for (var i = 0; i < segments.length; ++i) {
            segments[i] = encodeURIComponent(segments[i])
        }
        return "file://" + segments.join("/")
    }

    function folderDialogPaths() {
        var url = fileDialogSurfaces.engineFolderSelectedFolder
        var path = url && url.scheme === "file" ? url.toLocalFile() : ""
        return path ? [path] : []
    }

    function desktopPortalMode(ui) {
        return ui && ui.desktop_portal_mode
                ? String(ui.desktop_portal_mode) : "auto"
    }

    function desktopPortalCapabilityStatus(ui, capability) {
        // Portal availability is user-session global, so secondary and popup
        // browser objects use the root probe result instead of maintaining
        // independent D-Bus probes.
        return browserUi && browserUi.portal_capability_status
                ? browserUi.portal_capability_status(capability) : "not-probed"
    }

    function openPendingEngineFileDialog() {
        var request = window.pendingFileDialogRequest
        if (!request) {
            return
        }
        window.pendingFileDialogWaitingForPortal = false
        if (request.mode === FileDialogRequest.FileModeUploadFolder) {
            fileDialogSurfaces.openEngineFolder()
            return
        }
        fileDialogSurfaces.engineFileMode = request.mode === FileDialogRequest.FileModeOpenMultiple
                ? FileDialog.OpenFiles
                : request.mode === FileDialogRequest.FileModeSave
                    ? FileDialog.SaveFile
                    : FileDialog.OpenFile
        fileDialogSurfaces.engineFileNameFilters = window.fileDialogNameFilters(request)
        fileDialogSurfaces.engineFileCurrentFile = request.defaultFileName.length > 0
                ? request.defaultFileName : ""
        fileDialogSurfaces.engineFileTitle = request.mode === FileDialogRequest.FileModeSave
                ? "Save file"
                : request.mode === FileDialogRequest.FileModeOpenMultiple
                    ? "Choose files"
                    : "Choose file"
        fileDialogSurfaces.openEngineFile()
    }

    function maybeOpenPendingEngineFileDialog() {
        if (!window.pendingFileDialogWaitingForPortal
                || !window.pendingFileDialogRequest) {
            return
        }
        if (Date.now() >= window.pendingFileDialogPortalDeadlineMs) {
            window.clearEngineFileDialog(true,
                                         "Desktop portal check timed out; file selection cancelled")
            return
        }
        var ui = window.pendingFileDialogUi || browserUi
        var status = window.desktopPortalCapabilityStatus(ui, "file_chooser")
        if (status === "pending" || status === "not-probed") {
            return
        }
        if (status !== "available") {
            window.clearEngineFileDialog(true,
                                         "Required desktop portal unavailable; file selection cancelled")
            return
        }
        window.openPendingEngineFileDialog()
    }

    function clearEngineFileDialog(cancelRequest, message) {
        var request = window.pendingFileDialogRequest
        var hadRequest = !!request
        var ui = window.pendingFileDialogUi || browserUi
        window.pendingFileDialogRequest = null
        window.pendingFileDialogView = null
        window.pendingFileDialogUi = null
        window.pendingFileDialogWaitingForPortal = false
        window.pendingFileDialogPortalDeadlineMs = 0
        if (fileDialogSurfaces.engineFileVisible) {
            fileDialogSurfaces.closeEngineFile()
        }
        if (fileDialogSurfaces.engineFolderVisible) {
            fileDialogSurfaces.closeEngineFolder()
        }
        if (hadRequest) {
            window.restoreOverlayFocus()
        }
        if (cancelRequest && request) {
            window.resolveQtRequest(ui, request, "file-dialog", "dialogReject", [])
        }
        if (message && ui) {
            ui.status_text = message
        }
    }

    function acceptEngineFileDialog() {
        var request = window.pendingFileDialogRequest
        var ui = window.pendingFileDialogUi || browserUi
        var paths = window.fileDialogPaths()
        window.pendingFileDialogRequest = null
        window.pendingFileDialogView = null
        window.pendingFileDialogUi = null
        if (!request || paths.length === 0) {
            if (request) {
                window.resolveQtRequest(ui, request, "file-dialog", "dialogReject", [])
            }
            if (request) {
                window.restoreOverlayFocus()
            }
            ui.status_text = "File selection returned no local paths"
            return
        }
        window.resolveQtRequest(ui, request, "file-dialog", "dialogAccept", [paths])
        window.restoreOverlayFocus()
        ui.status_text = paths.length === 1
                ? "File selected"
                : paths.length + " files selected"
    }

    function acceptEngineFolderDialog() {
        var request = window.pendingFileDialogRequest
        var paths = window.folderDialogPaths()
        var ui = window.pendingFileDialogUi || browserUi
        window.pendingFileDialogRequest = null
        window.pendingFileDialogView = null
        window.pendingFileDialogUi = null
        if (!request || paths.length === 0) {
            if (request) {
                window.resolveQtRequest(ui, request, "file-dialog", "dialogReject", [])
            }
            if (request) {
                window.restoreOverlayFocus()
            }
            ui.status_text = "Folder selection returned no local path"
            return
        }
        window.resolveQtRequest(ui, request, "file-dialog", "dialogAccept", [paths])
        window.restoreOverlayFocus()
        ui.status_text = "Folder selected"
    }

    function rejectEngineFileDialog() {
        var request = window.pendingFileDialogRequest
        var ui = window.pendingFileDialogUi || browserUi
        if (!request) {
            return
        }
        window.pendingFileDialogRequest = null
        window.pendingFileDialogView = null
        window.pendingFileDialogUi = null
        window.pendingFileDialogWaitingForPortal = false
        window.resolveQtRequest(ui, request, "file-dialog", "dialogReject", [])
        window.restoreOverlayFocus()
        ui.status_text = "File selection cancelled"
    }

    function handleFileDialogRequested(request, view, ui) {
        if (window.pendingFileDialogRequest) {
            window.clearEngineFileDialog(true, "Previous file selection cancelled")
            window.resolveQtRequest(ui || browserUi, request, "file-dialog", "dialogReject", [])
            (ui || browserUi).status_text = "Another file selection is already open"
            return
        }
        window.captureOverlayFocus(window, view || window.activeWebView())
        window.pendingFileDialogRequest = request
        window.pendingFileDialogView = view || null
        window.pendingFileDialogUi = ui || browserUi
        var requestUi = window.pendingFileDialogUi
        if (window.desktopPortalMode(requestUi) === "required") {
            var status = window.desktopPortalCapabilityStatus(requestUi, "file_chooser")
            if (status === "not-probed") {
                browserUi.probe_desktop_portals()
                status = window.desktopPortalCapabilityStatus(requestUi, "file_chooser")
            }
            if (status === "pending" || status === "not-probed") {
                window.pendingFileDialogWaitingForPortal = true
                window.pendingFileDialogPortalDeadlineMs = Date.now() + 5000
                requestUi.status_text = "Checking required desktop portal…"
                return
            }
            if (status !== "available") {
                window.clearEngineFileDialog(true,
                                             "Required desktop portal unavailable; file selection cancelled")
                return
            }
        }
        window.openPendingEngineFileDialog()
    }

    function clearFileDialogForView(view) {
        if (window.pendingFileDialogView === view) {
            window.clearEngineFileDialog(true, "File selection cancelled by navigation")
        }
    }

    ListModel {
        id: tabs
        ListElement {
            tabId: ""
            url: "about:blank"
            title: "New tab"
            loaded: true
            pinned: false
            muted: false
            zoom: 1.0
            suspended: false
            discarded: false
        }
    }

    property var tabViews: []
    property var preparedTransferFallback: null

    function tabViewAt(index) {
        return index >= 0 && index < tabViews.length ? tabViews[index] : null
    }

    function removeTabViewAt(index, transferParent) {
        var view = tabViewAt(index)
        if (!view) {
            return null
        }
        tabViews.splice(index, 1)
        if (transferParent) {
            view.parent = transferParent
        } else {
            // WebEngineView owns a compositor texture. Remove it from the
            // scene before deferred destruction so its last frame cannot
            // overlap the newly selected tab during renderer teardown.
            view.visible = false
            view.enabled = false
            view.parent = null
            Qt.callLater(function() {
                if (view) {
                    view.destroy()
                }
            })
        }
        return view
    }

    function detachTabViewAt(index) {
        var view = tabViewAt(index)
        if (!view) {
            return null
        }
        tabViews.splice(index, 1)
        view.parent = null
        view.visible = false
        return view
    }

    function prepareTransferFallback() {
        if (browserUi.tab_count !== 1) {
            return true
        }
        if (window.preparedTransferFallback) {
            return true
        }
        var fallback = webViewComponent.createObject(webViews, {
            tabIndex: -1,
            viewUi: browserUi,
            viewTabs: tabs,
            viewModel: {
                loaded: true,
                url: "about:blank",
                title: "New tab",
                muted: false,
                zoom: 1.0,
                scrollX: -1,
                scrollY: -1
            },
            viewProfile: browserProfile,
            viewInterceptor: requestInterceptor,
            viewTransientProfile: window.temporaryProfile,
            viewTransferred: false
        })
        if (!fallback) {
            browserUi.status_text = "Transfer fallback view could not be created"
            return false
        }
        fallback.visible = false
        window.preparedTransferFallback = fallback
        return true
    }

    function discardPreparedTransferFallback() {
        if (window.preparedTransferFallback) {
            window.preparedTransferFallback.destroy()
            window.preparedTransferFallback = null
        }
    }

    ListModel {
        id: namedSessions
    }

    ListModel {
        id: profiles
    }

    ListModel {
        id: downloads
    }

    ListModel {
        id: libraryEntries
    }

    ListModel {
        id: libraryGraphEntries
    }

    ListModel {
        id: libraryGraphNodes
    }

    function refreshSessions() {
        namedSessions.clear()
        var names = browserUi.list_named_sessions()
        if (names.length === 0) {
            return
        }
        var values = names.split("\n")
        for (var i = 0; i < values.length; ++i) {
            if (values[i].length > 0) {
                namedSessions.append({ name: values[i] })
            }
        }
    }

    function showSessionPreview(name, append) {
        if (!browserUi.request_session_preview(name)) {
            return
        }
        sessionPreviewName = name
        sessionPreviewAppend = append
        sessionPreviewLoading = true
        sessionPreviewError = false
        sessionPreviewText = "Loading validated session descriptors…"
        window.openInternalSurface()
        sessionPreviewVisible = true
    }

    function showCommandSessionPreview(payload) {
        var lines = payload.split("\n")
        if (lines.length < 2) {
            return
        }
        sessionPreviewName = lines[1]
        sessionPreviewAppend = lines[0] === "append"
        sessionPreviewLoading = true
        sessionPreviewError = false
        sessionPreviewText = "Loading validated session descriptors…"
        window.openInternalSurface()
        sessionPreviewVisible = true
    }

    function loadPreviewedSession() {
        if (sessionPreviewError
                || !browserUi.request_named_session_load(sessionPreviewName, sessionPreviewAppend)) {
            return
        }
        sessionPreviewLoading = true
        sessionPreviewError = false
        sessionPreviewText = "Loading validated session…"
        window.syncTabModel()
        window.executePendingEngineAction()
    }

    function showReopenWindowConfirmation(sourceUi, sourceHost, action) {
        reopenWindowConfirmationUi = sourceUi
        reopenWindowConfirmationHost = sourceHost
        reopenWindowConfirmationAction = action
        window.openInternalSurface()
        reopenWindowConfirmationVisible = true
    }

    function confirmReopenWindow() {
        var action = reopenWindowConfirmationAction
        var sourceUi = reopenWindowConfirmationUi
        var sourceHost = reopenWindowConfirmationHost
        reopenWindowConfirmationVisible = false
        reopenWindowConfirmationAction = ""
        reopenWindowConfirmationUi = null
        reopenWindowConfirmationHost = null
        window.closeInternalSurface()
        if (!sourceUi || !sourceHost || action.indexOf("reopen-window-confirm\t") !== 0) {
            browserUi.status_text = "Reopen-in-window confirmation expired"
            return
        }
        var confirmedAction = "reopen-window\t" + action.split("\t").slice(1).join("\t")
        window.openReopenedWindow(sourceUi, sourceHost, confirmedAction)
    }

    function cancelReopenWindow() {
        reopenWindowConfirmationVisible = false
        reopenWindowConfirmationAction = ""
        reopenWindowConfirmationUi = null
        reopenWindowConfirmationHost = null
        window.closeInternalSurface()
        browserUi.status_text = "Reopen-in-window cancelled"
    }

    function refreshProfiles() {
        profiles.clear()
        if (!browserUi.request_profile_list()) {
            return
        }
        profileRefreshAttempts = 0
        profileSessionSurfaces.restartProfileRefresh()
    }

    function applyProfileValues() {
        var values = browserUi.profile_values
        if (values.length === 0) {
            return
        }
        var lines = values.split("\n")
        for (var i = 0; i < lines.length; ++i) {
            var fields = lines[i].split("\t")
            if (fields.length >= 3) {
                profiles.append({ name: fields[0], label: fields[1], id: fields[2] })
            }
        }
    }

    function scheduleProfileRefresh() {
        refreshProfiles()
    }

    function showSiteLedger() {
        var payload = browserUi.refresh_site_status()
        if (!payload || payload.length === 0) {
            return
        }
        try {
            window.siteLedgerData = JSON.parse(payload)
            var activeCapture = window.siteLedgerData.private
                    ? null : window.captureSessionForView(window.activeWebView())
            var facts = window.siteLedgerData.facts || []
            for (var factIndex = 0; factIndex < facts.length; factIndex++) {
                if (facts[factIndex] && facts[factIndex].id === "capture") {
                    facts[factIndex].value = activeCapture
                            ? { status: activeCapture.status,
                                origin: activeCapture.origin,
                                provenance: "browser-owned capture ledger" }
                            : { status: "inactive",
                                reason: "no active capture is recorded for this tab",
                                provenance: "browser-owned capture ledger" }
                    facts[factIndex].state = activeCapture ? "active" : "inactive"
                    facts[factIndex].capability = activeCapture ? "available" : "not-active"
                    break
                }
            }
            window.siteLedgerData.facts = facts
            if (!window.siteLedgerVisible) {
                window.openInternalSurface()
            }
            window.siteLedgerVisible = true
        } catch (error) {
            browserUi.status_text = "Site Ledger response was invalid"
        }
    }

    function refreshDiagnostics() {
        var payload = browserUi.diagnostics_json()
        diagnosticsText = payload && payload.length > 0
                ? payload
                : "{\"error\":\"Diagnostics snapshot was unavailable\"}"
    }

    function refreshEngineUpdateNotice() {
        var payload = browserUi.diagnostics_json()
        if (!payload || payload.length === 0) {
            engineUpdateStatus = "unavailable"
            engineUpdateNotice = "Engine qualification status unavailable; inspect diagnostics"
            return
        }
        try {
            var snapshot = JSON.parse(payload)
            var policy = snapshot.build && snapshot.build.engine_update_policy
                    ? snapshot.build.engine_update_policy : null
            var status = policy && policy.status ? String(policy.status) : "unavailable"
            engineUpdateStatus = status
            if (status === "blocked") {
                engineUpdateNotice = "QtWebEngine build is blocked; update it with the system package manager"
            } else if (status === "unqualified") {
                engineUpdateNotice = "QtWebEngine build is unqualified; verify the installed package before relying on compatibility"
            } else if (status === "unavailable") {
                engineUpdateNotice = "Engine qualification status unavailable; inspect diagnostics"
            } else {
                engineUpdateNotice = ""
            }
        } catch (error) {
            engineUpdateStatus = "unavailable"
            engineUpdateNotice = "Engine qualification status unavailable; inspect diagnostics"
        }
    }

    function showDiagnostics() {
        if (!window.diagnosticsVisible) {
            window.openInternalSurface()
        }
        window.refreshDiagnostics()
        window.diagnosticsVisible = true
    }

    function closeDiagnostics() {
        var wasVisible = window.diagnosticsVisible
        window.diagnosticsVisible = false
        if (wasVisible) {
            window.closeInternalSurface()
        }
    }

    function rebuildBindingHelpRows() {
        var kinds = browserUi.binding_help_row_kinds || []
        var titles = browserUi.binding_help_row_titles || []
        var modes = browserUi.binding_help_row_modes || []
        var commands = browserUi.binding_help_row_commands || []
        var descriptions = browserUi.binding_help_row_descriptions || []
        var keys = browserUi.binding_help_row_keys || []
        var sources = browserUi.binding_help_row_sources || []
        var counts = browserUi.binding_help_row_counts || []
        var rows = []
        for (var index = 0; index < kinds.length; ++index) {
            rows.push({
                kind: kinds[index],
                title: titles[index],
                mode: modes[index],
                command: commands[index],
                description: descriptions[index],
                keys: keys[index],
                source: sources[index],
                count: counts[index]
            })
        }
        window.bindingHelpRows = rows
    }

    function refreshBindingHelp() {
        browserUi.refresh_binding_help(window.bindingHelpSearch)
        window.rebuildBindingHelpRows()
    }

    function showBindingHelp(search) {
        if (!window.bindingHelpVisible) {
            window.openInternalSurface()
        }
        if (search !== undefined) {
            window.bindingHelpSearch = String(search)
        }
        window.refreshBindingHelp()
        window.bindingHelpVisible = true
        Qt.callLater(function() { helpSearchInput.forceActiveFocus() })
    }

    function closeBindingHelp() {
        var wasVisible = window.bindingHelpVisible
        window.bindingHelpVisible = false
        if (wasVisible) {
            window.closeInternalSurface()
        }
    }

    function refreshSettings() {
        var query = (window.settingsSearch || "").trim().toLowerCase()
        var visibleRows = []
        for (var index = 0; index < browserUi.settings_rows.length; ++index) {
            var row = browserUi.settings_rows[index]
            var searchable = [row.key, row.label, row.scope, row.apply]
                             .join(" ").toLowerCase()
            if (!query || searchable.indexOf(query) >= 0) {
                visibleRows.push(row)
            }
        }
        var replaced = settingsModel.replaceRows(visibleRows)
        if (!replaced) {
            window.settingsNotice = "Settings could not be displayed because their row data was inconsistent"
        }
    }

    function settingLiteral(row, value) {
        if (row.type === "bool") {
            return value ? "true" : "false"
        }
        if (row.type === "number") {
            return String(value)
        }
        if (row.type === "languages") {
            var languages = String(value || "").split(",")
            var normalized = []
            for (var index = 0; index < languages.length && index < 16; ++index) {
                var language = languages[index].trim()
                if (language.length > 0 && normalized.indexOf(language) < 0) {
                    normalized.push(language)
                }
            }
            return JSON.stringify(normalized)
        }
        return JSON.stringify(String(value))
    }

    function applySetting(row, value) {
        if (!browserUi.set_runtime_setting(
                    row.key, settingLiteral(row, value), window.settingsTemporary)) {
            window.settingsNotice = row.key + ": " + browserUi.runtime_setting_error
            return false
        }
        window.settingsNotice = "Applied " + row.key + " ("
                + (window.settingsTemporary ? "temporary" : "persistent or memory-only") + ")"
        window.refreshSettings()
        return true
    }

    function resetSetting(row) {
        if (!browserUi.unset_runtime_setting(row.key, window.settingsTemporary)) {
            window.settingsNotice = row.key + ": " + browserUi.runtime_setting_error
            return false
        }
        window.settingsNotice = "Reset " + row.key
        window.refreshSettings()
        return true
    }

    function showSettings(search) {
        if (!window.settingsVisible) {
            window.openInternalSurface()
        }
        window.settingsNotice = ""
        if (search !== undefined) {
            window.settingsSearch = String(search)
        }
        window.refreshUserscriptInventory()
        window.refreshSettings()
        window.settingsVisible = true
        Qt.callLater(function() { settingsSearchInput.forceActiveFocus() })
    }

    function chooseUserscriptManifest() {
        fileDialogSurfaces.openUserscriptManifest()
    }

    function installUserscriptManifest() {
        var selected = fileDialogSurfaces.userscriptManifestSelectedFile
        if (!selected || selected.scheme !== "file") {
            window.settingsNotice = "Choose a local userscript manifest"
            return
        }
        if (!browserUi.install_userscript_manifest(selected.toLocalFile())) {
            window.settingsNotice = "Userscript installation could not be started"
            return
        }
        window.settingsNotice = "Installing userscript…"
        window.refreshUserscriptInventory()
        window.refreshSettings()
    }

    function confirmRemoveUserscript(name) {
        if (!name || window.temporaryProfile) {
            return
        }
        window.pendingUserscriptRemoval = String(name)
        userscriptRemovalDialog.open()
    }

    function refreshUserscriptInventory() {
        browserUi.refresh_userscript_inventory()
    }

    function userscriptInventoryRows() {
        var names = browserUi.userscript_names
        var enabled = browserUi.userscript_enabled_values
        var pageWorld = browserUi.userscript_page_world_values
        var actions = browserUi.userscript_action_counts
        if (names.length !== enabled.length || names.length !== pageWorld.length
                || names.length !== actions.length) {
            return []
        }
        var rows = []
        for (var index = 0; index < names.length; ++index) {
            var actionCount = Number(actions[index])
            rows.push({
                name: String(names[index]),
                enabled: String(enabled[index]) === "true",
                page_world: String(pageWorld[index]) === "true",
                actions: isFinite(actionCount) && actionCount >= 0
                         ? Math.floor(actionCount) : 0
            })
        }
        return rows
    }

    function closeSettings() {
        var wasVisible = window.settingsVisible
        window.settingsVisible = false
        if (wasVisible) {
            window.closeInternalSurface()
        }
    }

    function siteDoctorUserscriptsDisabled(ui) {
        return !!ui && ui.site_experiment_active
                && ui.site_experiment_kind === "userscripts-off"
    }

    function siteDoctorBadge(ui) {
        if (!ui || !ui.site_experiment_active || !ui.site_experiment_kind) {
            return ""
        }
        return " · Site Doctor: " + ui.site_experiment_kind
    }

    function finishSiteDoctorAfterLoad(ui, succeeded) {
        if (!ui || !ui.site_experiment_active || !ui.site_experiment_id) {
            return
        }
        var freshView = ui.site_experiment_kind === "fresh-view"
        ui.finish_site_doctor_experiment(ui.site_experiment_id, succeeded)
        if (freshView) {
            Qt.callLater(function() { window.executePendingEngineAction() })
        }
        if (window.siteLedgerVisible) {
            Qt.callLater(window.showSiteLedger)
        }
    }

    function openProfile(name, label, requestedUrl) {
        var hasRequestedUrl = requestedUrl !== undefined
                && String(requestedUrl).length > 0
        var startupUrl = hasRequestedUrl ? String(requestedUrl) : browserUi.current_url
        var entries = window.browserWindowRegistry || []
        for (var existingIndex = 0; existingIndex < entries.length; ++existingIndex) {
            var existing = entries[existingIndex]
            if (existing && existing.ui && !existing.ephemeralProfile
                    && String(existing.ui.profile_name || existing.profileName || "") === String(name)) {
                window.activateBrowserWindow(existing.host, existing.ui)
                if (hasRequestedUrl) {
                    existing.ui.navigate_initial(startupUrl, "external-open", false)
                }
                profileManagerVisible = false
                window.closeInternalSurface()
                return
            }
        }
        browserWindowComponent.createObject(null, {
            windowStartupUrl: startupUrl,
            windowProfileName: name,
            windowProfileLabel: label,
            windowPrivateProfile: false
        })
        profileManagerVisible = false
        window.closeInternalSurface()
    }

    function showProfileDeletePreview(name) {
        if (!browserUi.request_profile_delete_preview(name)) {
            return
        }
        profileDeleteName = name
        profileDeletePreviewText = "Loading validated profile deletion preview…"
        window.openInternalSurface()
        profileDeletePreviewVisible = true
        profileSessionSurfaces.restartProfileDeletePreview()
    }

}

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

    function statusBarVisibleForMode(mode) {
        return ChromePresentation.statusBarVisible(window.statusbarMode, mode)
    }

    function showCommandNotice(text, isError) {
        var message = String(text || "").trim()
        if (message.length === 0 || message === "Normal mode") {
            return
        }
        window.commandNoticeText = message
        window.commandNoticeError = !!isError
        window.commandNoticeVisible = true
        window.runtime_commandNoticeTimer.restart()
    }

    function contrastText(background) {
        return window.runtime_chromePresentationController.contrastText(background)
    }

    function readableTextColor(candidate, background) {
        return window.runtime_chromePresentationController.readableTextColor(candidate, background)
    }

    function renderedContrastReport() {
        return window.runtime_chromePresentationController.renderedContrastReport()
    }

    function refreshChromeAppearance() {
        window.runtime_chromePresentationController.refreshChromeAppearance()
    }

    function noteTabActivity() {
        if (window.tabsMode === "switching") {
            window.tabSwitchingVisible = true
            window.runtime_tabSwitchingTimer.restart()
        }
    }

    function spellcheckEnabled(ui) {
        return SpellcheckPresentation.enabled(
                    !ui || ui.feature_spellcheck_enabled,
                    window.spellcheckDictionaryStatus(ui))
    }

    function spellcheckDictionaryStatus(ui) {
        return SpellcheckPresentation.dictionaryStatus(
                    ui ? ui.feature_spellcheck_languages : [],
                    window.spellcheckInventory || [],
                    Qt.locale().name.replace("_", "-"))
    }

    function spellcheckLanguages(ui) {
        var status = window.spellcheckDictionaryStatus(ui)
        return status.active.length > 0 ? status.active : []
    }

    function refreshSpellcheckInventory(ui) {
        var values = ui && ui.spellcheck_dictionaries ? ui.spellcheck_dictionaries() : []
        window.spellcheckInventory = values && values.length ? values.slice(0, 64) : []
    }

    function spellcheckStatusText(ui) {
        return SpellcheckPresentation.statusText(window.spellcheckDictionaryStatus(ui))
    }

    function desktopNotificationsEnabled(ui) {
        return !ui || ui.feature_desktop_notifications_enabled !== false
    }

    function pushServiceEnabled(ui, privateProfile) {
        if (privateProfile) {
            return false
        }
        return !!(ui && ui.feature_push_service_enabled)
    }

    function mediaKeysEnabled(ui) {
        return !ui || ui.feature_desktop_media_keys_enabled !== false
    }

    function handleMediaKey(event) {
        if (!event || event.key !== Qt.Key_MediaTogglePlayPause
                || !window.mediaKeysEnabled(window.browserUi)) {
            return false
        }
        var view = window.activeWebView()
        if (!window.triggerMediaToggle(view)) {
            window.browserUi.status_text = "Media toggle unavailable without an active page"
            return true
        }
        window.browserUi.status_text = "Media play/pause toggle sent to the page"
        return true
    }

    function triggerMediaToggle(view) {
        var url = view && view.url ? String(view.url) : ""
        if (!/^https?:\/\//i.test(url) || view.recentlyAudible !== true) {
            return false
        }
        view.triggerWebAction(WebEngineView.ToggleMediaPlayPause)
        return true
    }

    function updateMprisForPrimaryView(view) {
        if (!window.mprisController.available || !view || view !== window.activeWebView()) {
            return
        }
        var url = view.url ? view.url.toString() : ""
        if (!/^https?:\/\//i.test(url) && /^https?:\/\//i.test(window.browserUi.current_url || "")) {
            url = String(window.browserUi.current_url)
        }
        if (!/^https?:\/\//i.test(url)) {
            return
        }
        window.mprisController.update(
                    String(view.title || ""), url,
                    view.recentlyAudible === true, view.audioMuted === true,
                    window.temporaryProfile)
    }

    function safeNotificationOrigin(notification) {
        var raw = notification && notification.origin
                ? notification.origin.toString() : ""
        if (!/^https?:\/\//i.test(raw)) {
            return "opaque or unavailable origin"
        }
        var parts = raw.split("/")
        var authority = parts.length > 2 ? parts[2] : ""
        var at = authority.lastIndexOf("@")
        if (at >= 0) {
            authority = authority.slice(at + 1)
        }
        return window.boundedPageDialogText(parts[0] + "//" + authority)
    }

    function addressPresentation(value, maxCharacters) {
        var safe = String(value || "")
        var limit = Number(maxCharacters)
        if (!isFinite(limit) || limit <= 0) {
            limit = 256
        }
        limit = Math.max(24, Math.floor(limit))
        if (safe.length <= limit) {
            return safe
        }
        var schemeEnd = safe.indexOf("://")
        if (schemeEnd < 0) {
            return safe.slice(0, limit)
        }
        var authorityStart = schemeEnd + 3
        var authorityEnd = safe.length
        for (var i = authorityStart; i < safe.length; ++i) {
            if (safe[i] === "/" || safe[i] === "?") {
                authorityEnd = i
                break
            }
        }
        var origin = safe.slice(0, authorityEnd)
        if (origin.length >= limit) {
            return origin
        }
        var tail = safe.slice(authorityEnd)
        var separator = tail.charAt(0) === "/" ? "/…" : "…"
        var tailLimit = limit - origin.length - separator.length
        return origin + separator + (tailLimit > 0 ? tail.slice(-tailLimit) : "")
    }

    function closeWebNotifications() {
        return window.runtime_webNotificationController.closeWebNotifications()
    }

    function removePendingPortalNotification(notification) {
        return window.runtime_webNotificationController.removePendingPortalNotification(notification)
    }

    function notificationOwnerFor(notification, requireKnown) {
        return window.runtime_webNotificationController.notificationOwnerFor(notification, requireKnown)
    }

    function rememberNotificationOwner(notification, ui) {
        return window.runtime_webNotificationController.rememberNotificationOwner(notification, ui)
    }

    function forgetNotificationOwner(notification) {
        return window.runtime_webNotificationController.forgetNotificationOwner(notification)
    }

    function deferredNotificationOwnerValid(entry) {
        return window.runtime_webNotificationController.deferredNotificationOwnerValid(entry)
    }

    function deferNotificationUntilPortal(ui, notification, privateProfile, previous) {
        return window.runtime_webNotificationController.deferNotificationUntilPortal(ui, notification, privateProfile, previous)
    }

    function processPendingPortalNotifications() {
        return window.runtime_webNotificationController.processPendingPortalNotifications()
    }

    function cancelPendingExternalUris(message) {
        var pending = window.pendingExternalUris || []
        window.pendingExternalUris = []
        for (var index = 0; index < pending.length; ++index) {
            if (pending[index] && pending[index].ui) {
                pending[index].ui.status_text = message
            }
        }
        window.externalOpenPortalTimer.stop()
    }

    function processPendingExternalUris() {
        var pending = (window.pendingExternalUris || []).slice(0)
        if (pending.length === 0) {
            window.externalOpenPortalTimer.stop()
            return
        }
        var remaining = []
        for (var index = 0; index < pending.length; ++index) {
            var entry = pending[index]
            if (!entry || !entry.uri) {
                continue
            }
            var status = window.desktopPortalCapabilityStatus(entry.ui, "open_uri")
            if (Date.now() >= Number(entry.deadlineMs)) {
                if (entry.ui) {
                    entry.ui.status_text = "Desktop portal check timed out; external action cancelled"
                }
                continue
            }
            if (status === "pending" || status === "not-probed") {
                remaining.push(entry)
                continue
            }
            if (status !== "available") {
                if (entry.ui) {
                    entry.ui.status_text = "Required OpenURI portal unavailable; external action cancelled"
                }
                continue
            }
            Qt.openUrlExternally(String(entry.uri))
        }
        window.pendingExternalUris = remaining
        if (remaining.length === 0) {
            window.externalOpenPortalTimer.stop()
        }
    }

    function sharesPermissionSession(left, right) {
        return !!left && !!right && window.runtime_permissionScopeBridge.sharesPermissionSession(left, right)
    }

    function closeWebNotificationsForOrigin(origin, sourceUi) {
        return window.runtime_webNotificationController.closeWebNotificationsForOrigin(origin, sourceUi)
    }

    function activeWebNotification(notification) {
        return window.runtime_webNotificationController.activeWebNotification(notification)
    }

    function notificationProfileScope(ui, privateProfile) {
        return window.runtime_webNotificationController.notificationProfileScope(ui, privateProfile)
    }

    function presentWebNotification(ui, notification, privateProfile, previous) {
        return window.runtime_webNotificationController.presentWebNotification(ui, notification, privateProfile, previous)
    }

    function focusTargetAvailable(target) {
        return window.runtime_focusOverlayController.focusTargetAvailable(target)
    }

    function beginNativePromptLease(ui, view, category, tabIndex) {
        if (!ui || !ui.begin_native_prompt_lease) {
            return ""
        }
        var index = Number(tabIndex)
        if (tabIndex === undefined || !Number.isInteger(index) || index < 0) {
            var viewIndex = view && view.tabIndex !== undefined
                    ? Number(view.tabIndex) : -1
            index = Number.isInteger(viewIndex) && viewIndex >= 0
                    ? viewIndex : Number(ui.active_tab_index)
        }
        if (!Number.isInteger(index) || index < 0) {
            return ""
        }
        return String(ui.begin_native_prompt_lease(index, String(category)) || "")
    }

    function completeNativePromptLease(ui, token, view) {
        if (!ui || !token || !ui.complete_native_prompt_lease) {
            return false
        }
        var needsFocusProbe = ui.complete_native_prompt_lease(String(token))
        if (needsFocusProbe && (window.runtimeDisposing
                || (view && view.viewHost && view.viewHost.runtimeReleased))) {
            ui.finish_native_prompt_focus_probe(String(token))
        } else if (needsFocusProbe) {
            Qt.callLater(function() {
                if (window.probeNativePromptFocus) {
                    window.probeNativePromptFocus(ui, view, String(token))
                } else {
                    ui.finish_native_prompt_focus_probe(String(token))
                }
            })
        }
        return needsFocusProbe
    }

    function nativePromptActiveFor(ui) {
        if (!ui) {
            return false
        }
        if (ui.has_native_prompt_lease && ui.has_native_prompt_lease()) {
            return true
        }
        if (ui === window.browserUi) {
            var activeView = window.activeWebView()
            if (activeView && activeView.startPage
                    && activeView.startPage.inputActive) {
                return true
            }
        }
        return window.presentedPromptActiveFor(ui)
    }

    function presentedPromptActiveFor(ui) {
        if (!ui) return false
        return (window.switcherVisible && window.switcherUi === ui)
                || ui.link_preview_visible || (window.pendingFileDialogUi === ui
                    && (!!window.pendingFileDialogRequest
                        || window.pendingFileDialogLeaseToken.length > 0))
                || (window.pendingPermissionUi === ui
                    && (!!window.permissionPromptVisible
                        || window.pendingPermissionLeaseToken.length > 0))
                || (window.pendingDesktopMediaUi === ui
                    && (!!window.desktopMediaPromptVisible
                        || window.pendingDesktopMediaLeaseToken.length > 0))
                || (window.pendingPageDialogUi === ui
                    && (!!window.pendingPageDialogRequest
                        || !!window.pendingAuthenticationRequest
                        || window.pendingPageDialogLeaseToken.length > 0))
                || (window.pendingClientCertificateUi === ui
                    && (!!window.pendingClientCertificateSelection
                        || window.pendingClientCertificateLeaseToken.length > 0))
                || (window.pendingCertificateErrorUi === ui
                    && (!!window.pendingCertificateError
                        || window.pendingCertificateErrorLeaseToken.length > 0))
                || (window.pendingWebAuthUi === ui
                    && (!!window.pendingWebAuthRequest
                        || window.pendingWebAuthLeaseToken.length > 0))
                || (window.pendingContextMenuUi === ui
                    && (!!window.pendingContextMenuRequest
                        || window.pendingContextMenuLeaseToken.length > 0))
                || (window.contextRouteUi === ui && window.contextRouteVisible)
                || (window.diagnosticsUi === ui && window.diagnosticsVisible)
    }

    function captureOverlayFocus(hostWindow, fallbackTarget) {
        window.runtime_focusOverlayController.captureOverlayFocus(hostWindow, fallbackTarget)
    }

    function restoreOverlayFocus(hostWindow) {
        window.runtime_focusOverlayController.restoreOverlayFocus(hostWindow || window)
    }

    function discardOverlayFocus(hostWindow) {
        window.runtime_focusOverlayController.discardHost(hostWindow || window)
    }

    function captureModeFocus() {
        window.runtime_focusOverlayController.captureModeFocus()
    }

    function restoreModeFocus() {
        window.runtime_focusOverlayController.restoreModeFocus()
    }

    function openInternalSurface() {
        window.captureOverlayFocus(window, window.activeWebView())
    }

    function closeInternalSurface() {
        window.restoreOverlayFocus()
    }

    function showContextRoute(ui) {
        if (!ui || !ui.context_route_id || !ui.context_route_context
                || !ui.context_route_profile || !ui.context_route_url) {
            return
        }
        var owner = window.browserWindowEntryForUi(ui)
        if (!owner) return
        window.contextRouteUi = ui
        window.contextRouteHost = owner.host
        window.contextRouteVisible = true
        window.captureOverlayFocus(owner.host, owner.view)
    }

    function closeContextRoutePrompt() {
        var wasVisible = window.contextRouteVisible
        var host = window.contextRouteHost
        window.contextRouteVisible = false
        window.contextRouteUi = null
        window.contextRouteHost = null
        if (wasVisible) {
            window.restoreOverlayFocus(host)
        }
    }

    function showContextMovePicker(tabId) {
        var sourceEntry = window.browserWindowEntryForUi(window.browserUi)
        var choices = []
        var names = window.browserUi.context_choice_names || []
        var labels = window.browserUi.context_choice_labels || []
        var profiles = window.browserUi.context_choice_profiles || []
        for (var i = 0; i < names.length; ++i) {
            var name = String(names[i] || "")
            if (name.length === 0
                    || name === String(window.browserUi.context_name || "")
                    || String(profiles[i] || "") !== String(window.profileName)) {
                continue
            }
            var targetEntry = window.browserWindowEntryForContext(name, sourceEntry)
            choices.push({
                name: name,
                label: String(labels[i] || name),
                available: window.sameProfileWindow(sourceEntry, targetEntry)
            })
        }
        if (choices.length === 0) {
            window.browserUi.status_text = "No other same-profile contexts are configured"
            return
        }
        window.contextMoveTabId = String(tabId || "")
        window.contextMoveChoices = choices
        window.contextMoveVisible = true
        window.captureOverlayFocus(window, window.activeWebView())
    }

    function chooseContextMove(name) {
        var tabId = window.contextMoveTabId
        window.contextMoveVisible = false
        window.contextMoveTabId = ""
        window.contextMoveChoices = []
        window.restoreOverlayFocus()
        if (!tabId || !name) {
            return false
        }
        var command = ":tab-move " + window.libraryCommandArgument(tabId)
                + " --context " + window.libraryCommandArgument(name)
        if (!window.browserUi.execute_command(command)) {
            return false
        }
        window.executePendingEngineAction()
        return true
    }

    function executeNewWindowAction(ui, action) {
        var fields = action.split("\t")
        var restoreContext = fields[0] === "context-window"
        var owner = window.browserWindowEntryForUi(ui)
        if (!owner || !owner.host || owner.host.runtimeReleased === true
                || (fields.length !== 4 && fields.length !== 5 && fields.length !== 6)
                || (fields[0] !== "new-window" && !restoreContext)
                || (restoreContext && fields.length !== 5)
                || (fields[1] !== "true" && fields[1] !== "false")
                || (fields.length === 6 && fields[4] !== "true" && fields[4] !== "false")
                || !fields[2] || !fields[fields.length - 1]) {
            if (ui) ui.status_text = "This window request is no longer available. Try again."
            return false
        }
        var inherit = fields[1] === "false" && fields[2] === owner.profileName
                && (fields.length === 6 ? fields[4] === "true"
                    : (fields.length === 4 || (!owner.privateProfile && !owner.ephemeralProfile)))
        if (fields.length === 6 && fields[4] === "true"
                && !inherit) {
            ui.status_text = "The source profile changed. Request the new window again."
            return false
        }
        var privateProfile = fields[1] === "true" || (inherit && owner.privateProfile)
        var temporaryProfile = inherit && owner.ephemeralProfile
        var context = fields.length >= 5 && fields[3].length > 0 ? fields[3]
                    : (inherit && !privateProfile && !temporaryProfile ? String(ui.context_name || "") : "")
        var child = window.browserWindowFactory.createObject(null, {
            windowStartupUrl: fields[fields.length - 1],
            windowProfileName: fields[2],
            windowProfileLabel: inherit
                ? (owner.host === window ? window.profileLabel : owner.host.windowProfileLabel) : fields[2],
            windowStorageBasePath: owner.host === window
                ? window.storageBasePath : owner.host.windowStorageBasePath,
            windowPrivateProfile: privateProfile,
            windowEphemeralProfile: temporaryProfile,
            windowEphemeralInvocationToken: temporaryProfile ? owner.ephemeralInvocationToken : "",
            windowSharedProfile: inherit ? owner.profile : null,
            windowInheritedUi: inherit ? ui : null,
            windowStartupContext: context,
            windowStartupContextRestore: restoreContext,
            windowStartupRoutePreflighted: true
        })
        if (!child) {
            ui.status_text = "The window couldn't be opened. Try again."
            return false
        }
        return true
    }

    function executeContextRouteWindowAction(ui) {
        var owner = window.browserWindowEntryForUi(ui)
        if (owner) window.completeBrowserKeyAction(ui, owner.host)
    }

    function acceptContextRoute() {
        var ui = window.contextRouteUi
        if (!ui) {
            window.closeContextRoutePrompt()
            return
        }
        var accepted = ui.accept_context_route()
        window.closeContextRoutePrompt()
        if (accepted) {
            window.executeContextRouteWindowAction(ui)
        }
    }

    function dismissContextRoute() {
        var ui = window.contextRouteUi
        var accepted = ui && ui.dismiss_context_route()
        window.closeContextRoutePrompt()
        if (accepted) window.executeContextRouteWindowAction(ui)
    }

    function registerBrowserWindow(hostWindow, ui, view, profile, profileName,
                                   invocationToken, isEphemeral) {
        if (!hostWindow || !ui || !ui.profile_available()) {
            return false
        }
        if (!ui.register_ipc_owner()) return false
        ui.set_ipc_window_active(!!hostWindow.active)
        var existing = (window.browserWindowRegistry || []).filter(function(entry) {
            return entry && entry.ui === ui
        })
        for (var existingIndex = 0; existingIndex < existing.length; ++existingIndex) {
            if (existing[existingIndex].ephemeralProfile) {
                window.releaseEphemeralProfileOwner(
                            existing[existingIndex].profile,
                            existing[existingIndex].ephemeralInvocationToken)
            }
        }
        var token = String(invocationToken || "")
        if (isEphemeral && !window.retainEphemeralProfileOwner(profile, token)) {
            ui.status_text = "This temporary window couldn't be opened. Close it and try opening a new temporary window."
            return false
        }
        if (profile && profile.offTheRecord && ui && ui.set_private_start_owner) {
            var owner = window.liveWindowEntries().find(function(entry) { return entry.profile === profile && entry.ui.start_page_owner_token.length > 0 })
            ui.set_private_start_owner(owner ? owner.ui.start_page_owner_token : ui.window_token)
        }
        var entries = (window.browserWindowRegistry || []).filter(function(entry) {
            return entry && entry.ui && entry.ui !== ui
        })
        entries.push({
            host: hostWindow,
            ui: ui,
            view: view || null,
            profile: profile || null,
            profileName: String(profileName || ""),
            coreWindowId: String(ui.core_window_id || ""),
            windowToken: String(ui.window_token || ""),
            privateProfile: hostWindow === window
                    ? (window.temporaryProfile && !window.ephemeralProfile)
                    : !!hostWindow.windowPrivateProfile,
            ephemeralProfile: !!isEphemeral,
            ephemeralInvocationToken: String(invocationToken || "")
        })
        window.browserWindowRegistry = entries
        window.publishBrowserWindowRegistry()
        return true
    }

    function liveWindowEntries() {
        return (window.browserWindowRegistry || []).concat(window.popupWindowRegistry || [])
                .filter(function(entry) { return entry && entry.host && entry.ui && !entry.host.runtimeReleased })
    }

    function publishBrowserWindowRegistry() {
        var ids = []
        var ownerTokens = []
        var profiles = []
        var privateFlags = []
        var ephemeralFlags = []
        var tabCounts = []
        var entries = window.liveWindowEntries()
        for (var i = 0; i < entries.length && ids.length < 64; ++i) {
            var entry = entries[i]
            if (!entry || !entry.ui || !entry.host) {
                continue
            }
            ids.push(String(entry.coreWindowId || ""))
            ownerTokens.push(String(entry.windowToken || ""))
            profiles.push(String(entry.profileName || ""))
            privateFlags.push(entry.privateProfile ? "true" : "false")
            ephemeralFlags.push(entry.ephemeralProfile ? "true" : "false")
            tabCounts.push(String(entry.ui === window.browserUi
                                  ? Number(window.browserUi.tab_count || 0)
                                  : Number(entry.ui.tab_count || 0)))
        }
        window.browserUi.publish_live_window_registry(
                    ids, ownerTokens, profiles, privateFlags, ephemeralFlags, tabCounts)
    }

    function updateBrowserWindowView(ui, view) {
        var groups = [window.browserWindowRegistry || [],window.popupWindowRegistry || []]
        for (var groupIndex=0;groupIndex<groups.length;++groupIndex) {
            var entries=groups[groupIndex]
            for (var i=0;i<entries.length;++i) {
                if (entries[i] && entries[i].ui===ui) {
                    entries[i].view=view || null
                    if (groupIndex===0) window.browserWindowRegistry=entries
                    else window.popupWindowRegistry=entries
                    window.publishBrowserWindowRegistry()
                    return true
                }
            }
        }
        return false
    }

    function browserWindowEntryForTarget(target) {
        var requested = String(target || "")
        if (requested.length === 0) return null
        var entries = window.liveWindowEntries()
        var exact = entries.filter(function(entry) { return entry.windowToken === requested })
        if (exact.length === 1) return exact[0]
        // Core IDs are local to each BrowserUi runtime. Keep a unique alias
        // usable, but never choose the first of several matching windows.
        var aliases = entries.filter(function(entry) { return entry.coreWindowId === requested })
        return aliases.length === 1 ? aliases[0] : null
    }

    function browserWindowEntryForContext(contextName, sourceEntry) {
        var requested = String(contextName || "")
        if (requested.length === 0 || !sourceEntry) {
            return null
        }
        var entries = window.liveWindowEntries()
        for (var i = 0; i < entries.length; ++i) {
            var entry = entries[i]
            if (entry && entry.host && entry.ui && entry !== sourceEntry
                    && String(entry.ui.context_name || "") === requested
                    && entry.profileName === sourceEntry.profileName
                    && !!entry.privateProfile === !!sourceEntry.privateProfile
                    && !!entry.ephemeralProfile === !!sourceEntry.ephemeralProfile) {
                return entry
            }
        }
        return null
    }

    function browserWindowEntryForUi(ui) {
        var entries = window.liveWindowEntries()
        for (var i = 0; i < entries.length; ++i) {
            if (entries[i] && entries[i].ui === ui) {
                return entries[i]
            }
        }
        return null
    }

    function focusedBrowserWindowEntry(preferredUi, preferredHost) {
        var preferred = preferredUi ? window.browserWindowEntryForUi(preferredUi) : null
        if (preferred && (!preferredHost || preferred.host === preferredHost)) {
            return preferred
        }
        var entries = window.liveWindowEntries()
        for (var i = 0; i < entries.length; ++i) {
            var entry = entries[i]
            if (entry && entry.host && entry.ui && entry.host.active) {
                return entry
            }
        }
        return window.browserWindowEntryForUi(window.browserUi)
    }

    function unregisterBrowserWindow(ui) {
        if (window.diagnosticsUi === ui) window.closeDiagnostics()
        if (ui) ui.release_ipc_owner()
        var entries = window.browserWindowRegistry || []
        for (var i = 0; i < entries.length; ++i) {
            if (entries[i] && entries[i].ui === ui && entries[i].ephemeralProfile) {
                window.releaseEphemeralProfileOwner(
                            entries[i].profile, entries[i].ephemeralInvocationToken)
            }
        }
        window.browserWindowRegistry = (window.browserWindowRegistry || []).filter(
                    function(entry) { return entry && entry.ui && entry.ui !== ui })
        window.publishBrowserWindowRegistry()
    }

    function registerPopupWindow(hostWindow, ui, view) {
        if (!hostWindow || !ui || !ui.profile_available() || !ui.register_ipc_owner()) return false
        ui.set_ipc_window_active(!!hostWindow.active)
        var previous=(window.popupWindowRegistry || []).find(function(entry) { return entry && entry.host===hostWindow })
        var profile=hostWindow.windowWebEngineProfile
        var token=String(hostWindow.windowEphemeralInvocationToken || "")
        if (previous && previous.ephemeralProfile)
            window.releaseEphemeralProfileOwner(previous.profile,previous.ephemeralInvocationToken)
        if (hostWindow.windowEphemeralProfile && !window.retainEphemeralProfileOwner(profile,token))
            return false
        if (profile && profile.offTheRecord && ui.set_private_start_owner) {
            var owner=window.liveWindowEntries().find(function(entry) { return entry.profile===profile && entry.ui!==ui && entry.ui.start_page_owner_token.length>0 })
            ui.set_private_start_owner(owner ? owner.ui.start_page_owner_token : ui.window_token)
        }
        var entries = (window.popupWindowRegistry || []).filter(function(entry) {
            return entry && entry.host && entry.host !== hostWindow
        })
        entries.push({host:hostWindow,ui:ui,view:view || null,profile:profile,
            coreWindowId:String(ui.core_window_id || ""),windowToken:String(ui.window_token || ""),
            profileName:String(hostWindow.windowProfileName || ""),
            privateProfile:!!hostWindow.windowPrivateProfile,
            ephemeralProfile:!!hostWindow.windowEphemeralProfile,ephemeralInvocationToken:token})
        window.popupWindowRegistry = entries
        window.publishBrowserWindowRegistry()
        return true
    }

    function unregisterPopupWindow(hostWindow) {
        if (window.diagnosticsHost === hostWindow) window.closeDiagnostics()
        var previous=(window.popupWindowRegistry || []).find(function(entry) { return entry && entry.host===hostWindow })
        if (previous && previous.ui) previous.ui.release_ipc_owner()
        if (previous && previous.ephemeralProfile)
            window.releaseEphemeralProfileOwner(previous.profile,previous.ephemeralInvocationToken)
        window.popupWindowRegistry = (window.popupWindowRegistry || []).filter(
                    function(entry) { return entry && entry.host && entry.host !== hostWindow })
        window.publishBrowserWindowRegistry()
    }

    function shutdownParticipantCount() {
        var count = 0
        var windows = window.browserWindowRegistry || []
        for (var i = 0; i < windows.length; ++i) {
            if (windows[i] && windows[i].host && windows[i].host !== window) {
                ++count
            }
        }
        var popups = window.popupWindowRegistry || []
        for (var j = 0; j < popups.length; ++j) {
            if (popups[j] && popups[j].host) {
                ++count
            }
        }
        return count
    }

    function abortApplicationShutdown() {
        if (!window.applicationShutdownInProgress) {
            return
        }
        window.applicationShutdownInProgress = false
        window.applicationShutdownGeneration += 1
        window.applicationShutdownStage = "Shutdown cancelled"
        window.applicationShutdownForcePromptVisible = false
        window.shutdownPageProbeGeneration += 1
        window.applicationShutdownTimer.stop()
        window.shutdownPageProbeTimer.stop()
        window.browserUi.end_shutdown_gate()
        window.shutdownConfirmationVisible = false
        window.shutdownPromptVisible = false
        window.shutdownPagePromptVisible = false
        var windows = window.browserWindowRegistry || []
        for (var i = 0; i < windows.length; ++i) {
            var host = windows[i] && windows[i].host
            if (host && host !== window) {
                host.windowCloseConfirmationVisible = false
                host.windowShutdownPromptVisible = false
                host.windowShutdownStoragePromptVisible = false
                if (host.cancelPageStateProbe) {
                    host.cancelPageStateProbe()
                }
            }
        }
        var popups = window.popupWindowRegistry || []
        for (var j = 0; j < popups.length; ++j) {
            var popup = popups[j] && popups[j].host
            if (popup) {
                popup.windowCloseConfirmationVisible = false
                popup.windowShutdownPromptVisible = false
                if (popup.cancelPageStateProbe) {
                    popup.cancelPageStateProbe()
                }
            }
        }
        window.browserUi.status_text = "Shutdown cancelled"
    }

    function finishApplicationShutdown() {
        if (!window.applicationShutdownInProgress
                || window.shutdownParticipantCount() > 0) {
            return
        }
        window.applicationShutdownStage = "Saving browser data"
        if (!window.browserUi.flush_durable_state()) {
            window.applicationShutdownStage = "Browser data couldn't be saved"
            window.applicationShutdownForcePromptVisible = true
            return
        }
        if (!window.browserUi.request_shutdown()) {
            window.applicationShutdownStage = "Shutdown couldn't finish"
            window.applicationShutdownForcePromptVisible = true
            return
        }
        window.applicationShutdownTimer.stop()
        window.applicationShutdownInProgress = false
        window.applicationShutdownForcePromptVisible = false
        window.shutdownPageProbeGeneration += 1
        window.shutdownApproved = true
        Qt.quit()
    }

    function advanceApplicationShutdown() {
        if (!window.applicationShutdownInProgress) {
            window.applicationShutdownTimer.stop()
            return
        }
        var remaining = window.shutdownParticipantCount()
        var elapsed = Date.now() - window.applicationShutdownStartedAt
        window.applicationShutdownStage = remaining > 0
                ? "Closing " + remaining + " browser window(s)"
                : "Saving browser data"
        if (remaining === 0) {
            window.finishApplicationShutdown()
            return
        }
        if (elapsed >= 10000) {
            window.applicationShutdownStage = "Waiting for " + remaining
                    + " browser window(s) to finish closing"
            window.applicationShutdownForcePromptVisible = true
            window.applicationShutdownTimer.stop()
        }
    }

    function beginApplicationShutdown() {
        if (window.applicationShutdownInProgress) {
            return
        }
        if (!window.browserUi.begin_shutdown_gate()) {
            window.browserUi.status_text = "Could not start the shutdown gate"
            return
        }
        window.applicationShutdownInProgress = true
        window.applicationShutdownForcePromptVisible = false
        window.applicationShutdownStage = "Resolving browser windows"
        window.applicationShutdownStartedAt = Date.now()
        window.applicationShutdownGeneration += 1
        var windows = (window.browserWindowRegistry || []).slice(0)
        for (var i = 0; i < windows.length; ++i) {
            var host = windows[i] && windows[i].host
            if (host && host !== window && !host.windowShutdownApproved) {
                host.continueQuitRequest()
            }
        }
        var popups = (window.popupWindowRegistry || []).slice(0)
        for (var j = 0; j < popups.length; ++j) {
            var popup = popups[j] && popups[j].host
            if (popup && !popup.windowShutdownApproved) {
                popup.continueQuitRequest()
            }
        }
        window.applicationShutdownTimer.restart()
        window.advanceApplicationShutdown()
    }

    function forceApplicationShutdown() {
        if (!window.applicationShutdownInProgress) {
            return
        }
        window.applicationShutdownStage = "Force quitting. Recent changes may not be saved."
        window.browserUi.force_quit()
        window.applicationShutdownInProgress = false
        window.applicationShutdownForcePromptVisible = false
        window.shutdownApproved = true
        Qt.quit()
    }

    function retainEphemeralProfileOwner(profile, token) {
        return window.runtime_windowRegistryController.retainEphemeralProfileOwner(profile, token)
    }

    function releaseEphemeralProfileOwner(profile, token) {
        return window.runtime_windowRegistryController.releaseEphemeralProfileOwner(profile, token)
    }

    function switcherOwner(result) {
        return window.runtime_windowRegistryController.switcherOwner(result)
    }

    function ephemeralProfileForToken(token) {
        return window.runtime_windowRegistryController.ephemeralProfileForToken(token)
    }

    function ephemeralTokenForUi(ui) {
        return window.runtime_windowRegistryController.ephemeralTokenForUi(ui)
    }

    function executeWindowFocusAction(sourceUi, action) {
        var parts = action.split("\t")
        var target = parts.length >= 2 ? parts[1] : ""
        var operationId = parts.length >= 3 ? parts[2] : ""
        var entry = window.browserWindowEntryForTarget(target)
        if (!entry || !entry.host || !entry.ui) {
            sourceUi.status_text = "The requested window is no longer open"
            if (operationId.length > 0) sourceUi.complete_window_focus(operationId, "stale")
            return
        }
        // The issuing UI owns the operation, even when another window receives focus.
        if (window.activateBrowserWindow(entry.host, sourceUi, operationId) && entry.host.active)
            sourceUi.status_text = "Window focused"
    }

    function activateBrowserWindow(hostWindow, targetUi, operationId) {
        if (!hostWindow) {
            return false
        }
        var ui = targetUi || window.browserUi
        var focusOperationId = String(operationId || "")
        try {
            hostWindow.show()
            hostWindow.raise()
            hostWindow.requestActivate()
            if (hostWindow.active) {
                window.activationStatus = "activated"
                if (focusOperationId.length > 0) {
                    ui.complete_window_focus(focusOperationId, "activated")
                }
            } else {
                window.activationStatus = "requested"
                window.pendingActivationHost = hostWindow
                window.pendingActivationUi = ui
                window.pendingActivationOperationId = focusOperationId
                ui.status_text = "Window activation requested; waiting for compositor"
                window.activationOutcomeTimer.restart()
            }
            return true
        } catch (error) {
            window.activationStatus = "unknown"
            ui.status_text = "Window activation outcome unknown"
            if (focusOperationId.length > 0) {
                ui.complete_window_focus(focusOperationId, "unknown")
            }
            return false
        }
    }

    function focusNotificationOrigin(profileScope, origin) {
        var expectedScope = String(profileScope || "")
        var expectedOrigin = String(origin || "").toLowerCase()
        function tryEntry(entry, view, privateProfile) {
            if (!entry || !entry.host || !entry.ui || !view
                    || window.notificationProfileScope(entry.ui, privateProfile)
                        !== expectedScope
                    || window.permissionOriginForView(view) !== expectedOrigin) {
                return false
            }
            window.activateBrowserWindow(entry.host, entry.ui)
            Qt.callLater(function() {
                if (view && view.forceActiveFocus) {
                    view.forceActiveFocus()
                }
            })
            entry.ui.status_text = "Notification activated"
            return true
        }

        var entries = window.browserWindowRegistry || []
        for (var i = 0; i < entries.length; ++i) {
            var entry = entries[i]
            var view = entry && entry.ui === window.browserUi
                    ? window.activeWebView() : entry && entry.view
            if (tryEntry(entry, view, entry && entry.privateProfile)) {
                return true
            }
        }
        var popups = window.popupWindowRegistry || []
        for (var j = 0; j < popups.length; ++j) {
            var popup = popups[j]
            if (tryEntry(popup, popup && popup.view,
                         popup && popup.host && popup.host.popupPrivateProfile)) {
                return true
            }
        }
        window.browserUi.status_text = "Notification source is no longer open"
        return false
    }

    function focusExistingContextWindow(commandText, sourceUi) {
        var fields = String(commandText || "").trim().split(/\s+/)
        if (fields.length !== 2 || fields[0] !== "context-enter" || !fields[1]) {
            return false
        }
        var entries = window.browserWindowRegistry || []
        for (var i = 0; i < entries.length; ++i) {
            var entry = entries[i]
            if (!entry || !entry.ui || entry.ui === sourceUi
                    || String(entry.ui.context_name || "") !== fields[1]) {
                continue
            }
            window.activateBrowserWindow(entry.host, entry.ui)
            window.routeContextWorkspace(entry.ui)
            entry.ui.status_text = "Context window focused"
            return true
        }
        return false
    }

    function contextStatus(ui, privateProfile, profileName, ephemeralProfile) {
        var profile = ephemeralProfile
                ? String(profileName || "ephemeral")
                : (privateProfile ? "private" : String(profileName || "unknown"))
        var result = " · profile " + profile
        if (ui && ui.context_name && String(ui.context_name).length > 0) {
            result += " · context " + String(ui.context_label || ui.context_name)
                    + " [" + String(ui.context_name) + "]"
            if (ui.context_workspace && String(ui.context_workspace).length > 0) {
                result += " · workspace " + String(ui.context_workspace)
            }
        }
        return result
    }

    function contextStatusColor(ui, fallback) {
        var accent = ui && ui.context_accent ? String(ui.context_accent) : ""
        var candidate = /^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(accent)
                ? accent.slice(0, 7) : fallback
        return window.readableTextColor(candidate, window.surfaceColor)
    }

    function statusTransport(view) {
        if (!view || !view.url) {
            return ""
        }
        var raw = view.url.toString()
        var match = /^([A-Za-z][A-Za-z0-9+.-]*):/.exec(raw)
        if (!match || !match[1] || raw === "about:blank") {
            return ""
        }
        var scheme = match[1].toLowerCase()
        if (scheme === "https") {
            return " · HTTPS transport (site trust separate)"
        }
        if (scheme === "http") {
            return " · HTTP transport (not secure)"
        }
        return " · " + scheme + " transport"
    }

    function statusLoad(view) {
        if (!view || view.loading !== true) {
            return ""
        }
        var progress = Number(view.loadProgress)
        if (!isFinite(progress)) {
            return " · loading"
        }
        return " · loading " + Math.max(0, Math.min(100, Math.round(progress))) + "%"
    }

    function statusMedia(view) {
        if (!view) {
            return ""
        }
        if (view.recentlyAudible === true) {
            return view.audioMuted === true ? " · audible (muted)" : " · audible"
        }
        return view.audioMuted === true ? " · muted" : ""
    }

    function statusPermission(ui) {
        if (!ui) {
            return ""
        }
        if (window.pendingPermissionUi === ui
                && Number(window.pendingPermissionGroupCount || 0) > 0) {
            return " · permission pending ("
                    + Number(window.pendingPermissionGroupCount) + ")"
        }
        var queue = window.permissionPromptQueue || []
        for (var i = 0; i < queue.length; ++i) {
            if (queue[i] && queue[i].ui === ui) {
                return " · permission queued"
            }
        }
        return ""
    }

    function statusCapture(hostWindow) {
        if (!hostWindow) {
            return ""
        }
        var active = 0
        var sessions = window.captureSessions || []
        for (var i = 0; i < sessions.length; ++i) {
            if (sessions[i] && sessions[i].host === hostWindow
                    && sessions[i].status === "active") {
                active += 1
            }
        }
        if (active > 0) {
            return " · capture active (" + active + ")"
        }
        return window.pendingDesktopMediaHost === hostWindow
                ? " · capture pending" : ""
    }

    function statusDownloads(hostWindow) {
        if (!hostWindow) {
            return ""
        }
        var active = hostWindow.activeDownloads || ({})
        var pending = hostWindow.pendingDownloadRequests || ({})
        var count = 0
        var bytes = 0
        var inProgress = 0
        var paused = 0
        var selecting = 0
        var finishing = window.profileResourceRegistry.finishingForHost(hostWindow)
        count += finishing.count
        bytes += finishing.bytes
        for (var key in active) {
            var download = active[key]
            if (!download || download.isFinished === true) {
                continue
            }
            count += 1
            bytes += Math.max(0, Number(download.receivedBytes || 0))
            if (download.isPaused === true) {
                paused += 1
            } else {
                inProgress += 1
            }
        }
        for (var pendingKey in pending) {
            if (pending[pendingKey]) {
                count += 1
                selecting += 1
            }
        }
        if (count === 0) {
            return ""
        }
        var lifecycle = []
        if (finishing.count > 0) lifecycle.push(finishing.count + " finishing")
        if (inProgress > 0) {
            lifecycle.push(inProgress + " active")
        }
        if (paused > 0) {
            lifecycle.push(paused + " paused")
        }
        if (selecting > 0) {
            lifecycle.push(selecting + " selecting")
        }
        return " · downloads " + count + " (" + lifecycle.join(", ")
                + "; " + bytes + " bytes)"
    }

    function statusTabBlocking(view) {
        if (!view || Number(view.blockedRequestCount || 0) <= 0) {
            return ""
        }
        return " · tab blocked " + Number(view.blockedRequestCount)
    }

    function statusDetails(ui, hostWindow, view,
                           privateProfile, profileName, ephemeralProfile) {
        return window.contextStatus(ui, privateProfile, profileName, ephemeralProfile)
                + window.statusTransport(view)
                + window.statusLoad(view)
                + window.statusMedia(view)
                + window.statusPermission(ui)
                + window.statusCapture(hostWindow)
                + window.statusDownloads(hostWindow)
                + window.statusTabBlocking(view)
                + window.siteDoctorBadge(ui)
    }

    function routeContextWorkspace(ui) {
        if (!ui || !ui.context_workspace || ui.context_workspace.length === 0) {
            return false
        }
        var entries = window.browserWindowRegistry || []
        for (var i = 0; i < entries.length; ++i) {
            var entry = entries[i]
            if (!entry || entry.ui !== ui) {
                continue
            }
            if (!entry.host || entry.host.active !== true) {
                window.pendingWorkspaceRouteHost = entry.host
                window.pendingWorkspaceRouteUi = ui
                window.pendingWorkspaceRoute = String(ui.context_workspace)
                ui.status_text = "Workspace routing pending until the browser window is active"
                window.activateBrowserWindow(entry.host, ui)
                return false
            }
            var workspace = String(ui.context_workspace)
            window.pendingWorkspaceRouteHost = null
            window.pendingWorkspaceRouteUi = null
            window.pendingWorkspaceRoute = ""
            return ui.hyprland_route_workspace(workspace)
        }
        ui.status_text = "Workspace routing denied for an unregistered browser window"
        return false
    }

    function noteQtResolution(ui, kind, outcome) {
        var target = ui || window.browserUi
        var counts = window.qtResolutionCounts || ({})
        var key = String(kind) + ":" + String(outcome)
        counts[key] = Number(counts[key] || 0) + 1
        window.qtResolutionCounts = counts
        if (target && target.record_request_resolution) {
            target.record_request_resolution(String(kind), String(outcome))
        }
    }

    function resolveQtRequest(ui, request, kind, method, args) {
        if (!request) {
            window.noteQtResolution(ui, kind, "stale")
            return false
        }
        var resolved = window.resolvedQtRequests || []
        for (var i = 0; i < resolved.length; ++i) {
            if (resolved[i] && resolved[i].request === request) {
                window.noteQtResolution(ui, kind, "duplicate")
                return false
            }
        }
        if (request.isValid !== undefined && !request.isValid) {
            window.noteQtResolution(ui, kind, "stale")
            return false
        }
        var resolver = request[method]
        if (typeof resolver !== "function") {
            window.noteQtResolution(ui, kind, "error")
            return false
        }
        resolved.push({ request: request, kind: String(kind), method: String(method) })
        if (resolved.length > 256) {
            resolved.shift()
        }
        window.resolvedQtRequests = resolved
        try {
            resolver.apply(request, args || [])
        } catch (error) {
            window.noteQtResolution(ui, kind, "error")
            return false
        }
        window.noteQtResolution(ui, kind, "resolved")
        return true
    }

    function refreshBlockingEvidence() {
        var blockedRequests = Number(window.requestInterceptor.blockedCount)
        var unknownRequests = Number(window.requestInterceptor.unknownContextCount)
        window.browserUi.publish_blocking_live_counts(
                    blockedRequests, unknownRequests, window.activeBlockedRequestCount)
        var activeView = window.activeWebView()
        var activeHost = activeView && activeView.url ? activeView.url.host : ""
        window.browserUi.set_blocking_active_evidence(
                    window.requestInterceptor.blockedRequestExplanationFields(activeHost),
                    window.requestInterceptor.blockedRequestDecisionFields(activeHost))
    }
}

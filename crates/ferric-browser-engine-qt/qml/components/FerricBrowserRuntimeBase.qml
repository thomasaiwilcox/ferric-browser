import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import QtWebEngine
import io.github.ferricbrowser 1.0
import "../scripts/BrowserScripts.js" as BrowserScripts
import "../scripts/SpellcheckPresentation.js" as SpellcheckPresentation

ApplicationWindow {
    id: window
    readonly property alias primaryBrowserUi: browserUi
    readonly property alias blocklistUpdaterObject: blocklistUpdater
    readonly property alias linkRuleUpdaterObject: linkRuleUpdater
    readonly property alias mprisControllerObject: mprisController
    readonly property alias browserKeyRouterObject: browserKeyRouter
    readonly property alias applicationShutdownTimerObject: applicationShutdownTimer
    readonly property alias shutdownPageProbeTimerObject: shutdownPageProbeTimer
    readonly property alias blocklistUpdateTimerObject: blocklistUpdateTimer
    readonly property alias linkCleaningUpdateTimerObject: linkCleaningUpdateTimer
    readonly property alias requestPresentationControllerObject: requestPresentationController
    readonly property alias browserProfilePrototypeObject: browserProfilePrototype
    readonly property alias bindingOverlayTimerObject: bindingOverlayTimer
    readonly property alias engineUpdateNoticeTimerObject: engineUpdateNoticeTimer
    readonly property alias externalOpenPortalTimerObject: externalOpenPortalTimer
    readonly property alias notificationPresenterObject: notificationPresenter
    readonly property alias siteDataClearPollTimerObject: siteDataClearPollTimer
    property var permissionPromptSurfaceComponentObject: null
    property var captureIndicatorComponentObject: null
    property var desktopMediaSurfaceComponentObject: null
    property var rendererFailureSurfaceComponentObject: null
    property var browserWindowComponentObject: null
    property var popupWindowComponentObject: null
    property var webViewComponentObject: null
    property var webViewsObject: null
    readonly property alias permissionPromptFactory: window.permissionPromptSurfaceComponentObject
    readonly property alias captureIndicatorFactory: window.captureIndicatorComponentObject
    readonly property alias desktopMediaFactory: window.desktopMediaSurfaceComponentObject
    readonly property alias rendererFailureFactory: window.rendererFailureSurfaceComponentObject
    readonly property alias browserWindowFactory: window.browserWindowComponentObject
    readonly property alias popupWindowFactory: window.popupWindowComponentObject
    readonly property alias webViewFactory: window.webViewComponentObject
    readonly property alias primaryRequestInterceptor: requestInterceptor
    readonly property alias devToolsWindowFactory: devToolsWindowComponent
    objectName: "ferric_browserWindow"
    width: 1280
    height: 800
    visible: true
    title: window.ephemeralProfile
           ? "Ferric Browser · " + window.profileLabel
             + (!window.tabStripVisible ? " · " + browserUi.tab_count + " tabs" : "")
           : "Ferric Browser"
             + (!window.tabStripVisible ? " · " + browserUi.tab_count + " tabs" : "")
    color: window.backgroundColor
    font.family: window.chromeFontFamily
    font.pointSize: window.chromeFontPointSize
    palette.window: window.backgroundColor
    palette.windowText: window.primaryTextColor
    palette.base: window.surfaceColor
    palette.alternateBase: window.panelColor
    palette.text: window.primaryTextColor
    palette.button: window.surfaceColor
    palette.buttonText: window.primaryTextColor
    palette.highlight: window.selectionColor
    palette.highlightedText: window.selectionTextColor
    palette.placeholderText: window.mutedTextColor
    palette.link: window.accentColor
    property string chromeFontFamily: "monospace"
    property real chromeFontPointSize: 10.0
    property real systemFontScale: 1.0
    // System mode is conservative: this surface has no optional motion unless
    // the user explicitly opts out of reduced motion. The setting can still be
    // changed without rejecting a theme or relying on color alone for state.
    property bool reducedMotionActive: false
    property string systemReducedMotionStatus: "pending"
    property string themeContrastStatus: "unknown"
    property string themeContrastWarning: ""
    property string renderedThemeContrastStatus: "unknown"
    property alias focusReturnStack: focusOverlayController.focusReturnStack
    property alias modeFocusReturnTarget: focusOverlayController.modeFocusReturnTarget
    property alias modeFocusCaptured: focusOverlayController.modeFocusCaptured
    readonly property bool browserKeyFocusActive: !window.browserChromeInputActive
    property color backgroundColor: "#1e1e2e"
    property color surfaceColor: "#313244"
    property color panelColor: "#181825"
    property color primaryTextColor: "#cdd6f4"
    property color secondaryTextColor: "#cdd6f4"
    property color mutedTextColor: "#a6adc8"
    property color borderColor: "#585b70"
    property color accentColor: "#89b4fa"
    property color warningColor: "#f9e2af"
    property color errorColor: "#f38ba8"
    property color successColor: "#a6e3a1"
    property color privateColor: "#cba6f7"
    property color modeInsertColor: "#f9e2af"
    property color selectionColor: "#45475a"
    property color selectionTextColor: "#cdd6f4"
    property string statusbarMode: "always"
    property string tabsMode: "multiple"
    property string tabPosition: "top"
    property bool tabSwitchingVisible: false
    property string tabIdentitySignature: ""
    readonly property bool tabStripVisible:
        tabsMode === "always"
        || (tabsMode === "multiple" && browserUi.tab_count > 1)
        || (tabsMode === "switching" && tabSwitchingVisible)
    readonly property bool sideTabs: tabPosition === "left" || tabPosition === "right"
    readonly property real sideTabWidth: Math.max(160, Math.min(280, width * 0.22))
    readonly property bool inputBarActive: browserUi.mode === "command"
                                           || browserUi.mode === "search"
    readonly property bool normalStatusVisible: statusbarMode === "always"
                                                && !inputBarActive
    readonly property real bottomChromeHeight:
        (inputBarActive ? inputBarHeight
         : (normalStatusVisible ? statusBarHeight : 0))
        + (tabStripVisible && tabPosition === "bottom"
           ? tabBarHeight : 0)
    readonly property real chromeOpacity: 1.0
    property var browserWindowRegistry: []
    property var popupWindowRegistry: []
    property bool applicationShutdownInProgress: false
    property bool applicationShutdownForcePromptVisible: false
    property bool devToolsVisible: false
    property bool devToolsDetached: false
    property var devToolsExternalView: null
    property var devToolsDetachedWindow: null
    property real devToolsPanelHeight: 280
    property string applicationShutdownStage: ""
    property double applicationShutdownStartedAt: 0
    property int applicationShutdownGeneration: 0
    property alias ephemeralProfileOwners: windowRegistryController.ephemeralProfileOwners
    property bool libraryJourneyGraphMode: false
    property bool libraryJourneyCurrentOnly: false
    property string libraryJourneySearchText: ""
    property string libraryJourneyExpandedNode: ""
    property var libraryGraphLineData: []
    property bool journeyExportPreviewVisible: false
    property bool journeyExportAwaiting: false
    property string journeyExportPreviewText: ""
    property string ephemeralInvocationToken: ""
    property string activationStatus: "unknown"
    property var pendingActivationHost: null
    property var pendingActivationUi: null
    property string pendingActivationOperationId: ""
    property var pendingWorkspaceRouteHost: null
    property var pendingWorkspaceRouteUi: null
    property string pendingWorkspaceRoute: ""
    readonly property real chromePadding: Math.max(4, Math.ceil(chromeFontMetrics.height * 0.25))
    readonly property real statusBarHeight: Math.ceil(
        chromeFontMetrics.height + (chromePadding * 2))
    readonly property real inputBarHeight: Math.ceil(
        chromeFontMetrics.height + (chromePadding * 2))
    readonly property real tabBarHeight: Math.ceil(
        chromeFontMetrics.height + (chromePadding * 2))

    readonly property real chromeRowHeight: Math.ceil(
        chromeFontMetrics.height + (chromePadding * 1.25))
    // Qt Quick dimensions are already logical pixels. This scale only grows
    // browser-owned overlays with the configured font metrics; multiplying
    // Screen.devicePixelRatio here would double-scale high-DPI displays.
    readonly property real chromeScale: Math.max(1.0, chromeFontMetrics.height / 14.0)
    readonly property real displayScale: Screen.devicePixelRatio > 0
                                         ? Screen.devicePixelRatio : 1.0
    readonly property real logicalPixelDensity: Screen.logicalPixelDensity > 0
                                                ? Screen.logicalPixelDensity : 1.0
    readonly property string displayScaleLabel: Math.round(window.displayScale * 100)
                                               + "% (" + Math.round(window.logicalPixelDensity)
                                               + " logical dpi)"
    FontMetrics {
        id: chromeFontMetrics
        font: window.font
    }

    FerricFocusOverlayController {
        id: focusOverlayController
        browserWindow: window
    }

    FerricChromePresentationController {
        id: chromePresentationController
        browserWindow: window
        browserUi: window.primaryBrowserUi
    }

    FerricWindowRegistryController {
        id: windowRegistryController
        browserWindow: window
        browserUi: window.primaryBrowserUi
    }

    FerricRequestPresentationController {
        id: requestPresentationController
        browserWindow: window
    }

    function contrastText(background) {
        return chromePresentationController.contrastText(background)
    }

    function readableTextColor(candidate, background) {
        return chromePresentationController.readableTextColor(candidate, background)
    }

    function renderedContrastReport() {
        return chromePresentationController.renderedContrastReport()
    }

    function refreshChromeAppearance() {
        chromePresentationController.refreshChromeAppearance()
    }

    function noteTabActivity() {
        if (window.tabsMode === "switching") {
            window.tabSwitchingVisible = true
            tabSwitchingTimer.restart()
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
                || !window.mediaKeysEnabled(browserUi)) {
            return false
        }
        var view = window.activeWebView()
        if (!window.triggerMediaToggle(view)) {
            browserUi.status_text = "Media toggle unavailable without an active page"
            return true
        }
        browserUi.status_text = "Media play/pause toggle sent to the page"
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
        if (!mprisController.available || !view || view !== window.activeWebView()) {
            return
        }
        var url = view.url ? view.url.toString() : ""
        if (!/^https?:\/\//i.test(url) && /^https?:\/\//i.test(browserUi.current_url || "")) {
            url = String(browserUi.current_url)
        }
        if (!/^https?:\/\//i.test(url)) {
            return
        }
        mprisController.update(
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
        var active = window.activeWebNotifications || ({})
        for (var key in active) {
            if (active[key]) {
                active[key].close()
            }
        }
        window.activeWebNotifications = ({})
        window.notificationOwners = []
        var pending = window.pendingPortalNotifications || []
        for (var pendingIndex = 0; pendingIndex < pending.length; ++pendingIndex) {
            if (pending[pendingIndex].notification) {
                pending[pendingIndex].notification.close()
            }
        }
        window.pendingPortalNotifications = []
    }

    function removePendingPortalNotification(notification) {
        var pending = window.pendingPortalNotifications || []
        window.pendingPortalNotifications = pending.filter(function(entry) {
            return entry && entry.notification !== notification
        })
    }

    function notificationOwnerFor(notification) {
        var owners = window.notificationOwners || []
        for (var index = 0; index < owners.length; ++index) {
            if (owners[index] && owners[index].notification === notification) {
                return owners[index].ui || browserUi
            }
        }
        return browserUi
    }

    function rememberNotificationOwner(notification, ui) {
        var owners = (window.notificationOwners || []).filter(function(entry) {
            return entry && entry.notification !== notification
        })
        owners.push({ notification: notification, ui: ui || browserUi })
        window.notificationOwners = owners.slice(-64)
    }

    function forgetNotificationOwner(notification) {
        window.notificationOwners = (window.notificationOwners || []).filter(function(entry) {
            return entry && entry.notification !== notification
        })
    }

    function deferNotificationUntilPortal(ui, notification, privateProfile) {
        var pending = window.pendingPortalNotifications || []
        window.removePendingPortalNotification(notification)
        pending = window.pendingPortalNotifications || []
        pending.push({
            ui: ui || browserUi,
            notification: notification,
            privateProfile: privateProfile,
            deadlineMs: Date.now() + 5000
        })
        window.pendingPortalNotifications = pending.slice(-32)
        notification.closed.connect(function() {
            window.removePendingPortalNotification(notification)
        })
        notificationPortalTimer.start()
    }

    function processPendingPortalNotifications() {
        var pending = (window.pendingPortalNotifications || []).slice(0)
        if (pending.length === 0) {
            notificationPortalTimer.stop()
            return
        }
        var remaining = []
        for (var index = 0; index < pending.length; ++index) {
            var entry = pending[index]
            if (!entry || !entry.notification) {
                continue
            }
            var status = window.desktopPortalCapabilityStatus(entry.ui, "notifications")
            if (Date.now() >= Number(entry.deadlineMs)
                    || (status !== "pending" && status !== "not-probed"
                        && status !== "available")) {
                entry.notification.close()
                if (entry.ui) {
                    entry.ui.status_text = Date.now() >= Number(entry.deadlineMs)
                            ? "Desktop portal check timed out; notification cancelled"
                            : "Required Notification portal unavailable; notification cancelled"
                }
                continue
            }
            if (status === "pending" || status === "not-probed") {
                remaining.push(entry)
                continue
            }
            window.presentWebNotification(entry.ui, entry.notification, entry.privateProfile)
        }
        window.pendingPortalNotifications = remaining
        if (remaining.length === 0) {
            notificationPortalTimer.stop()
        }
    }

    function cancelPendingExternalUris(message) {
        var pending = window.pendingExternalUris || []
        window.pendingExternalUris = []
        for (var index = 0; index < pending.length; ++index) {
            if (pending[index] && pending[index].ui) {
                pending[index].ui.status_text = message
            }
        }
        externalOpenPortalTimer.stop()
    }

    function processPendingExternalUris() {
        var pending = (window.pendingExternalUris || []).slice(0)
        if (pending.length === 0) {
            externalOpenPortalTimer.stop()
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
            externalOpenPortalTimer.stop()
        }
    }

    function closeWebNotificationsForOrigin(origin) {
        var expected = String(origin || "").toLowerCase()
        if (expected.length === 0) {
            return
        }
        var active = window.activeWebNotifications || ({})
        for (var key in active) {
            var parts = String(key).split("\n")
            if (parts.length >= 3 && parts[1].toLowerCase() === expected) {
                if (active[key]) {
                    active[key].close()
                }
                delete active[key]
            }
        }
        window.activeWebNotifications = active
        var pending = window.pendingPortalNotifications || []
        var retained = []
        for (var pendingIndex = 0; pendingIndex < pending.length; ++pendingIndex) {
            var entry = pending[pendingIndex]
            var pendingOrigin = window.safeNotificationOrigin(
                        entry && entry.notification ? entry.notification : null)
            if (pendingOrigin.toLowerCase() === expected) {
                if (entry.notification) {
                    entry.notification.close()
                }
                if (entry.ui) {
                    entry.ui.status_text = "Desktop notification revoked for " + pendingOrigin
                }
            } else {
                retained.push(entry)
            }
        }
        window.pendingPortalNotifications = retained
        if (retained.length === 0) {
            notificationPortalTimer.stop()
        }
    }

    function activeWebNotification(notification) {
        var active = window.activeWebNotifications || ({})
        var keys = Object.keys(active)
        for (var index = 0; index < keys.length; ++index) {
            if (active[keys[index]] === notification) {
                return true
            }
        }
        return false
    }

    function notificationProfileScope(ui, privateProfile) {
        var kind = privateProfile ? "private" : "normal"
        var name = ui && ui.profile_name ? String(ui.profile_name) : "default"
        name = window.boundedPageDialogText(name).replace(/[\u0000-\u001f\u007f\n\r]/g, "�")
        return kind + ":" + (name.length > 0 ? name : "default")
    }

    function presentWebNotification(ui, notification, privateProfile) {
        if (!notification) {
            return
        }
        var origin = window.safeNotificationOrigin(notification)
        var decision = ui && ui.permission_decision
                ? String(ui.permission_decision(origin, "notifications")) : "deny"
        if (origin === "opaque or unavailable origin"
                || decision !== "allow"
                || !window.desktopNotificationsEnabled(ui)) {
            notification.close()
            if (ui) {
                ui.status_text = "Desktop notification blocked for " + origin
            }
            return
        }
        if (window.desktopPortalMode(ui) === "required") {
            var portalStatus = window.desktopPortalCapabilityStatus(ui, "notifications")
            if (portalStatus === "not-probed") {
                browserUi.probe_desktop_portals()
                portalStatus = window.desktopPortalCapabilityStatus(ui, "notifications")
            }
            if (portalStatus === "pending" || portalStatus === "not-probed") {
                window.deferNotificationUntilPortal(ui, notification, privateProfile)
                ui.status_text = "Checking required Notification portal…"
                return
            }
            if (portalStatus !== "available") {
                notification.close()
                ui.status_text = "Required Notification portal unavailable; notification cancelled"
                return
            }
        }
        var tag = notification.tag ? String(notification.tag) : ""
        var profileScope = window.notificationProfileScope(ui, privateProfile)
        var notificationKey = tag.length > 0
                ? profileScope + "\n" + origin + "\n" + window.boundedPageDialogText(tag)
                : profileScope + "\n" + origin + "\n#" + (++window.webNotificationSequence)
        var active = window.activeWebNotifications || ({})
        var previous = active[notificationKey]
        if (previous && previous !== notification) {
            previous.close()
        }
        active[notificationKey] = notification
        window.activeWebNotifications = active
        window.rememberNotificationOwner(notification, ui)
        notification.closed.connect(function() {
            var current = window.activeWebNotifications || ({})
            if (current[notificationKey] === notification) {
                delete current[notificationKey]
                window.activeWebNotifications = current
            }
            window.forgetNotificationOwner(notification)
        })
        if (notificationPresenter.present(notification, profileScope, origin)) {
            ui.status_text = privateProfile
                    ? "Private desktop notification shown for " + origin
                    : "Desktop notification shown for " + origin
            return
        }
        notification.show()
        ui.status_text = privateProfile
                ? "Private desktop notification shown for " + origin
                : "Desktop notification shown for " + origin
    }

    function focusTargetAvailable(target) {
        return focusOverlayController.focusTargetAvailable(target)
    }

    function captureOverlayFocus(hostWindow, fallbackTarget) {
        focusOverlayController.captureOverlayFocus(hostWindow, fallbackTarget)
    }

    function restoreOverlayFocus() {
        focusOverlayController.restoreOverlayFocus()
    }

    function captureModeFocus() {
        focusOverlayController.captureModeFocus()
    }

    function restoreModeFocus() {
        focusOverlayController.restoreModeFocus()
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
        window.contextRouteUi = ui
        window.contextRouteVisible = true
        window.captureOverlayFocus(window, window.activeWebView())
    }

    function closeContextRoutePrompt() {
        var wasVisible = window.contextRouteVisible
        window.contextRouteVisible = false
        window.contextRouteUi = null
        if (wasVisible) {
            window.restoreOverlayFocus()
        }
    }

    function showContextMovePicker(tabId) {
        var sourceEntry = window.browserWindowEntryForUi(browserUi)
        var choices = []
        var names = browserUi.context_choice_names || []
        var labels = browserUi.context_choice_labels || []
        var profiles = browserUi.context_choice_profiles || []
        for (var i = 0; i < names.length; ++i) {
            var name = String(names[i] || "")
            if (name.length === 0
                    || name === String(browserUi.context_name || "")
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
            browserUi.status_text = "No other same-profile contexts are configured"
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
        if (!browserUi.execute_command(command)) {
            return false
        }
        window.executePendingEngineAction()
        return true
    }

    function executeContextRouteWindowAction(ui) {
        var action = ui ? ui.take_engine_action() : ""
        if (action.indexOf("new-window\t") !== 0) {
            return
        }
        var fields = action.split("\t")
        if (fields.length >= 5) {
            browserWindowComponent.createObject(null, {
                windowStartupUrl: fields.slice(4).join("\t"),
                windowProfileName: fields[2],
                windowProfileLabel: fields[2],
                windowPrivateProfile: fields[1] === "true",
                windowStartupContext: fields[3]
            })
        }
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
        if (ui) {
            ui.dismiss_context_route()
        }
        window.closeContextRoutePrompt()
    }

    function registerBrowserWindow(hostWindow, ui, view, profile, profileName,
                                   invocationToken, isEphemeral) {
        if (!hostWindow || !ui) {
            return false
        }
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
            ui.status_text = "Ephemeral profile owner token is already bound to another live profile"
            return false
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

    function publishBrowserWindowRegistry() {
        var ids = []
        var ownerTokens = []
        var profiles = []
        var privateFlags = []
        var ephemeralFlags = []
        var tabCounts = []
        var entries = window.browserWindowRegistry || []
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
            tabCounts.push(String(entry.ui === browserUi
                                  ? Number(browserUi.tab_count || 0)
                                  : Number(entry.ui.tab_count || 0)))
        }
        browserUi.publish_live_window_registry(
                    ids, ownerTokens, profiles, privateFlags, ephemeralFlags, tabCounts)
    }

    function updateBrowserWindowView(ui, view) {
        var entries = window.browserWindowRegistry || []
        for (var i = 0; i < entries.length; ++i) {
            if (entries[i] && entries[i].ui === ui) {
                entries[i].view = view || null
                window.browserWindowRegistry = entries
                window.publishBrowserWindowRegistry()
                return true
            }
        }
        return false
    }

    function browserWindowEntryForTarget(target) {
        var requested = String(target || "")
        if (requested.length === 0) {
            return null
        }
        var entries = window.browserWindowRegistry || []
        for (var i = 0; i < entries.length; ++i) {
            var entry = entries[i]
            if (entry && entry.host && entry.ui
                    && (entry.coreWindowId === requested || entry.windowToken === requested)) {
                return entry
            }
        }
        return null
    }

    function browserWindowEntryForContext(contextName, sourceEntry) {
        var requested = String(contextName || "")
        if (requested.length === 0 || !sourceEntry) {
            return null
        }
        var entries = window.browserWindowRegistry || []
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
        var entries = window.browserWindowRegistry || []
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
        var entries = window.browserWindowRegistry || []
        for (var i = 0; i < entries.length; ++i) {
            var entry = entries[i]
            if (entry && entry.host && entry.ui && entry.host.active) {
                return entry
            }
        }
        return window.browserWindowEntryForUi(browserUi)
    }

    function unregisterBrowserWindow(ui) {
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

    function registerPopupWindow(hostWindow, openerUi, view) {
        if (!hostWindow) {
            return false
        }
        var entries = (window.popupWindowRegistry || []).filter(function(entry) {
            return entry && entry.host && entry.host !== hostWindow
        })
        entries.push({ host: hostWindow, ui: openerUi || browserUi, view: view || null })
        window.popupWindowRegistry = entries
        return true
    }

    function unregisterPopupWindow(hostWindow) {
        window.popupWindowRegistry = (window.popupWindowRegistry || []).filter(
                    function(entry) { return entry && entry.host && entry.host !== hostWindow })
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
        applicationShutdownTimer.stop()
        shutdownPageProbeTimer.stop()
        browserUi.end_shutdown_gate()
        window.shutdownPromptVisible = false
        window.shutdownPagePromptVisible = false
        var windows = window.browserWindowRegistry || []
        for (var i = 0; i < windows.length; ++i) {
            var host = windows[i] && windows[i].host
            if (host && host !== window) {
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
                popup.windowShutdownPromptVisible = false
                if (popup.cancelPageStateProbe) {
                    popup.cancelPageStateProbe()
                }
            }
        }
        browserUi.status_text = "Shutdown cancelled"
    }

    function finishApplicationShutdown() {
        if (!window.applicationShutdownInProgress
                || window.shutdownParticipantCount() > 0) {
            return
        }
        window.applicationShutdownStage = "Flushing Rust metadata"
        if (!browserUi.flush_durable_state()) {
            window.applicationShutdownStage = "Durable metadata flush failed"
            window.applicationShutdownForcePromptVisible = true
            return
        }
        if (!browserUi.request_shutdown()) {
            window.applicationShutdownStage = "Rust shutdown request failed"
            window.applicationShutdownForcePromptVisible = true
            return
        }
        applicationShutdownTimer.stop()
        window.applicationShutdownInProgress = false
        window.applicationShutdownForcePromptVisible = false
        window.shutdownPageProbeGeneration += 1
        window.shutdownApproved = true
        Qt.quit()
    }

    function advanceApplicationShutdown() {
        if (!window.applicationShutdownInProgress) {
            applicationShutdownTimer.stop()
            return
        }
        var remaining = window.shutdownParticipantCount()
        var elapsed = Date.now() - window.applicationShutdownStartedAt
        window.applicationShutdownStage = remaining > 0
                ? "Closing " + remaining + " browser window(s)"
                : "Flushing Rust metadata"
        if (remaining === 0) {
            window.finishApplicationShutdown()
            return
        }
        if (elapsed >= 10000) {
            window.applicationShutdownStage = "Waiting for " + remaining
                    + " browser window(s) to finish closing"
            window.applicationShutdownForcePromptVisible = true
            applicationShutdownTimer.stop()
        }
    }

    function beginApplicationShutdown() {
        if (window.applicationShutdownInProgress) {
            return
        }
        if (!browserUi.begin_shutdown_gate()) {
            browserUi.status_text = "Could not start the shutdown gate"
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
                host.beginQuitRequest()
            }
        }
        var popups = (window.popupWindowRegistry || []).slice(0)
        for (var j = 0; j < popups.length; ++j) {
            var popup = popups[j] && popups[j].host
            if (popup && !popup.windowShutdownApproved) {
                popup.beginQuitRequest()
            }
        }
        applicationShutdownTimer.restart()
        window.advanceApplicationShutdown()
    }

    function forceApplicationShutdown() {
        if (!window.applicationShutdownInProgress) {
            return
        }
        window.applicationShutdownStage = "Forced quit; crash marker preserved"
        browserUi.force_quit()
        window.applicationShutdownInProgress = false
        window.applicationShutdownForcePromptVisible = false
        window.shutdownApproved = true
        Qt.quit()
    }

    function retainEphemeralProfileOwner(profile, token) {
        return windowRegistryController.retainEphemeralProfileOwner(profile, token)
    }

    function releaseEphemeralProfileOwner(profile, token) {
        return windowRegistryController.releaseEphemeralProfileOwner(profile, token)
    }

    function switcherOwner(result) {
        return windowRegistryController.switcherOwner(result)
    }

    function ephemeralProfileForToken(token) {
        return windowRegistryController.ephemeralProfileForToken(token)
    }

    function ephemeralTokenForUi(ui) {
        return windowRegistryController.ephemeralTokenForUi(ui)
    }

    function activateBrowserWindow(hostWindow, targetUi, operationId) {
        if (!hostWindow) {
            return false
        }
        var ui = targetUi || browserUi
        var focusOperationId = String(operationId || "")
        try {
            hostWindow.show()
            hostWindow.raise()
            hostWindow.requestActivate()
            if (hostWindow.active) {
                window.activationStatus = "activated"
                hostWindow.forceActiveFocus()
                if (focusOperationId.length > 0) {
                    ui.complete_window_focus(focusOperationId, "activated")
                }
            } else {
                window.activationStatus = "requested"
                window.pendingActivationHost = hostWindow
                window.pendingActivationUi = ui
                window.pendingActivationOperationId = focusOperationId
                ui.status_text = "Window activation requested; waiting for compositor"
                activationOutcomeTimer.restart()
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
            var view = entry && entry.ui === browserUi
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
        browserUi.status_text = "Notification source is no longer open"
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
        return contextStatus(ui, privateProfile, profileName, ephemeralProfile)
                + statusTransport(view)
                + statusLoad(view)
                + statusMedia(view)
                + statusPermission(ui)
                + statusCapture(hostWindow)
                + statusDownloads(hostWindow)
                + statusTabBlocking(view)
                + siteDoctorBadge(ui)
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

    onClosing: function(close) {
        if (window.shutdownApproved) {
            return
        }
        close.accepted = false
        window.beginQuitRequest()
    }
    property string startupUrl: "about:blank"
    property var startupAdditionalUrls: []
    property string startupEntryPoint: "typed-initial-url"
    property bool startupTrustedLocalInput: false
    property bool startupBackground: false
    property string startupContext: ""
    property bool startupCleanLink: false
    property bool temporaryProfile: false
    property bool ephemeralProfile: false
    property bool safeMode: false
    property bool userscriptsOff: false
    property bool softwareRendering: false
    property string instanceLockPath: ""
    property string instanceSelector: ""
    property string profileName: "default"
    property string profileLabel: "Default"
    property string storageBasePath: ""
    property bool recoveryAvailable: false
    property bool rendererFailureVisible: false
    property var rendererFailureView: null
    property var rendererFailureHost: null
    property var rendererFailureUi: null
    property int rendererFailureTabIndex: -1
    property string rendererFailureSafeUrl: ""
    property string rendererFailureReason: ""
    property int rendererFailureExitCode: 0
    property int rendererFailureCount: 0
    property string startupConfigJson: ""
    property string startupConfigBaseJson: ""
    property string startupCliOverridesJson: "{}"
    property string startupProfileOverridesJson: "{}"
    property string startupConfigPath: ""
    property string startupConfigSource: "built-in"
    property string startupContextsJson: ""
    property bool contextRouteVisible: false
    property var contextRouteUi: null
    property bool contextMoveVisible: false
    property string contextMoveTabId: ""
    property var contextMoveChoices: []
    property bool sessionManagerVisible: false
    property bool sessionPreviewVisible: false
    property bool sessionPreviewLoading: false
    property bool sessionPreviewError: false
    property string sessionPreviewName: ""
    property bool sessionPreviewAppend: false
    property string sessionPreviewText: ""
    property bool reopenWindowConfirmationVisible: false
    property string reopenWindowConfirmationAction: ""
    property var reopenWindowConfirmationUi: null
    property var reopenWindowConfirmationHost: null
    property string pendingDeleteName: ""
    property bool profileManagerVisible: false
    property int profileRefreshAttempts: 0
    property bool profileDeletePreviewVisible: false
    property string profileDeleteName: ""
    property string profileDeletePreviewText: ""
    property bool downloadManagerVisible: false
    property var downloadSamples: ({})
    property var spellcheckInventory: []
    property var activeWebNotifications: ({})
    property int webNotificationSequence: 0
    property var activeDownloads: ({})
    property var pendingDownloadRequests: ({})
    property var notificationOwners: []
    property var pendingPortalNotifications: []
    property var pendingExternalUris: []
    property string pendingDownloadId: ""
    property string pendingDownloadSuggestedName: ""
    property var pendingFileDialogRequest: null
    property var pendingFileDialogView: null
    property var pendingFileDialogUi: null
    property bool pendingFileDialogWaitingForPortal: false
    property double pendingFileDialogPortalDeadlineMs: 0
    property var pendingPermissionRequest: null
    property var pendingPermissionUi: null
    property int pendingPermissionTabIndex: -1
    property string pendingPermissionOrigin: ""
    property string pendingPermissionName: ""
    property bool pendingPermissionPrivate: false
    property bool permissionPromptVisible: false
    property var permissionPromptQueue: []
    property var permissionCooldowns: ({})
    readonly property int permissionPromptQueueLimit: 8
    property int pendingPermissionGroupCount: 0
    property var pendingPermissionHost: null
    property var pendingDesktopMediaRequest: null
    property var pendingDesktopMediaUi: null
    property var pendingDesktopMediaView: null
    property var pendingDesktopMediaHost: null
    property bool pendingDesktopMediaWaitingForPortal: false
    property double pendingDesktopMediaPortalDeadlineMs: 0
    property bool desktopMediaPromptVisible: false
    property string desktopMediaOrigin: ""
    property var desktopMediaSurface: null
    property var captureIndicatorSurface: null
    property var captureSessions: []
    property int captureSessionSequence: 0
    property var rendererFailureSurface: null
    property var pendingPageDialogRequest: null
    property var pendingAuthenticationRequest: null
    property var pendingPageDialogUi: null
    property var pendingPageDialogView: null
    property bool pageDialogIsAuthentication: false
    property string pageDialogMessage: ""
    property string pageDialogTitle: "Page dialog"
    property string pageDialogOrigin: ""
    property string pageDialogDefaultText: ""
    property bool pageDialogSuppressChecked: false
    readonly property int pageDialogLimit: 3
    property var pendingClientCertificateSelection: null
    property var pendingClientCertificateUi: null
    property var pendingClientCertificateView: null
    property var pendingClientCertificateOptions: []
    property string pendingClientCertificateHost: ""
    property var pendingCertificateError: null
    property var pendingCertificateErrorUi: null
    property var pendingCertificateErrorView: null
    property string pendingCertificateErrorHost: ""
    property string pendingCertificateErrorDescription: ""
    property var pendingWebAuthRequest: null
    property var pendingWebAuthUi: null
    property var pendingWebAuthView: null
    property var pendingWebAuthUserNames: []
    property var pendingWebAuthState: null
    property string pendingWebAuthRelyingParty: ""
    property string webAuthStatusText: ""
    property var pendingContextMenuRequest: null
    property var pendingContextMenuView: null
    property var pendingContextMenuUi: null
    property var pendingContextMenuHost: null
    property var contextMenuItems: []
    property bool shutdownPromptVisible: false
    property bool shutdownPagePromptVisible: false
    property string shutdownPagePromptReason: ""
    property int shutdownPageProbeGeneration: 0
    property bool shutdownApproved: false
    property bool switcherVisible: false
    property var switcherResults: []
    property string switcherScope: "all"
    property var switcherBatchEntries: []
    property var switcherBatchMerged: []
    property int switcherBatchIndex: 0
    property int switcherBatchGeneration: 0
    property int switcherBatchScheduledGeneration: 0
    property string switcherBatchQuery: ""
    property string switcherBatchScope: "all"
    property bool siteLedgerVisible: false
    property var siteLedgerData: ({
        safe_remediation_actions: [],
        facts: [],
        blocking: ({ active_site_decisions: [] }),
        site_doctor: ({ active: ({ kind: "" }), experiments: [] })
    })
    property bool diagnosticsVisible: false
    property string diagnosticsText: ""
    property string engineUpdateStatus: ""
    property string engineUpdateNotice: ""
    property var resolvedQtRequests: []
    property var qtResolutionCounts: ({})
    property bool bindingHelpVisible: false
    property string bindingHelpSearch: ""

    function noteQtResolution(ui, kind, outcome) {
        var target = ui || browserUi
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
        resolved.push({ request: request, kind: String(kind) })
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
    property var bindingHelpRows: []
    property bool bindingOverlayVisible: false
    property bool settingsVisible: false
    property bool settingsTemporary: false
    property string settingsSearch: ""
    property string settingsNotice: ""
    property string pendingUserscriptRemoval: ""
    property bool libraryManagerVisible: false
    property string pendingLibraryDelete: ""
    property bool privateHistoryTransferVisible: false
    property string privateHistoryTransferId: ""
    property string privateHistoryTransferTitle: ""
    property string privateHistoryTransferUrl: ""
    property int libraryPage: 0
    property int libraryPageSize: 100
    property int libraryTotalEntries: 0
    property bool linkPreviewVisible: false
    property var hintResults: []
    property string hintInput: ""
    property bool rapidHintConfirmationVisible: false
    property string copiedText: ""
    property string copiedValue: ""
    property bool copyNoticeSensitive: false
    property bool copyNoticeVisible: false
    readonly property bool browserChromeInputActive:
        window.inputBarActive
        || window.applicationShutdownForcePromptVisible
        || window.devToolsVisible
        || window.journeyExportPreviewVisible
        || window.rendererFailureVisible
        || window.contextRouteVisible
        || window.contextMoveVisible
        || window.sessionManagerVisible
        || window.sessionPreviewVisible
        || window.reopenWindowConfirmationVisible
        || window.profileManagerVisible
        || window.profileDeletePreviewVisible
        || window.downloadManagerVisible
        || window.permissionPromptVisible
        || window.desktopMediaPromptVisible
        || window.shutdownPromptVisible
        || window.shutdownPagePromptVisible
        || window.switcherVisible
        || window.siteLedgerVisible
        || window.diagnosticsVisible
        || window.bindingHelpVisible
        || window.settingsVisible
        || window.libraryManagerVisible
        || window.privateHistoryTransferVisible
        || window.linkPreviewVisible
        || window.siteDataClearPending
        || browserUi.external_navigation_visible
        || window.pendingPageDialogRequest !== null
        || window.pendingAuthenticationRequest !== null
        || window.pendingClientCertificateSelection !== null
        || window.pendingCertificateError !== null
        || window.pendingWebAuthRequest !== null
        || window.pendingContextMenuRequest !== null
    property bool caretSelecting: false
    property bool blocklistInstallFailed: false
    property string blocklistInstallError: ""
    property bool linkRuleInstallFailed: false
    property string linkRuleInstallError: ""
    property bool linkCleaningUpdateConfigured: false
    property int linkCleaningUpdateIntervalHours: 24
    property double linkCleaningLastUpdateAt: 0
    property bool siteDataClearPending: false
    property string siteDataClearOrigin: ""
    property var siteDataClearView: null
    property var siteDataClearUi: null
    property int blocklistUpdateIntervalHours: 24
    readonly property int activeBlockedRequestCount: {
        var view = window.activeWebView()
        var hostValue = view && view.url ? view.url.host : ""
        var host = hostValue === undefined || hostValue === null
                ? "" : String(hostValue)
        var counts = requestInterceptor.blockedSiteCounts || ({})
        return host.length > 0 && counts[host] !== undefined ? Number(counts[host]) : 0
    }
    readonly property int browserScriptWorld: WebEngineScript.ApplicationWorld
    readonly property string browserScriptBundleVersion: "1"

    WebEngineProfilePrototype {
        id: browserProfilePrototype
        storageName: window.temporaryProfile ? "" : "ferric-browser-" + window.profileName
        persistentStoragePath: window.temporaryProfile || window.storageBasePath.length === 0
                ? ""
                : window.storageBasePath + "/webengine/" + window.profileName
        cachePath: window.temporaryProfile || window.storageBasePath.length === 0
                ? ""
                : window.storageBasePath + "/webengine-cache/" + window.profileName
        persistentPermissionsPolicy: WebEngineProfile.AskEveryTime
    }

    property var browserProfile: null

    Connections {
        target: browserProfile

        function onDownloadRequested(download) {
            window.handleDownloadRequested(download)
        }
        function onDownloadFinished(download) {
            window.handleDownloadFinished(download)
        }
        function onPresentNotification(notification) {
            window.presentWebNotification(browserUi, notification, window.temporaryProfile)
        }
    }

    RequestInterceptor {
        id: requestInterceptor
        enabled: browserUi.blocking_enabled || securityDenyHosts.length > 0
        blockedHosts: browserUi.blocking_hosts
        exceptionHosts: browserUi.blocking_exceptions
        blockedRuleHosts: browserUi.blocking_rule_hosts
        blockedRuleListIds: browserUi.blocking_rule_list_ids
        exceptionRuleHosts: browserUi.blocking_exception_rule_hosts
        exceptionRuleListIds: browserUi.blocking_exception_rule_list_ids
        adblockEngineHandle: browserUi.blocking_adblock_handle
        bypassSites: browserUi.blocking_bypass_sites
        securityDenyHosts: browserUi.blocking_security_deny_hosts
        onBlockedCountChanged: window.refreshBlockingEvidence()
        onUnknownContextCountChanged: window.refreshBlockingEvidence()
        onBlockedSiteCountsChanged: window.refreshBlockingEvidence()
    }

    function refreshBlockingEvidence() {
        var blockedRequests = Number(requestInterceptor.blockedCount)
        var unknownRequests = Number(requestInterceptor.unknownContextCount)
        browserUi.publish_blocking_live_counts(
                    blockedRequests, unknownRequests, window.activeBlockedRequestCount)
        var activeView = window.activeWebView()
        var activeHost = activeView && activeView.url ? activeView.url.host : ""
        browserUi.set_blocking_active_evidence(
                    requestInterceptor.blockedRequestExplanationFields(activeHost),
                    requestInterceptor.blockedRequestDecisionFields(activeHost))
    }

    NotificationPresenter {
        id: notificationPresenter
        onNotificationClicked: function(profileScope, origin) {
            window.focusNotificationOrigin(profileScope, origin)
        }
        onNotificationUnavailable: function(notification, profileScope, origin) {
            if (!notification || !window.activeWebNotification(notification)) {
                return
            }
            notification.show()
            var ui = window.notificationOwnerFor(notification)
            if (ui) {
                ui.status_text = "Desktop notification service unavailable; using Qt fallback"
            }
        }
    }

    BlocklistUpdater {
        id: blocklistUpdater
    }

    LinkRuleUpdater {
        id: linkRuleUpdater
    }

    MprisController {
        id: mprisController
        onMediaToggleRequested: {
            var view = window.activeWebView()
            if (window.triggerMediaToggle(view)) {
                browserUi.status_text = "MPRIS media play/pause toggle sent to the page"
            }
        }
        onRaiseRequested: window.raise()
    }

    Timer {
        id: notificationPortalTimer
        interval: 50
        repeat: true
        onTriggered: window.processPendingPortalNotifications()
    }

    Timer {
        id: externalOpenPortalTimer
        interval: 50
        repeat: true
        onTriggered: window.processPendingExternalUris()
    }

    Timer {
        id: blocklistUpdateTimer
        interval: window.blocklistUpdateIntervalHours * 60 * 60 * 1000
        repeat: true
        running: !window.temporaryProfile
                 || (window.browserWindowRegistry || []).some(function(entry) {
                     return entry && !entry.privateProfile && !entry.ephemeralProfile
                 })
                 && window.storageBasePath.length > 0
                 && window.blocklistUpdateIntervalHours > 0
        onTriggered: window.requestBlocklistUpdate()
    }

    Timer {
        id: linkCleaningUpdateTimer
        interval: window.linkCleaningUpdateIntervalHours * 60 * 60 * 1000
        repeat: true
        running: !window.temporaryProfile
                 && window.storageBasePath.length > 0
                 && window.linkCleaningUpdateConfigured
                 && window.linkCleaningUpdateIntervalHours > 0
        onTriggered: window.requestLinkCleaningUpdate(true)
    }

    Timer {
        id: storageLibraryPollTimer
        interval: 50
        repeat: true
        running: !window.temporaryProfile
        onTriggered: {
            var entries = window.browserWindowRegistry || []
            if (entries.length === 0) {
                entries = [{ ui: browserUi }]
            }
            var consumed = false
            for (var i = 0; i < entries.length; ++i) {
                var entry = entries[i]
                if (entry && entry.ui && entry.ui.poll_storage_library()) {
                    consumed = true
                }
            }
            if (consumed && window.switcherVisible) {
                window.refreshSwitcher()
            }
            if (consumed && window.downloadManagerVisible) {
                window.refreshDownloads()
            }
            if (consumed && window.sessionManagerVisible) {
                window.refreshSessions()
            }
            if (consumed && window.libraryManagerVisible) {
                window.refreshLibraryManager()
            }
            if (window.journeyExportAwaiting
                    && browserUi.journey_export_preview_text.length > 0) {
                window.journeyExportPreviewText = browserUi.journey_export_preview_text
                window.journeyExportPreviewVisible = true
                window.journeyExportAwaiting = false
            }
        }
    }

    Timer {
        id: siteDataClearPollTimer
        interval: 100
        repeat: true
        running: window.siteDataClearPending
        onTriggered: {
            var view = window.siteDataClearView
            var ui = window.siteDataClearUi || browserUi
            if (!view) {
                siteDataClearPollTimer.stop()
                window.siteDataClearPending = false
                window.siteDataClearView = null
                window.siteDataClearUi = null
                ui.site_data_clear_finished(
                            window.siteDataClearOrigin,
                            JSON.stringify({"error": "active view disappeared"}))
                window.siteDataClearOrigin = ""
                return
            }
            view.runJavaScript(
                        BrowserScripts.siteDataClearResult(),
                        window.browserScriptWorld,
                        function(value) {
                            if (typeof value !== "string" || value.length === 0) {
                                return
                            }
                            try {
                                var result = JSON.parse(value)
                                if (result.pending === true) {
                                    return
                                }
                                siteDataClearPollTimer.stop()
                                window.siteDataClearPending = false
                                window.siteDataClearView = null
                                window.siteDataClearUi = null
                                ui.site_data_clear_finished(
                                            window.siteDataClearOrigin, value)
                                window.siteDataClearOrigin = ""
                                if (window.siteLedgerVisible) {
                                    window.showSiteLedger()
                                }
                            } catch (error) {
                                // Keep polling until the isolated-world result is complete.
                            }
                        })
        }
    }

    Connections {
        target: blocklistUpdater
        function onListReady(listId, content, etag, lastModified) {
            if (browserUi.install_blocking_list(listId, content, etag, lastModified)) {
                browserUi.reload_blocking_policy()
            } else {
                window.blocklistInstallFailed = true
                window.blocklistInstallError = browserUi.status_text
            }
        }
        function onUpdateFinished(success, message) {
            browserUi.status_text = window.blocklistInstallFailed
                    ? window.blocklistInstallError
                    : message
            window.blocklistInstallFailed = false
            window.blocklistInstallError = ""
        }
    }

    Connections {
        target: browserUi
        function onStorage_library_revisionChanged() {
            if (commandSurface.commandVisible) {
                browserUi.update_completion(commandSurface.commandText,
                                            commandSurface.cursorPosition)
            }
        }
    }

    Connections {
        target: linkRuleUpdater
        function onRuleReady(content, expectedChecksum, etag, lastModified) {
            if (!browserUi.accept_link_cleaning_bundle(
                        content, expectedChecksum, etag, lastModified)) {
                window.linkRuleInstallFailed = true
                window.linkRuleInstallError = browserUi.status_text
            }
        }
        function onUpdateFinished(success, message) {
            browserUi.status_text = window.linkRuleInstallFailed
                    ? window.linkRuleInstallError
                    : message
            window.linkRuleInstallFailed = false
            window.linkRuleInstallError = ""
        }
    }

    Component {
        id: devToolsWindowComponent

        FerricDevToolsWindow {
            id: devToolsWindow
            browserWindow: window
            onOwnerWindowClosed: {
                if (ownerWindow.devToolsDetachedWindow === devToolsWindow) {
                    ownerWindow.devToolsDetachedWindow = null
                    ownerWindow.devToolsExternalView = null
                    ownerWindow.devToolsDetached = false
                    ownerWindow.devToolsVisible = false
                    ownerWindow.browserUi.status_text = "DevTools closed"
                }
            }
            onInspectedViewClosed: {
                if (inspectView) {
                    inspectView.devToolsView = null
                }
            }
        }
    }

    BrowserUi {
        id: browserUi
        status_text: "Ready"
        onRuntime_work_available: window.scheduleRuntimeWork(0)
        onStatus_textChanged: window.scheduleRuntimeWork(0)
    }

    BrowserKeyRouter {
        id: browserKeyRouter
        targetWindow: window
        enabled: window.active && window.browserKeyFocusActive
                 && (browserUi.mode === "normal"
                     || browserUi.mode === "hint"
                     || browserUi.mode === "caret"
                     || browserUi.mode === "insert"
                     || browserUi.mode === "pass-through")
        onKeyPressed: function(text, key, modifiers) {
            var event = {
                text: text,
                key: key,
                modifiers: modifiers,
                accepted: false
            }
            if (window.handleBrowserKey(browserUi, window, event)) {
                browserKeyRouter.acceptCurrentEvent()
            }
        }
    }

    Timer {
        id: sessionPreviewPollTimer
        interval: 50
        repeat: true
        running: window.sessionPreviewLoading
        onTriggered: {
            var preview = browserUi.take_session_preview()
            if (preview.length > 0) {
                if (preview.indexOf("ERROR\t") === 0) {
                    window.sessionPreviewText = preview.slice(6)
                    window.sessionPreviewError = true
                } else {
                    window.sessionPreviewText = preview
                }
                window.sessionPreviewLoading = false
            }
            var restored = browserUi.take_session_restore_values()
            if (restored.length > 0) {
                if (window.recoveryAvailable) {
                    tabs.clear()
                    window.applyRestorePayload(restored, true)
                    window.recoveryAvailable = false
                } else {
                    window.sessionPreviewVisible = false
                    window.sessionManagerVisible = false
                    window.sessionPreviewError = false
                    window.closeInternalSurface()
                }
                window.sessionPreviewLoading = false
                window.syncTabModel()
            }
        }
    }

    Timer {
        id: bindingOverlayTimer
        interval: 350
        repeat: false
        onTriggered: {
            window.bindingOverlayVisible = browserUi.binding_overlay.length > 0
        }
    }

    Timer {
        id: tabSwitchingTimer
        interval: 1500
        repeat: false
        onTriggered: window.tabSwitchingVisible = false
    }

    Timer {
        id: applicationShutdownTimer
        interval: 100
        repeat: true
        running: window.applicationShutdownInProgress
        onTriggered: window.advanceApplicationShutdown()
    }

    Timer {
        id: shutdownPageProbeTimer
        interval: 2500
        repeat: false
        onTriggered: {
            window.shutdownPageProbeGeneration += 1
            window.shutdownPagePromptReason = "Page state check timed out."
            window.shutdownPagePromptVisible = true
            browserUi.status_text = "Page state check timed out; choose Close anyway"
        }
    }

    Timer {
        id: engineUpdateNoticeTimer
        interval: 250
        repeat: false
        onTriggered: window.refreshEngineUpdateNotice()
    }

}

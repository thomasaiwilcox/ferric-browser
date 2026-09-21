import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import QtWebEngine
import io.github.ferricbrowser 1.0

ApplicationWindow {
    id: window
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
    property var focusReturnStack: []
    property var modeFocusReturnTarget: null
    property bool modeFocusCaptured: false
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
    property var ephemeralProfileOwners: ({})
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

    function macroStatusText(raw) {
        try {
            var state = JSON.parse(String(raw || "{}"))
            if (state.recording && state.recording.register) {
                return " · recording macro @" + state.recording.register
                        + " (" + Number(state.recording.command_count || 0) + ")"
            }
            var registers = state.registers || []
            if (registers.length === 0) {
                return ""
            }
            var labels = []
            for (var index = 0; index < registers.length; index++) {
                var entry = registers[index]
                if (entry && entry.register) {
                    labels.push("@" + entry.register + ":" + Number(entry.command_count || 0))
                }
            }
            return labels.length > 0 ? " · macros " + labels.join(" ") : ""
        } catch (error) {
            return ""
        }
    }

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

    function colorChannels(value) {
        if (typeof value === "string"
                && /^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(value)) {
            return {
                r: parseInt(value.slice(1, 3), 16) / 255,
                g: parseInt(value.slice(3, 5), 16) / 255,
                b: parseInt(value.slice(5, 7), 16) / 255,
                a: value.length === 9 ? parseInt(value.slice(7, 9), 16) / 255 : 1.0
            }
        }
        return value
    }

    function renderedColorLuminance(color) {
        function linear(channel) {
            return channel <= 0.03928 ? channel / 12.92
                                       : Math.pow((channel + 0.055) / 1.055, 2.4)
        }
        return 0.2126 * linear(color.r) + 0.7152 * linear(color.g)
                + 0.0722 * linear(color.b)
    }

    function renderedContrastRatio(foreground, background) {
        foreground = colorChannels(foreground)
        background = colorChannels(background)
        var alpha = typeof foreground.a === "number" ? foreground.a : 1.0
        var compositedForeground = Qt.rgba(
                    foreground.r * alpha + background.r * (1.0 - alpha),
                    foreground.g * alpha + background.g * (1.0 - alpha),
                    foreground.b * alpha + background.b * (1.0 - alpha), 1.0)
        var foregroundLuminance = renderedColorLuminance(compositedForeground)
        var backgroundLuminance = renderedColorLuminance(background)
        var lighter = Math.max(foregroundLuminance, backgroundLuminance)
        var darker = Math.min(foregroundLuminance, backgroundLuminance)
        return (lighter + 0.05) / (darker + 0.05)
    }

    function contrastText(background) {
        return renderedContrastRatio("#000000", background)
                >= renderedContrastRatio("#ffffff", background)
                ? "#000000" : "#ffffff"
    }

    function readableTextColor(candidate, background) {
        return renderedContrastRatio(candidate, background) >= 4.5
                ? candidate : contrastText(background)
    }

    function renderedContrastReport() {
        var checks = [
            { name: "primary-on-background", ratio: renderedContrastRatio(
                window.primaryTextColor, window.backgroundColor) },
            { name: "secondary-on-surface", ratio: renderedContrastRatio(
                window.secondaryTextColor, window.surfaceColor) },
            { name: "muted-on-panel", ratio: renderedContrastRatio(
                window.mutedTextColor, window.panelColor) },
            { name: "accent-on-background", ratio: renderedContrastRatio(
                window.accentColor, window.backgroundColor) }
        ]
        var failing = []
        for (var index = 0; index < checks.length; index++) {
            if (checks[index].ratio < 4.5) {
                failing.push(checks[index].name)
            }
        }
        return { status: failing.length === 0 ? "pass" : "warning", failing: failing }
    }

    function refreshChromeAppearance() {
        var config = {}
        try {
            config = JSON.parse(browserUi.config_json)
        } catch (error) {
            config = {}
        }
        var ui = config.ui || {}
        var palette = {}
        try {
            palette = JSON.parse(browserUi.theme_palette_json)
        } catch (error) {
            palette = {}
        }
        var contrast = {}
        try {
            contrast = JSON.parse(browserUi.theme_contrast_json)
        } catch (error) {
            contrast = {}
        }
        var systemMotion = {}
        try {
            systemMotion = JSON.parse(browserUi.system_reduced_motion_json)
        } catch (error) {
            systemMotion = {}
        }
        var systemFont = {}
        try {
            systemFont = JSON.parse(browserUi.system_font_scale_json)
        } catch (error) {
            systemFont = {}
        }
        function paletteColor(value, fallback, opaque) {
            if (typeof value !== "string"
                    || !(/^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(value))) {
                return fallback
            }
            var alpha = value.length === 9 ? parseInt(value.slice(7, 9), 16) : 255
            if (opaque || alpha < 218) {
                return value.slice(0, 7) + (opaque ? "ff" : "da")
            }
            return value
        }
        window.backgroundColor = paletteColor(palette.background, "#1e1e2e", true)
        window.surfaceColor = paletteColor(
                    palette.surface || palette.lighter_background, "#313244", true)
        window.panelColor = paletteColor(palette.dark_background, "#181825", true)
        window.primaryTextColor = readableTextColor(
                    paletteColor(palette.foreground, "#cdd6f4"), window.backgroundColor)
        window.secondaryTextColor = readableTextColor(
                    paletteColor(palette.selection_foreground || palette.foreground, "#cdd6f4"),
                    window.surfaceColor)
        window.mutedTextColor = readableTextColor(
                    paletteColor(palette.muted, "#a6adc8"), window.panelColor)
        window.borderColor = paletteColor(palette.border, "#585b70", true)
        window.accentColor = readableTextColor(
                    paletteColor(palette.accent, "#89b4fa"), window.backgroundColor)
        window.warningColor = readableTextColor(
                    paletteColor(palette.warning || palette.yellow, "#f9e2af"),
                    window.backgroundColor)
        window.errorColor = readableTextColor(
                    paletteColor(palette.error || palette.red, "#f38ba8"),
                    window.backgroundColor)
        window.successColor = readableTextColor(
                    paletteColor(palette.success || palette.green, "#a6e3a1"),
                    window.backgroundColor)
        window.privateColor = readableTextColor(
                    paletteColor(palette.private, "#cba6f7"), window.backgroundColor)
        window.modeInsertColor = readableTextColor(
                    paletteColor(palette.mode_insert, "#f9e2af"), window.backgroundColor)
        window.selectionColor = paletteColor(
                    palette.selection_background || palette.selection, "#45475a", true)
        window.selectionTextColor = readableTextColor(
                    paletteColor(palette.selection_foreground || palette.foreground, "#cdd6f4"),
                    window.selectionColor)
        var renderedContrast = window.renderedContrastReport()
        window.renderedThemeContrastStatus = renderedContrast.status
        if (typeof ui.font_family === "string" && ui.font_family.trim().length > 0) {
            window.chromeFontFamily = ui.font_family
        }
        window.systemFontScale = systemFont.status === "available"
                && typeof systemFont.value === "number"
                && systemFont.value >= 0.5 && systemFont.value <= 3.0
                ? systemFont.value : 1.0
        if (typeof ui.font_size_pt === "number" && ui.font_size_pt >= 6 && ui.font_size_pt <= 40) {
            window.chromeFontPointSize = Math.min(60, Math.max(6, ui.font_size_pt
                                                               * window.systemFontScale))
        }
        window.statusbarMode = ["always", "command", "never"].indexOf(ui.statusbar) >= 0
                ? ui.statusbar : "always"
        window.tabsMode = ["always", "multiple", "switching", "never"].indexOf(ui.tabs) >= 0
                ? ui.tabs : "multiple"
        window.tabPosition = ["top", "bottom", "left", "right"].indexOf(ui.tab_position) >= 0
                ? ui.tab_position : "top"
        window.systemReducedMotionStatus = typeof systemMotion.status === "string"
                ? systemMotion.status : "unknown"
        if (ui.reduced_motion === "on") {
            window.reducedMotionActive = true
        } else if (ui.reduced_motion === "off") {
            window.reducedMotionActive = false
        } else {
            // Unknown or unavailable desktop preferences fail closed: optional
            // motion stays disabled until a positive probe says otherwise.
            window.reducedMotionActive = systemMotion.status === "available"
                    ? systemMotion.value === true : true
        }
        window.themeContrastStatus = typeof contrast.status === "string"
                ? contrast.status : "unknown"
        window.themeContrastWarning = (window.themeContrastStatus === "warning"
                || window.renderedThemeContrastStatus === "warning")
                ? String(contrast.reason || "Some theme colors may be difficult to read") : ""
    }

    function noteTabActivity() {
        if (window.tabsMode === "switching") {
            window.tabSwitchingVisible = true
            tabSwitchingTimer.restart()
        }
    }

    function spellcheckEnabled(ui) {
        var config = {}
        try {
            config = JSON.parse(ui && ui.config_json ? ui.config_json : "{}")
        } catch (error) {
            return true
        }
        return (!config.spellcheck || config.spellcheck.enabled !== false)
                && window.spellcheckLanguages(ui).length > 0
    }

    function spellcheckLanguageKey(language) {
        return String(language || "").trim().replace(/-/g, "_").toLowerCase()
    }

    function spellcheckLanguageIsValid(language) {
        var grandfathered = [
            "art-lojban", "cel-gaulish", "en-gb-oed", "i-ami", "i-bnn",
            "i-default", "i-enochian", "i-hak", "i-klingon", "i-lux",
            "i-mingo", "i-navajo", "i-pwn", "i-tao", "i-tay", "i-tsu",
            "no-bok", "no-nyn", "sgn-be-fr", "sgn-be-nl", "sgn-ch-de",
            "zh-guoyu", "zh-hakka", "zh-min", "zh-min-nan", "zh-xiang"
        ]
        var normalized = String(language || "").toLowerCase()
        return grandfathered.indexOf(normalized) >= 0
                || /^(?:[A-Za-z]{2,8}(?:-[A-Za-z0-9]{1,8})*|x(?:-[A-Za-z0-9]{1,8})+)$/.test(language)
    }

    function spellcheckDictionaryStatus(ui) {
        var config = {}
        try {
            config = JSON.parse(ui && ui.config_json ? ui.config_json : "{}")
        } catch (error) {
            config = {}
        }
        var configured = config.spellcheck && config.spellcheck.languages
        if (!Array.isArray(configured) || configured.length === 0) {
            configured = ["system"]
        }
        var requested = []
        for (var i = 0; i < configured.length && i < 16; ++i) {
            var language = String(configured[i] || "").trim()
            if (language === "system") {
                language = Qt.locale().name.replace("_", "-")
            }
            if (window.spellcheckLanguageIsValid(language)
                    && requested.indexOf(language) < 0) {
                requested.push(language)
            }
        }
        if (requested.length === 0) {
            requested = [Qt.locale().name.replace("_", "-")]
        }
        var installed = window.spellcheckInventory || []
        var installedKeys = []
        for (var installedIndex = 0; installedIndex < installed.length; ++installedIndex) {
            var installedKey = window.spellcheckLanguageKey(installed[installedIndex])
            if (installedKey.length > 0 && installedKeys.indexOf(installedKey) < 0) {
                installedKeys.push(installedKey)
            }
        }
        var active = []
        var missing = []
        for (var requestedIndex = 0; requestedIndex < requested.length; ++requestedIndex) {
            var requestedLanguage = requested[requestedIndex]
            var requestedKey = window.spellcheckLanguageKey(requestedLanguage)
            var baseKey = requestedKey.split("_")[0]
            var found = installedKeys.indexOf(requestedKey) >= 0
                    || installedKeys.indexOf(baseKey) >= 0
            if (found) {
                active.push(requestedLanguage)
            } else {
                missing.push(requestedLanguage)
            }
        }
        return { requested: requested, active: active, missing: missing, installed: installed }
    }

    function spellcheckLanguages(ui) {
        var status = window.spellcheckDictionaryStatus(ui)
        return status.active.length > 0 ? status.active : []
    }

    function refreshSpellcheckInventory(ui) {
        try {
            var payload = ui && ui.spellcheck_dictionaries ? ui.spellcheck_dictionaries() : "[]"
            var values = JSON.parse(payload || "[]")
            window.spellcheckInventory = Array.isArray(values) ? values.slice(0, 64) : []
        } catch (error) {
            window.spellcheckInventory = []
        }
    }

    function spellcheckStatusText(ui) {
        var status = window.spellcheckDictionaryStatus(ui)
        if (status.missing.length === 0) {
            return status.installed.length > 0
                    ? "Spellcheck dictionaries: " + status.active.join(", ")
                    : "Spellcheck dictionary inventory unavailable; no download was attempted"
        }
        return "Missing spellcheck dictionaries: " + status.missing.join(", ")
                + " (no download was attempted)"
    }

    function desktopNotificationsEnabled(ui) {
        try {
            var config = JSON.parse(ui && ui.config_json ? ui.config_json : "{}")
            return !config.desktop || config.desktop.notifications !== false
        } catch (error) {
            return true
        }
    }

    function pushServiceEnabled(ui, privateProfile) {
        if (privateProfile) {
            return false
        }
        try {
            var config = JSON.parse(ui && ui.config_json ? ui.config_json : "{}")
            return !!(config.privacy && config.privacy.push_service === true)
        } catch (error) {
            return false
        }
    }

    function mediaKeysEnabled(ui) {
        try {
            var config = JSON.parse(ui && ui.config_json ? ui.config_json : "{}")
            return !config.desktop || config.desktop.media_keys !== false
        } catch (error) {
            return true
        }
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
        return !!target && !!target.forceActiveFocus
                && target.visible !== false && target.enabled !== false
    }

    function captureOverlayFocus(hostWindow, fallbackTarget) {
        var host = hostWindow || window
        var target = null
        try {
            target = host.activeFocusItem
        } catch (error) {
            target = null
        }
        if (!window.focusTargetAvailable(target)) {
            target = fallbackTarget || null
        }
        var stack = (window.focusReturnStack || []).slice(0)
        stack.push({ host: host, target: target, fallback: fallbackTarget || null })
        window.focusReturnStack = stack
    }

    function restoreOverlayFocus() {
        var stack = (window.focusReturnStack || []).slice(0)
        var entry = stack.length > 0 ? stack.pop() : null
        window.focusReturnStack = stack
        if (!entry) {
            return
        }
        Qt.callLater(function() {
            if (window.focusTargetAvailable(entry.target)) {
                entry.target.forceActiveFocus()
            } else if (window.focusTargetAvailable(entry.fallback)) {
                entry.fallback.forceActiveFocus()
            } else if (entry.host === window) {
                var view = window.activeWebView()
                if (window.focusTargetAvailable(view)) {
                    view.forceActiveFocus()
                }
            }
        })
    }

    function captureModeFocus() {
        if (window.modeFocusCaptured) {
            return
        }
        var target = window.activeFocusItem
        if (!window.focusTargetAvailable(target)) {
            target = window.activeWebView()
        }
        window.modeFocusReturnTarget = target
        window.modeFocusCaptured = true
    }

    function restoreModeFocus() {
        var target = window.modeFocusCaptured
                ? window.modeFocusReturnTarget : window.activeWebView()
        window.modeFocusReturnTarget = null
        window.modeFocusCaptured = false
        Qt.callLater(function() {
            if (window.focusTargetAvailable(target)) {
                target.forceActiveFocus()
            } else {
                var view = window.activeWebView()
                if (window.focusTargetAvailable(view)) {
                    view.forceActiveFocus()
                }
            }
        })
    }

    function openInternalSurface() {
        window.captureOverlayFocus(window, window.activeWebView())
    }

    function closeInternalSurface() {
        window.restoreOverlayFocus()
    }

    function showContextRoute(ui) {
        if (!ui || !ui.context_route_json || ui.context_route_json === "{}") {
            return
        }
        try {
            var data = JSON.parse(ui.context_route_json)
            if (!data || !data.route_id || !data.context || !data.profile || !data.url) {
                ui.status_text = "Context route data was invalid"
                return
            }
            window.contextRouteUi = ui
            window.contextRouteData = data
            window.contextRouteVisible = true
            window.captureOverlayFocus(window, window.activeWebView())
        } catch (error) {
            ui.status_text = "Context route data was invalid"
        }
    }

    function closeContextRoutePrompt() {
        var wasVisible = window.contextRouteVisible
        window.contextRouteVisible = false
        window.contextRouteUi = null
        window.contextRouteData = ({})
        if (wasVisible) {
            window.restoreOverlayFocus()
        }
    }

    function showContextMovePicker(tabId) {
        var sourceEntry = window.browserWindowEntryForUi(browserUi)
        var choices = []
        try {
            var config = JSON.parse(browserUi.contexts_json || "{}")
            var contexts = config && Array.isArray(config.contexts) ? config.contexts : []
            for (var i = 0; i < contexts.length; ++i) {
                var context = contexts[i]
                if (!context || typeof context.name !== "string"
                        || context.name.length === 0
                        || String(context.name) === String(browserUi.context_name || "")
                        || String(context.profile || "") !== String(window.profileName)) {
                    continue
                }
                var targetEntry = window.browserWindowEntryForContext(
                            context.name, sourceEntry)
                choices.push({
                    name: context.name,
                    label: context.label || context.name,
                    available: window.sameProfileWindow(sourceEntry, targetEntry)
                })
            }
        } catch (error) {
            browserUi.status_text = "Context move choices are unavailable"
            return
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
        var values = []
        var entries = window.browserWindowRegistry || []
        for (var i = 0; i < entries.length && values.length < 64; ++i) {
            var entry = entries[i]
            if (!entry || !entry.ui || !entry.host) {
                continue
            }
            values.push({
                "id": String(entry.coreWindowId || ""),
                "owner_token": String(entry.windowToken || ""),
                "profile": String(entry.profileName || ""),
                "private": !!entry.privateProfile,
                "ephemeral": !!entry.ephemeralProfile,
                "tab_count": entry.ui === browserUi
                        ? Number(browserUi.tab_count || 0)
                        : Number(entry.ui.tab_count || 0)
            })
        }
        browserUi.window_registry_json = JSON.stringify(values)
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
        var requested = String(token || "")
        if (!profile || requested.length === 0) {
            return false
        }
        var owners = window.ephemeralProfileOwners || ({})
        var current = owners[requested]
        if (current && current.profile && current.profile !== profile
                && Number(current.count || 0) > 0) {
            return false
        }
        owners[requested] = {
            profile: profile,
            count: Number(current && current.count || 0) + 1
        }
        window.ephemeralProfileOwners = owners
        return true
    }

    function releaseEphemeralProfileOwner(profile, token) {
        var requested = String(token || "")
        if (!profile || requested.length === 0) {
            return false
        }
        var owners = window.ephemeralProfileOwners || ({})
        var current = owners[requested]
        if (!current || current.profile !== profile) {
            return false
        }
        var count = Number(current.count || 0) - 1
        if (count > 0) {
            owners[requested] = { profile: profile, count: count }
        } else {
            delete owners[requested]
        }
        window.ephemeralProfileOwners = owners
        return true
    }

    function switcherOwner(result) {
        var ownerToken = result && result.owner_token ? String(result.owner_token) : ""
        var entries = window.browserWindowRegistry || []
        for (var i = 0; i < entries.length; ++i) {
            if (entries[i] && entries[i].ui
                    && String(entries[i].ui.window_token) === ownerToken) {
                return entries[i]
            }
        }
        return { host: window, ui: browserUi }
    }

    function ephemeralProfileForToken(token) {
        var requested = String(token || "")
        if (requested.length === 0) {
            return null
        }
        var owner = (window.ephemeralProfileOwners || ({}))[requested]
        if (owner && owner.profile && Number(owner.count || 0) > 0) {
            return owner.profile
        }
        var entries = window.browserWindowRegistry || []
        for (var i = 0; i < entries.length; ++i) {
            var entry = entries[i]
            if (entry && entry.ephemeralProfile
                    && entry.profile
                    && String(entry.ephemeralInvocationToken || "") === requested) {
                return entry.profile
            }
        }
        return null
    }

    function ephemeralTokenForUi(ui) {
        var entries = window.browserWindowRegistry || []
        for (var i = 0; i < entries.length; ++i) {
            var entry = entries[i]
            if (entry && entry.ui === ui && entry.ephemeralProfile) {
                return String(entry.ephemeralInvocationToken || "")
            }
        }
        return ""
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
        return /^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(accent)
                ? accent.slice(0, 7) : fallback
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
    property string startupAdditionalUrlsJson: "[]"
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
    property var contextRouteData: ({})
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
    property string profileRenameName: ""
    property int profileRefreshAttempts: 0
    property string profileRenameLabel: ""
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
    property var bindingHelpData: ({})

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
    property var settingsRows: []
    property bool libraryManagerVisible: false
    property string pendingLibraryDelete: ""
    property bool privateHistoryTransferVisible: false
    property string privateHistoryTransferId: ""
    property string privateHistoryTransferTitle: ""
    property string privateHistoryTransferUrl: ""
    property int libraryPage: 0
    property int libraryPageSize: 100
    property int libraryTotalEntries: 0
    property var linkPreviewData: ({})
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
        blockedHosts: {
            try {
                var hosts = JSON.parse(browserUi.blocking_hosts)
                return Array.isArray(hosts) ? hosts : []
            } catch (error) {
                return []
            }
        }
        exceptionHosts: {
            try {
                var hosts = JSON.parse(browserUi.blocking_exceptions)
                return Array.isArray(hosts) ? hosts : []
            } catch (error) {
                return []
            }
        }
        blockedRuleLists: {
            try {
                var lists = JSON.parse(browserUi.blocking_rule_lists)
                return lists && typeof lists === "object" && !Array.isArray(lists) ? lists : ({})
            } catch (error) {
                return ({})
            }
        }
        exceptionRuleLists: {
            try {
                var lists = JSON.parse(browserUi.blocking_exception_rule_lists)
                return lists && typeof lists === "object" && !Array.isArray(lists) ? lists : ({})
            } catch (error) {
                return ({})
            }
        }
        adblockEngineHandle: browserUi.blocking_adblock_handle
        bypassSites: {
            try {
                var sites = JSON.parse(browserUi.blocking_bypass_sites)
                return Array.isArray(sites) ? sites : []
            } catch (error) {
                return []
            }
        }
        securityDenyHosts: {
            try {
                var hosts = JSON.parse(browserUi.blocking_security_deny_hosts)
                return Array.isArray(hosts) ? hosts : []
            } catch (error) {
                return []
            }
        }
        onBlockedCountChanged: window.refreshBlockingEvidence()
        onUnknownContextCountChanged: window.refreshBlockingEvidence()
        onBlockedSiteCountsChanged: window.refreshBlockingEvidence()
    }

    function refreshBlockingEvidence() {
        var blockedRequests = Number(requestInterceptor.blockedCount)
        if (isFinite(blockedRequests)) {
            browserUi.blocking_blocked_count = Math.max(0, Math.floor(blockedRequests))
        }
        var unknownRequests = Number(requestInterceptor.unknownContextCount)
        if (isFinite(unknownRequests)) {
            browserUi.blocking_unknown_context_count = Math.max(
                        0, Math.floor(unknownRequests))
        }
        browserUi.blocking_active_site_count = Math.max(
                    0, Math.floor(window.activeBlockedRequestCount))
        var activeView = window.activeWebView()
        var activeHost = activeView && activeView.url ? activeView.url.host : ""
        browserUi.blocking_active_explanation = JSON.stringify(
                    requestInterceptor.blockedRequestExplanation(activeHost))
        browserUi.blocking_active_decisions = JSON.stringify(
                    requestInterceptor.blockedRequestDecisions(activeHost))
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
            if (consumed && browserUi.mode === "command") {
                browserUi.update_completion(commandLine.text, commandLine.cursorPosition)
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
                        "window.__ferric_browserSiteDataClearResult || ''",
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
            if (commandBar.visible) {
                browserUi.update_completion(commandLine.text, commandLine.cursorPosition)
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

        ApplicationWindow {
            id: devToolsWindow
            property var inspectView: null
            property var ownerWindow: null
            property alias inspectorView: detachedDevToolsView
            width: 980
            height: 620
            visible: false
            title: "Ferric Browser · DevTools"
            color: ownerWindow ? ownerWindow.backgroundColor : "#1e1e2e"
            palette: window.palette
            onClosing: {
                if (ownerWindow) {
                    if (ownerWindow.devToolsDetachedWindow === devToolsWindow) {
                        ownerWindow.devToolsDetachedWindow = null
                        ownerWindow.devToolsExternalView = null
                        ownerWindow.devToolsDetached = false
                        ownerWindow.devToolsVisible = false
                        ownerWindow.browserUi.status_text = "DevTools closed"
                    }
                } else if (devToolsWindow.inspectView) {
                    devToolsWindow.inspectView.devToolsView = null
                }
            }

            WebEngineView {
                id: detachedDevToolsView
                anchors.fill: parent
                profile: devToolsWindow.inspectView ? devToolsWindow.inspectView.profile : null
                inspectedView: devToolsWindow.inspectView
                Accessible.name: "Detached developer tools"
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

    Rectangle {
        id: bindingOverlay
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: window.statusBarHeight + 8
        width: Math.min(620, parent.width - 40)
        height: Math.max(34, bindingOverlayLabel.implicitHeight + 14)
        z: 30
        visible: window.bindingOverlayVisible
        enabled: false
        color: window.panelColor
        border.color: window.accentColor
        border.width: 1
        radius: 3

        Label {
            id: bindingOverlayLabel
            anchors.fill: parent
            anchors.margins: 7
            text: browserUi.binding_overlay
            color: window.primaryTextColor
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            wrapMode: Text.WordWrap
            Accessible.name: "Keyboard binding continuation"
            Accessible.role: Accessible.StatusBar
        }
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
            var detached = devToolsWindowComponent.createObject(null, {
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
        return "(function(){"
            + "var kind=" + JSON.stringify(String(kind)) + ";"
            + "var direction=" + JSON.stringify(String(direction)) + ";"
            + "var half=" + (half ? "true" : "false") + ";"
            + "var count=" + String(Math.max(1, Math.min(9999, Number(count) || 1))) + ";"
            + "var root=document.scrollingElement||document.documentElement;"
            + "var target=root;var node=document.activeElement;"
            + "while(node&&node!==document.body&&node!==document.documentElement){"
            + "var style=window.getComputedStyle(node);"
            + "var vertical=(style.overflowY==='auto'||style.overflowY==='scroll')&&node.scrollHeight>node.clientHeight;"
            + "var horizontal=(style.overflowX==='auto'||style.overflowX==='scroll')&&node.scrollWidth>node.clientWidth;"
            + "if((direction==='up'||direction==='down')?vertical:horizontal){target=node;break;}"
            + "node=node.parentElement;}"
            + "var width=Math.max(window.innerWidth||0,target.clientWidth||0);"
            + "var height=Math.max(window.innerHeight||0,target.clientHeight||0);"
            + "if(kind==='scroll-to'){"
            + "var top=direction==='top'?0:Math.max(0,target.scrollHeight-height);"
            + "var left=direction==='top'?0:Math.max(0,target.scrollWidth-width);"
            + "target.scrollTo(left,top);"
            + "}else{"
            + "var step=kind==='scroll-page'?(half?0.5:0.9):0.15;"
            + "var dx=(direction==='left'?-1:direction==='right'?1:0)*Math.max(40,width*step)*count;"
            + "var dy=(direction==='up'?-1:direction==='down'?1:0)*Math.max(40,height*step)*count;"
            + "target.scrollBy(dx,dy);}"
            + "return true;})()"
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
            "(function(){var root=document.scrollingElement||document.documentElement;"
            + "if(!root)return null;"
            + "return {x:Math.max(0,root.scrollLeft||0),y:Math.max(0,root.scrollTop||0)};})()",
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
            "(function(){var root=document.scrollingElement||document.documentElement;"
            + "if(!root)return false;root.scrollTo(" + x + "," + y + ");return true;})()")
    }

    function siteDataClearScript() {
        return "(function(){"
            + "var result={pending:true,local_storage:'unavailable',cache_storage:'unavailable',"
            + "service_workers:'unavailable',cookies:'page-visible-only',http_cache:'profile-wide-only'};"
            + "try{localStorage.clear();result.local_storage='cleared';}catch(error){"
            + "result.local_storage='unavailable';}"
            + "try{var names=document.cookie?document.cookie.split(';'):[];"
            + "for(var i=0;i<names.length;i++){var name=names[i].split('=')[0].trim();"
            + "if(name)document.cookie=name+'=;expires=Thu, 01 Jan 1970 00:00:00 GMT;path=/';}"
            + "result.cookies='page-visible-only';}catch(error){result.cookies='unavailable';}"
            + "var jobs=[];"
            + "if(self.caches&&self.caches.keys){result.cache_storage='pending';"
            + "jobs.push(self.caches.keys().then(function(keys){return Promise.all(keys.map(function(key){"
            + "return self.caches.delete(key);}));}).then(function(){result.cache_storage='cleared';}"
            + ").catch(function(){result.cache_storage='unavailable';}));}"
            + "if(navigator.serviceWorker&&navigator.serviceWorker.getRegistrations){"
            + "result.service_workers='pending';jobs.push(navigator.serviceWorker.getRegistrations().then("
            + "function(registrations){return Promise.all(registrations.map(function(registration){"
            + "return registration.unregister();}));}).then(function(){result.service_workers='cleared';}"
            + ").catch(function(){result.service_workers='unavailable';}));}"
            + "Promise.all(jobs).then(function(){result.pending=false;"
            + "window.__ferric_browserSiteDataClearResult=JSON.stringify(result);});"
            + "window.__ferric_browserSiteDataClearResult=JSON.stringify(result);"
            + "return window.__ferric_browserSiteDataClearResult;})()"
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
        if (!ui || !ui.site_rule_settings || !url) {
            return { values: {}, matched_rules: [] }
        }
        try {
            var experiment = JSON.parse(ui.site_experiment_json || "{}")
            var experimentDocument = String(experiment.url || "").split(/[?#]/)[0]
            var currentDocument = String(url).split(/[?#]/)[0]
            if (experiment.kind === "compiled-defaults"
                    && experimentDocument === currentDocument) {
                return { values: {}, matched_rules: [] }
            }
        } catch (error) {
            // Rust will reject malformed experiment state; keep the normal
            // site-rule path usable while that state is being cleared.
        }
        try {
            var result = JSON.parse(ui.site_rule_settings(url))
            return result && result.values ? result : { values: {}, matched_rules: [] }
        } catch (error) {
            return { values: {}, matched_rules: [] }
        }
    }

    function siteRuleValue(settings, key, fallback) {
        if (settings && settings.values
                && settings.values[key] !== undefined) {
            return settings.values[key]
        }
        return fallback
    }

    function focusObserverSource() {
        return "(function(){"
            + "if(window.__ferric_browserFocusObserverInstalled)return;"
            + "window.__ferric_browserFocusObserverInstalled=true;"
            + "var sequence=0;var userGestureUntil=0;"
            + "function classify(){"
            + "var element=document.activeElement;var depth=0;"
            + "while(element&&element.shadowRoot&&depth<16&&element.shadowRoot.activeElement){"
            + "element=element.shadowRoot.activeElement;depth++;}"
            + "var tag=element&&element.tagName?String(element.tagName).toLowerCase():'';"
            + "var type=element&&element.type?String(element.type).toLowerCase():'';"
            + "var excluded=['button','checkbox','file','hidden','image','radio','range','reset','submit'];"
            + "var editable=!!element&&(element.isContentEditable===true||tag==='textarea'||tag==='select'||"
            + "(tag==='input'&&excluded.indexOf(type)<0));"
            + "var kind=editable?(tag==='textarea'?'textarea':element.isContentEditable?'contenteditable':"
            + "tag==='input'&&type==='password'?'password':'input'):tag==='iframe'?'frame':'other';"
            + "return {editable:editable,kind:kind};}"
            + "function publish(userActivated){var state=classify();"
            + "window.__ferric_browserFocusState={sequence:++sequence,editable:state.editable,user_activated:!!userActivated,kind:state.kind};}"
            + "window.__ferric_browserAuthorizeExplicitFocus=function(){userGestureUntil=performance.now()+1500;};"
            + "function noteGesture(event){if(event.isTrusted!==false){userGestureUntil=performance.now()+1500;publish(true);}}"
            + "document.addEventListener('pointerdown',noteGesture,true);"
            + "document.addEventListener('keydown',function(event){if(event.key==='Tab')noteGesture(event);},true);"
            + "document.addEventListener('focusin',function(){publish(performance.now()<=userGestureUntil);},true);"
            + "document.addEventListener('focusout',function(){setTimeout(function(){publish(false);},0);},true);"
            + "publish(false);})();"
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
        return "(function(){var state=window.__ferric_browserFocusState;"
            + "return state?{sequence:Number(state.sequence)||0,editable:!!state.editable,"
            + "user_activated:!!state.user_activated,kind:String(state.kind||'unknown')}"
            + ":{sequence:0,editable:false,user_activated:false,kind:'unknown'};})()"
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
        if (window.safeMode || !ui || !view || !ui.blocking_cosmetic_rules) {
            return
        }
        var rules = []
        var exceptions = []
        try {
            rules = JSON.parse(ui.blocking_cosmetic_rules)
            exceptions = JSON.parse(ui.blocking_cosmetic_exceptions || "[]")
        } catch (error) {
            ui.status_text = "Cosmetic filter metadata was invalid"
            return
        }
        if (!Array.isArray(rules) || !Array.isArray(exceptions)) {
            return
        }
        var host = view.url && view.url.host ? String(view.url.host) : ""
        var selectors = []
        for (var i = 0; i < rules.length && selectors.length < 256; ++i) {
            var rule = rules[i]
            if (!rule || !cosmeticHostMatches(host, rule.host)) {
                continue
            }
            var excluded = false
            for (var j = 0; j < exceptions.length; ++j) {
                var exception = exceptions[j]
                if (exception && exception.selector === rule.selector
                        && cosmeticHostMatches(host, exception.host)) {
                    excluded = true
                    break
                }
            }
            if (!excluded && selectors.indexOf(String(rule.selector || "")) < 0) {
                selectors.push(String(rule.selector || ""))
            }
        }
        if (selectors.length === 0) {
            return
        }
        var css = selectors.join(" { display: none !important; }\n")
                + " { display: none !important; }"
        var source = "(function(){try{"
                + "var id='ferric-browser-cosmetic-filter';"
                + "var old=document.getElementById(id);if(old)old.remove();"
                + "var style=document.createElement('style');style.id=id;"
                + "style.textContent=" + JSON.stringify(css) + ";"
                + "(document.head||document.documentElement).appendChild(style);"
                + "}catch(error){return false;}return true;})()"
        window.runBrowserScript(view, source, function(ok) {
            if (ok === false) {
                ui.status_text = "Cosmetic filter injection failed"
            }
        }, WebEngineScript.MainWorld)
    }

    function injectPageUserscripts(ui, view, url, privateProfile, phase) {
        if (window.safeMode || window.userscriptsOff
                || !ui || !view || !url || !ui.matching_page_scripts) {
            return
        }
        if (window.siteDoctorUserscriptsDisabled(ui)) {
            return
        }
        var scripts = []
        try {
            scripts = JSON.parse(ui.matching_page_scripts(url, privateProfile))
        } catch (error) {
            ui.status_text = "Page userscript metadata was invalid"
            return
        }
        for (var i = 0; i < scripts.length; ++i) {
            var script = scripts[i]
            if (script.run_at !== phase) {
                continue
            }
            if (script.runs_on_sub_frames) {
                continue
            }
            (function(pageUi, scriptName, scriptSource) {
                var source = "(function(){try{" + scriptSource
                        + "\n}catch(error){return false;}return true;})()"
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
                || !ui || !view || !url || !ui.matching_page_scripts || !view.userScripts) {
            return
        }
        if (window.siteDoctorUserscriptsDisabled(ui)) {
            return
        }
        window.clearInstalledPageUserscripts(view)
        var scripts = []
        try {
            scripts = JSON.parse(ui.matching_page_scripts(url, privateProfile))
        } catch (error) {
            ui.status_text = "Page userscript metadata was invalid"
            return
        }
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
            installedScript.sourceCode = "(function(){try{" + script.source
                    + "\n}catch(error){}})()"
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

    FileDialog {
        id: downloadChooser
        title: "Choose download destination"
        fileMode: FileDialog.SaveFile
        nameFilters: ["All files (*)"]
        onAccepted: window.acceptPendingDownload()
        onRejected: window.cancelPendingDownload()
    }

    FileDialog {
        id: journeyExportChooser
        title: "Choose journey export destination"
        fileMode: FileDialog.SaveFile
        nameFilters: ["JSON files (*.json)", "All files (*)"]
        onAccepted: window.finishJourneyExport()
        onRejected: window.journeyExportPreviewVisible = true
    }

    FileDialog {
        id: diagnosticsExportChooser
        title: "Choose diagnostics export destination"
        fileMode: FileDialog.SaveFile
        nameFilters: ["JSON files (*.json)", "All files (*)"]
        onAccepted: window.finishDiagnosticsExport()
        onRejected: window.diagnosticsVisible = true
    }

    FileDialog {
        id: engineFileChooser
        title: "Choose file"
        onAccepted: window.acceptEngineFileDialog()
        onRejected: window.rejectEngineFileDialog()
    }

    FileDialog {
        id: userscriptManifestChooser
        title: "Install userscript manifest"
        fileMode: FileDialog.OpenFile
        nameFilters: ["Userscript manifests (*.toml)", "All files (*)"]
        onAccepted: window.installUserscriptManifest()
    }

    Dialog {
        id: userscriptRemovalDialog
        title: "Remove userscript"
        modal: true
        width: Math.min(520, window.width - 48)
        height: Math.min(180, window.height - 48)
        standardButtons: Dialog.Ok | Dialog.Cancel
        contentItem: Label {
            text: "Remove userscript '" + window.pendingUserscriptRemoval
                    + "' and its copied assets?"
            wrapMode: Text.WordWrap
            padding: 16
            color: window.primaryTextColor
        }
        onAccepted: {
            var name = window.pendingUserscriptRemoval
            window.pendingUserscriptRemoval = ""
            if (browserUi.remove_userscript(name)) {
                window.settingsNotice = "Removing userscript…"
            }
        }
        onRejected: window.pendingUserscriptRemoval = ""
    }

    FolderDialog {
        id: engineFolderChooser
        title: "Choose folder"
        onAccepted: window.acceptEngineFolderDialog()
        onRejected: window.rejectEngineFileDialog()
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
        var urls = engineFileChooser.fileMode === FileDialog.OpenFiles
                ? engineFileChooser.selectedFiles
                : [engineFileChooser.selectedFile]
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
        var url = engineFolderChooser.selectedFolder
        var path = url && url.scheme === "file" ? url.toLocalFile() : ""
        return path ? [path] : []
    }

    function desktopPortalMode(ui) {
        var config = JSON.parse(ui && ui.config_json ? ui.config_json : "{}")
        return config.desktop && config.desktop.portals
                ? String(config.desktop.portals) : "auto"
    }

    function desktopPortalCapabilityStatus(ui, capability) {
        // Portal availability is user-session global, so secondary and popup
        // browser objects use the root probe result instead of maintaining
        // independent D-Bus probes.
        var value = JSON.parse(browserUi && browserUi.desktop_portal_status
                                ? browserUi.desktop_portal_status : "{}")
        var serviceStatus = value.service && value.service.status
                ? String(value.service.status) : "not-probed"
        if (serviceStatus !== "available") {
            return serviceStatus
        }
        var interfaceValue = value.interfaces && value.interfaces[capability]
        return interfaceValue && interfaceValue.status
                ? String(interfaceValue.status) : "unavailable"
    }

    function openPendingEngineFileDialog() {
        var request = window.pendingFileDialogRequest
        if (!request) {
            return
        }
        window.pendingFileDialogWaitingForPortal = false
        if (request.mode === FileDialogRequest.FileModeUploadFolder) {
            engineFolderChooser.open()
            return
        }
        engineFileChooser.fileMode = request.mode === FileDialogRequest.FileModeOpenMultiple
                ? FileDialog.OpenFiles
                : request.mode === FileDialogRequest.FileModeSave
                    ? FileDialog.SaveFile
                    : FileDialog.OpenFile
        engineFileChooser.nameFilters = window.fileDialogNameFilters(request)
        engineFileChooser.currentFile = request.defaultFileName.length > 0
                ? request.defaultFileName : ""
        engineFileChooser.title = request.mode === FileDialogRequest.FileModeSave
                ? "Save file"
                : request.mode === FileDialogRequest.FileModeOpenMultiple
                    ? "Choose files"
                    : "Choose file"
        engineFileChooser.open()
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
        if (engineFileChooser.visible) {
            engineFileChooser.close()
        }
        if (engineFolderChooser.visible) {
            engineFolderChooser.close()
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
        profileRefreshTimer.restart()
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
        var data = window.bindingHelpData || {}
        var commands = data.commands || []
        var bindings = data.bindings || []
        var conflicts = data.conflicts || []
        var modes = ["normal", "insert", "command", "search", "hint", "caret", "pass-through"]
        var query = (window.bindingHelpSearch || "").trim().toLowerCase()
        var rows = []
        function matches(value) {
            return !query || String(value || "").toLowerCase().indexOf(query) >= 0
        }
        for (var modeIndex = 0; modeIndex < modes.length; ++modeIndex) {
            var mode = modes[modeIndex]
            var modeRows = []
            for (var commandIndex = 0; commandIndex < commands.length; ++commandIndex) {
                var command = commands[commandIndex]
                if ((command.modes || []).indexOf(mode) < 0) {
                    continue
                }
                var commandBindings = []
                for (var bindingIndex = 0; bindingIndex < bindings.length; ++bindingIndex) {
                    var binding = bindings[bindingIndex]
                    if (binding.mode === mode && binding.command_name === command.name) {
                        commandBindings.push(binding)
                    }
                }
                var keyText = commandBindings.length > 0
                        ? commandBindings.map(function(binding) {
                            return (binding.keys || []).join(" ")
                        }).join(" · ")
                        : "unbound"
                var sourceText = commandBindings.length > 0
                        ? commandBindings.map(function(binding) { return binding.source }).join(" · ")
                        : "—"
                var haystack = [mode, command.name, command.description, keyText, sourceText].join(" ")
                if (!matches(haystack)) {
                    continue
                }
                modeRows.push({
                    kind: "command",
                    mode: mode,
                    command: command.name,
                    description: command.description,
                    keys: keyText,
                    source: sourceText,
                    count: command.count && command.count.supported
                           ? "count ≤ " + command.count.maximum
                           : "no count"
                })
            }
            if (modeRows.length > 0) {
                rows.push({ kind: "heading", title: mode + " mode" })
                rows = rows.concat(modeRows)
            }
        }
        var visibleConflicts = []
        for (var conflictIndex = 0; conflictIndex < conflicts.length; ++conflictIndex) {
            var conflict = conflicts[conflictIndex]
            var conflictText = [conflict.mode, (conflict.keys || []).join(" / "),
                                 (conflict.commands || []).join(" / "), conflict.description].join(" ")
            if (matches(conflictText)) {
                visibleConflicts.push({
                    kind: "conflict",
                    mode: conflict.mode,
                    command: (conflict.keys || []).join(" / "),
                    description: conflict.description,
                    keys: (conflict.commands || []).join(" · "),
                    source: "prefix ambiguity",
                    count: "timeout " + (data.bindings && data.bindings.length > 0
                          ? (data.bindings[0].timeout_ms || 1000) : 1000) + " ms"
                })
            }
        }
        if (visibleConflicts.length > 0) {
            rows.push({ kind: "heading", title: "Binding conflicts" })
            rows = rows.concat(visibleConflicts)
        }
        window.bindingHelpRows = rows
    }

    function refreshBindingHelp() {
        var payload = browserUi.bindings_json()
        try {
            var parsed = JSON.parse(payload)
            window.bindingHelpData = parsed && typeof parsed === "object" ? parsed : {}
        } catch (error) {
            window.bindingHelpData = {
                commands: [],
                bindings: [],
                conflicts: [],
                error: "Binding map response was invalid"
            }
        }
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

    function settingConfigValue(config, key, fallback) {
        var value = config
        var parts = key.split(".")
        for (var i = 0; i < parts.length; ++i) {
            if (!value || value[parts[i]] === undefined) {
                return fallback
            }
            value = value[parts[i]]
        }
        return value
    }

    function refreshSettings() {
        var config = {}
        try {
            config = JSON.parse(browserUi.config_json)
        } catch (error) {
            window.settingsRows = []
            window.settingsNotice = "The validated configuration could not be displayed"
            return
        }
        var specs = [
            { key: "ui.font_family", label: "Chrome font family", type: "text",
              scope: "global", apply: "live", fallback: "monospace" },
            { key: "ui.font_size_pt", label: "Chrome font size (pt)", type: "number",
              scope: "global", apply: "live", fallback: 10.0 },
            { key: "ui.reduced_motion", label: "Reduced motion", type: "enum",
              options: ["system", "on", "off"], scope: "global", apply: "live", fallback: "system" },
            { key: "input.entry_mode", label: "Page entry mode", type: "enum",
              options: ["normal", "insert", "pass-through"], scope: "global/profile/site",
              apply: "next navigation", fallback: "normal" },
            { key: "discovery.learning_mode", label: "Learning mode", type: "bool",
              scope: "global/profile", apply: "live", fallback: false },
            { key: "links.cleaning.enabled", label: "Clean-link operations", type: "bool",
              scope: "global/profile", apply: "live", fallback: true },
            { key: "content.javascript", label: "JavaScript", type: "bool",
              scope: "global/profile/site", apply: "next navigation", fallback: true },
            { key: "content.images", label: "Images", type: "bool",
              scope: "global/profile/site", apply: "next navigation", fallback: true },
            { key: "content.force_dark", label: "Force dark pages", type: "bool",
              scope: "global/profile/site", apply: "next navigation", fallback: false },
            { key: "content.autoplay", label: "Autoplay policy", type: "enum",
              options: ["engine-default", "require-gesture"], scope: "global/profile/site",
              apply: "next navigation", fallback: "engine-default" },
            { key: "content.zoom", label: "Default page zoom", type: "number",
              scope: "global/profile/site", apply: "live", fallback: 1.0 },
            { key: "privacy.remote_suggestions", label: "Remote suggestions", type: "bool",
              scope: "global/profile", apply: "live", fallback: false },
            { key: "blocking.enabled", label: "Network blocking", type: "bool",
              scope: "global/profile/site", apply: "live", fallback: true },
            { key: "blocking.update_interval_hours", label: "Blocklist update interval (hours)",
              type: "number", scope: "global", apply: "live", fallback: 24 },
            { key: "downloads.ask_destination", label: "Ask for download destination", type: "bool",
              scope: "global/profile", apply: "live", fallback: true },
            { key: "downloads.collision", label: "Download collision policy", type: "enum",
              options: ["ask", "rename"], scope: "global/profile", apply: "live", fallback: "ask" },
            { key: "spellcheck.enabled", label: "Spellcheck", type: "bool",
              scope: "profile", apply: "live", fallback: true },
            { key: "spellcheck.languages", label: "Spellcheck languages", type: "languages",
              scope: "profile", apply: "live", fallback: ["system"] },
            { key: "desktop.portals", label: "Desktop portals", type: "enum",
              options: ["auto", "required"], scope: "global", apply: "restart", fallback: "auto" },
            { key: "desktop.notifications", label: "Desktop notifications", type: "bool",
              scope: "global/profile", apply: "live", fallback: true },
            { key: "logging.level", label: "Logging level", type: "enum",
              options: ["error", "warn", "info", "debug"], scope: "global", apply: "live", fallback: "info" }
        ]
        var query = (window.settingsSearch || "").trim().toLowerCase()
        var visibleSpecs = []
        for (var index = 0; index < specs.length; ++index) {
            specs[index].value = settingConfigValue(
                        config, specs[index].key, specs[index].fallback)
            var searchable = [specs[index].key, specs[index].label,
                              specs[index].scope, specs[index].apply].join(" ").toLowerCase()
            if (!query || searchable.indexOf(query) >= 0) {
                visibleSpecs.push(specs[index])
            }
        }
        window.settingsRows = visibleSpecs
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
        var response = browserUi.set_runtime_setting(
                    row.key, settingLiteral(row, value), window.settingsTemporary)
        try {
            var result = JSON.parse(response)
            if (result.error) {
                window.settingsNotice = row.key + ": " + result.error
                return false
            }
            window.settingsNotice = "Applied " + row.key + " ("
                    + (result.temporary ? "temporary" : "persistent or memory-only") + ")"
            window.refreshSettings()
            return true
        } catch (error) {
            window.settingsNotice = "Setting response was invalid"
            return false
        }
    }

    function resetSetting(row) {
        var response = browserUi.unset_runtime_setting(row.key, window.settingsTemporary)
        try {
            var result = JSON.parse(response)
            if (result.error) {
                window.settingsNotice = row.key + ": " + result.error
                return false
            }
            window.settingsNotice = "Reset " + row.key
            window.refreshSettings()
            return true
        } catch (error) {
            window.settingsNotice = "Setting response was invalid"
            return false
        }
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
        userscriptManifestChooser.open()
    }

    function installUserscriptManifest() {
        var selected = userscriptManifestChooser.selectedFile
        if (!selected || selected.scheme !== "file") {
            window.settingsNotice = "Choose a local userscript manifest"
            return
        }
        var response = browserUi.install_userscript_manifest(selected.toLocalFile())
        try {
            var result = JSON.parse(response)
            if (result.error) {
                window.settingsNotice = "Userscript installation failed: " + result.error
                return
            }
            if (result.pending) {
                window.settingsNotice = "Installing userscript…"
                window.refreshUserscriptInventory()
                window.refreshSettings()
                return
            }
            window.settingsNotice = "Installed userscript " + result.name
                    + "; registered actions: " + result.actions
            window.refreshUserscriptInventory()
            window.refreshSettings()
        } catch (error) {
            window.settingsNotice = "Userscript installation response was invalid"
        }
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
        try {
            var rows = JSON.parse(browserUi.userscript_inventory || "[]")
            return Array.isArray(rows) ? rows : []
        } catch (error) {
            return []
        }
    }

    function closeSettings() {
        var wasVisible = window.settingsVisible
        window.settingsVisible = false
        if (wasVisible) {
            window.closeInternalSurface()
        }
    }

    function siteDoctorUserscriptsDisabled(ui) {
        if (!ui || !ui.site_experiment_json || ui.site_experiment_json.length === 0) {
            return false
        }
        try {
            var experiment = JSON.parse(ui.site_experiment_json)
            return experiment.kind === "userscripts-off"
        } catch (error) {
            return false
        }
    }

    function siteDoctorBadge(ui) {
        if (!ui || !ui.site_experiment_json || ui.site_experiment_json.length === 0
                || ui.site_experiment_json === "{}") {
            return ""
        }
        try {
            var experiment = JSON.parse(ui.site_experiment_json)
            return experiment.kind ? " · Site Doctor: " + experiment.kind : ""
        } catch (error) {
            return " · Site Doctor: invalid state"
        }
    }

    function finishSiteDoctorAfterLoad(ui, succeeded) {
        if (!ui || !ui.site_experiment_json || ui.site_experiment_json.length === 0) {
            return
        }
        try {
            var experiment = JSON.parse(ui.site_experiment_json)
            if (experiment.id) {
                var freshView = experiment.kind === "fresh-view"
                ui.finish_site_doctor_experiment(experiment.id, succeeded)
                if (freshView) {
                    Qt.callLater(function() { window.executePendingEngineAction() })
                }
                if (window.siteLedgerVisible) {
                    Qt.callLater(window.showSiteLedger)
                }
            }
        } catch (error) {
            ui.status_text = "Site Doctor state was invalid"
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
        profilePreviewTimer.restart()
    }

    function refreshDownloads() {
        downloads.clear()
        var values = browserUi.list_downloads()
        if (values.length === 0) {
            return
        }
        var lines = values.split("\n")
        for (var i = 0; i < lines.length; ++i) {
            var fields = lines[i].split("\t")
            if (fields.length >= 4) {
                var live = window.liveDownloadForId(fields[0])
                var metrics = window.downloadMetrics(fields[0], live)
                downloads.append({
                    id: fields[0], state: fields[1], bytes: fields[2], destination: fields[3],
                    total: metrics.total, speed: metrics.speed, reason: metrics.reason
                })
            }
        }
    }

    function liveDownloadForId(id) {
        var requested = String(id)
        function find(host) {
            if (!host) {
                return null
            }
            var active = host.activeDownloads || ({})
            if (active[requested]) {
                return active[requested]
            }
            var pending = host.pendingDownloadRequests || ({})
            return pending[requested] || null
        }
        var result = find(window)
        if (result) {
            return result
        }
        var entries = window.browserWindowRegistry || []
        for (var i = 0; i < entries.length; ++i) {
            result = find(entries[i] && entries[i].host)
            if (result) {
                return result
            }
        }
        var popups = window.popupWindowRegistry || []
        for (var j = 0; j < popups.length; ++j) {
            result = find(popups[j] && popups[j].host)
            if (result) {
                return result
            }
        }
        return null
    }

    function downloadMetrics(id, download) {
        var requested = String(id)
        if (!download) {
            delete window.downloadSamples[requested]
            return { total: -1, speed: 0, reason: "" }
        }
        var now = Date.now()
        var bytes = Math.max(0, Number(download.receivedBytes || 0))
        var sample = window.downloadSamples[requested]
        var speed = 0
        if (sample && now > sample.at && bytes >= sample.bytes) {
            var elapsed = now - sample.at
            if (elapsed >= 200) {
                speed = Math.max(0, Math.round((bytes - sample.bytes) * 1000 / elapsed))
            }
        }
        window.downloadSamples[requested] = { at: now, bytes: bytes }
        var total = Number(download.totalBytes)
        return {
            total: isFinite(total) && total >= 0 ? total : -1,
            speed: speed,
            reason: window.boundedPageDialogText(download.interruptReasonString)
        }
    }

    function formatDownloadRate(bytesPerSecond) {
        var value = Number(bytesPerSecond)
        if (!isFinite(value) || value <= 0) {
            return ""
        }
        var units = ["B/s", "KiB/s", "MiB/s", "GiB/s"]
        var index = 0
        while (value >= 1024 && index < units.length - 1) {
            value /= 1024
            index += 1
        }
        return value.toFixed(index === 0 ? 0 : 1) + " " + units[index]
    }

    function openDownload(id, reveal) {
        var payload = browserUi.download_desktop_action(id, reveal)
        if (payload.length === 0) {
            return
        }
        var data = JSON.parse(payload)
        if (data.uri) {
            window.openExternalUri(browserUi, data.uri)
        }
    }

    function externalUriAllowed(uri) {
        var value = String(uri || "")
        if (value.length === 0 || value.length > 8192
                || /[\u0000-\u001f\u007f\n\r]/.test(value)) {
            return false
        }
        var match = /^([A-Za-z][A-Za-z0-9+.-]*):/.exec(value)
        if (!match) {
            return false
        }
        var scheme = match[1].toLowerCase()
        if (scheme === "file") {
            return /^file:\/\/\/[^/]/i.test(value)
        }
        return scheme === "mailto" || scheme === "tel"
                || scheme === "sms" || scheme === "geo"
    }

    function openExternalUri(ui, uri) {
        var targetUi = ui || browserUi
        if (!uri || String(uri).length === 0) {
            return false
        }
        if (!window.externalUriAllowed(uri)) {
            targetUi.status_text = "External URI rejected by scheme policy"
            return false
        }
        if (window.desktopPortalMode(targetUi) === "required") {
            var status = window.desktopPortalCapabilityStatus(targetUi, "open_uri")
            if (status === "not-probed") {
                browserUi.probe_desktop_portals()
                status = window.desktopPortalCapabilityStatus(targetUi, "open_uri")
            }
            if (status === "pending" || status === "not-probed") {
                var pending = window.pendingExternalUris || []
                pending.push({
                    ui: targetUi,
                    uri: String(uri),
                    deadlineMs: Date.now() + 5000
                })
                window.pendingExternalUris = pending.slice(-16)
                targetUi.status_text = "Checking required OpenURI portal…"
                externalOpenPortalTimer.start()
                return true
            }
            if (status !== "available") {
                targetUi.status_text = status === "pending"
                        ? "Checking required OpenURI portal; external action cancelled"
                        : "Required OpenURI portal unavailable; external action cancelled"
                return false
            }
        }
        Qt.openUrlExternally(String(uri))
        return true
    }

    function requestDownloadAction(id, action) {
        var actionIds = {
            pause: "browser.download.pause",
            resume: "browser.download.resume",
            cancel: "browser.download.cancel",
            retry: "browser.download.retry"
        }
        var actionId = actionIds[action]
        if (!actionId) {
            browserUi.status_text = "Unknown download action"
            return
        }
        if (browserUi.execute_ui_action(actionId, id)) {
            window.executePendingEngineAction()
        }
    }

    function hasActiveDownloads() {
        if (window.pendingDownloadId.length > 0) {
            return true
        }
        for (var key in window.pendingDownloadRequests) {
            if (window.pendingDownloadRequests[key]) {
                return true
            }
        }
        for (var activeKey in window.activeDownloads) {
            var download = window.activeDownloads[activeKey]
            if (download && !download.isFinished) {
                return true
            }
        }
        return false
    }

    function hasActiveShutdownRequestsFor(ui, hostWindow) {
        if (!ui) {
            return false
        }
        if (window.pendingFileDialogUi === ui
                || window.pendingPermissionUi === ui
                || window.pendingPageDialogUi === ui
                || window.pendingClientCertificateUi === ui
                || window.pendingCertificateErrorUi === ui
                || window.pendingWebAuthUi === ui
                || window.pendingContextMenuUi === ui) {
            return true
        }
        if (window.pendingDesktopMediaHost === hostWindow) {
            return true
        }
        return (window.captureSessions || []).some(function(session) {
            return session && session.host === hostWindow && session.status === "active"
        })
    }

    function cancelShutdownRequestsFor(ui, hostWindow) {
        if (!ui) {
            return false
        }
        var cancelled = false
        if (window.pendingFileDialogUi === ui) {
            if (hostWindow === window) {
                window.clearEngineFileDialog(true, "File selection cancelled for shutdown")
            } else if (hostWindow && hostWindow.pendingFileDialogView
                       && hostWindow.clearFileDialogForView) {
                hostWindow.clearFileDialogForView(hostWindow.pendingFileDialogView)
            }
            cancelled = true
        } else if (hostWindow && hostWindow !== window
                   && hostWindow.pendingFileDialogRequest
                   && hostWindow.clearFileDialogForView) {
            hostWindow.clearFileDialogForView(hostWindow.pendingFileDialogView)
            cancelled = true
        }
        if (window.pendingPermissionUi === ui) {
            window.cancelPermissionForUi(ui)
            cancelled = true
        }
        if (window.pendingPageDialogUi === ui) {
            window.clearPageDialogForView(window.pendingPageDialogView)
            cancelled = true
        }
        if (window.pendingClientCertificateUi === ui) {
            window.clearClientCertificateRequest(true)
            cancelled = true
        }
        if (window.pendingCertificateErrorUi === ui) {
            window.clearCertificateErrorRequest(true)
            cancelled = true
        }
        if (window.pendingWebAuthUi === ui) {
            window.clearWebAuthRequest(true)
            cancelled = true
        }
        if (window.pendingContextMenuUi === ui) {
            window.clearContextMenuRequest()
            cancelled = true
        }
        if (window.pendingDesktopMediaHost === hostWindow) {
            window.clearDesktopMediaRequest(true)
            cancelled = true
        }
        var sessions = (window.captureSessions || []).slice(0)
        for (var i = 0; i < sessions.length; ++i) {
            var session = sessions[i]
            if (session && session.ui === ui && session.host === hostWindow
                    && session.status === "active") {
                session.status = "stop-requested"
                if (session.view) {
                    session.view.reload()
                }
                cancelled = true
            }
        }
        window.captureSessions = sessions
        if (cancelled) {
            ui.status_text = "Browser requests cancelled for shutdown"
        }
        return cancelled
    }

    function tabSuspensionBlockReason(index, id, view) {
        if (index < 0 || !id || id.length === 0) {
            return "tab target is stale"
        }
        if (index === browserUi.active_tab_index) {
            return "only hidden tabs may be suspended"
        }
        if (!view) {
            return "tab view is not ready"
        }
        if (view.devToolsView) {
            return "DevTools is attached"
        }
        if (view.recentlyAudible) {
            return "audio is active or was recently audible"
        }
        if (view.lifecycleState !== WebEngineView.LifecycleState.Active) {
            return "engine view is not active"
        }
        if (view.recommendedState !== WebEngineView.LifecycleState.Frozen) {
            return "the engine has not approved freezing this tab"
        }
        if (window.hasActiveDownloads() || window.pendingDownloadId.length > 0) {
            return "a download is active"
        }
        if (window.hasActiveShutdownRequestsFor(browserUi, window)) {
            return "a browser request or capture is active"
        }
        if (browserUi.tab_has_active_operations(id)) {
            return "a browser operation is active"
        }
        return ""
    }

    function requestTabSuspension(id) {
        var index = browserUi.tab_index_for_id(id)
        var view = tabViewAt(index)
        var reason = window.tabSuspensionBlockReason(index, id, view)
        if (reason.length > 0) {
            browserUi.status_text = "Tab suspension refused: " + reason
            return
        }
        var probe = "(function() {"
                + "var elements = document.querySelectorAll('input,textarea,select,[contenteditable=\"true\"]');"
                + "for (var i = 0; i < elements.length; ++i) {"
                + "var e = elements[i];"
                + "if (e.isContentEditable) return 'unknown';"
                + "if (e.tagName === 'INPUT' && (e.type === 'checkbox' || e.type === 'radio')"
                + " && e.checked !== e.defaultChecked) return 'dirty';"
                + "if (e.tagName === 'SELECT' && e.selectedIndex !== e.defaultSelectedIndex) return 'dirty';"
                + "if (e.tagName !== 'SELECT' && e.value !== e.defaultValue) return 'dirty';"
                + "} return 'safe'; })()"
        view.runJavaScript(probe, function(result) {
            if (result !== "safe") {
                browserUi.status_text = "Tab suspension refused: editable page state is not safely empty"
                return
            }
            if (!browserUi.commit_tab_suspend(id)) {
                return
            }
            window.executePendingEngineAction()
        })
    }

    function tabDiscardBlockReason(index, id, view) {
        if (index < 0 || !id || id.length === 0) {
            return "tab target is stale"
        }
        if (index === browserUi.active_tab_index) {
            return "only hidden tabs may be discarded"
        }
        if (!view) {
            return "tab view is not ready"
        }
        if (view.devToolsView) {
            return "DevTools is attached"
        }
        if (view.recentlyAudible) {
            return "audio is active or was recently audible"
        }
        if (view.lifecycleState !== WebEngineView.LifecycleState.Active) {
            return "engine view is not active"
        }
        if (view.recommendedState !== WebEngineView.LifecycleState.Discarded) {
            return "the engine has not approved discarding this tab"
        }
        if (window.hasActiveDownloads() || window.pendingDownloadId.length > 0) {
            return "a download is active"
        }
        if (window.hasActiveShutdownRequestsFor(browserUi, window)) {
            return "a browser request or capture is active"
        }
        if (browserUi.tab_has_active_operations(id)) {
            return "a browser operation is active"
        }
        return ""
    }

    function requestTabDiscard(id) {
        var index = browserUi.tab_index_for_id(id)
        var view = tabViewAt(index)
        var reason = window.tabDiscardBlockReason(index, id, view)
        if (reason.length > 0) {
            browserUi.status_text = "Tab discard refused: " + reason
            return
        }
        var probe = "(function() {"
                + "var elements = document.querySelectorAll('input,textarea,select,[contenteditable=\"true\"]');"
                + "for (var i = 0; i < elements.length; ++i) {"
                + "var e = elements[i];"
                + "if (e.isContentEditable) return 'unknown';"
                + "if (e.tagName === 'INPUT' && (e.type === 'checkbox' || e.type === 'radio')"
                + " && e.checked !== e.defaultChecked) return 'dirty';"
                + "if (e.tagName === 'SELECT' && e.selectedIndex !== e.defaultSelectedIndex) return 'dirty';"
                + "if (e.tagName !== 'SELECT' && e.value !== e.defaultValue) return 'dirty';"
                + "} return 'safe'; })()"
        view.runJavaScript(probe, function(result) {
            if (result !== "safe") {
                browserUi.status_text = "Tab discard refused: editable page state is not safely empty"
                return
            }
            if (!browserUi.commit_tab_discard(id)) {
                return
            }
            window.executePendingEngineAction()
        })
    }

    function shutdownPageProbeScript() {
        return "(function() {"
                + "if (typeof window.onbeforeunload === 'function') return 'unknown';"
                + "var elements = document.querySelectorAll('input,textarea,select,[contenteditable=\"true\"]');"
                + "if (elements.length > 128) return 'unknown';"
                + "for (var i = 0; i < elements.length; ++i) {"
                + "var e = elements[i];"
                + "if (e.isContentEditable) return 'unknown';"
                + "if (e.tagName === 'INPUT' && (e.type === 'checkbox' || e.type === 'radio')"
                + " && e.checked !== e.defaultChecked) return 'dirty';"
                + "if (e.tagName === 'SELECT') {"
                + "if (e.options.length > 512) return 'unknown';"
                + "for (var j = 0; j < e.options.length; ++j)"
                + " if (e.options[j].selected !== e.options[j].defaultSelected) return 'dirty';"
                + "} else if (e.value !== e.defaultValue) return 'dirty';"
                + "} return 'clean'; })()"
    }

    function checkPageStateBeforeQuit() {
        var generation = ++window.shutdownPageProbeGeneration
        shutdownPageProbeTimer.restart()
        var candidates = []
        for (var i = 0; i < tabViews.length; ++i) {
            var view = tabViewAt(i)
            if (view && view.lifecycleState === WebEngineView.LifecycleState.Active) {
                candidates.push(view)
            }
        }
        var probe = function(position) {
            if (generation !== window.shutdownPageProbeGeneration) {
                return
            }
            if (position >= candidates.length) {
                shutdownPageProbeTimer.stop()
                window.finalizeQuit()
                return
            }
            candidates[position].runJavaScript(window.shutdownPageProbeScript(), function(result) {
                if (generation !== window.shutdownPageProbeGeneration) {
                    return
                }
                if (result === "dirty") {
                    shutdownPageProbeTimer.stop()
                    window.shutdownPagePromptReason = "A page has unsaved form or editor state."
                    window.shutdownPagePromptVisible = true
                    return
                }
                if (result !== "clean") {
                    shutdownPageProbeTimer.stop()
                    window.shutdownPagePromptReason = "Ferric Browser could not verify every page before closing."
                    window.shutdownPagePromptVisible = true
                    return
                }
                probe(position + 1)
            })
        }
        probe(0)
    }

    function beginQuitRequest() {
        if (window.hasActiveDownloads()
                || window.hasActiveShutdownRequestsFor(browserUi, window)) {
            window.shutdownPromptVisible = true
            return
        }
        window.checkPageStateBeforeQuit()
    }

    function finalizeQuit() {
        shutdownPageProbeTimer.stop()
        window.shutdownPromptVisible = false
        window.shutdownPagePromptVisible = false
        window.beginApplicationShutdown()
    }

    function cancelDownloadsAndQuit() {
        if (window.pendingDownloadId.length > 0) {
            window.cancelPendingDownload()
        }
        for (var pendingKey in window.pendingDownloadRequests) {
            var pending = window.pendingDownloadRequests[pendingKey]
            if (pending) {
                window.resolveQtRequest(browserUi, pending, "download", "cancel", [])
                browserUi.update_download(pendingKey, "cancelled", pending.receivedBytes, true)
            }
        }
        window.pendingDownloadRequests = ({})
        for (var activeKey in window.activeDownloads) {
            var download = window.activeDownloads[activeKey]
            if (download && !download.isFinished) {
                window.resolveQtRequest(browserUi, download, "download", "cancel", [])
                browserUi.update_download(activeKey, "cancelled", download.receivedBytes, true)
            }
        }
        window.activeDownloads = ({})
        window.cancelShutdownRequestsFor(browserUi, window)
        window.beginQuitRequest()
    }

    function refreshLibraryManager() {
        libraryEntries.clear()
        libraryGraphEntries.clear()
        libraryGraphNodes.clear()
        window.libraryGraphLineData = []
        var kind = browserUi.library_kind
        var values = browserUi.library_values
        if (values.length === 0) {
            window.libraryTotalEntries = 0
            window.libraryPage = 0
            return
        }
        var lines = values.split("\n")
        window.libraryTotalEntries = lines.length
        var visibleLines = lines
        if (kind !== "journey") {
            var pageCount = Math.max(1, Math.ceil(lines.length / window.libraryPageSize))
            window.libraryPage = Math.max(0, Math.min(window.libraryPage, pageCount - 1))
            var pageStart = window.libraryPage * window.libraryPageSize
            visibleLines = lines.slice(pageStart, pageStart + window.libraryPageSize)
        }
        for (var i = 0; i < visibleLines.length; ++i) {
            var fields = visibleLines[i].split("\t")
            if ((kind === "history" || kind === "bookmarks") && fields.length >= 3) {
                libraryEntries.append({
                    label: fields[1],
                    secondary: fields[2],
                    entryId: fields[0],
                    entryKind: kind === "history" ? "history" : "bookmark"
                })
            } else if (kind === "journey" && fields.length >= 4) {
                var journeyLabel = fields[1].length > 0 ? fields[1] : fields[2]
                libraryEntries.append({
                    label: journeyLabel + " · " + fields[3],
                    secondary: fields[2] + (fields.length >= 5 && fields[4].length > 0
                                           ? " · " + fields[4] : ""),
                    nodeId: fields[0]
                })
            } else if (kind === "quickmarks" && fields.length >= 2) {
                libraryEntries.append({
                    label: fields[0],
                    secondary: fields[1],
                    entryId: fields[0],
                    entryKind: "quickmark"
                })
            } else if (kind === "actions" && fields.length >= 3) {
                libraryEntries.append({
                    label: fields[0] + " · " + fields[1],
                    secondary: fields[2] + (fields.length >= 4 && fields[3].length > 0
                                           ? " · Example: " + fields[3] : "")
                })
            }
        }
        if (kind === "journey" && browserUi.library_graph_values.length > 0) {
            try {
                var graph = JSON.parse(browserUi.library_graph_values)
                var nodeLabels = ({})
                for (var n = 0; n < lines.length; ++n) {
                    var nodeFields = lines[n].split("\t")
                    if (nodeFields.length >= 4) {
                        nodeLabels[nodeFields[0]] = nodeFields[1].length > 0
                                ? nodeFields[1] : nodeFields[2]
                    }
                }
                var edges = graph.edges || []
                var nodeIndex = ({})
                var graphNodeLimit = Math.min(lines.length, 120)
                var graphNodeWidth = 170
                var graphNodeHeight = 48
                var graphNodeGapX = 24
                var graphNodeGapY = 18
                var graphColumns = 4
                for (var g = 0; g < graphNodeLimit; ++g) {
                    var graphFields = lines[g].split("\t")
                    if (graphFields.length < 4 || nodeIndex[graphFields[0]] !== undefined) {
                        continue
                    }
                    var graphIndex = libraryGraphNodes.count
                    var graphX = (graphIndex % graphColumns)
                            * (graphNodeWidth + graphNodeGapX)
                    var graphY = Math.floor(graphIndex / graphColumns)
                            * (graphNodeHeight + graphNodeGapY)
                    nodeIndex[graphFields[0]] = graphIndex
                    libraryGraphNodes.append({
                        nodeId: graphFields[0],
                        label: graphFields[1].length > 0 ? graphFields[1] : graphFields[2],
                        transition: graphFields[3],
                        source: graphFields.length >= 5 ? graphFields[4] : "",
                        x: graphX,
                        y: graphY
                    })
                }
                for (var e = 0; e < edges.length; ++e) {
                    var edge = edges[e]
                    var source = nodeLabels[String(edge.source)] || String(edge.source)
                    var target = nodeLabels[String(edge.target)] || String(edge.target)
                    libraryGraphEntries.append({
                        label: source + " → " + target,
                        secondary: String(edge.transition || "navigate")
                    })
                    if (nodeIndex[String(edge.source)] !== undefined
                            && nodeIndex[String(edge.target)] !== undefined) {
                        var sourceNode = libraryGraphNodes.get(nodeIndex[String(edge.source)])
                        var targetNode = libraryGraphNodes.get(nodeIndex[String(edge.target)])
                        window.libraryGraphLineData.push({
                            x1: sourceNode.x + graphNodeWidth / 2,
                            y1: sourceNode.y + graphNodeHeight / 2,
                            x2: targetNode.x + graphNodeWidth / 2,
                            y2: targetNode.y + graphNodeHeight / 2,
                            transition: String(edge.transition || "navigate")
                        })
                    }
                }
                if (lines.length > graphNodeLimit) {
                    libraryGraphEntries.insert(0, {
                        label: "Graph layout capped at " + graphNodeLimit + " nodes",
                        secondary: "Use Search or Expand for a smaller neighborhood"
                    })
                }
                if (libraryGraphEntries.count === 0) {
                    libraryGraphEntries.append({
                        label: "No retained relationships",
                        secondary: "The current node has no visible edge in this bounded view"
                    })
                }
            } catch (error) {
                libraryGraphEntries.append({
                        label: "Relationship graph unavailable",
                        secondary: "The graph data was malformed"
                    })
            }
        }
        graphCanvas.requestPaint()
    }

    function libraryPageCount() {
        return Math.max(1, Math.ceil(window.libraryTotalEntries / window.libraryPageSize))
    }

    function changeLibraryPage(delta) {
        var next = window.libraryPage + delta
        if (next < 0 || next >= window.libraryPageCount()) {
            return
        }
        window.libraryPage = next
        window.refreshLibraryManager()
    }

    function runJourneyQuery(argument) {
        var command = ":journey"
        if (argument.length > 0) {
            command += " " + argument
        }
        if (browserUi.execute_command(command)) {
            window.syncTabModel()
            window.refreshLibraryManager()
        }
    }

    function runJourneyCurrentQuery() {
        window.libraryJourneyCurrentOnly = true
        window.libraryJourneySearchText = ""
        window.libraryJourneyExpandedNode = ""
        journeySearchField.text = ""
        window.runJourneyQuery("--current")
    }

    function runJourneyAllQuery() {
        window.libraryJourneyCurrentOnly = false
        window.libraryJourneySearchText = ""
        window.libraryJourneyExpandedNode = ""
        journeySearchField.text = ""
        window.runJourneyQuery("")
    }

    function libraryCommandArgument(value) {
        return '"' + String(value).replace(/\\/g, "\\\\").replace(/"/g, '\\"') + '"'
    }

    function openLibraryEntry(entryKind, entryId) {
        var command = entryKind === "history" ? "history-open"
                : entryKind === "bookmark" ? "bookmark-open" : "quickmark-open"
        if (browserUi.execute_command(":" + command + " " + libraryCommandArgument(entryId))) {
            window.syncTabModel()
            window.executePendingEngineAction()
            window.libraryManagerVisible = false
            window.closeInternalSurface()
        }
    }

    function showPrivateHistoryTransfer(entryId, title, url) {
        if (!window.temporaryProfile || String(url).length === 0) {
            browserUi.status_text = "Private history transfer is unavailable"
            return
        }
        window.privateHistoryTransferId = String(entryId)
        window.privateHistoryTransferTitle = String(title || "")
        window.privateHistoryTransferUrl = String(url)
        window.privateHistoryTransferVisible = true
        window.refreshProfiles()
        window.openInternalSurface()
    }

    function cancelPrivateHistoryTransfer() {
        window.privateHistoryTransferVisible = false
        window.privateHistoryTransferId = ""
        window.privateHistoryTransferTitle = ""
        window.privateHistoryTransferUrl = ""
        window.closeInternalSurface()
    }

    function confirmPrivateHistoryTransfer(profileName) {
        var command = ":profile-open " + window.libraryCommandArgument(profileName)
                + " " + window.libraryCommandArgument(window.privateHistoryTransferUrl)
        if (!browserUi.execute_command(command)) {
            return
        }
        window.privateHistoryTransferVisible = false
        window.privateHistoryTransferId = ""
        window.privateHistoryTransferTitle = ""
        window.privateHistoryTransferUrl = ""
        window.libraryManagerVisible = false
        window.closeInternalSurface()
        Qt.callLater(function() { window.executePendingEngineAction() })
    }

    function confirmPrivateHistoryBookmark(profileName, profileLabel) {
        var entries = window.browserWindowRegistry || []
        for (var index = 0; index < entries.length; ++index) {
            var entry = entries[index]
            if (entry && entry.ui && !entry.ephemeralProfile
                    && String(entry.profileName || "") === String(profileName)) {
                if (!entry.ui.queue_bookmark_transfer(
                            window.privateHistoryTransferUrl,
                            window.privateHistoryTransferTitle)) {
                    browserUi.status_text = "Destination profile is not ready for bookmark transfer"
                    return
                }
                window.activateBrowserWindow(entry.host, entry.ui)
                window.cancelPrivateHistoryTransfer()
                return
            }
        }
        browserWindowComponent.createObject(null, {
            windowStartupUrl: window.privateHistoryTransferUrl,
            windowProfileName: profileName,
            windowProfileLabel: profileLabel,
            windowPrivateProfile: false,
            windowBookmarkTransferUrl: window.privateHistoryTransferUrl,
            windowBookmarkTransferTitle: window.privateHistoryTransferTitle
        })
        window.privateHistoryTransferVisible = false
        window.privateHistoryTransferId = ""
        window.privateHistoryTransferTitle = ""
        window.privateHistoryTransferUrl = ""
        window.libraryManagerVisible = false
        window.closeInternalSurface()
    }

    function deleteLibraryEntry(entryKind, entryId) {
        var command = entryKind === "bookmark" ? "bookmark-delete" : "quickmark-delete"
        var pendingKey = entryKind + "\t" + entryId
        if (window.pendingLibraryDelete !== pendingKey) {
            window.pendingLibraryDelete = pendingKey
            return
        }
        if (browserUi.execute_command(":" + command + " " + libraryCommandArgument(entryId))) {
            window.pendingLibraryDelete = ""
            window.refreshLibraryManager()
        }
    }

    function editLibraryEntry(entryKind, entryId, value) {
        var trimmed = String(value).trim()
        if (trimmed.length === 0) {
            browserUi.status_text = "Library edit requires a non-empty value"
            return false
        }
        var command = entryKind === "bookmark" ? "bookmark-edit" : "quickmark-edit"
        var argumentsText = entryKind === "bookmark"
                ? libraryCommandArgument(entryId) + " --title " + libraryCommandArgument(trimmed)
                : libraryCommandArgument(entryId) + " " + libraryCommandArgument(trimmed)
        if (browserUi.execute_command(":" + command + " " + argumentsText)) {
            window.pendingLibraryDelete = ""
            window.refreshLibraryManager()
            return true
        }
        return false
    }

    function beginJourneyExport() {
        var preview = browserUi.journey_export_preview()
        if (preview.length === 0) {
            window.journeyExportAwaiting = true
            return
        }
        window.journeyExportAwaiting = false
        window.journeyExportPreviewText = preview
        window.journeyExportPreviewVisible = true
    }

    function finishJourneyExport() {
        var selected = journeyExportChooser.selectedFile
        var path = selected && selected.scheme === "file" ? selected.toLocalFile() : ""
        if (path.length === 0 || !browserUi.export_journey(path)) {
            window.journeyExportPreviewVisible = true
            return
        }
        window.journeyExportPreviewVisible = false
    }

    function finishDiagnosticsExport() {
        var selected = diagnosticsExportChooser.selectedFile
        var path = selected && selected.scheme === "file" ? selected.toLocalFile() : ""
        if (!path || !browserUi.export_diagnostics(path)) {
            window.diagnosticsVisible = true
        }
    }

    function journeySearchArgument(value) {
        return value.length > 0 ? "--search " + JSON.stringify(value) : ""
    }

    function journeyExpandArgument(nodeId) {
        return "--expand " + JSON.stringify(nodeId)
    }

    function cancelSwitcherBatch() {
        switcherBatchGeneration += 1
        switcherBatchTimer.stop()
        switcherBatchEntries = []
        switcherBatchMerged = []
        switcherBatchIndex = 0
    }

    function switcherMaxResults() {
        var fallback = 100
        try {
            var config = JSON.parse(browserUi.config_json)
            var configured = Number(config.switcher && config.switcher.max_results)
            if (isFinite(configured)) {
                return Math.max(10, Math.min(1000, Math.floor(configured)))
            }
        } catch (error) {
            // The validated Rust configuration remains authoritative. If its
            // presentation snapshot is temporarily unavailable, retain the
            // bounded UI default rather than exposing an unbounded query.
        }
        return fallback
    }

    function processSwitcherBatch() {
        if (switcherBatchScheduledGeneration !== switcherBatchGeneration
                || !window.switcherVisible
                || switcherBatchQuery !== switcherInput.text
                || switcherBatchScope !== window.switcherScope) {
            window.cancelSwitcherBatch()
            return
        }
        if (switcherBatchIndex >= switcherBatchEntries.length) {
            switcherResults = switcherBatchMerged
            return
        }
        var entry = switcherBatchEntries[switcherBatchIndex]
        switcherBatchIndex += 1
        if (entry && entry.ui) {
        var raw = entry.ui.switcher_query(switcherBatchQuery, switcherBatchScope)
            if (raw && raw.length > 0) {
                try {
                    var rows = JSON.parse(raw).results || []
                    for (var j = 0; j < rows.length; ++j) {
                        rows[j].owner_token = String(entry.ui.window_token)
                        switcherBatchMerged.push(rows[j])
                    }
                } catch (error) {
                    // A destroyed or unavailable source contributes no rows.
                }
            }
        }
        // Publish each source as it arrives. The next source is scheduled on
        // the GUI event loop, so a large multi-window search cannot monopolize
        // keyboard, focus, or portal callbacks.
        switcherBatchMerged.sort(function(left, right) {
            return Number(right.rank || 0) - Number(left.rank || 0)
                    || Number(right.recency || 0) - Number(left.recency || 0)
                    || String(left.kind || "").localeCompare(String(right.kind || ""))
                    || String(left.id || "").localeCompare(String(right.id || ""))
        })
        switcherResults = switcherBatchMerged.slice(0)
        switcherBatchTimer.start()
    }

    function refreshSwitcher() {
        window.cancelSwitcherBatch()
        var entries = window.browserWindowRegistry || []
        if (entries.length === 0) {
            entries = [{ host: window, ui: browserUi }]
        }
        switcherBatchEntries = entries.slice(0)
        switcherBatchMerged = []
        switcherBatchIndex = 0
        switcherBatchQuery = switcherInput.text
        switcherBatchScope = window.switcherScope
        switcherBatchScheduledGeneration = switcherBatchGeneration
        switcherResults = []
        switcherBatchTimer.start()
    }

    // Querying every key event synchronously across all browser windows can
    // build a backlog while the user is typing. Coalesce edits into one
    // latest-input refresh; activation and scope changes still refresh
    // immediately at their explicit boundaries.
    Timer {
        id: switcherRefreshTimer
        interval: 35
        repeat: false
        onTriggered: {
            if (window.switcherVisible) {
                window.refreshSwitcher()
            }
        }
    }

    Timer {
        id: switcherBatchTimer
        interval: 0
        repeat: false
        onTriggered: window.processSwitcherBatch()
    }

    function showSwitcherWith(scope, query) {
        window.captureOverlayFocus(window, window.activeWebView())
        switcherVisible = true
        switcherScope = scope && scope.length > 0 ? scope : "all"
        switcherRefreshTimer.stop()
        switcherInput.text = query || ""
        refreshSwitcher()
        switcherInput.forceActiveFocus()
    }

    function showSwitcher() {
        window.showSwitcherWith("all", "")
    }

    function closeSwitcher() {
        var wasVisible = switcherVisible
        switcherRefreshTimer.stop()
        window.cancelSwitcherBatch()
        switcherVisible = false
        switcherResults = []
        if (wasVisible) {
            window.restoreOverlayFocus()
        }
    }

    function setSwitcherScope(scope) {
        if (!scope || scope.length === 0) {
            return
        }
        switcherScope = scope
        switcherRefreshTimer.stop()
        window.cancelSwitcherBatch()
        refreshSwitcher()
        switcherInput.forceActiveFocus()
    }

    function showLinkPreview() {
        try {
            linkPreviewData = JSON.parse(browserUi.link_preview)
        } catch (error) {
            linkPreviewData = {}
        }
        if (!linkPreviewVisible && browserUi.link_preview_visible) {
            window.openInternalSurface()
        }
        linkPreviewVisible = browserUi.link_preview_visible
    }

    function closeLinkPreview() {
        var wasVisible = linkPreviewVisible
        linkPreviewVisible = false
        browserUi.clear_link_preview()
        if (wasVisible) {
            window.closeInternalSurface()
        }
    }

    function hintSelector(linksOnly) {
        if (linksOnly) {
            return "a[href],area[href],link[href],[role='link'][href]"
        }
        return "a,area,textarea,select,input:not([type='hidden']),button,frame,iframe,img,link,summary,"
            + "[contenteditable]:not([contenteditable='false']),[onclick],[onmousedown],"
            + "[role='link'],[role='option'],[role='button'],[role='tab'],[role='checkbox'],"
            + "[role='switch'],[role='menuitem'],[role='menuitemcheckbox'],"
            + "[role='menuitemradio'],[role='treeitem'],[aria-haspopup],[ng-click],[ngClick],"
            + "[data-ng-click],[x-ng-click],[tabindex]:not([tabindex='-1'])"
    }

    function hintCollectorScript(linksOnly) {
        var selector = window.hintSelector(linksOnly)
        var linkSelector = window.hintSelector(true)
        return "(function(){"
            + "const out=[];const seen=new Set();const selector=" + JSON.stringify(selector) + ";"
            + "const linkSelector=" + JSON.stringify(linkSelector) + ";"
            + "const elements=new Map();window.__ferric_browserHintElements=elements;let nextElementId=1;"
            + "function add(el,ox,oy,framePath){if(out.length>=5000||seen.has(el))return;seen.add(el);"
            + "const s=getComputedStyle(el),r=el.getBoundingClientRect();"
            + "if(s.display==='none'||s.visibility==='hidden'||s.pointerEvents==='none'||Number(s.opacity)===0||r.width<=0||r.height<=0)return;"
            + "let kind='aria';if(el.matches(linkSelector))kind='link';else if(el.matches('button,[role=button]'))kind='button';"
            + "else if(el.matches('input'))kind='input';else if(el.matches('select'))kind='select';"
            + "else if(el.matches('textarea'))kind='textarea';else if(el.isContentEditable)kind='contenteditable';"
            + "const text=String(el.getAttribute('aria-label')||el.getAttribute('title')||el.innerText||el.value||'').replace(/[\\n\\r]+/g,' ').trim().slice(0,512);"
            + "const href=kind==='link'?String(el.href||el.getAttribute('href')||''):null;"
            + "const elementId=nextElementId++;elements.set(elementId,{element:el,framePath:framePath});"
            + "out.push({element_id:elementId,kind:kind,frame_path:framePath,text:text,href:href,geometry:{x:r.x+ox,y:r.y+oy,width:r.width,height:r.height}});}"
            + "function visit(root,framePath,ox,oy,depth){if(out.length>=5000||depth>8)return;"
            + "root.querySelectorAll(selector).forEach(function(el){add(el,ox,oy,framePath);});if(out.length>=5000)return;"
            + "root.querySelectorAll('*').forEach(function(el){if(el.shadowRoot)visit(el.shadowRoot,framePath,ox,oy,depth);});"
            + "if(root.nodeType!==9)return;root.querySelectorAll('iframe,frame').forEach(function(frame,index){if(out.length>=5000||depth>=8)return;"
            + "const fs=getComputedStyle(frame),fr=frame.getBoundingClientRect();if(fs.display==='none'||fs.visibility==='hidden'||fr.width<=0||fr.height<=0)return;"
            + "try{if(frame.contentDocument)visit(frame.contentDocument,framePath+'.'+index,ox+fr.x,oy+fr.y,depth+1);}catch(error){}});}"
            + "visit(document,'0',0,0,0);return {candidates:out};})()"
    }

    function selectionScript() {
        return "(function(){var e=document.activeElement;"
            + "if(e&&e.matches('input[type=password],textarea[data-password],*[aria-multiline=true][data-password]'))"
            + "return {error:'password fields are not copied'};"
            + "var s=window.getSelection(),t=s?String(s.toString()):'';"
            + "if(t.length>1048576)return {error:'selection is too large'};"
            + "return {text:t};})()"
    }

    function downloadLinkScript(url) {
        return "(function(){var a=document.createElement('a');a.href="
            + JSON.stringify(url)
            + ";a.download='';a.rel='noreferrer';document.body.appendChild(a);a.click();a.remove();return true;})()"
    }

    function editorScript() {
        return "(function(){var e=document.activeElement;if(!e)return {error:'no focused control'};"
            + "if(e.matches('input[type=password],textarea[data-password],*[aria-multiline=true][data-password]'))"
            + "return {error:'password fields are not editable externally'};"
            + "if(e.matches('textarea,input[type=text],input[type=search],input[type=email],input[type=url]'))"
            + "return {ok:true,text:String(e.value||'')};"
            + "if(e.isContentEditable&&e.contentEditable==='plaintext-only')"
            + "return {ok:true,text:String(e.innerText||e.textContent||'')};"
            + "return {error:'focused control is not a supported plain-text editor'};})()"
    }

    function editorApplyScript(original, updated) {
        return "(function(){var e=document.activeElement;if(!e)return {error:'no focused control'};"
            + "if(e.matches('input[type=password],textarea[data-password],*[aria-multiline=true][data-password]'))"
            + "return {error:'password fields are not editable externally'};"
            + "var current=e.matches('textarea,input[type=text],input[type=search],input[type=email],input[type=url]')?String(e.value||''):e.isContentEditable&&e.contentEditable==='plaintext-only'?String(e.innerText||e.textContent||''):null;"
            + "if(current===null)return {error:'focused control is not a supported plain-text editor'};"
            + "if(current!==" + JSON.stringify(original) + ")return {error:'field changed while editor was open'};"
            + "var next=" + JSON.stringify(updated) + ";if(e.matches('textarea,input[type=text],input[type=search],input[type=email],input[type=url]'))e.value=next;else e.textContent=next;"
            + "e.dispatchEvent(new Event('input',{bubbles:true}));e.dispatchEvent(new Event('change',{bubbles:true}));return {ok:true};})()"
    }

    function caretScript(operation, selecting) {
        var fields = operation.split("\t")
        if (fields[0] === "select") {
            var next = fields[1] === "toggle" ? !selecting : fields[1] === "on"
            return "(function(){return {ok:true,selecting:" + (next ? "true" : "false") + "};})()"
        }
        if (fields.length !== 3 || fields[0] !== "move") {
            return "(function(){return {error:'invalid caret operation'};})()"
        }
        var direction = fields[1]
        var count = Number(fields[2])
        var directionMap = {
            left: { direction: "backward", granularity: "character" },
            right: { direction: "forward", granularity: "character" },
            up: { direction: "backward", granularity: "line" },
            down: { direction: "forward", granularity: "line" },
            "word-prev": { direction: "backward", granularity: "word" },
            "word-next": { direction: "forward", granularity: "word" },
            "line-start": { direction: "backward", granularity: "lineboundary" },
            "line-end": { direction: "forward", granularity: "lineboundary" }
        }
        var movement = directionMap[direction]
        if (!movement || !Number.isInteger(count) || count < 1 || count > 9999) {
            return "(function(){return {error:'invalid caret movement'};})()"
        }
        return "(function(){var s=window.getSelection();if(!s)return {error:'selection API unavailable'};"
            + "if(typeof s.modify!=='function')return {error:'caret movement API unavailable'};"
            + "if(s.rangeCount===0){var w=document.createTreeWalker(document.body,NodeFilter.SHOW_TEXT),n=w.nextNode();"
            + "if(!n)return {error:'document has no text'};var r=document.createRange();r.setStart(n,0);r.collapse(true);s.removeAllRanges();s.addRange(r);}"
            + "for(var i=0;i<" + count + ";i++)s.modify(" + (selecting ? "'extend'" : "'move'") + ","
            + JSON.stringify(movement.direction) + "," + JSON.stringify(movement.granularity) + ");"
            + "return {ok:true};})()"
    }

    function hintFreshScript(candidate) {
        var selector = window.hintSelector(false)
        var linkSelector = window.hintSelector(true)
        return "(function(){const elementId=" + Number(candidate.element_id) + ",path=" + JSON.stringify(candidate.frame_path || "0") + ";"
            + "const selector=" + JSON.stringify(selector) + ",linkSelector=" + JSON.stringify(linkSelector) + ";"
            + "const elements=window.__ferric_browserHintElements,record=elements&&elements.get(elementId);"
            + "if(!record||record.framePath!==path||!record.element||!record.element.isConnected)return {visible:false,element_id:elementId};"
            + "const el=record.element;if(!el.matches(selector))return {visible:false,element_id:elementId};"
            + "const ownerView=el.ownerDocument&&el.ownerDocument.defaultView;if(!ownerView)return {visible:false,element_id:elementId};"
            + "const s=ownerView.getComputedStyle(el),r=el.getBoundingClientRect();let bx=0,by=0,view=ownerView;"
            + "try{while(view&&view!==window){const frame=view.frameElement;if(!frame)return {visible:false,element_id:elementId};const fr=frame.getBoundingClientRect();bx+=fr.x;by+=fr.y;view=frame.ownerDocument.defaultView;}}catch(error){return {visible:false,element_id:elementId};}"
            + "if(view!==window)return {visible:false,element_id:elementId};"
            + "let kind='aria';if(el.matches(linkSelector))kind='link';else if(el.matches('button,[role=button]'))kind='button';"
            + "else if(el.matches('input'))kind='input';else if(el.matches('select'))kind='select';else if(el.matches('textarea'))kind='textarea';else if(el.isContentEditable)kind='contenteditable';"
            + "return {visible:s.display!=='none'&&s.visibility!=='hidden'&&s.pointerEvents!=='none'&&Number(s.opacity)!==0&&r.width>0&&r.height>0,element_id:elementId,kind:kind,frame_path:path,"
            + "text:String(el.getAttribute('aria-label')||el.getAttribute('title')||el.innerText||el.value||'').replace(/[\\n\\r]+/g,' ').trim().slice(0,512),"
            + "href:kind==='link'?String(el.href||el.getAttribute('href')||''):null,geometry:{x:r.x+bx,y:r.y+by,width:r.width,height:r.height}};})()"
    }

    function hintFocusScript(elementId) {
        return "(function(){const elements=window.__ferric_browserHintElements,record=elements&&elements.get("
            + Number(elementId) + ");const el=record&&record.element;"
            + "if(!el||!el.isConnected||!el.matches('input,select,textarea,[contenteditable]:not([contenteditable=false])'))return false;"
            + "const ownerView=el.ownerDocument&&el.ownerDocument.defaultView;if(ownerView&&typeof ownerView.__ferric_browserAuthorizeExplicitFocus==='function')ownerView.__ferric_browserAuthorizeExplicitFocus();"
            + "el.focus();return true;})()"
    }

    function hintClickScript(elementId) {
        var selector = window.hintSelector(false)
        return "(function(){const selector=" + JSON.stringify(selector) + ",elements=window.__ferric_browserHintElements,record=elements&&elements.get("
            + Number(elementId) + ");const el=record&&record.element;"
            + "if(!el||!el.isConnected||!el.matches(selector)||typeof el.click!=='function')return false;el.click();return true;})()"
    }

    function startHintCollection() {
        var view = window.activeWebView()
        if (!view) {
            browserUi.cancel_hints()
            return
        }
        var startedAt = Date.now()
        window.runBrowserScript(view, window.hintCollectorScript(browserUi.hint_links_only), function(value) {
            if (Date.now() - startedAt > 200) {
                window.closeHints()
                browserUi.status_text = "Hint collection timed out"
                return
            }
            var payload = value && value.candidates !== undefined
                    ? {candidates: value.candidates}
                    : {candidates: value || []}
            var response = {}
            try {
                response = JSON.parse(browserUi.begin_hint_session(JSON.stringify(payload)))
            } catch (error) {
                response = {}
            }
            if (!response.hints || response.hints.length === 0) {
                window.hintResults = []
                browserUi.cancel_hints()
                return
            }
            window.hintInput = ""
            window.hintResults = response.hints
        })
    }

    function closeHints() {
        window.hintInput = ""
        window.hintResults = []
        window.rapidHintConfirmationVisible = false
        browserUi.cancel_hints()
    }

    function openEphemeralWindow(url, requestedToken) {
        var marker = Date.now().toString(36) + "-" + window.browserWindowRegistry.length
        var token = String(requestedToken || "")
        var sharedProfile = window.ephemeralProfileForToken(token)
        if (token.length === 0) {
            token = "ephemeral-" + marker
        }
        var child = browserWindowComponent.createObject(null, {
            windowStartupUrl: url,
            windowProfileName: sharedProfile ? "ephemeral-shared" : "ephemeral-" + marker,
            windowProfileLabel: "Ephemeral · " + marker,
            windowPrivateProfile: false,
            windowEphemeralProfile: true,
            windowEphemeralInvocationToken: token,
            windowSharedProfile: sharedProfile,
            windowStartupRoutePreflighted: true
        })
        if (!child) {
            browserUi.status_text = "Could not create ephemeral window"
            return false
        }
        browserUi.status_text = "Opened link in a fresh ephemeral window"
        return true
    }

    function openReopenedWindow(ui, hostWindow, action) {
        var fields = String(action || "").split("\t")
        if (fields.length < 2 || fields.slice(1).join("\t").length === 0) {
            if (ui) {
                ui.status_text = "Reopen-in-window payload was invalid"
            }
            return false
        }
        var url = fields.slice(1).join("\t")
        var secondary = hostWindow !== window
        var profileName = secondary ? hostWindow.windowProfileName : window.profileName
        var profileLabel = secondary ? hostWindow.windowProfileLabel : window.profileLabel
        var privateProfile = secondary
                ? hostWindow.windowPrivateProfile
                : (window.temporaryProfile && !window.ephemeralProfile)
        var ephemeralProfile = secondary
                ? hostWindow.windowEphemeralProfile : window.ephemeralProfile
        var sharedProfile = secondary
                ? hostWindow.windowWebEngineProfile : browserProfile
        var invocationToken = secondary
                ? hostWindow.windowEphemeralInvocationToken
                : window.ephemeralInvocationToken
        var child = browserWindowComponent.createObject(null, {
            windowStartupUrl: url,
            windowProfileName: profileName,
            windowProfileLabel: profileLabel,
            windowPrivateProfile: privateProfile,
            windowEphemeralProfile: ephemeralProfile,
            windowEphemeralInvocationToken: invocationToken,
            windowSharedProfile: (privateProfile || ephemeralProfile) ? sharedProfile : null,
            windowStartupRoutePreflighted: true
        })
        if (!child) {
            if (ui) {
                ui.status_text = "Could not create reopen-in-window target"
            }
            return false
        }
        if (ui) {
            ui.status_text = "Opened safe URL in a new same-profile window; live state was lost"
        }
        return true
    }

    function transferPayloadForUi(sourceUi) {
        if (!sourceUi) {
            return ""
        }
        var index = Number(sourceUi.active_tab_index)
        var id = index >= 0 ? sourceUi.tab_id_for_index(index) : ""
        return id.length > 0 ? sourceUi.tab_transfer_payload(id) : ""
    }

    function detachSourceView(sourceUi, sourceHost) {
        if (sourceHost === window) {
            var index = sourceUi ? sourceUi.active_tab_index : -1
            var view = tabViewAt(index)
            return view ? window.detachTabViewAt(index) : null
        }
        return sourceHost && sourceHost.detachActiveViewForTransfer
                ? sourceHost.detachActiveViewForTransfer() : null
    }

    function restoreSourceView(sourceUi, sourceHost, view) {
        if (!view) {
            return false
        }
        if (sourceHost === window) {
            var index = sourceUi ? sourceUi.active_tab_index : -1
            if (index < 0 || index > tabViews.length) {
                index = tabViews.length
            }
            tabViews.splice(index, 0, view)
            view.parent = webViews
            view.visible = true
            for (var restoredIndex = index; restoredIndex < tabViews.length; ++restoredIndex) {
                tabViews[restoredIndex].tabIndex = restoredIndex
                tabViews[restoredIndex].viewModel = tabs.get(restoredIndex)
            }
            return true
        }
        return sourceHost && sourceHost.restoreDetachedView
                ? sourceHost.restoreDetachedView(view) : false
    }

    function sameProfileWindow(sourceEntry, targetEntry) {
        return !!sourceEntry && !!targetEntry
                && sourceEntry.host !== targetEntry.host
                && sourceEntry.profileName === targetEntry.profileName
                && !!sourceEntry.privateProfile === !!targetEntry.privateProfile
                && !!sourceEntry.ephemeralProfile === !!targetEntry.ephemeralProfile
    }

    function attachTransferredView(view, payload, sourceUi, sourceHost) {
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
                || !browserUi.adopt_tab_transfer(payload)) {
            return false
        }
        var adoptedTabId = browserUi.tab_id_for_index(browserUi.active_tab_index)
        window.syncTabModel()
        var index = browserUi.active_tab_index
        if (index < 0 || index >= tabs.count) {
            browserUi.rollback_tab_transfer(adoptedTabId)
            window.syncTabModel()
            window.restoreSourceView(sourceUi, sourceHost, view)
            return false
        }
        tabs.setProperty(index, "url", transferData.url || "about:blank")
        tabs.setProperty(index, "title", transferData.title || "New tab")
        tabs.setProperty(index, "loaded", true)
        tabs.setProperty(index, "muted", !!transferData.muted)
        tabs.setProperty(index, "zoom", Number(transferData.zoom_hundredths || 100) / 100.0)
        var placeholder = tabViewAt(index)
        if (!placeholder) {
            browserUi.rollback_tab_transfer(adoptedTabId)
            window.syncTabModel()
            window.restoreSourceView(sourceUi, sourceHost, view)
            return false
        }
        window.removeTabViewAt(index, null)
        view.viewUi = browserUi
        view.viewTabs = tabs
        view.viewModel = tabs.get(index)
        view.viewTransferred = false
        view.stableTabId = adoptedTabId
        if (view.viewHost !== undefined) {
            view.viewHost = window
        }
        if (view.viewIsSecondaryStatic === true) {
            view.url = sourceUrl
        }
        view.tabIndex = index
        view.anchors.fill = undefined
        view.parent = webViews
        view.visible = true
        tabViews.splice(index, 0, view)
        for (var reindex = index; reindex < tabViews.length; ++reindex) {
            tabViews[reindex].tabIndex = reindex
            tabViews[reindex].viewModel = tabs.get(reindex)
        }
        window.updateBrowserWindowView(browserUi, view)
        if (!window.completeDetachedSource(sourceUi, sourceHost, payload)) {
            view.parent = null
            view.visible = false
            browserUi.rollback_tab_transfer(adoptedTabId)
            window.syncTabModel()
            window.restoreSourceView(sourceUi, sourceHost, view)
            return false
        }
        if (arguments.length >= 5 && typeof arguments[4] === "string"
                && arguments[4].length > 0 && sourceUi) {
            sourceUi.complete_transfer_operation(arguments[4], true)
        }
        view.forceActiveFocus()
        browserUi.status_text = "Live tab attached without navigation"
        return true
    }

    function executeBrowserTransferAction(sourceUi, sourceHost, action) {
        var fields = String(action || "").split("\t")
        var kind = fields[0]
        var operationId = ""
        if (kind === "tab-detach" && fields.length >= 2
                && fields[1].indexOf("op-") === 0) {
            operationId = fields[1]
        } else if (kind === "tab-give" && fields.length >= 3
                && fields[2].indexOf("op-") === 0) {
            operationId = fields[2]
        } else if (kind === "tab-move-context" && fields.length >= 4
                && fields[3].indexOf("op-") === 0) {
            operationId = fields[3]
        }
        var sourceEntry = window.browserWindowEntryForUi(sourceUi)
        if (!sourceEntry || !sourceUi || !sourceHost) {
            if (sourceUi) {
                sourceUi.status_text = "Live tab source window is unavailable"
                if (operationId.length > 0) {
                    sourceUi.complete_transfer_operation(operationId, false)
                }
            }
            return false
        }
        var payload = window.transferPayloadForUi(sourceUi)
        if (payload.length === 0) {
            sourceUi.status_text = "Active tab cannot be transferred"
            if (operationId.length > 0) {
                sourceUi.complete_transfer_operation(operationId, false)
            }
            return false
        }
        var targetEntry = kind === "tab-give"
                ? window.browserWindowEntryForTarget(fields.length >= 2 ? fields[1] : "")
                : (kind === "tab-move-context"
                        ? window.browserWindowEntryForContext(
                            fields.length >= 2 ? fields[1] : "", sourceEntry)
                        : null)
        if ((kind === "tab-give" || kind === "tab-move-context")
                && !window.sameProfileWindow(sourceEntry, targetEntry)) {
            sourceUi.status_text = !targetEntry
                    ? (kind === "tab-move-context"
                            ? "Context target window is unavailable; use context-enter to open it"
                            : "Tab give target window is unavailable")
                    : (kind === "tab-move-context"
                            ? "Context move requires a different same-profile window"
                    : "Tab give requires a different same-profile window")
            return false
        }
        var view = window.detachSourceView(sourceUi, sourceHost)
        if (!view) {
            sourceUi.status_text = "Live tab source view is unavailable"
            if (operationId.length > 0) {
                sourceUi.complete_transfer_operation(operationId, false)
            }
            return false
        }
        var transferAction = operationId.length > 0
                ? "tab-detach\t" + operationId + "\t" + payload
                : "tab-detach\t" + payload
        if (kind === "tab-give" || kind === "tab-move-context") {
            var attach = targetEntry.host === window
                    ? window.attachTransferredView : targetEntry.host.attachTransferredView
            if (!attach || !attach.call(targetEntry.host,
                        view, payload, sourceUi, sourceHost, operationId)) {
                window.restoreSourceView(sourceUi, sourceHost, view)
                sourceUi.status_text = "Tab give was rejected by the target window"
                if (operationId.length > 0) {
                    sourceUi.complete_transfer_operation(operationId, false)
                }
                return false
            }
            return true
        }
        if (!window.openDetachedWindow(sourceUi, sourceHost, transferAction, view)) {
            window.restoreSourceView(sourceUi, sourceHost, view)
            return false
        }
        return true
    }

    function openDetachedWindow(sourceUi, hostWindow, action, view) {
        var fields = String(action || "").split("\t")
        if (fields.length < 2 || !view) {
            if (sourceUi) {
                sourceUi.status_text = "Live tab detach payload was invalid"
            }
            return false
        }
        var operationId = fields.length >= 3 && fields[1].indexOf("op-") === 0
                ? fields[1] : ""
        var payloadStart = operationId.length > 0 ? 2 : 1
        var payload = fields.slice(payloadStart).join("\t")
        var secondary = hostWindow !== window
        var profileName = secondary ? hostWindow.windowProfileName : window.profileName
        var profileLabel = secondary ? hostWindow.windowProfileLabel : window.profileLabel
        var privateProfile = secondary
                ? hostWindow.windowPrivateProfile
                : (window.temporaryProfile && !window.ephemeralProfile)
        var ephemeralProfile = secondary
                ? hostWindow.windowEphemeralProfile : window.ephemeralProfile
        var sharedProfile = secondary
                ? hostWindow.windowWebEngineProfile : browserProfile
        var sharedInterceptor = secondary
                ? hostWindow.windowRequestInterceptor : requestInterceptor
        var invocationToken = secondary
                ? hostWindow.windowEphemeralInvocationToken
                : window.ephemeralInvocationToken
        var child = browserWindowComponent.createObject(null, {
            windowStartupUrl: "about:blank",
            windowProfileName: profileName,
            windowProfileLabel: profileLabel,
            windowPrivateProfile: privateProfile,
            windowEphemeralProfile: ephemeralProfile,
            windowEphemeralInvocationToken: invocationToken,
            windowSharedProfile: sharedProfile,
            windowSharedRequestInterceptor: sharedInterceptor,
            windowTransferView: view,
            windowTransferSourceUi: sourceUi,
            windowTransferSourceHost: hostWindow,
            windowTransferOperationId: operationId,
            windowTransferPayload: payload,
            windowStartupRoutePreflighted: true
        })
        if (!child) {
            if (sourceUi) {
                sourceUi.status_text = "Could not create detach target"
                if (operationId.length > 0) {
                    sourceUi.complete_transfer_operation(operationId, false)
                }
            }
            return false
        }
        if (sourceUi) {
            sourceUi.status_text = "Live tab moved to same-profile window"
        }
        return true
    }

    function completeDetachedSource(sourceUi, sourceHost, payload) {
        var transfer = null
        try {
            transfer = JSON.parse(String(payload || ""))
        } catch (error) {
            transfer = null
        }
        if (!transfer || typeof transfer.tab_id !== "string") {
            return false
        }
        var index = sourceUi ? sourceUi.tab_index_for_id(transfer.tab_id) : -1
        var preparedSecondaryFallback = sourceHost && sourceHost.prepareTransferFallback
                ? sourceHost.prepareTransferFallback() : true
        if (!preparedSecondaryFallback) {
            return false
        }
        if (!sourceUi || index < 0 || !sourceUi.complete_tab_transfer(transfer.tab_id)) {
            if (sourceHost && sourceHost.discardPreparedTransferFallback) {
                sourceHost.discardPreparedTransferFallback()
            }
            return false
        }
        if (sourceHost === window && index < tabs.count) {
            tabs.remove(index)
            if (window.preparedTransferFallback) {
                tabs.append({
                    url: "about:blank",
                    title: "New tab",
                    loaded: true,
                    pinned: false,
                    muted: false,
                    zoom: 1.0,
                    suspended: false,
                    discarded: false
                })
                var fallback = window.preparedTransferFallback
                window.preparedTransferFallback = null
                fallback.viewUi = browserUi
                fallback.viewTabs = tabs
                fallback.viewModel = tabs.get(0)
                fallback.viewTransferred = false
                fallback.viewHost = window
                fallback.tabIndex = 0
                fallback.stableTabId = browserUi.tab_id_for_index(0)
                tabs.setProperty(0, "tabId", fallback.stableTabId)
                fallback.parent = webViews
                fallback.visible = true
                tabViews.push(fallback)
                browserUi.status_text = "Live tab moved; blank fallback tab created"
            } else {
                window.syncTabModel()
            }
        } else if (sourceHost && sourceHost.resetAfterTransferSource) {
            if (!sourceHost.resetAfterTransferSource()) {
                return false
            }
        }
        return true
    }

    function showHintActions(label) {
        var view = window.activeWebView()
        if (!view) {
            window.closeHints()
            return
        }
        var items = []
        try {
            var actions = JSON.parse(browserUi.userscript_actions("link") || "[]")
            for (var i = 0; i < actions.length; ++i) {
                var action = actions[i]
                if (action && action.id && action.label && action.availability
                        && action.availability.state === "available") {
                    items.push(window.contextMenuItem(action.label,
                                                       "hint-userscript-action",
                                                       label, action.id))
                }
            }
        } catch (error) {
            browserUi.status_text = "Hint actions unavailable"
        }
        if (items.length === 0) {
            browserUi.status_text = "No userscript actions are available for this hint"
            return
        }
        window.pendingContextMenuRequest = { hintAction: true }
        window.pendingContextMenuView = view
        window.pendingContextMenuUi = browserUi
        window.pendingContextMenuHost = window
        window.contextMenuItems = items
        window.captureOverlayFocus(window, view)
        contextMenuPopup.open()
        browserUi.status_text = "Hint actions"
    }

    function activateHint(label, actionId) {
        var selected = null
        for (var i = 0; i < window.hintResults.length; ++i) {
            if (window.hintResults[i].label === label) {
                selected = window.hintResults[i]
                break
            }
        }
        var view = window.activeWebView()
        if (!selected || !view) {
            window.closeHints()
            return
        }
        var startedAt = Date.now()
        window.runBrowserScript(view, window.hintFreshScript(selected), function(value) {
            if (Date.now() - startedAt > 200) {
                window.closeHints()
                browserUi.status_text = "Hint target validation timed out"
                return
            }
            var result = {}
            try {
                if (actionId && actionId.length > 0) {
                    result = JSON.parse(browserUi.select_hint_action(
                                label, JSON.stringify(value), actionId))
                } else {
                    result = JSON.parse(browserUi.select_hint(label, JSON.stringify(value)))
                }
            } catch (error) {
                result = {}
            }
            if (result.action === "navigate") {
                window.hintResults = []
                window.hintInput = ""
                window.executePendingEngineAction()
            } else if (result.action === "yank") {
                window.hintResults = []
                window.hintInput = ""
                Qt.callLater(window.startHintCollection)
            } else if (result.action === "clean-yank") {
                window.hintResults = []
                window.hintInput = ""
                Qt.callLater(window.startHintCollection)
            } else if (result.action === "tab-bg") {
                window.hintResults = []
                window.hintInput = ""
                window.syncTabModel()
                window.executePendingEngineAction()
                Qt.callLater(window.startHintCollection)
            } else if (result.action === "userscript" && browserUi.mode === "hint") {
                window.hintResults = []
                window.hintInput = ""
                Qt.callLater(window.startHintCollection)
            } else if (result.action === "download" && browserUi.mode === "hint") {
                window.hintResults = []
                window.hintInput = ""
                Qt.callLater(window.startHintCollection)
            } else if (result.action === "ephemeral") {
                window.hintResults = []
                window.hintInput = ""
                window.openEphemeralWindow(
                            result.url, window.ephemeralTokenForUi(browserUi))
            } else if (result.error === "rapid-tab-limit") {
                window.hintResults = []
                window.hintInput = ""
                window.rapidHintConfirmationVisible = true
            } else if (result.action === "focus") {
                var focusScript = window.hintFocusScript(result.element_id)
                window.runBrowserScript(view, focusScript, function(value) {
                    if (value !== true) {
                        window.closeHints()
                        browserUi.status_text = "Hint focus target is no longer available"
                        return
                    }
                    window.hintResults = []
                    window.hintInput = ""
                    browserUi.enter_insert()
                })
            } else if (result.action === "click") {
                var clickScript = window.hintClickScript(result.element_id)
                window.runBrowserScript(view, clickScript, function(value) {
                    window.hintResults = []
                    window.hintInput = ""
                    browserUi.status_text = value === true
                            ? "Hint target activated"
                            : "Hint target is no longer clickable"
                })
            } else {
                window.hintResults = []
                window.hintInput = ""
                var rejectionStatus = browserUi.status_text
                browserUi.cancel_hints()
                browserUi.status_text = rejectionStatus
            }
        })
    }

    Connections {
        target: browserUi
        function onActive_tab_indexChanged() {
            window.noteTabActivity()
        }
        function onTab_countChanged() {
            window.noteTabActivity()
        }
        function onUserscript_install_stateChanged() {
            var state = browserUi.userscript_install_state || "idle"
            if (state === "installed") {
                window.settingsNotice = "Userscript installed"
            } else if (state === "removed") {
                window.settingsNotice = "Userscript removed"
            } else if (state.indexOf("error:") === 0) {
                window.settingsNotice = "Userscript operation failed: "
                        + state.substring(6)
            }
        }
        function onContext_route_jsonChanged() {
            window.showContextRoute(browserUi)
        }
        function onContext_workspaceChanged() {
            window.routeContextWorkspace(browserUi)
        }
        function onModeChanged() {
            if (browserUi.mode === "command" || browserUi.mode === "search") {
                window.captureModeFocus()
                Qt.callLater(function() {
                    if (browserUi.mode === "command") {
                        commandLine.forceActiveFocus()
                    } else if (browserUi.mode === "search") {
                        searchLine.forceActiveFocus()
                    }
                })
            } else {
                window.restoreModeFocus()
            }
            if (browserUi.mode === "hint") {
                window.startHintCollection()
            } else {
                if (browserUi.mode !== "caret") {
                    window.caretSelecting = false
                }
                window.rapidHintConfirmationVisible = false
                if (window.hintResults.length > 0) {
                    window.hintResults = []
                    window.hintInput = ""
                }
            }
        }
        function onConfig_jsonChanged() {
            window.refreshChromeAppearance()
            window.configureLinkCleaningUpdateTimer()
            window.refreshSpellcheckInventory(browserUi)
            if (browserProfile) {
                browserProfile.spellCheckEnabled = window.spellcheckEnabled(browserUi)
                browserProfile.spellCheckLanguages = window.spellcheckLanguages(browserUi)
            }
            if (window.settingsVisible) {
                window.refreshSettings()
            }
        }
        function onTheme_palette_jsonChanged() {
            window.refreshChromeAppearance()
        }
        function onSystem_font_scale_jsonChanged() {
            window.refreshChromeAppearance()
        }
        function onExternal_navigation_visibleChanged() {
            if (browserUi.external_navigation_visible) {
                window.openInternalSurface()
            } else {
                window.closeInternalSurface()
            }
        }
        function onBinding_overlayChanged() {
            window.bindingOverlayVisible = false
            bindingOverlayTimer.stop()
            if (browserUi.binding_overlay.length > 0) {
                bindingOverlayTimer.restart()
            }
        }
    }

    function activateSwitcher(index) {
        var result = switcherResults[index]
        if (!result) {
            return
        }
        var owner = window.switcherOwner(result)
        var generation = result.generation === null || result.generation === undefined
                ? "" : String(result.generation)
        if (owner.ui.activate_switcher_result(result.kind, result.id, generation)) {
            if (owner.host === window) {
                window.syncTabModel()
                window.executePendingEngineAction()
            } else if (owner.host.applySwitcherEngineAction) {
                owner.host.applySwitcherEngineAction()
            }
            closeSwitcher()
            window.activateBrowserWindow(owner.host, owner.ui)
            if (result.workspace) {
                window.routeContextWorkspace(owner.ui)
            }
        }
    }

    function activateSwitcherAction(index, action) {
        var result = switcherResults[index]
        if (!result || !action) {
            return
        }
        var owner = window.switcherOwner(result)
        var generation = result.generation === null || result.generation === undefined
                ? "" : String(result.generation)
        if (owner.ui.activate_switcher_action(result.kind, result.id, action, generation)) {
            if (owner.host === window) {
                window.syncTabModel()
                window.executePendingEngineAction()
            } else if (owner.host.applySwitcherEngineAction) {
                owner.host.applySwitcherEngineAction()
            }
            closeSwitcher()
            window.activateBrowserWindow(owner.host, owner.ui)
            if (result.workspace) {
                window.routeContextWorkspace(owner.ui)
            }
        }
    }

    Shortcut {
        sequence: "Ctrl+P"
        onActivated: window.showSwitcher()
    }

    function downloadStateName(download) {
        if (download.isPaused) {
            return "paused"
        }
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

    function navigationFailureKind(loadRequest) {
        if (loadRequest.isDownload === true) {
            return "download"
        }
        var domain = String(loadRequest.errorDomain)
        if (domain === "6" || domain.indexOf("Dns") >= 0) {
            return "dns"
        }
        if (domain === "3" || domain.indexOf("Certificate") >= 0) {
            return "tls"
        }
        if (domain === "4" || domain === "7"
                || domain.indexOf("Http") >= 0) {
            return "http"
        }
        if (domain === "2" || domain.indexOf("Connection") >= 0) {
            return "network"
        }
        return "unknown"
    }

    function navigationFailureDetail(loadRequest) {
        var detail = String(loadRequest.errorString || "")
        var code = Number(loadRequest.errorCode)
        if (Number.isFinite(code) && code !== 0) {
            detail = detail.length > 0 ? detail + " (code " + code + ")" : "code " + code
        }
        return detail
    }

    function downloadsAskDestination() {
        try {
            var config = JSON.parse(browserUi.config_json)
            return !config.downloads || config.downloads.ask_destination !== false
        } catch (error) {
            return true
        }
    }

    function permissionTypeName(permissionType) {
        if (permissionType === WebEnginePermission.MediaAudioCapture) {
            return "microphone"
        }
        if (permissionType === WebEnginePermission.MediaVideoCapture) {
            return "camera"
        }
        if (permissionType === WebEnginePermission.MediaAudioVideoCapture) {
            return "camera-and-microphone"
        }
        if (permissionType === WebEnginePermission.DesktopVideoCapture
                || permissionType === WebEnginePermission.DesktopAudioVideoCapture) {
            return "screen-capture"
        }
        if (permissionType === WebEnginePermission.Notifications) {
            return "notifications"
        }
        if (permissionType === WebEnginePermission.Geolocation) {
            return "geolocation"
        }
        if (permissionType === WebEnginePermission.ClipboardReadWrite) {
            return "clipboard"
        }
        if (permissionType === WebEnginePermission.LocalFontsAccess) {
            return "local-fonts"
        }
        return ""
    }

    function permissionDisplayName(permissionName) {
        return {
            "camera": "use your camera",
            "microphone": "use your microphone",
            "camera-and-microphone": "use your camera and microphone",
            "screen-capture": "capture your screen or window",
            "notifications": "show notifications",
            "geolocation": "access your location",
            "clipboard": "read and write your clipboard",
            "local-fonts": "use your local fonts"
        }[permissionName] || "use an unsupported browser capability"
    }

    function permissionCanRemember(origin, permissionName) {
        if (permissionName === "screen-capture") {
            return false
        }
        return origin.indexOf("https://") === 0
                || origin.indexOf("http://localhost") === 0
                || origin.indexOf("http://127.0.0.1") === 0
                || origin.indexOf("http://[::1]") === 0
    }

    function permissionCanRememberForSite(origin, permissionName) {
        return !window.pendingPermissionPrivate && window.permissionCanRemember(origin, permissionName)
    }

    function boundedPageDialogText(value) {
        var text = String(value || "")
        text = text.replace(/[\u0000-\u001f\u007f]/g, "�")
        return text.length > 4096 ? text.slice(0, 4096) + "…" : text
    }

    function pageDialogType(request) {
        if (!request) {
            return "unknown"
        }
        if (request.type === JavaScriptDialogRequest.DialogTypeAlert) {
            return "alert"
        }
        if (request.type === JavaScriptDialogRequest.DialogTypeConfirm) {
            return "confirm"
        }
        if (request.type === JavaScriptDialogRequest.DialogTypePrompt) {
            return "prompt"
        }
        if (request.type === JavaScriptDialogRequest.DialogTypeBeforeUnload) {
            return "beforeunload"
        }
        return "unknown"
    }

    function pageDialogKind() {
        if (window.pendingAuthenticationRequest) {
            return "authentication"
        }
        return window.pageDialogType(window.pendingPageDialogRequest)
    }

    function clearPageDialog(rejectRequest) {
        var request = window.pendingAuthenticationRequest || window.pendingPageDialogRequest
        var hadRequest = !!request
        var ui = window.pendingPageDialogUi || browserUi
        var authentication = window.pageDialogIsAuthentication
        window.pendingPageDialogRequest = null
        window.pendingAuthenticationRequest = null
        window.pendingPageDialogUi = null
        window.pendingPageDialogView = null
        window.pageDialogIsAuthentication = false
        window.pageDialogMessage = ""
        window.pageDialogTitle = "Page dialog"
        window.pageDialogOrigin = ""
        window.pageDialogDefaultText = ""
        window.pageDialogSuppressChecked = false
        pageDialogPopup.usernameText = ""
        pageDialogPopup.passwordText = ""
        pageDialogPopup.inputText = ""
        pageDialogPopup.close()
        if (hadRequest) {
            window.restoreOverlayFocus()
        }
        if (rejectRequest && request) {
            window.resolveQtRequest(ui, request, authentication ? "authentication" : "page-dialog", "dialogReject", [])
            if (ui) {
                ui.status_text = authentication
                        ? "Authentication prompt dismissed"
                        : "Page dialog dismissed"
            }
        }
    }

    function clearPageDialogForView(view) {
        if (window.pendingPageDialogView === view) {
            window.clearPageDialog(true)
        }
    }

    function resetPageDialogBudget(view) {
        if (!view) {
            return
        }
        view.pageDialogDocumentKey = ""
        view.pageDialogCount = 0
        view.pageDialogSuppressed = false
    }

    function pageDialogDocumentKey(view) {
        var url = view && view.url ? view.url.toString() : ""
        var fragment = url.indexOf("#")
        return fragment >= 0 ? url.slice(0, fragment) : url
    }

    function handleJavaScriptDialogRequested(ui, view, request, privateProfile) {
        if (!request) {
            return
        }
        if (window.pendingPageDialogRequest || window.pendingAuthenticationRequest) {
            window.clearPageDialog(true)
        }
        var kind = window.pageDialogType(request)
        if (kind === "unknown") {
            window.resolveQtRequest(ui, request, "page-dialog", "dialogReject", [])
            ui.status_text = "Blocked unsupported page dialog"
            return
        }
        var documentKey = window.pageDialogDocumentKey(view)
        if (view.pageDialogDocumentKey !== documentKey) {
            window.resetPageDialogBudget(view)
            view.pageDialogDocumentKey = documentKey
        }
        if (view.pageDialogSuppressed || view.pageDialogCount >= window.pageDialogLimit) {
            window.resolveQtRequest(ui, request, "page-dialog", "dialogReject", [])
            ui.status_text = view.pageDialogSuppressed
                    ? "Suppressed repeated page dialog"
                    : "Page dialog limit reached for this page"
            return
        }
        view.pageDialogCount += 1
        window.pendingPageDialogRequest = request
        window.pendingPageDialogUi = ui
        window.pendingPageDialogView = view
        window.captureOverlayFocus(window, view)
        window.pageDialogMessage = window.boundedPageDialogText(request.message)
        var title = window.boundedPageDialogText(request.title)
        window.pageDialogTitle = title.length > 0 ? title : "Page " + kind
        window.pageDialogOrigin = request.securityOrigin && request.securityOrigin.host
                ? window.boundedPageDialogText(request.securityOrigin.host)
                : "opaque or unavailable origin"
        window.pageDialogDefaultText = window.boundedPageDialogText(request.defaultText)
        window.pageDialogSuppressChecked = false
        pageDialogPopup.inputText = window.pageDialogDefaultText
        pageDialogPopup.open()
        ui.status_text = privateProfile ? "Private page dialog" : "Page dialog"
    }

    function handleAuthenticationDialogRequested(ui, view, request, privateProfile) {
        if (!request) {
            return
        }
        if (window.pendingPageDialogRequest || window.pendingAuthenticationRequest) {
            window.clearPageDialog(true)
        }
        var documentKey = window.pageDialogDocumentKey(view)
        if (view.pageDialogDocumentKey !== documentKey) {
            window.resetPageDialogBudget(view)
            view.pageDialogDocumentKey = documentKey
        }
        if (view.pageDialogSuppressed || view.pageDialogCount >= window.pageDialogLimit) {
            window.resolveQtRequest(ui, request, "authentication", "dialogReject", [])
            ui.status_text = view.pageDialogSuppressed
                    ? "Suppressed repeated authentication prompt"
                    : "Authentication prompt limit reached for this page"
            return
        }
        view.pageDialogCount += 1
        window.pendingAuthenticationRequest = request
        window.pendingPageDialogUi = ui
        window.pendingPageDialogView = view
        window.captureOverlayFocus(window, view)
        window.pageDialogIsAuthentication = true
        window.pageDialogTitle = request.proxyHost
                ? "Proxy authentication"
                : "HTTP authentication"
        window.pageDialogOrigin = request.proxyHost
                ? window.boundedPageDialogText(request.proxyHost)
                : (request.url && request.url.host
                   ? window.boundedPageDialogText(request.url.host)
                   : "opaque or unavailable origin")
        var realm = window.boundedPageDialogText(request.realm)
        window.pageDialogMessage = realm.length > 0
                ? "Credentials requested for realm: " + realm
                : "This site is requesting credentials"
        window.pageDialogSuppressChecked = false
        pageDialogPopup.usernameText = ""
        pageDialogPopup.passwordText = ""
        pageDialogPopup.open()
        ui.status_text = privateProfile ? "Private authentication prompt" : "Authentication prompt"
    }

    function clearClientCertificateRequest(selectNone) {
        var selection = window.pendingClientCertificateSelection
        var hadSelection = !!selection
        var ui = window.pendingClientCertificateUi || browserUi
        window.pendingClientCertificateSelection = null
        window.pendingClientCertificateUi = null
        window.pendingClientCertificateView = null
        window.pendingClientCertificateOptions = []
        window.pendingClientCertificateHost = ""
        clientCertificatePopup.close()
        if (hadSelection && selectNone) {
            window.resolveQtRequest(ui, selection, "client-certificate", "selectNone", [])
        }
        if (hadSelection) {
            window.restoreOverlayFocus()
        }
        return hadSelection ? ui : null
    }

    function clearClientCertificateForView(view) {
        if (window.pendingClientCertificateView === view) {
            window.clearClientCertificateRequest(true)
        }
    }

    function handleClientCertificateRequested(ui, view, selection, privateProfile, hostWindow) {
        if (!selection) {
            return
        }
        if (window.pendingPageDialogRequest || window.pendingAuthenticationRequest) {
            window.clearPageDialog(true)
        }
        if (window.pendingClientCertificateSelection) {
            window.clearClientCertificateRequest(true)
        }
        var offered = selection.certificates || []
        if (offered.length === 0) {
            window.resolveQtRequest(ui, selection, "client-certificate", "selectNone", [])
            ui.status_text = "No client certificates available; request cancelled"
            return
        }
        var options = []
        for (var i = 0; i < offered.length && i < 32; ++i) {
            var option = offered[i]
            options.push({
                index: i,
                subject: window.boundedPageDialogText(option.subject || "Unnamed certificate"),
                issuer: window.boundedPageDialogText(option.issuer || "Issuer unavailable"),
                selfSigned: !!option.isSelfSigned
            })
        }
        if (options.length === 0) {
            window.resolveQtRequest(ui, selection, "client-certificate", "selectNone", [])
            ui.status_text = "Client certificate choices unavailable; request cancelled"
            return
        }
        window.pendingClientCertificateSelection = selection
        window.pendingClientCertificateUi = ui
        window.pendingClientCertificateView = view
        window.pendingClientCertificateOptions = options
        window.pendingClientCertificateHost = selection.host && selection.host.host
                ? window.boundedPageDialogText(selection.host.host)
                : "opaque or unavailable host"
        window.captureOverlayFocus(hostWindow || window, view)
        clientCertificatePopup.open()
        ui.status_text = privateProfile
                ? "Private client certificate selection"
                : "Client certificate selection"
    }

    function acceptClientCertificate(index) {
        var selection = window.pendingClientCertificateSelection
        var ui = window.pendingClientCertificateUi || browserUi
        var options = window.pendingClientCertificateOptions || []
        var valid = false
        for (var i = 0; i < options.length; ++i) {
            if (options[i] && options[i].index === index) {
                valid = true
                break
            }
        }
        if (!selection || !valid) {
            return
        }
        window.resolveQtRequest(ui, selection, "client-certificate", "select", [index])
        window.clearClientCertificateRequest(false)
        ui.status_text = "Client certificate selected"
    }

    function rejectClientCertificate() {
        var ui = window.clearClientCertificateRequest(true) || browserUi
        ui.status_text = "Client certificate request cancelled"
    }

    function googleCertificateHost(host) {
        var value = String(host || "").toLowerCase()
        return value === "google.com" || value.indexOf("google.") === 0
                || value.indexOf(".google.") >= 0 || value.endsWith(".google")
    }

    function clearCertificateErrorRequest(rejectRequest) {
        var error = window.pendingCertificateError
        var hadError = !!error
        var ui = window.pendingCertificateErrorUi || browserUi
        window.pendingCertificateError = null
        window.pendingCertificateErrorUi = null
        window.pendingCertificateErrorView = null
        window.pendingCertificateErrorHost = ""
        window.pendingCertificateErrorDescription = ""
        certificateErrorPopup.close()
        if (hadError && rejectRequest) {
            window.resolveQtRequest(ui, error, "client-certificate", "rejectCertificate", [])
        }
        if (hadError) {
            window.restoreOverlayFocus()
        }
        return hadError ? ui : null
    }

    function clearCertificateErrorForView(view) {
        if (window.pendingCertificateErrorView === view) {
            window.clearCertificateErrorRequest(true)
        }
    }

    function handleCertificateError(ui, view, error, hostWindow) {
        if (!error) {
            return
        }
        if (window.pendingCertificateError) {
            window.clearCertificateErrorRequest(true)
        }
        if (window.pendingPageDialogRequest || window.pendingAuthenticationRequest) {
            window.clearPageDialog(true)
        }
        if (window.pendingClientCertificateSelection) {
            window.clearClientCertificateRequest(true)
        }
        var host = error.url && error.url.host ? String(error.url.host) : ""
        if (!error.overridable || !error.isMainFrame) {
            window.resolveQtRequest(ui, error, "client-certificate", "rejectCertificate", [])
            ui.status_text = error.isMainFrame
                    ? "TLS certificate error blocked"
                    : "TLS certificate error blocked for a subresource"
            return
        }
        if (window.googleCertificateHost(host)) {
            window.resolveQtRequest(ui, error, "client-certificate", "rejectCertificate", [])
            ui.status_text = "TLS certificate error blocked for protected host"
            return
        }
        window.pendingCertificateError = error
        window.pendingCertificateErrorUi = ui
        window.pendingCertificateErrorView = view
        window.pendingCertificateErrorHost = window.boundedPageDialogText(
                    host || "opaque or unavailable host")
        window.pendingCertificateErrorDescription = window.boundedPageDialogText(
                    error.description || "The certificate could not be verified")
        window.captureOverlayFocus(hostWindow || window, view)
        certificateErrorPopup.open()
        ui.status_text = "TLS certificate confirmation required"
    }

    function acceptCertificateError() {
        var error = window.pendingCertificateError
        var ui = window.pendingCertificateErrorUi || browserUi
        if (!error) {
            return
        }
        window.resolveQtRequest(ui, error, "client-certificate", "acceptCertificate", [])
        window.clearCertificateErrorRequest(false)
        ui.status_text = "TLS certificate accepted for this request only"
    }

    function rejectCertificateError() {
        var ui = window.clearCertificateErrorRequest(true) || browserUi
        ui.status_text = "TLS certificate rejected"
    }

    function webAuthStateLabel(state) {
        if (state === WebEngineWebAuthUxRequest.SelectAccount) {
            return "Select an account"
        }
        if (state === WebEngineWebAuthUxRequest.CollectPin) {
            return "Enter your security-key PIN"
        }
        if (state === WebEngineWebAuthUxRequest.FinishTokenCollection) {
            return "Touch or confirm your security key"
        }
        if (state === WebEngineWebAuthUxRequest.RequestFailed) {
            return "Security-key request failed"
        }
        if (state === WebEngineWebAuthUxRequest.Completed) {
            return "Security-key request completed"
        }
        if (state === WebEngineWebAuthUxRequest.Cancelled) {
            return "Security-key request cancelled"
        }
        return "Preparing security-key request"
    }

    function updateWebAuthState(request, state) {
        if (window.pendingWebAuthRequest !== request) {
            return
        }
        window.pendingWebAuthState = state
        window.webAuthStatusText = window.webAuthStateLabel(state)
        if (state === WebEngineWebAuthUxRequest.Completed) {
            var completedUi = window.clearWebAuthRequest(false) || browserUi
            completedUi.status_text = "WebAuthn completed"
        } else if (state === WebEngineWebAuthUxRequest.Cancelled) {
            var cancelledUi = window.clearWebAuthRequest(false) || browserUi
            cancelledUi.status_text = "WebAuthn cancelled"
        }
    }

    function clearWebAuthRequest(cancelRequest) {
        var request = window.pendingWebAuthRequest
        var hadRequest = !!request
        var ui = window.pendingWebAuthUi || browserUi
        window.pendingWebAuthRequest = null
        window.pendingWebAuthUi = null
        window.pendingWebAuthView = null
        window.pendingWebAuthUserNames = []
        window.pendingWebAuthState = null
        window.pendingWebAuthRelyingParty = ""
        window.webAuthStatusText = ""
        webAuthPinField.text = ""
        webAuthPopup.close()
        if (hadRequest && cancelRequest
                && request.state !== WebEngineWebAuthUxRequest.Completed
                && request.state !== WebEngineWebAuthUxRequest.Cancelled) {
            window.resolveQtRequest(ui, request, "webauth", "cancel", [])
        }
        if (hadRequest) {
            window.restoreOverlayFocus()
        }
        return hadRequest ? ui : null
    }

    function clearWebAuthForView(view) {
        if (window.pendingWebAuthView === view) {
            window.clearWebAuthRequest(true)
        }
    }

    function handleWebAuthRequested(ui, view, request, hostWindow) {
        if (!request) {
            return
        }
        if (window.pendingWebAuthRequest) {
            window.clearWebAuthRequest(true)
        }
        if (window.pendingPageDialogRequest || window.pendingAuthenticationRequest) {
            window.clearPageDialog(true)
        }
        if (window.pendingClientCertificateSelection) {
            window.clearClientCertificateRequest(true)
        }
        if (window.pendingCertificateError) {
            window.clearCertificateErrorRequest(true)
        }
        var names = []
        var offered = request.userNames || []
        for (var i = 0; i < offered.length && i < 16; ++i) {
            names.push(window.boundedPageDialogText(offered[i]))
        }
        window.pendingWebAuthRequest = request
        window.pendingWebAuthUi = ui
        window.pendingWebAuthView = view
        window.pendingWebAuthUserNames = names
        window.pendingWebAuthState = request.state
        window.pendingWebAuthRelyingParty = window.boundedPageDialogText(
                    request.relyingPartyId || "opaque or unavailable relying party")
        window.webAuthStatusText = window.webAuthStateLabel(request.state)
        request.stateChanged.connect(function(state) {
            window.updateWebAuthState(request, state)
        })
        window.captureOverlayFocus(hostWindow || window, view)
        webAuthPopup.open()
        ui.status_text = "WebAuthn security-key prompt"
    }

    function selectWebAuthAccount(account) {
        var request = window.pendingWebAuthRequest
        if (!request || window.pendingWebAuthState !== WebEngineWebAuthUxRequest.SelectAccount) {
            return
        }
        request.setSelectedAccount(window.boundedPageDialogText(account))
    }

    function submitWebAuthPin() {
        var request = window.pendingWebAuthRequest
        if (!request || window.pendingWebAuthState !== WebEngineWebAuthUxRequest.CollectPin) {
            return
        }
        var pin = window.boundedPageDialogText(webAuthPinField.text)
        webAuthPinField.text = ""
        request.setPin(pin)
        pin = ""
    }

    function retryWebAuth() {
        var request = window.pendingWebAuthRequest
        if (request && window.pendingWebAuthState === WebEngineWebAuthUxRequest.RequestFailed) {
            request.retry()
        }
    }

    function cancelWebAuth() {
        var ui = window.clearWebAuthRequest(true) || browserUi
        ui.status_text = "WebAuthn cancelled"
    }

    function safeContextUrl(value) {
        var result = String(value || "")
        var fragment = result.indexOf("#")
        if (fragment >= 0) {
            result = result.slice(0, fragment)
        }
        result = result.replace(/^(https?:\/\/)[^\/?#@]*@/i, "$1")
        var question = result.indexOf("?")
        if (question < 0) {
            return result
        }
        var base = result.slice(0, question)
        var queryParts = result.slice(question + 1).split("&")
        var kept = []
        for (var i = 0; i < queryParts.length; ++i) {
            var part = queryParts[i]
            var key = part.split("=", 1)[0].toLowerCase()
            var decodedKey = key
            try {
                decodedKey = decodeURIComponent(key).toLowerCase()
            } catch (error) {
                continue
            }
            if (/(pass|token|secret|auth|session|signature|sig|api[_-]?key|private[_-]?key|client[_-]?secret|credential|jwt|bearer|nonce|^code$|^key$)/.test(decodedKey)) {
                continue
            }
            if (part.length > 0) {
                kept.push(part)
            }
        }
        return kept.length > 0 ? base + "?" + kept.join("&") : base
    }

    function clearContextMenuRequest() {
        var hadRequest = !!window.pendingContextMenuRequest
        window.pendingContextMenuRequest = null
        window.pendingContextMenuView = null
        window.pendingContextMenuUi = null
        window.pendingContextMenuHost = null
        window.contextMenuItems = []
        contextMenuPopup.close()
        if (hadRequest) {
            window.restoreOverlayFocus()
        }
    }

    function clearContextMenuForView(view) {
        if (window.pendingContextMenuView === view) {
            window.clearContextMenuRequest()
        }
    }

    function contextMenuItem(label, action, value, actionId) {
        return { label: label, action: action, value: value || "",
                 actionId: actionId || "" }
    }

    function appendExternalActions(items, ui, subject, value) {
        try {
            var raw = ui.external_action_values(subject)
            var actions = JSON.parse(raw || "[]")
            for (var i = 0; i < actions.length; ++i) {
                var action = actions[i]
                if (action && action.id && action.label
                        && action.availability
                        && action.availability.state === "available") {
                    items.push(window.contextMenuItem(
                                  action.label,
                                  "external-" + subject + "-send",
                                  value || "",
                                  action.id))
                }
            }
        } catch (error) {
            ui.status_text = "External actions unavailable"
        }
    }

    function appendUserscriptActions(items, ui, subject, value) {
        try {
            var raw = ui.userscript_actions(subject)
            var actions = JSON.parse(raw || "[]")
            for (var i = 0; i < actions.length; ++i) {
                var action = actions[i]
                if (action && action.id && action.label
                        && action.availability
                        && action.availability.state === "available") {
                    items.push(window.contextMenuItem(action.label, "userscript-action",
                                                       value, action.id))
                }
            }
        } catch (error) {
            ui.status_text = "Userscript actions unavailable"
        }
    }

    function handleContextMenuRequested(ui, view, request, hostWindow) {
        if (!request || !view) {
            return
        }
        if (window.pendingContextMenuRequest) {
            window.clearContextMenuRequest()
        }
        var items = []
        var link = request.linkUrl ? request.linkUrl.toString() : ""
        var safeLink = window.safeContextUrl(link)
        if (safeLink.length > 0) {
            items.push(window.contextMenuItem("Open link", "open-link", safeLink,
                                              "browser.link.open"))
            items.push(window.contextMenuItem("Copy link", "copy-link", safeLink,
                                              "browser.link.copy"))
            items.push(window.contextMenuItem("Clean-copy link", "copy-link", safeLink,
                                              "browser.link.clean-copy"))
            items.push(window.contextMenuItem("Download link", "download-link", safeLink,
                                              "browser.link.download"))
            window.appendExternalActions(items, ui, "link", safeLink)
            window.appendUserscriptActions(items, ui, "link", safeLink)
        }
        var media = request.mediaUrl ? request.mediaUrl.toString() : ""
        if (media.length > 0 && Number(request.mediaType) !== 0) {
            items.push(window.contextMenuItem("Copy media URL", "copy-link",
                                               window.safeContextUrl(media),
                                               "browser.link.copy"))
            items.push(window.contextMenuItem("Download media", "download-link",
                                               window.safeContextUrl(media),
                                               "browser.link.download"))
        }
        var editFlags = Number(request.editFlags || 0)
        if (editFlags & 8 || String(request.selectedText || "").length > 0) {
            items.push(window.contextMenuItem("Copy selection", "web-copy", "",
                                              "browser.selection.copy"))
            items.push(window.contextMenuItem("Search selection", "selection-search", "",
                                              "browser.selection.search"))
            window.appendExternalActions(items, ui, "selection", "")
            window.appendUserscriptActions(items, ui, "selection",
                                           String(request.selectedText || ""))
        }
        if (editFlags & 1) {
            items.push(window.contextMenuItem("Undo", "web-undo"))
        }
        if (editFlags & 2) {
            items.push(window.contextMenuItem("Redo", "web-redo"))
        }
        if (editFlags & 4) {
            items.push(window.contextMenuItem("Cut", "web-cut"))
        }
        if (editFlags & 16) {
            items.push(window.contextMenuItem("Paste", "web-paste"))
        }
        if (editFlags & 32) {
            items.push(window.contextMenuItem("Delete", "web-delete"))
        }
        if (editFlags & 64) {
            items.push(window.contextMenuItem("Select all", "web-select-all"))
        }
        var suggestions = request.spellCheckerSuggestions || []
        if (String(request.misspelledWord || "").length > 0 && suggestions.length > 0) {
            for (var suggestionIndex = 0; suggestionIndex < suggestions.length
                 && suggestionIndex < 8; ++suggestionIndex) {
                var suggestion = window.boundedPageDialogText(suggestions[suggestionIndex])
                if (suggestion.length > 0) {
                    items.push(window.contextMenuItem(
                                  "Replace with " + suggestion, "spell", suggestion))
                }
            }
        }
        items.push(window.contextMenuItem("Inspect element", "inspect"))
        window.pendingContextMenuRequest = request
        window.pendingContextMenuView = view
        window.pendingContextMenuUi = ui
        window.pendingContextMenuHost = hostWindow || window
        window.contextMenuItems = items
        request.accepted = true
        window.captureOverlayFocus(hostWindow || window, view)
        contextMenuPopup.open()
        ui.status_text = "Context menu"
    }

    function activateContextMenuItem(item) {
        var view = window.pendingContextMenuView
        var ui = window.pendingContextMenuUi || browserUi
        var host = window.pendingContextMenuHost
        if (!item || !view) {
            window.clearContextMenuRequest()
            return
        }
        var action = item.action
        var value = item.value
        window.clearContextMenuRequest()
        if (action === "hint-userscript-action") {
            window.activateHint(value, item.actionId || "")
            return
        }
        if (item.actionId && item.actionId.length > 0) {
            if (!ui.execute_ui_action(item.actionId, value)) {
                return
            }
            if (ui === browserUi) {
                window.executePendingEngineAction()
            } else if (action === "open-link" && view) {
                var pending = ui.take_engine_action()
                if (pending.indexOf("navigate\t") === 0) {
                    var parts = pending.split("\t")
                    if (parts.length >= 3) {
                        view.url = parts.slice(2).join("\t")
                    }
                }
            } else if (action === "download-link" && view) {
                var downloadRequest = ui.take_download_request()
                if (downloadRequest.length > 0) {
                    var downloadData = JSON.parse(downloadRequest)
                    window.runBrowserScript(view, window.downloadLinkScript(downloadData.url),
                                            function() {
                                                ui.complete_download_request(downloadData.token, true)
                                            })
                }
            } else if (action === "copy-link" && view) {
                var clipboardRequest = ui.take_clipboard_request()
                if (clipboardRequest.length > 0) {
                    window.copyToClipboard(
                                clipboardRequest,
                                !ui.clipboard_request_sensitive,
                                ui.clipboard_request_primary)
                }
            } else if ((action === "web-copy" || action === "selection-search"
                        || action === "external-selection-send") && view) {
                var selectionToken = ui.take_selection_request()
                if (selectionToken.length > 0) {
                    window.runBrowserScript(view, window.selectionScript(), function(value) {
                        ui.deliver_selection(selectionToken, JSON.stringify(value))
                    })
                }
            }
            return
        }
        if (action === "open-link") {
            if (host && host.popupPermissionUi !== undefined) {
                view.url = value
            } else {
                ui.navigate(value)
                if (ui === browserUi) {
                    window.updateActiveTabUrl()
                } else if (ui.initial_url !== undefined) {
                    view.url = ui.initial_url
                } else {
                    view.url = value
                }
            }
            ui.status_text = "Link opened"
        } else if (action === "copy-link") {
            window.copyToClipboard(value, false)
            ui.status_text = "Link copied"
        } else if (action === "download-link") {
            window.runBrowserScript(view, window.downloadLinkScript(value))
            ui.status_text = "Download requested"
        } else if (action === "spell") {
            view.replaceMisspelledWord(window.boundedPageDialogText(value))
            ui.status_text = "Spelling replacement applied"
        } else if (action === "inspect") {
            view.triggerWebAction(WebEngineView.InspectElement)
            ui.status_text = "Inspect element requested"
        } else {
            var webAction = action === "web-copy" ? WebEngineView.Copy
                    : action === "web-undo" ? WebEngineView.Undo
                    : action === "web-redo" ? WebEngineView.Redo
                    : action === "web-cut" ? WebEngineView.Cut
                    : action === "web-paste" ? WebEngineView.Paste
                    : action === "web-delete" ? WebEngineView.Delete
                    : action === "web-select-all" ? WebEngineView.SelectAll
                    : WebEngineView.NoWebAction
            if (webAction !== WebEngineView.NoWebAction) {
                view.triggerWebAction(webAction)
                ui.status_text = "Edit action applied"
            }
        }
    }

    function acceptPageDialog() {
        var request = window.pendingAuthenticationRequest || window.pendingPageDialogRequest
        var ui = window.pendingPageDialogUi || browserUi
        var view = window.pendingPageDialogView
        var authentication = !!window.pendingAuthenticationRequest
        if (!request) {
            return
        }
        if (window.pendingAuthenticationRequest) {
            var username = window.boundedPageDialogText(pageDialogPopup.usernameText)
            var password = window.boundedPageDialogText(pageDialogPopup.passwordText)
            pageDialogPopup.usernameText = ""
            pageDialogPopup.passwordText = ""
            window.resolveQtRequest(
                ui, request, authentication ? "authentication" : "page-dialog",
                "dialogAccept", [username, password])
            username = ""
            password = ""
        } else if (window.pageDialogType(request) === "prompt") {
            window.resolveQtRequest(
                ui, request, "page-dialog", "dialogAccept",
                [window.boundedPageDialogText(pageDialogPopup.inputText)])
        } else {
            window.resolveQtRequest(
                ui, request, "page-dialog", "dialogAccept", [])
        }
        if (view && window.pageDialogSuppressChecked) {
            view.pageDialogSuppressed = true
        }
        window.clearPageDialog(false)
        ui.status_text = authentication ? "Authentication accepted" : "Page dialog accepted"
    }

    function rejectPageDialog() {
        var request = window.pendingAuthenticationRequest || window.pendingPageDialogRequest
        var ui = window.pendingPageDialogUi || browserUi
        var view = window.pendingPageDialogView
        var kind = window.pageDialogKind()
        if (!request) {
            return
        }
        window.resolveQtRequest(
            ui, request, kind === "authentication" ? "authentication" : "page-dialog",
            "dialogReject", [])
        pageDialogPopup.usernameText = ""
        pageDialogPopup.passwordText = ""
        if (view && window.pageDialogSuppressChecked) {
            view.pageDialogSuppressed = true
        }
        window.clearPageDialog(false)
        ui.status_text = kind === "beforeunload"
                ? "Stayed on page"
                : kind === "authentication"
                  ? "Authentication rejected"
                  : "Page dialog rejected"
    }

    function permissionQueueKey(origin, permissionName, privateProfile) {
        return origin + "\t" + permissionName + "\t" + (privateProfile ? "private" : "normal")
    }

    function denyPermissionGroup(group) {
        if (!group || !group.requests) {
            return
        }
        for (var i = 0; i < group.requests.length; ++i) {
            var request = group.requests[i]
            window.resolveQtRequest(group.ui, request, "permission", "deny", [])
        }
    }

    function permissionQueueRequestCount(queue) {
        var count = 0
        for (var i = 0; i < queue.length; ++i) {
            count += queue[i].requests ? queue[i].requests.length : 0
        }
        return count
    }

    function showNextPermissionPrompt() {
        var queue = window.permissionPromptQueue
        var wasVisible = window.permissionPromptVisible
        if (!queue || queue.length === 0) {
            window.permissionPromptVisible = false
            window.pendingPermissionRequest = null
            window.pendingPermissionUi = null
            window.pendingPermissionTabIndex = -1
            window.pendingPermissionOrigin = ""
            window.pendingPermissionName = ""
            window.pendingPermissionPrivate = false
            window.pendingPermissionGroupCount = 0
            window.pendingPermissionHost = null
            permissionPromptExpiry.stop()
            if (wasVisible) {
                window.restoreOverlayFocus()
            }
            return
        }
        var group = queue[0]
        window.pendingPermissionRequest = group.requests[0] || null
        window.pendingPermissionUi = group.ui
        window.pendingPermissionTabIndex = group.tabIndex
        window.pendingPermissionOrigin = group.origin
        window.pendingPermissionName = group.name
        window.pendingPermissionPrivate = group.privateProfile
        window.pendingPermissionGroupCount = group.requests.length
        window.pendingPermissionHost = group.host || window
        if (!wasVisible) {
            window.captureOverlayFocus(window.pendingPermissionHost)
        }
        window.permissionPromptVisible = true
        permissionPromptExpiry.restart()
    }

    function clearPermissionPrompt(denyRequest) {
        var queue = (window.permissionPromptQueue || []).slice(0)
        var group = queue.length > 0 ? queue.shift() : null
        if (denyRequest) {
            window.denyPermissionGroup(group)
        }
        window.permissionPromptQueue = queue
        window.showNextPermissionPrompt()
    }

    function removePermissionGroups(predicate, denyRequest) {
        var queue = window.permissionPromptQueue || []
        var kept = []
        for (var i = 0; i < queue.length; ++i) {
            var group = queue[i]
            if (predicate(group)) {
                if (denyRequest) {
                    window.denyPermissionGroup(group)
                }
            } else {
                kept.push(group)
            }
        }
        if (kept.length !== queue.length) {
            window.permissionPromptQueue = kept
            window.showNextPermissionPrompt()
        }
    }

    function queuePermissionPrompt(ui, request, tabIndex, privateProfile, hostWindow) {
        var origin = request.origin.toString()
        var name = window.permissionTypeName(request.permissionType)
        var key = window.permissionQueueKey(origin, name, privateProfile)
        var now = Date.now()
        var cooldowns = window.permissionCooldowns
        if (cooldowns[key] !== undefined && cooldowns[key] <= now) {
            delete cooldowns[key]
        }
        if (cooldowns[key] !== undefined) {
            window.resolveQtRequest(ui, request, "permission", "deny", [])
            ui.status_text = "Permission requests temporarily cooled down"
            return
        }
        var queue = (window.permissionPromptQueue || []).slice(0)
        var groupIndex = -1
        for (var i = 0; i < queue.length; ++i) {
            if (queue[i].key === key) {
                groupIndex = i
                break
            }
        }
        if (groupIndex < 0 && window.permissionQueueRequestCount(queue)
                >= window.permissionPromptQueueLimit) {
            window.resolveQtRequest(ui, request, "permission", "deny", [])
            if (Object.keys(cooldowns).length >= 64) {
                cooldowns = ({})
            }
            cooldowns[key] = now + 30000
            window.permissionCooldowns = cooldowns
            ui.status_text = "Permission queue full; repeated requests cooled down"
            return
        }
        if (groupIndex >= 0) {
            var grouped = queue[groupIndex]
            grouped.requests = grouped.requests.concat([request])
            queue[groupIndex] = grouped
        } else {
            queue.push({
                key: key,
                requests: [request],
                ui: ui,
                tabIndex: tabIndex,
                origin: origin,
                name: name,
                privateProfile: privateProfile,
                host: hostWindow || window
            })
        }
        window.permissionPromptQueue = queue
        if (!window.permissionPromptVisible) {
            window.showNextPermissionPrompt()
        } else if (groupIndex === 0) {
            window.pendingPermissionGroupCount = queue[0].requests.length
        }
    }

    function handlePermissionRequested(request, tabIndex) {
        if (!request || !request.isValid) {
            return
        }
        var permissionName = window.permissionTypeName(request.permissionType)
        if (permissionName.length === 0) {
            window.resolveQtRequest(browserUi, request, "permission", "deny", [])
            browserUi.status_text = "Blocked unsupported permission request"
            return
        }
        var origin = request.origin.toString()
        var decision = permissionName === "camera-and-microphone"
                ? "ask"
                : browserUi.permission_decision(origin, permissionName)
        if (permissionName === "camera-and-microphone") {
            var cameraDecision = browserUi.permission_decision(origin, "camera")
            var microphoneDecision = browserUi.permission_decision(origin, "microphone")
            if (cameraDecision === "deny" || microphoneDecision === "deny") {
                decision = "deny"
            } else if (cameraDecision === "allow" && microphoneDecision === "allow") {
                decision = "allow"
            } else {
                decision = "ask"
            }
        }
        if (decision === "allow") {
            window.resolveQtRequest(browserUi, request, "permission", "grant", [])
            browserUi.status_text = "Allowed " + window.permissionDisplayName(permissionName)
            return
        }
        if (decision === "deny") {
            window.resolveQtRequest(browserUi, request, "permission", "deny", [])
            browserUi.status_text = "Denied " + window.permissionDisplayName(permissionName)
            return
        }
        window.queuePermissionPrompt(
            browserUi, request, tabIndex, window.temporaryProfile, window)
    }

    function decidePermission(allow, lifetime) {
        var queue = window.permissionPromptQueue || []
        var group = queue.length > 0 ? queue[0] : null
        var ui = group && group.ui ? group.ui : browserUi
        if (!group || !group.requests || group.requests.length === 0) {
            window.clearPermissionPrompt(false)
            ui.status_text = "Permission request expired"
            return
        }
        var validCount = 0
        for (var i = 0; i < group.requests.length; ++i) {
            if (group.requests[i] && group.requests[i].isValid) {
                validCount += 1
            }
        }
        if (validCount === 0) {
            window.clearPermissionPrompt(false)
            ui.status_text = "Permission request expired"
            return
        }
        if (lifetime.length > 0 && !ui.remember_permission(
                    window.pendingPermissionOrigin,
                    window.pendingPermissionName,
                    allow ? "allow" : "deny",
                    lifetime)) {
            return
        }
        for (var j = 0; j < group.requests.length; ++j) {
            var request = group.requests[j]
            if (!request || !request.isValid) {
                continue
            }
            if (allow) {
                window.resolveQtRequest(ui, request, "permission", "grant", [])
            } else {
                window.resolveQtRequest(ui, request, "permission", "deny", [])
            }
        }
        ui.status_text = (allow ? "Allowed " : "Denied ")
                + window.permissionDisplayName(window.pendingPermissionName)
        window.clearPermissionPrompt(false)
    }

    function handleImmediatePermissionRequested(ui, request, privateProfile, hostWindow) {
        if (!request || !request.isValid) {
            return
        }
        var permissionName = window.permissionTypeName(request.permissionType)
        if (permissionName.length === 0) {
            window.resolveQtRequest(ui, request, "permission", "deny", [])
            ui.status_text = "Blocked unsupported permission request"
            return
        }
        var requestOrigin = request.origin.toString()
        var decision = permissionName === "camera-and-microphone"
                ? "ask"
                : ui.permission_decision(requestOrigin, permissionName)
        if (permissionName === "camera-and-microphone") {
            var cameraDecision = ui.permission_decision(requestOrigin, "camera")
            var microphoneDecision = ui.permission_decision(requestOrigin, "microphone")
            if (cameraDecision === "deny" || microphoneDecision === "deny") {
                decision = "deny"
            } else if (cameraDecision === "allow" && microphoneDecision === "allow") {
                decision = "allow"
            }
        }
        if (decision === "allow") {
            window.resolveQtRequest(ui, request, "permission", "grant", [])
            ui.status_text = "Allowed " + window.permissionDisplayName(permissionName)
        } else {
            if (decision === "ask") {
                window.queuePermissionPrompt(ui, request, -1, privateProfile, hostWindow)
            } else {
                window.resolveQtRequest(ui, request, "permission", "deny", [])
                ui.status_text = "Denied " + window.permissionDisplayName(permissionName)
            }
        }
    }

    function cancelPermissionForTab(tabIndex) {
        window.removePermissionGroups(function(group) {
            return group.tabIndex === tabIndex
        }, true)
    }

    function cancelPermissionForUi(ui) {
        window.removePermissionGroups(function(group) {
            return group.ui === ui
        }, true)
    }

    function permissionPendingForTab(tabIndex) {
        var queue = window.permissionPromptQueue || []
        for (var i = 0; i < queue.length; ++i) {
            if (queue[i].tabIndex === tabIndex) {
                return true
            }
        }
        return false
    }

    function clearDesktopMediaRequest(cancelRequest) {
        var request = window.pendingDesktopMediaRequest
        var hadRequest = !!request
        var ui = window.pendingDesktopMediaUi || browserUi
        window.pendingDesktopMediaRequest = null
        window.pendingDesktopMediaUi = null
        window.pendingDesktopMediaView = null
        window.pendingDesktopMediaHost = null
        window.pendingDesktopMediaWaitingForPortal = false
        window.pendingDesktopMediaPortalDeadlineMs = 0
        window.desktopMediaPromptVisible = false
        window.desktopMediaOrigin = ""
        if (hadRequest) {
            window.restoreOverlayFocus()
        }
        if (cancelRequest && request) {
            window.resolveQtRequest(ui, request, "desktop-media", "cancel", [])
            if (ui) {
                ui.status_text = "Screen sharing cancelled"
            }
        }
    }

    function maybeOpenPendingDesktopMediaRequest() {
        if (!window.pendingDesktopMediaWaitingForPortal
                || !window.pendingDesktopMediaRequest) {
            return
        }
        if (Date.now() >= window.pendingDesktopMediaPortalDeadlineMs) {
            var timedOutUi = window.pendingDesktopMediaUi || browserUi
            window.clearDesktopMediaRequest(true)
            if (timedOutUi) {
                timedOutUi.status_text =
                        "Desktop portal check timed out; screen sharing cancelled"
            }
            return
        }
        var ui = window.pendingDesktopMediaUi || browserUi
        var status = window.desktopPortalCapabilityStatus(ui, "screen_cast")
        if (status === "pending" || status === "not-probed") {
            return
        }
        if (status !== "available") {
            window.clearDesktopMediaRequest(true)
            ui.status_text = "Required ScreenCast portal unavailable; screen sharing cancelled"
            return
        }
        window.pendingDesktopMediaWaitingForPortal = false
        window.desktopMediaPromptVisible = true
        ui.status_text = "Choose a screen or window to share"
    }

    function handleDesktopMediaRequested(ui, view, request, hostWindow) {
        if (!request) {
            return
        }
        if (window.pendingDesktopMediaRequest) {
            window.clearDesktopMediaRequest(true)
        }
        window.pendingDesktopMediaRequest = request
        window.pendingDesktopMediaUi = ui
        window.pendingDesktopMediaView = view
        window.pendingDesktopMediaHost = hostWindow || window
        window.captureOverlayFocus(window.pendingDesktopMediaHost, view)
        window.desktopMediaOrigin = view && view.url && view.url.host
                ? window.boundedPageDialogText(view.url.host)
                : "opaque or unavailable origin"
        var requestUi = window.pendingDesktopMediaUi || browserUi
        if (window.desktopPortalMode(requestUi) === "required") {
            var portalStatus = window.desktopPortalCapabilityStatus(requestUi, "screen_cast")
            if (portalStatus === "not-probed") {
                browserUi.probe_desktop_portals()
                portalStatus = window.desktopPortalCapabilityStatus(requestUi, "screen_cast")
            }
            if (portalStatus === "pending" || portalStatus === "not-probed") {
                window.pendingDesktopMediaWaitingForPortal = true
                window.pendingDesktopMediaPortalDeadlineMs = Date.now() + 5000
                requestUi.status_text = "Checking required ScreenCast portal…"
                return
            }
            if (portalStatus !== "available") {
                window.clearDesktopMediaRequest(true)
                requestUi.status_text = "Required ScreenCast portal unavailable; screen sharing cancelled"
                return
            }
        }
        window.pendingDesktopMediaWaitingForPortal = false
        window.desktopMediaPromptVisible = true
        requestUi.status_text = "Choose a screen or window to share"
    }

    function selectDesktopScreen(index) {
        var request = window.pendingDesktopMediaRequest
        var ui = window.pendingDesktopMediaUi || browserUi
        if (!request || !request.screensModel) {
            window.clearDesktopMediaRequest(true)
            return
        }
        window.resolveQtRequest(
            ui, request, "desktop-media", "selectScreen", [request.screensModel.index(index, 0)])
        window.recordCaptureSession(ui, window.pendingDesktopMediaView,
                                    window.pendingDesktopMediaHost,
                                    window.desktopMediaOrigin)
        window.clearDesktopMediaRequest(false)
        ui.status_text = "Screen sharing source selected"
    }

    function selectDesktopWindow(index) {
        var request = window.pendingDesktopMediaRequest
        var ui = window.pendingDesktopMediaUi || browserUi
        if (!request || !request.windowsModel) {
            window.clearDesktopMediaRequest(true)
            return
        }
        window.resolveQtRequest(
            ui, request, "desktop-media", "selectWindow", [request.windowsModel.index(index, 0)])
        window.recordCaptureSession(ui, window.pendingDesktopMediaView,
                                    window.pendingDesktopMediaHost,
                                    window.desktopMediaOrigin)
        window.clearDesktopMediaRequest(false)
        ui.status_text = "Window sharing source selected"
    }

    function clearDesktopMediaForView(view) {
        if (window.pendingDesktopMediaView === view) {
            window.clearDesktopMediaRequest(true)
        }
    }

    function recordCaptureSession(ui, view, hostWindow, origin) {
        if (!view || !hostWindow) {
            return
        }
        var sessions = (window.captureSessions || []).filter(function(session) {
            return session && session.view !== view
        })
        var sequence = window.captureSessionSequence + 1
        window.captureSessionSequence = sequence
        sessions.push({
            id: sequence,
            ui: ui,
            view: view,
            host: hostWindow,
            origin: window.boundedPageDialogText(origin || "opaque or unavailable origin"),
            status: "active"
        })
        window.captureSessions = sessions.slice(-8)
    }

    function captureSessionForView(view) {
        var sessions = window.captureSessions || []
        for (var i = 0; i < sessions.length; ++i) {
            if (sessions[i] && sessions[i].view === view) {
                return sessions[i]
            }
        }
        return null
    }

    function noteCaptureNavigation(view) {
        var session = window.captureSessionForView(view)
        if (!session) {
            return
        }
        var sessions = (window.captureSessions || []).slice(0)
        for (var i = 0; i < sessions.length; ++i) {
            if (sessions[i] && sessions[i].view === view) {
                sessions[i].status = "ended-by-navigation"
            }
        }
        window.captureSessions = sessions
        session.ui.status_text = "Capture ended with page navigation"
    }

    function stopCaptureSession(id) {
        var sessions = (window.captureSessions || []).slice(0)
        var target = null
        for (var i = 0; i < sessions.length; ++i) {
            if (sessions[i] && sessions[i].id === id) {
                target = sessions[i]
                break
            }
        }
        if (!target || !target.view) {
            return
        }
        target.status = "stop-requested"
        window.captureSessions = sessions
        target.ui.status_text = "Capture stop requested; reloading the page"
        target.view.reload()
    }

    function dismissCaptureSession(id) {
        window.captureSessions = (window.captureSessions || []).filter(function(session) {
            return session && session.id !== id
        })
    }

    function clearCaptureSessionForView(view) {
        window.captureSessions = (window.captureSessions || []).filter(function(session) {
            return session && session.view !== view
        })
    }

    function clearCaptureSessionsForHost(hostWindow) {
        window.captureSessions = (window.captureSessions || []).filter(function(session) {
            return session && session.host !== hostWindow
        })
    }

    function permissionOriginForView(view) {
        var raw = view && view.url ? view.url.toString() : ""
        var schemeEnd = raw.indexOf("://")
        if (schemeEnd < 1) {
            return ""
        }
        var authorityStart = schemeEnd + 3
        var authorityEnd = raw.length
        for (var i = 0; i < 3; ++i) {
            var separator = raw.indexOf(["/", "?", "#"][i], authorityStart)
            if (separator >= 0 && separator < authorityEnd) {
                authorityEnd = separator
            }
        }
        var authority = raw.slice(authorityStart, authorityEnd)
        var at = authority.lastIndexOf("@")
        if (at >= 0) {
            authority = authority.slice(at + 1)
        }
        return raw.slice(0, schemeEnd).toLowerCase() + "://"
                + authority.toLowerCase()
    }

    function reloadViewsForPermission(origin) {
        var expected = String(origin || "").toLowerCase()
        var candidates = []
        function addCandidate(view, ui) {
            if (!view || window.permissionOriginForView(view) !== expected) {
                return
            }
            for (var i = 0; i < candidates.length; ++i) {
                if (candidates[i].view === view) {
                    return
                }
            }
            candidates.push({ view: view, ui: ui || browserUi })
        }

        addCandidate(window.activeWebView(), browserUi)
        var entries = window.browserWindowRegistry || []
        for (var i = 0; i < entries.length; ++i) {
            var entry = entries[i]
            if (entry && entry.host !== window) {
                addCandidate(entry.view, entry.ui)
            }
        }
        var sessions = window.captureSessions || []
        for (var j = 0; j < sessions.length; ++j) {
            var session = sessions[j]
            if (session && session.status === "active") {
                addCandidate(session.view, session.ui)
            }
        }

        window.removePermissionGroups(function(group) {
            return group && String(group.origin || "").toLowerCase() === expected
        }, true)
        for (var k = 0; k < candidates.length; ++k) {
            window.noteCaptureNavigation(candidates[k].view)
            candidates[k].view.reload()
        }
        return candidates.length
    }

    function rendererTerminationName(status) {
        if (status === WebEngineView.CrashedTerminationStatus) {
            return "crashed"
        }
        if (status === WebEngineView.KilledTerminationStatus) {
            return "killed"
        }
        if (status === WebEngineView.AbnormalTerminationStatus) {
            return "abnormal termination"
        }
        return "terminated"
    }

    function handleRendererProcessTerminated(ui, view, hostWindow, tabIndex,
                                              terminationStatus, exitCode) {
        if (!view || terminationStatus === WebEngineView.NormalTerminationStatus) {
            return
        }
        if (ui && ui.note_renderer_process_terminated) {
            ui.note_renderer_process_terminated(tabIndex)
        }
        var now = Date.now()
        var count = view.rendererFailureAt > 0
                && now - view.rendererFailureAt <= 60000
                ? view.rendererFailureCount + 1 : 1
        view.rendererFailureAt = now
        view.rendererFailureCount = count
        view.rendererFailed = true
        view.rendererFailureSafeUrl = ui.display_url || "about:blank"
        view.rendererFailureReason = window.rendererTerminationName(terminationStatus)
        view.rendererFailureExitCode = exitCode
        window.rendererFailureView = view
        window.rendererFailureHost = hostWindow || window
        window.rendererFailureUi = ui
        window.rendererFailureTabIndex = tabIndex
        window.rendererFailureSafeUrl = view.rendererFailureSafeUrl
        window.rendererFailureReason = view.rendererFailureReason
        window.rendererFailureExitCode = exitCode
        window.rendererFailureCount = count
        var visible = window.rendererFailureHost !== window
                || tabIndex === browserUi.active_tab_index
        if (visible) {
            window.captureOverlayFocus(window.rendererFailureHost || window, view)
        }
        window.rendererFailureVisible = visible
        ui.status_text = visible
                ? "Renderer " + view.rendererFailureReason
                : "Renderer failed in background tab"
    }

    function showRendererFailureForTab(tabIndex) {
        var view = tabViewAt(tabIndex)
        if (!view || !view.rendererFailed) {
            return
        }
        window.rendererFailureView = view
        window.rendererFailureHost = window
        window.rendererFailureUi = browserUi
        window.rendererFailureTabIndex = tabIndex
        window.rendererFailureSafeUrl = view.rendererFailureSafeUrl
        window.rendererFailureReason = view.rendererFailureReason
        window.rendererFailureExitCode = view.rendererFailureExitCode
        window.rendererFailureCount = view.rendererFailureCount
        window.captureOverlayFocus(window, view)
        window.rendererFailureVisible = true
    }

    function clearRendererFailureForView(view) {
        if (!view) {
            return
        }
        view.rendererFailed = false
        if (window.rendererFailureView === view) {
            var wasVisible = window.rendererFailureVisible
            window.rendererFailureVisible = false
            window.rendererFailureView = null
            window.rendererFailureHost = null
            window.rendererFailureUi = null
            window.rendererFailureTabIndex = -1
            if (wasVisible) {
                window.restoreOverlayFocus()
            }
        }
    }

    function reloadRendererFailure() {
        var view = window.rendererFailureView
        var ui = window.rendererFailureUi || browserUi
        if (!view) {
            return
        }
        if (ui && ui.prepare_renderer_recovery
                && !ui.prepare_renderer_recovery(window.rendererFailureTabIndex)) {
            ui.status_text = "Renderer recovery target is stale"
            return
        }
        window.rendererFailureVisible = false
        view.rendererFailed = false
        view.reload()
        window.restoreOverlayFocus()
        ui.status_text = window.rendererFailureCount > 1
                ? "Renderer reload requested; repeated crashes will not auto-reload"
                : "Renderer reload requested"
    }

    function closeRendererFailure() {
        var view = window.rendererFailureView
        var host = window.rendererFailureHost
        var index = window.rendererFailureTabIndex
        if (host === window && index >= 0) {
            view.rendererFailed = false
            if (window.closeTabAtIndex(index)) {
                window.executePendingEngineAction()
            }
        } else if (host && host.close) {
            host.close()
        }
        if (view && host !== window) {
            view.rendererFailed = false
        }
        window.rendererFailureVisible = false
        window.restoreOverlayFocus()
    }

    function showRendererFailureDiagnostics() {
        var ui = window.rendererFailureUi || browserUi
        var payload = ui.refresh_site_status()
        if (payload && payload.length > 0) {
            try {
                window.siteLedgerData = JSON.parse(payload)
                if (!window.siteLedgerVisible) {
                    window.openInternalSurface()
                }
                window.siteLedgerVisible = true
                return
            } catch (error) {
                // Fall through to the explicit status below.
            }
        }
        ui.status_text = "Renderer diagnostics unavailable"
    }

    function restartSoftwareRendering() {
        var ui = window.rendererFailureUi || browserUi
        if (window.softwareRendering) {
            ui.status_text = "Software rendering is already enabled"
            return
        }
        if (!browserUi.restart_software_rendering(
                    window.instanceLockPath,
                    window.storageBasePath,
                    window.temporaryProfile,
                    window.safeMode,
                    window.userscriptsOff,
                    window.instanceSelector)) {
            ui.status_text = "Software-rendering restart could not be started"
            return
        }
        ui.status_text = "Restarting with software rendering"
        window.beginQuitRequest()
    }

    Timer {
        id: permissionPromptExpiry
        interval: 30000
        repeat: false
        onTriggered: {
            var ui = window.pendingPermissionUi || browserUi
            window.clearPermissionPrompt(true)
            ui.status_text = "Permission request expired"
        }
    }

    FerricPageDialog {
        id: pageDialogPopup
        browserWindow: window
    }

    Popup {
        id: clientCertificatePopup
        parent: Overlay.overlay
        modal: true
        focus: true
        closePolicy: Popup.NoAutoClose
        width: Math.min(760 * window.chromeScale, window.width - 48)
        height: Math.min(520 * window.chromeScale, window.height - 32)
        padding: 14
        x: Math.round((window.width - width) / 2)
        y: Math.round((window.height - height) / 2)

        background: Rectangle {
            color: window.panelColor
            border.color: window.accentColor
            radius: 4
        }

        contentItem: ColumnLayout {
            focus: true
            Accessible.role: Accessible.Dialog
            Accessible.name: "Client certificate selection"
            spacing: 10

            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    window.rejectClientCertificate()
                    event.accepted = true
                }
            }

            Label {
                Layout.fillWidth: true
                text: "Select a client certificate"
                color: window.primaryTextColor
                font.bold: true
                Accessible.name: "Client certificate selection title"
            }

            Label {
                Layout.fillWidth: true
                text: "Host: " + (window.pendingClientCertificateHost
                                   || "opaque or unavailable host")
                color: window.mutedTextColor
                elide: Text.ElideMiddle
                Accessible.name: "Client certificate host"
            }

            Label {
                Layout.fillWidth: true
                text: "Choose one of the identities offered by the browser engine. Private keys are never exposed here."
                color: window.secondaryTextColor
                wrapMode: Text.WordWrap
                Accessible.name: "Client certificate guidance"
            }

            ListView {
                id: clientCertificateList
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: window.pendingClientCertificateOptions
                Accessible.role: Accessible.List
                Accessible.name: "Available client certificates"

                delegate: Rectangle {
                    required property var modelData
                    width: clientCertificateList.width
                    height: Math.max(72 * window.chromeScale, 64)
                    color: index % 2 === 0 ? window.surfaceColor : window.panelColor
                    border.color: window.borderColor
                    border.width: 1

                    RowLayout {
                        anchors.fill: parent
                        anchors.margins: 8
                        spacing: 10

                        ColumnLayout {
                            Layout.fillWidth: true
                            Label {
                                Layout.fillWidth: true
                                text: modelData.subject
                                color: window.primaryTextColor
                                elide: Text.ElideMiddle
                                Accessible.name: "Certificate subject"
                            }
                            Label {
                                Layout.fillWidth: true
                                text: "Issuer: " + modelData.issuer
                                      + (modelData.selfSigned ? " (self-signed)" : "")
                                color: window.mutedTextColor
                                elide: Text.ElideMiddle
                                Accessible.name: "Certificate issuer"
                            }
                        }

                        Button {
                            text: "Use"
                            Accessible.name: "Use this client certificate"
                            onClicked: window.acceptClientCertificate(modelData.index)
                        }
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                Item { Layout.fillWidth: true }
                Button {
                    text: "Cancel"
                    Accessible.name: "Cancel client certificate selection"
                    onClicked: window.rejectClientCertificate()
                }
            }
        }
    }

    Popup {
        id: certificateErrorPopup
        parent: Overlay.overlay
        modal: true
        focus: true
        closePolicy: Popup.NoAutoClose
        width: Math.min(680 * window.chromeScale, window.width - 48)
        height: Math.min(420 * window.chromeScale, window.height - 32)
        padding: 14
        x: Math.round((window.width - width) / 2)
        y: Math.round((window.height - height) / 2)

        background: Rectangle {
            color: window.panelColor
            border.color: window.warningColor
            radius: 4
        }

        contentItem: ColumnLayout {
            focus: true
            Accessible.role: Accessible.Dialog
            Accessible.name: "TLS certificate warning"
            spacing: 10

            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    window.rejectCertificateError()
                    event.accepted = true
                } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                    window.acceptCertificateError()
                    event.accepted = true
                }
            }

            Label {
                Layout.fillWidth: true
                text: "Certificate cannot be verified"
                color: window.warningColor
                font.bold: true
                Accessible.name: "TLS certificate warning title"
            }

            Label {
                Layout.fillWidth: true
                text: "Host: " + (window.pendingCertificateErrorHost
                                   || "opaque or unavailable host")
                color: window.mutedTextColor
                elide: Text.ElideMiddle
                Accessible.name: "TLS certificate host"
            }

            Label {
                Layout.fillWidth: true
                text: window.pendingCertificateErrorDescription
                color: window.primaryTextColor
                wrapMode: Text.WordWrap
                maximumLineCount: 12
                elide: Text.ElideRight
                Accessible.name: "TLS certificate error"
            }

            Label {
                Layout.fillWidth: true
                text: "Only continue if you recognize this host and understand the risk. This exception applies to this request only."
                color: window.secondaryTextColor
                wrapMode: Text.WordWrap
                Accessible.name: "TLS certificate warning guidance"
            }

            RowLayout {
                Layout.fillWidth: true
                Item { Layout.fillWidth: true }
                Button {
                    text: "Go back"
                    Accessible.name: "Reject TLS certificate"
                    onClicked: window.rejectCertificateError()
                }
                Button {
                    text: "Accept once"
                    Accessible.name: "Accept TLS certificate for this request only"
                    onClicked: window.acceptCertificateError()
                }
            }
        }
    }

    Popup {
        id: webAuthPopup
        parent: Overlay.overlay
        modal: true
        focus: true
        closePolicy: Popup.NoAutoClose
        width: Math.min(680 * window.chromeScale, window.width - 48)
        height: Math.min(520 * window.chromeScale, window.height - 32)
        padding: 14
        x: Math.round((window.width - width) / 2)
        y: Math.round((window.height - height) / 2)

        background: Rectangle {
            color: window.panelColor
            border.color: window.privateColor
            radius: 4
        }

        contentItem: ColumnLayout {
            focus: true
            Accessible.role: Accessible.Dialog
            Accessible.name: "WebAuthn security-key prompt"
            spacing: 10

            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    window.cancelWebAuth()
                    event.accepted = true
                } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                    if (window.pendingWebAuthState === WebEngineWebAuthUxRequest.CollectPin) {
                        window.submitWebAuthPin()
                        event.accepted = true
                    }
                }
            }

            Label {
                Layout.fillWidth: true
                text: "WebAuthn security-key request"
                color: window.privateColor
                font.bold: true
                Accessible.name: "WebAuthn title"
            }

            Label {
                Layout.fillWidth: true
                text: "Relying party: " + (window.pendingWebAuthRelyingParty
                                             || "opaque or unavailable relying party")
                color: window.mutedTextColor
                elide: Text.ElideMiddle
                Accessible.name: "WebAuthn relying party"
            }

            Label {
                Layout.fillWidth: true
                text: window.webAuthStatusText
                color: window.primaryTextColor
                wrapMode: Text.WordWrap
                Accessible.name: "WebAuthn status"
            }

            ListView {
                id: webAuthAccountList
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: window.pendingWebAuthState === WebEngineWebAuthUxRequest.SelectAccount
                clip: true
                model: window.pendingWebAuthUserNames
                Accessible.role: Accessible.List
                Accessible.name: "WebAuthn accounts"

                delegate: Button {
                    required property var modelData
                    width: webAuthAccountList.width
                    text: modelData
                    Accessible.name: "Use WebAuthn account " + modelData
                    onClicked: window.selectWebAuthAccount(modelData)
                }
            }

            Label {
                Layout.fillWidth: true
                visible: window.pendingWebAuthState === WebEngineWebAuthUxRequest.CollectPin
                text: "PIN attempts remaining: "
                      + (window.pendingWebAuthRequest
                         ? window.pendingWebAuthRequest.pinRequest.remainingAttempts : 0)
                      + "; minimum length: "
                      + (window.pendingWebAuthRequest
                         ? window.pendingWebAuthRequest.pinRequest.minPinLength : 0)
                color: window.mutedTextColor
                Accessible.name: "WebAuthn PIN guidance"
            }

            TextField {
                id: webAuthPinField
                Layout.fillWidth: true
                visible: window.pendingWebAuthState === WebEngineWebAuthUxRequest.CollectPin
                echoMode: TextInput.Password
                placeholderText: "Security-key PIN"
                Accessible.name: "WebAuthn PIN"
                Accessible.role: Accessible.EditableText
                Accessible.editable: true
                onVisibleChanged: if (visible) forceActiveFocus()
            }

            Label {
                Layout.fillWidth: true
                visible: window.pendingWebAuthState === WebEngineWebAuthUxRequest.FinishTokenCollection
                text: "Follow the security-key instruction, then wait for completion."
                color: window.secondaryTextColor
                wrapMode: Text.WordWrap
                Accessible.name: "WebAuthn security-key instruction"
            }

            Label {
                Layout.fillWidth: true
                visible: window.pendingWebAuthState === WebEngineWebAuthUxRequest.RequestFailed
                text: "The authenticator reported a failure. You may retry or cancel."
                color: window.errorColor
                wrapMode: Text.WordWrap
                Accessible.name: "WebAuthn failure guidance"
            }

            RowLayout {
                Layout.fillWidth: true
                Item { Layout.fillWidth: true }
                Button {
                    text: "Cancel"
                    Accessible.name: "Cancel WebAuthn request"
                    onClicked: window.cancelWebAuth()
                }
                Button {
                    visible: window.pendingWebAuthState === WebEngineWebAuthUxRequest.CollectPin
                    text: "Submit PIN"
                    Accessible.name: "Submit WebAuthn PIN"
                    onClicked: window.submitWebAuthPin()
                }
                Button {
                    visible: window.pendingWebAuthState === WebEngineWebAuthUxRequest.RequestFailed
                    text: "Retry"
                    Accessible.name: "Retry WebAuthn request"
                    onClicked: window.retryWebAuth()
                }
            }
        }
    }

    Popup {
        id: contextMenuPopup
        parent: Overlay.overlay
        modal: true
        focus: true
        closePolicy: Popup.NoAutoClose
        width: Math.min(420 * window.chromeScale, window.width - 48)
        height: Math.min(560 * window.chromeScale, window.height - 32)
        padding: 8
        x: Math.round((window.width - width) / 2)
        y: Math.round((window.height - height) / 2)

        background: Rectangle {
            color: window.panelColor
            border.color: window.accentColor
            radius: 4
        }

        contentItem: ColumnLayout {
            focus: true
            Accessible.role: Accessible.PopupMenu
            Accessible.name: "Web content context menu"

            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    window.clearContextMenuRequest()
                    event.accepted = true
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 4
                Accessible.role: Accessible.List
                Accessible.name: "Switcher scopes"
                Repeater {
                    model: ["all", "tabs", "windows", "contexts", "commands",
                        "actions", "history", "marks", "sessions", "downloads", "closed"]
                    delegate: Button {
                        text: modelData
                        checkable: true
                        checked: modelData === window.switcherScope
                        Accessible.role: Accessible.PageTab
                        Accessible.name: "Switcher scope " + modelData
                        Accessible.selected: checked
                        onClicked: window.setSwitcherScope(modelData)
                    }
                }
            }

            ListView {
                id: contextMenuList
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: window.contextMenuItems
                Accessible.role: Accessible.List
                Accessible.name: "Context menu actions"

                delegate: Button {
                    required property var modelData
                    width: contextMenuList.width
                    text: modelData.label
                    Accessible.role: Accessible.MenuItem
                    Accessible.name: modelData.label
                    onClicked: window.activateContextMenuItem(modelData)
                }
            }

            Button {
                Layout.fillWidth: true
                text: "Close"
                Accessible.name: "Close context menu"
                onClicked: window.clearContextMenuRequest()
            }
        }
    }

    function attachDownloadStateUpdates(download, id) {
        download.stateChanged.connect(function() {
            browserUi.update_download(id, window.downloadStateName(download), download.receivedBytes, false)
            if (window.downloadManagerVisible) {
                window.refreshDownloads()
            }
        })
        download.isPausedChanged.connect(function() {
            browserUi.update_download(id, window.downloadStateName(download), download.receivedBytes, false)
            if (window.downloadManagerVisible) {
                window.refreshDownloads()
            }
        })
    }

    function downloadStagingDirectory(ui, id) {
        return ui && ui.download_staging_directory
                ? String(ui.download_staging_directory(id)) : ""
    }

    function acceptDownloadRequest(download, id, directory, suggestedName) {
        var finalName = browserUi.accept_download(id, directory, suggestedName)
        if (finalName.length === 0) {
            window.resolveQtRequest(browserUi, download, "download", "cancel", [])
            delete window.activeDownloads[id]
            return false
        }
        var stagingDirectory = window.downloadStagingDirectory(browserUi, id)
        if (stagingDirectory.length === 0) {
            window.resolveQtRequest(browserUi, download, "download", "cancel", [])
            browserUi.update_download(id, "cancelled", download.receivedBytes, true)
            return false
        }
        download.downloadDirectory = stagingDirectory
        download.downloadFileName = finalName
        window.attachDownloadStateUpdates(download, id)
        window.resolveQtRequest(browserUi, download, "download", "accept", [])
        browserUi.update_download(id, "in-progress", 0, false)
        if (window.downloadManagerVisible) {
            window.refreshDownloads()
        }
        return true
    }

    function acceptPendingDownload() {
        var id = window.pendingDownloadId
        var download = window.pendingDownloadRequests[id]
        if (!download) {
            window.pendingDownloadId = ""
            window.restoreOverlayFocus()
            return
        }
        var selectedPath = downloadChooser.selectedFile.toLocalFile()
        var finalName = browserUi.accept_download_path(id, selectedPath)
        if (finalName.length === 0) {
            window.resolveQtRequest(browserUi, download, "download", "cancel", [])
            browserUi.update_download(id, "cancelled", download.receivedBytes, true)
        } else {
            var separator = selectedPath.lastIndexOf("/")
            var directory = separator > 0 ? selectedPath.slice(0, separator) : "/"
            var stagingDirectory = window.downloadStagingDirectory(browserUi, id)
            if (stagingDirectory.length === 0) {
                window.resolveQtRequest(browserUi, download, "download", "cancel", [])
                browserUi.update_download(id, "cancelled", download.receivedBytes, true)
                delete window.pendingDownloadRequests[id]
                window.pendingDownloadId = ""
                window.pendingDownloadSuggestedName = ""
                window.restoreOverlayFocus()
                return
            }
            download.downloadDirectory = stagingDirectory
            download.downloadFileName = finalName
            window.activeDownloads[id] = download
            window.attachDownloadStateUpdates(download, id)
            window.resolveQtRequest(browserUi, download, "download", "accept", [])
            browserUi.update_download(id, "in-progress", 0, false)
        }
        delete window.pendingDownloadRequests[id]
        window.pendingDownloadId = ""
        window.pendingDownloadSuggestedName = ""
        if (window.downloadManagerVisible) {
            window.refreshDownloads()
        }
        window.restoreOverlayFocus()
    }

    function cancelPendingDownload() {
        var id = window.pendingDownloadId
        var download = window.pendingDownloadRequests[id]
        if (download) {
            window.resolveQtRequest(browserUi, download, "download", "cancel", [])
            browserUi.update_download(id, "cancelled", download.receivedBytes, true)
        }
        delete window.pendingDownloadRequests[id]
        window.pendingDownloadId = ""
        window.pendingDownloadSuggestedName = ""
        if (window.downloadManagerVisible) {
            window.refreshDownloads()
        }
        window.restoreOverlayFocus()
    }

    function handleDownloadRequested(download) {
        var id = String(download.id)
        var safeName = browserUi.offer_download(id, download.url.toString(), download.suggestedFileName)
        var requestedPath = download.savePageFormat === WebEngineDownloadRequest.MimeHtmlSaveFormat
                ? browserUi.take_save_page_path() : ""
        if (requestedPath.length > 0) {
            var requestedName = browserUi.accept_download_path(id, requestedPath)
            if (requestedName.length === 0) {
                window.resolveQtRequest(browserUi, download, "download", "cancel", [])
                browserUi.update_download(id, "cancelled", download.receivedBytes, true)
                return
            }
            var requestedSeparator = requestedPath.lastIndexOf("/")
            var requestedDirectory = requestedSeparator > 0
                    ? requestedPath.slice(0, requestedSeparator) : "/"
            window.activeDownloads[id] = download
            var stagingDirectory = window.downloadStagingDirectory(browserUi, id)
            if (stagingDirectory.length === 0) {
                window.resolveQtRequest(browserUi, download, "download", "cancel", [])
                browserUi.update_download(id, "cancelled", download.receivedBytes, true)
                return
            }
            download.downloadDirectory = stagingDirectory
            download.downloadFileName = requestedName
            window.attachDownloadStateUpdates(download, id)
            window.resolveQtRequest(browserUi, download, "download", "accept", [])
            browserUi.update_download(id, "in-progress", 0, false)
            return
        }
        var directory = browserUi.default_download_directory()
        if (!window.downloadsAskDestination()) {
            window.activeDownloads[id] = download
            window.acceptDownloadRequest(download, id, directory, safeName)
            return
        }
        if (window.pendingDownloadId.length > 0) {
            window.resolveQtRequest(browserUi, download, "download", "cancel", [])
            browserUi.update_download(id, "cancelled", download.receivedBytes, true)
            return
        }
        window.pendingDownloadRequests[id] = download
        window.pendingDownloadId = id
        window.pendingDownloadSuggestedName = safeName
        window.captureOverlayFocus(window, window.activeWebView())
        browserUi.update_download(id, "selecting-destination", 0, false)
        downloadChooser.currentFile = window.fileUrlForPath(directory + "/" + safeName)
        downloadChooser.open()
    }

    function handleDownloadFinished(download) {
        var id = String(download.id)
        var state = window.downloadStateName(download)
        if (state === "completed" && !browserUi.finalize_download(id)) {
            state = "interrupted"
        } else if (state !== "completed") {
            browserUi.discard_download_staging(id)
        }
        browserUi.update_download(id, state, download.receivedBytes, true)
        delete window.activeDownloads[id]
        delete window.pendingDownloadRequests[id]
        if (window.pendingDownloadId === id) {
            window.pendingDownloadId = ""
            window.pendingDownloadSuggestedName = ""
        }
        if (window.downloadManagerVisible) {
            window.refreshDownloads()
        }
    }

    Component {
        id: popupWindowComponent

        ApplicationWindow {
            id: popupWindow
            width: 1024
            height: 720
            visible: true
            title: popupWindow.popupContextName.length > 0
                   ? "Ferric Browser popup · context "
                     + (popupWindow.popupContextLabel.length > 0
                        ? popupWindow.popupContextLabel : popupWindow.popupContextName)
                   : "Ferric Browser popup"
            color: window.backgroundColor
            font: window.font
            palette: window.palette
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
                popupView.runJavaScript(window.shutdownPageProbeScript(), function(result) {
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
                        || window.hasActiveShutdownRequestsFor(
                            popupWindow.popupPermissionUi || browserUi, popupWindow)) {
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
                        window.resolveQtRequest(ui, pendingDownload, "download", "cancel", [])
                        ui.update_download(
                            String(pendingKey), "cancelled", pendingDownload.receivedBytes, true)
                    }
                }
                popupWindow.pendingDownloadRequests = ({})
                for (var key in popupWindow.activeDownloads) {
                    var download = popupWindow.activeDownloads[key]
                    if (download && !download.isFinished) {
                        window.resolveQtRequest(ui, download, "download", "cancel", [])
                    }
                }
                popupWindow.activeDownloads = ({})
                window.cancelShutdownRequestsFor(
                            popupWindow.popupPermissionUi || browserUi, popupWindow)
                popupWindow.beginQuitRequest()
            }

            Rectangle {
                anchors.centerIn: parent
                width: Math.min(560, popupWindow.width - 80)
                height: Math.min(190 * window.chromeScale, popupWindow.height - 32)
                z: 100
                visible: popupWindow.windowShutdownPromptVisible
                color: window.panelColor
                border.color: window.warningColor
                border.width: 2

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 16
                    spacing: 12
                    Label {
                        Layout.fillWidth: true
                        text: "Popup browser work is still running"
                        color: window.primaryTextColor
                        font.bold: true
                    }
                    Label {
                        Layout.fillWidth: true
                        text: "Finish or cancel active popup work before closing."
                        color: window.secondaryTextColor
                        wrapMode: Text.WordWrap
                    }
                    RowLayout {
                        Layout.alignment: Qt.AlignRight
                        Button {
                            text: "Keep window open"
                            Accessible.name: "Keep popup window open"
                            onClicked: {
                                popupWindow.windowShutdownPromptVisible = false
                                window.abortApplicationShutdown()
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
                height: Math.min(220 * window.chromeScale, popupWindow.height - 32)
                z: 100
                visible: popupWindow.windowShutdownPagePromptVisible
                color: window.panelColor
                border.color: window.warningColor
                border.width: 2

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 16
                    spacing: 12
                    Label {
                        Layout.fillWidth: true
                        text: "Popup page state may be lost"
                        color: window.primaryTextColor
                        font.bold: true
                    }
                    Label {
                        Layout.fillWidth: true
                        text: popupWindow.windowShutdownPagePromptReason
                              + " Close anyway may lose that state."
                        color: window.secondaryTextColor
                        wrapMode: Text.WordWrap
                    }
                    RowLayout {
                        Layout.alignment: Qt.AlignRight
                        Button {
                            text: "Keep window open"
                            Accessible.name: "Keep popup window open"
                            onClicked: {
                                popupWindow.cancelPageStateProbe()
                                window.abortApplicationShutdown()
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
                return popupWindow.popupPermissionUi || browserUi
            }

            function attachPopupDownloadStateUpdates(download, id) {
                download.stateChanged.connect(function() {
                    popupWindow.popupDownloadUi().update_download(
                        id, window.downloadStateName(download), download.receivedBytes, false)
                })
                download.isPausedChanged.connect(function() {
                    popupWindow.popupDownloadUi().update_download(
                        id, window.downloadStateName(download), download.receivedBytes, false)
                })
            }

            function acceptPendingDownload() {
                var id = popupWindow.pendingDownloadId
                var download = popupWindow.pendingDownloadRequests[id]
                var ui = popupWindow.popupDownloadUi()
                var selectedPath = popupDownloadChooser.selectedFile.toLocalFile()
                if (!download) {
                    popupWindow.pendingDownloadId = ""
                    window.restoreOverlayFocus()
                    return
                }
                var finalName = ui.accept_download_path(id, selectedPath)
                if (finalName.length === 0) {
                    window.resolveQtRequest(ui, download, "download", "cancel", [])
                    ui.update_download(id, "cancelled", download.receivedBytes, true)
                } else {
                    var separator = selectedPath.lastIndexOf("/")
                    var directory = separator > 0 ? selectedPath.slice(0, separator) : "/"
                    var stagingDirectory = window.downloadStagingDirectory(ui, id)
                    if (stagingDirectory.length === 0) {
                        window.resolveQtRequest(ui, download, "download", "cancel", [])
                        ui.update_download(id, "cancelled", download.receivedBytes, true)
                        delete popupWindow.pendingDownloadRequests[id]
                        popupWindow.pendingDownloadId = ""
                        popupWindow.pendingDownloadSuggestedName = ""
                        window.restoreOverlayFocus()
                        return
                    }
                    download.downloadDirectory = stagingDirectory
                    download.downloadFileName = finalName
                    popupWindow.activeDownloads[id] = download
                    popupWindow.attachPopupDownloadStateUpdates(download, id)
                    window.resolveQtRequest(ui, download, "download", "accept", [])
                    ui.update_download(id, "in-progress", 0, false)
                }
                delete popupWindow.pendingDownloadRequests[id]
                popupWindow.pendingDownloadId = ""
                popupWindow.pendingDownloadSuggestedName = ""
                window.restoreOverlayFocus()
            }

            function cancelPendingDownload() {
                var id = popupWindow.pendingDownloadId
                var download = popupWindow.pendingDownloadRequests[id]
                var ui = popupWindow.popupDownloadUi()
                if (download) {
                    window.resolveQtRequest(ui, download, "download", "cancel", [])
                    ui.update_download(id, "cancelled", download.receivedBytes, true)
                }
                delete popupWindow.pendingDownloadRequests[id]
                popupWindow.pendingDownloadId = ""
                popupWindow.pendingDownloadSuggestedName = ""
                window.restoreOverlayFocus()
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
                        window.resolveQtRequest(ui, download, "download", "cancel", [])
                        ui.update_download(id, "cancelled", download.receivedBytes, true)
                        return
                    }
                    var requestedSeparator = requestedPath.lastIndexOf("/")
                    var requestedDirectory = requestedSeparator > 0
                            ? requestedPath.slice(0, requestedSeparator) : "/"
                    var stagingDirectory = window.downloadStagingDirectory(ui, id)
                    if (stagingDirectory.length === 0) {
                        window.resolveQtRequest(ui, download, "download", "cancel", [])
                        ui.update_download(id, "cancelled", download.receivedBytes, true)
                        return
                    }
                    download.downloadDirectory = stagingDirectory
                    download.downloadFileName = requestedName
                    popupWindow.activeDownloads[id] = download
                    popupWindow.attachPopupDownloadStateUpdates(download, id)
                    window.resolveQtRequest(ui, download, "download", "accept", [])
                    ui.update_download(id, "in-progress", 0, false)
                    return
                }
                var directory = ui.default_download_directory()
                if (!window.downloadsAskDestination()) {
                    popupWindow.activeDownloads[id] = download
                    var finalName = ui.accept_download(id, directory, safeName)
                    if (finalName.length === 0) {
                        delete popupWindow.activeDownloads[id]
                        window.resolveQtRequest(ui, download, "download", "cancel", [])
                        ui.update_download(id, "cancelled", download.receivedBytes, true)
                        return
                    }
                    var stagingDirectory = window.downloadStagingDirectory(ui, id)
                    if (stagingDirectory.length === 0) {
                        delete popupWindow.activeDownloads[id]
                        window.resolveQtRequest(ui, download, "download", "cancel", [])
                        ui.update_download(id, "cancelled", download.receivedBytes, true)
                        return
                    }
                    download.downloadDirectory = stagingDirectory
                    download.downloadFileName = finalName
                    popupWindow.attachPopupDownloadStateUpdates(download, id)
                    window.resolveQtRequest(ui, download, "download", "accept", [])
                    ui.update_download(id, "in-progress", 0, false)
                    return
                }
                if (popupWindow.pendingDownloadId.length > 0) {
                    window.resolveQtRequest(ui, download, "download", "cancel", [])
                    ui.update_download(id, "cancelled", download.receivedBytes, true)
                    return
                }
                popupWindow.pendingDownloadRequests[id] = download
                popupWindow.pendingDownloadId = id
                popupWindow.pendingDownloadSuggestedName = safeName
                window.captureOverlayFocus(popupWindow, popupView)
                ui.update_download(id, "selecting-destination", 0, false)
                popupDownloadChooser.currentFile = window.fileUrlForPath(directory + "/" + safeName)
                popupDownloadChooser.open()
            }

            function handleDownloadFinished(download) {
                var id = String(download.id)
                var ui = popupWindow.popupDownloadUi()
                var state = window.downloadStateName(download)
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
                var ui = popupWindow.popupPermissionUi || browserUi
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
                    window.restoreOverlayFocus()
                }
                if (cancelRequest && request) {
                    window.resolveQtRequest(ui, request, "file-dialog", "dialogReject", [])
                }
                if (message && ui) {
                    ui.status_text = message
                }
            }

            function acceptFileDialog() {
                var request = popupWindow.pendingFileDialogRequest
                var ui = popupWindow.popupPermissionUi || browserUi
                var paths = popupWindow.popupFileDialogPaths()
                popupWindow.pendingFileDialogRequest = null
                popupWindow.pendingFileDialogView = null
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

            function acceptFolderDialog() {
                var request = popupWindow.pendingFileDialogRequest
                var ui = popupWindow.popupPermissionUi || browserUi
                var paths = popupWindow.popupFolderDialogPaths()
                popupWindow.pendingFileDialogRequest = null
                popupWindow.pendingFileDialogView = null
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

            function rejectFileDialog() {
                var request = popupWindow.pendingFileDialogRequest
                var ui = popupWindow.popupPermissionUi || browserUi
                if (!request) {
                    return
                }
                popupWindow.pendingFileDialogRequest = null
                popupWindow.pendingFileDialogView = null
                window.resolveQtRequest(ui, request, "file-dialog", "dialogReject", [])
                window.restoreOverlayFocus()
                ui.status_text = "File selection cancelled"
            }

            function handleFileDialogRequested(request) {
                if (popupWindow.pendingFileDialogRequest) {
                    popupWindow.clearPopupFileDialog(true,
                            "Previous file selection cancelled")
                    window.resolveQtRequest(
                        popupWindow.popupPermissionUi || browserUi,
                        request, "file-dialog", "dialogReject", [])
                    (popupWindow.popupPermissionUi || browserUi).status_text =
                            "Another file selection is already open"
                    return
                }
                window.captureOverlayFocus(popupWindow, popupView)
                popupWindow.pendingFileDialogRequest = request
                popupWindow.pendingFileDialogView = popupView
                var requestUi = popupWindow.popupPermissionUi || browserUi
                if (window.desktopPortalMode(requestUi) === "required") {
                    var status = window.desktopPortalCapabilityStatus(requestUi, "file_chooser")
                    if (status === "not-probed") {
                        browserUi.probe_desktop_portals()
                        status = window.desktopPortalCapabilityStatus(requestUi, "file_chooser")
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
                popupFileChooser.nameFilters = window.fileDialogNameFilters(request)
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
                var ui = popupWindow.popupPermissionUi || browserUi
                var status = window.desktopPortalCapabilityStatus(ui, "file_chooser")
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
                popupFileChooser.nameFilters = window.fileDialogNameFilters(request)
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
                    effectiveSiteSettings = window.siteRuleSettingsFor(
                        popupWindow.popupPermissionUi, popupView.url.toString())
                }
                Connections {
                    target: popupWindow.popupPermissionUi || browserUi
                    function onSite_experiment_jsonChanged() {
                        popupView.refreshEffectiveSiteSettings()
                    }
                }
                anchors.fill: parent
                anchors.bottomMargin: window.statusBarHeight
                profile: popupWindow.popupProfile
                url: "about:blank"
                settings.javascriptEnabled: !!window.siteRuleValue(
                    effectiveSiteSettings, "content.javascript", true)
                settings.autoLoadImages: !!window.siteRuleValue(
                    effectiveSiteSettings, "content.images", true)
                settings.forceDarkMode: !!window.siteRuleValue(
                    effectiveSiteSettings, "content.force_dark", false)
                settings.playbackRequiresUserGesture:
                    window.siteRuleValue(effectiveSiteSettings, "content.autoplay", "engine-default")
                    === "require-gesture"
                Accessible.name: "Popup web content"
                onUrlChanged: {
                    refreshEffectiveSiteSettings()
                    if (popupWindow.popupJourneyToken.length > 0) {
                        (popupWindow.popupPermissionUi || browserUi).popup_navigation_url_changed(
                            popupWindow.popupJourneyToken, url.toString())
                    }
                }
                onLoadingChanged: function(loadRequest) {
                    if (loadRequest.status === WebEngineView.LoadStartedStatus) {
                        window.clearPageDialogForView(popupView)
                        window.clearClientCertificateForView(popupView)
                        window.clearCertificateErrorForView(popupView)
                        window.clearWebAuthForView(popupView)
                        window.clearContextMenuForView(popupView)
                        window.clearDesktopMediaForView(popupView)
                        window.clearSiteDataClearForView(popupView)
                        window.noteCaptureNavigation(popupView)
                        popupWindow.clearFileDialogForView(popupView)
                        window.clearRendererFailureForView(popupView)
                        window.resetPageDialogBudget(popupView)
                        if (popupWindow.popupRequestInterceptor) {
                            popupWindow.popupRequestInterceptor.clearSiteEvidence(popupView.url.host)
                        }
                        window.cancelPermissionForUi(popupWindow.popupPermissionUi)
                        window.installPageUserscripts(
                            popupWindow.popupPermissionUi, popupView, popupView.url.toString(),
                            popupWindow.popupPrivateProfile)
                        window.injectPageUserscripts(
                            popupWindow.popupPermissionUi, popupView, popupView.url.toString(),
                            popupWindow.popupPrivateProfile, "document_start")
                        window.injectCosmeticRules(popupWindow.popupPermissionUi, popupView)
                        if (popupWindow.popupJourneyToken.length > 0) {
                            (popupWindow.popupPermissionUi || browserUi).popup_navigation_started(
                                popupWindow.popupJourneyToken, loadRequest.url.toString())
                        }
                    } else if (loadRequest.status === WebEngineView.LoadSucceededStatus) {
                        if (popupWindow.popupJourneyToken.length > 0) {
                            (popupWindow.popupPermissionUi || browserUi).popup_navigation_committed(
                                popupWindow.popupJourneyToken, popupView.url.toString(), popupView.title)
                            (popupWindow.popupPermissionUi || browserUi).popup_navigation_completed(
                                popupWindow.popupJourneyToken)
                        }
                        window.injectPageUserscripts(
                            popupWindow.popupPermissionUi, popupView, popupView.url.toString(),
                            popupWindow.popupPrivateProfile, "document_end")
                        window.injectCosmeticRules(popupWindow.popupPermissionUi, popupView)
                        Qt.callLater(function() {
                            window.injectPageUserscripts(
                                popupWindow.popupPermissionUi, popupView, popupView.url.toString(),
                                popupWindow.popupPrivateProfile, "document_idle")
                        })
                    } else if (loadRequest.status === WebEngineView.LoadFailedStatus) {
                        if (popupWindow.popupJourneyToken.length > 0) {
                            (popupWindow.popupPermissionUi || browserUi).popup_navigation_failed(
                                popupWindow.popupJourneyToken)
                        }
                    }
                }

                onPermissionRequested: function(permissionRequest) {
                    if (popupWindow.popupPermissionUi) {
                        window.handleImmediatePermissionRequested(
                            popupWindow.popupPermissionUi,
                            permissionRequest,
                            popupWindow.popupPrivateProfile,
                            popupWindow)
                    } else {
                        window.resolveQtRequest(
                            popupWindow.popupPermissionUi || browserUi,
                            permissionRequest, "permission", "deny", [])
                    }
                }
                onJavaScriptDialogRequested: function(request) {
                    window.handleJavaScriptDialogRequested(
                        popupWindow.popupPermissionUi || browserUi,
                        popupView, request, popupWindow.popupPrivateProfile)
                }
                onAuthenticationDialogRequested: function(request) {
                    window.handleAuthenticationDialogRequested(
                        popupWindow.popupPermissionUi || browserUi,
                        popupView, request, popupWindow.popupPrivateProfile)
                }
                onSelectClientCertificate: function(selection) {
                    window.handleClientCertificateRequested(
                        popupWindow.popupPermissionUi || browserUi,
                        popupView, selection, popupWindow.popupPrivateProfile, popupWindow)
                }
                onCertificateError: function(error) {
                    window.handleCertificateError(
                        popupWindow.popupPermissionUi || browserUi,
                        popupView, error, popupWindow)
                }
                onWebAuthUxRequested: function(request) {
                    window.handleWebAuthRequested(
                        popupWindow.popupPermissionUi || browserUi,
                        popupView, request, popupWindow)
                }
                onContextMenuRequested: function(request) {
                    window.handleContextMenuRequested(
                        popupWindow.popupPermissionUi || browserUi,
                        popupView, request, popupWindow)
                }
                onFileDialogRequested: function(request) {
                    popupWindow.handleFileDialogRequested(request)
                }
                onDesktopMediaRequested: function(request) {
                    window.handleDesktopMediaRequested(
                        popupWindow.popupPermissionUi || browserUi,
                        popupView, request, popupWindow)
                }
                onRenderProcessTerminated: function(terminationStatus, exitCode) {
                    window.handleRendererProcessTerminated(
                        popupWindow.popupPermissionUi || browserUi,
                        popupView, popupWindow, -1, terminationStatus, exitCode)
                }

                Component.onCompleted: {
                    refreshEffectiveSiteSettings()
                    window.registerPopupWindow(
                                popupWindow, popupWindow.popupPermissionUi || browserUi, popupView)
                    if (!popupWindow.popupPermissionPromptSurface) {
                        popupWindow.popupPermissionPromptSurface =
                                permissionPromptSurfaceComponent.createObject(
                                    popupWindow.contentItem, { hostWindow: popupWindow })
                    }
                    if (!popupWindow.popupDesktopMediaSurface) {
                        popupWindow.popupDesktopMediaSurface =
                                desktopMediaSurfaceComponent.createObject(
                                    popupWindow.contentItem, { hostWindow: popupWindow })
                    }
                    if (!popupWindow.popupCaptureIndicatorSurface) {
                        popupWindow.popupCaptureIndicatorSurface =
                                captureIndicatorComponent.createObject(
                                    popupWindow.contentItem, { hostWindow: popupWindow })
                    }
                    if (!popupWindow.popupRendererFailureSurface) {
                        popupWindow.popupRendererFailureSurface =
                                rendererFailureSurfaceComponent.createObject(
                                    popupWindow.contentItem, { hostWindow: popupWindow })
                    }
                    window.installFocusObserver(popupView)
                    if (popupWindow.popupRequest) {
                        popupWindow.popupRequest.openIn(popupView)
                    }
                }
                Component.onDestruction: {
                    window.clearPageDialogForView(popupView)
                    window.clearClientCertificateForView(popupView)
                    window.clearCertificateErrorForView(popupView)
                    window.clearWebAuthForView(popupView)
                    window.clearContextMenuForView(popupView)
                    window.clearDesktopMediaForView(popupView)
                    window.clearCaptureSessionForView(popupView)
                    popupWindow.clearFileDialogForView(popupView)
                    window.clearSiteDataClearForView(popupView)
                    window.cancelPermissionForUi(popupWindow.popupPermissionUi)
                }
            }

            Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                height: window.statusBarHeight
                z: 10
                visible: window.statusbarMode === "always"
                color: window.surfaceColor
                opacity: window.chromeOpacity

                Label {
                    anchors.fill: parent
                    anchors.leftMargin: 10
                    verticalAlignment: Text.AlignVCenter
                    color: window.contextStatusColor(
                               popupWindow.popupPermissionUi || browserUi,
                               window.secondaryTextColor)
                    text: (popupWindow.popupPermissionUi || browserUi).mode + " · "
                          + (popupWindow.popupPermissionUi || browserUi).status_text
                          + window.statusDetails(
                              popupWindow.popupPermissionUi || browserUi,
                              popupWindow, popupView,
                              popupWindow.popupPrivateProfile,
                              popupWindow.popupProfileName,
                              popupWindow.popupEphemeralProfile)
                    Accessible.name: "Browser status bar"
                    Accessible.role: Accessible.StatusBar
                }
            }

            Component.onDestruction: {
                window.unregisterPopupWindow(popupWindow)
                if (!popupWindow.windowShutdownApproved
                        && popupWindow.popupPermissionUi
                        && popupWindow.popupJourneyToken.length > 0) {
                    popupWindow.popupPermissionUi.close_popup_tab(
                        popupWindow.popupJourneyToken)
                }
                if (popupWindow.popupJourneyToken.length > 0) {
                    (popupWindow.popupPermissionUi || browserUi).release_popup_journey_token(
                        popupWindow.popupJourneyToken)
                }
                popupWindow.clearPopupFileDialog(true, "File selection cancelled")
                window.cancelPermissionForUi(popupWindow.popupPermissionUi)
                if (window.pendingDesktopMediaHost === popupWindow) {
                    window.clearDesktopMediaRequest(true)
                }
                window.clearCaptureSessionsForHost(popupWindow)
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
    }

    Component {
        id: browserWindowComponent

        ApplicationWindow {
            id: secondaryWindow
            width: 1180
            height: 760
            visible: true
            title: secondaryWindow.windowEphemeralProfile
                   ? "Ferric Browser · " + secondaryWindow.windowProfileLabel
                   : "Ferric Browser · " + secondaryWindow.windowProfileName
            color: window.backgroundColor
            font: window.font
            palette: window.palette
            property string windowStartupUrl: "about:blank"
            property string windowBookmarkTransferUrl: ""
            property string windowBookmarkTransferTitle: ""
            property string windowProfileName: "secondary"
            property string windowStartupContext: ""
            property bool windowStartupContextRestore: false
            property bool windowStartupRoutePreflighted: true
            property string windowProfileLabel: "Secondary"
            property string windowStorageBasePath: window.storageBasePath
            property bool windowPrivateProfile: false
            property bool windowEphemeralProfile: false
            property string windowEphemeralInvocationToken: ""
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
            readonly property var activeView: windowTransferMode
                                                       ? windowTransferView
                                                       : (windowFallbackView || secondaryView)
            readonly property bool browserKeyFocusActive:
                !window.browserChromeInputActive
                && !windowShutdownPromptVisible
                && !windowShutdownPagePromptVisible
                && !windowShutdownStoragePromptVisible
                && !secondaryUi.external_navigation_visible
            readonly property bool windowTransientProfile:
                windowPrivateProfile || windowEphemeralProfile
            readonly property var windowWebEngineProfile:
                windowSharedProfile || secondaryProfile
            readonly property var windowRequestInterceptor:
                windowSharedProfile
                ? (windowSharedRequestInterceptor || requestInterceptor)
                : secondaryRequestInterceptor
            property var permissionPromptSurface: null
            property var desktopMediaSurface: null
            property var captureIndicatorSurface: null
            property var rendererFailureSurface: null
            property var pendingFileDialogRequest: null
            property var pendingFileDialogView: null
            property bool pendingFileDialogWaitingForPortal: false
            property double pendingFileDialogPortalDeadlineMs: 0
            property bool windowShutdownApproved: false
            property bool windowShutdownPromptVisible: false
            property bool windowShutdownPagePromptVisible: false
            property string windowShutdownPagePromptReason: ""
            property int windowShutdownPageProbeGeneration: 0
            property var activeDownloads: ({})

            BrowserKeyRouter {
                id: secondaryKeyRouter
                targetWindow: secondaryWindow
                enabled: secondaryWindow.active
                         && secondaryWindow.browserKeyFocusActive
                         && (secondaryUi.mode === "normal"
                             || secondaryUi.mode === "hint"
                             || secondaryUi.mode === "caret"
                             || secondaryUi.mode === "insert"
                             || secondaryUi.mode === "pass-through")
                onKeyPressed: function(text, key, modifiers) {
                    var event = {
                        text: text,
                        key: key,
                        modifiers: modifiers,
                        accepted: false
                    }
                    if (window.handleBrowserKey(
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
                    secondaryWindow.windowShutdownPagePromptReason = "Page state check timed out."
                    secondaryWindow.windowShutdownPagePromptVisible = true
                    secondaryUi.status_text =
                            "Page state check timed out; choose Close anyway"
                }
            }

            onClosing: function(close) {
                if (secondaryWindow.windowShutdownApproved) {
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

            function downloadOwnerFor(download) {
                var view = download && download.view
                if (!view) {
                    return secondaryWindow
                }
                var popups = window.popupWindowRegistry || []
                for (var i = 0; i < popups.length; ++i) {
                    if (popups[i] && popups[i].view === view && popups[i].host) {
                        return popups[i].host
                    }
                }
                return secondaryWindow
            }

            function checkPageStateBeforeQuit() {
                var generation = ++secondaryWindow.windowShutdownPageProbeGeneration
                var view = secondaryWindow.activeView
                if (!view || view.lifecycleState !== WebEngineView.LifecycleState.Active) {
                    secondaryWindow.finalizeQuit()
                    return
                }
                secondaryShutdownPageProbeTimer.restart()
                view.runJavaScript(window.shutdownPageProbeScript(), function(result) {
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
                })
            }

            function cancelPageStateProbe() {
                secondaryWindow.windowShutdownPageProbeGeneration += 1
                secondaryShutdownPageProbeTimer.stop()
                secondaryWindow.windowShutdownPagePromptVisible = false
            }

            function beginQuitRequest() {
                if (secondaryWindow.hasActiveDownloads()
                        || window.hasActiveShutdownRequestsFor(secondaryUi, secondaryWindow)) {
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
                    secondaryProfile.cancelPendingDownload()
                }
                for (var pendingKey in secondaryWindow.pendingDownloadRequests) {
                    var pendingDownload = secondaryWindow.pendingDownloadRequests[pendingKey]
                    if (pendingDownload) {
                        window.resolveQtRequest(secondaryUi, pendingDownload, "download", "cancel", [])
                        secondaryUi.update_download(
                            String(pendingKey), "cancelled", pendingDownload.receivedBytes, true)
                    }
                }
                secondaryWindow.pendingDownloadRequests = ({})
                for (var key in secondaryWindow.activeDownloads) {
                    var download = secondaryWindow.activeDownloads[key]
                    if (download && !download.isFinished) {
                        window.resolveQtRequest(secondaryUi, download, "download", "cancel", [])
                    }
                }
                secondaryWindow.activeDownloads = ({})
                window.cancelShutdownRequestsFor(secondaryUi, secondaryWindow)
                secondaryWindow.beginQuitRequest()
            }

            FileDialog {
                id: secondaryFileChooser
                title: "Choose file"
                onAccepted: secondaryWindow.acceptFileDialog()
                onRejected: secondaryWindow.rejectFileDialog()
            }

            FolderDialog {
                id: secondaryFolderChooser
                title: "Choose folder"
                onAccepted: secondaryWindow.acceptFolderDialog()
                onRejected: secondaryWindow.rejectFileDialog()
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
                spellCheckEnabled: window.spellcheckEnabled(secondaryUi)
                spellCheckLanguages: window.spellcheckLanguages(secondaryUi)
                isPushServiceEnabled: window.pushServiceEnabled(
                    secondaryUi, secondaryWindow.windowTransientProfile)

                function acceptPendingDownload() {
                    var id = secondaryWindow.pendingDownloadId
                    var download = secondaryWindow.pendingDownloadRequests[id]
                    var selectedPath = secondaryDownloadChooser.selectedFile.toLocalFile()
                    if (!download) {
                        secondaryWindow.pendingDownloadId = ""
                        window.restoreOverlayFocus()
                        return
                    }
                    var finalName = secondaryUi.accept_download_path(id, selectedPath)
                    if (finalName.length === 0) {
                        window.resolveQtRequest(secondaryUi, download, "download", "cancel", [])
                        secondaryUi.update_download(id, "cancelled", download.receivedBytes, true)
                    } else {
                        var separator = selectedPath.lastIndexOf("/")
                        var directory = separator > 0 ? selectedPath.slice(0, separator) : "/"
                        var stagingDirectory = window.downloadStagingDirectory(secondaryUi, id)
                        if (stagingDirectory.length === 0) {
                            window.resolveQtRequest(secondaryUi, download, "download", "cancel", [])
                            secondaryUi.update_download(id, "cancelled", download.receivedBytes, true)
                            delete secondaryWindow.pendingDownloadRequests[id]
                            secondaryWindow.pendingDownloadId = ""
                            secondaryWindow.pendingDownloadSuggestedName = ""
                            window.restoreOverlayFocus()
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
                        window.resolveQtRequest(secondaryUi, download, "download", "accept", [])
                        secondaryUi.update_download(id, "in-progress", 0, false)
                    }
                    delete secondaryWindow.pendingDownloadRequests[id]
                    secondaryWindow.pendingDownloadId = ""
                    secondaryWindow.pendingDownloadSuggestedName = ""
                    window.restoreOverlayFocus()
                }

                function cancelPendingDownload() {
                    var id = secondaryWindow.pendingDownloadId
                    var download = secondaryWindow.pendingDownloadRequests[id]
                    if (download) {
                        window.resolveQtRequest(secondaryUi, download, "download", "cancel", [])
                        secondaryUi.update_download(id, "cancelled", download.receivedBytes, true)
                    }
                    delete secondaryWindow.pendingDownloadRequests[id]
                    secondaryWindow.pendingDownloadId = ""
                    secondaryWindow.pendingDownloadSuggestedName = ""
                    window.restoreOverlayFocus()
                }

                onDownloadRequested: function(download) {
                    var owner = secondaryWindow.downloadOwnerFor(download)
                    if (owner !== secondaryWindow && owner.handleDownloadRequested) {
                        owner.handleDownloadRequested(download)
                        return
                    }
                    var id = String(download.id)
                    var safeName = secondaryUi.offer_download(id, download.url.toString(), download.suggestedFileName)
                    var requestedPath = download.savePageFormat === WebEngineDownloadRequest.MimeHtmlSaveFormat
                            ? secondaryUi.take_save_page_path() : ""
                    if (requestedPath.length > 0) {
                        var requestedName = secondaryUi.accept_download_path(id, requestedPath)
                        if (requestedName.length === 0) {
                            window.resolveQtRequest(secondaryUi, download, "download", "cancel", [])
                            secondaryUi.update_download(id, "cancelled", download.receivedBytes, true)
                            return
                        }
                        var requestedSeparator = requestedPath.lastIndexOf("/")
                        var requestedDirectory = requestedSeparator > 0
                                ? requestedPath.slice(0, requestedSeparator) : "/"
                        var stagingDirectory = window.downloadStagingDirectory(secondaryUi, id)
                        if (stagingDirectory.length === 0) {
                            window.resolveQtRequest(secondaryUi, download, "download", "cancel", [])
                            secondaryUi.update_download(id, "cancelled", download.receivedBytes, true)
                            return
                        }
                        download.downloadDirectory = stagingDirectory
                        download.downloadFileName = requestedName
                        secondaryWindow.activeDownloads[id] = download
                        download.stateChanged.connect(function() {
                            secondaryUi.update_download(id, secondaryWindow.secondaryDownloadStateName(download), download.receivedBytes, false)
                        })
                        window.resolveQtRequest(secondaryUi, download, "download", "accept", [])
                        secondaryUi.update_download(id, "in-progress", 0, false)
                        return
                    }
                    var directory = secondaryUi.default_download_directory()
                    if (window.downloadsAskDestination()) {
                        if (secondaryWindow.pendingDownloadId.length > 0) {
                            window.resolveQtRequest(secondaryUi, download, "download", "cancel", [])
                            secondaryUi.update_download(id, "cancelled", download.receivedBytes, true)
                            return
                        }
                        secondaryWindow.pendingDownloadRequests[id] = download
                        secondaryWindow.pendingDownloadId = id
                        secondaryWindow.pendingDownloadSuggestedName = safeName
                        window.captureOverlayFocus(secondaryWindow, secondaryWindow.activeView)
                        secondaryUi.update_download(id, "selecting-destination", 0, false)
                        secondaryDownloadChooser.currentFile = window.fileUrlForPath(
                            directory + "/" + safeName)
                        secondaryDownloadChooser.open()
                        return
                    }
                    var finalName = secondaryUi.accept_download(id, directory, safeName)
                    if (finalName.length === 0) {
                        window.resolveQtRequest(secondaryUi, download, "download", "cancel", [])
                        return
                    }
                    var stagingDirectory = window.downloadStagingDirectory(secondaryUi, id)
                    if (stagingDirectory.length === 0) {
                        delete secondaryWindow.activeDownloads[id]
                        window.resolveQtRequest(secondaryUi, download, "download", "cancel", [])
                        secondaryUi.update_download(id, "cancelled", download.receivedBytes, true)
                        return
                    }
                    download.downloadDirectory = stagingDirectory
                    download.downloadFileName = finalName
                    download.stateChanged.connect(function() {
                        secondaryUi.update_download(id, secondaryWindow.secondaryDownloadStateName(download), download.receivedBytes, false)
                    })
                    window.resolveQtRequest(secondaryUi, download, "download", "accept", [])
                    secondaryWindow.activeDownloads[id] = download
                    secondaryUi.update_download(id, "in-progress", 0, false)
                }
                onDownloadFinished: function(download) {
                    var owner = secondaryWindow.downloadOwnerFor(download)
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
                    if (secondaryWindow.pendingDownloadId === id) {
                        secondaryWindow.pendingDownloadId = ""
                        secondaryWindow.pendingDownloadSuggestedName = ""
                    }
                    secondaryUi.update_download(id, state, download.receivedBytes, true)
                }
                onPresentNotification: function(notification) {
                    window.presentWebNotification(
                        secondaryUi, notification, secondaryWindow.windowTransientProfile)
                }
            }

            BrowserUi {
                id: secondaryUi
                status_text: "Ready"
            }

            Rectangle {
                anchors.centerIn: parent
                width: Math.min(560, secondaryWindow.width - 80)
                height: Math.min(190 * window.chromeScale, secondaryWindow.height - 32)
                z: 100
                visible: secondaryWindow.windowShutdownPromptVisible
                color: window.panelColor
                border.color: window.warningColor
                border.width: 2

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 16
                    spacing: 12

                    Label {
                        Layout.fillWidth: true
                        text: "Active browser work is still running"
                        color: window.primaryTextColor
                        font.bold: true
                    }
                    Label {
                        Layout.fillWidth: true
                        text: "Cancel active work before closing, or keep this window open."
                        color: window.secondaryTextColor
                        wrapMode: Text.WordWrap
                    }
                    RowLayout {
                        Layout.alignment: Qt.AlignRight
                        spacing: 8
                        Button {
                            text: "Keep window open"
                            Accessible.name: "Keep window open"
                            onClicked: {
                                secondaryWindow.windowShutdownPromptVisible = false
                                window.abortApplicationShutdown()
                                secondaryUi.status_text = "Shutdown cancelled"
                            }
                        }
                        Button {
                            text: "Cancel and close"
                            Accessible.name: "Cancel and close"
                            onClicked: secondaryWindow.cancelDownloadsAndQuit()
                        }
                    }
                }
            }

            Rectangle {
                anchors.centerIn: parent
                width: Math.min(560, secondaryWindow.width - 80)
                height: Math.min(220 * window.chromeScale, secondaryWindow.height - 32)
                z: 100
                visible: secondaryWindow.windowShutdownPagePromptVisible
                color: window.panelColor
                border.color: window.warningColor
                border.width: 2

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 16
                    spacing: 12

                    Label {
                        Layout.fillWidth: true
                        text: "Page state may be lost"
                        color: window.primaryTextColor
                        font.bold: true
                    }
                    Label {
                        Layout.fillWidth: true
                        text: secondaryWindow.windowShutdownPagePromptReason
                              + " Close anyway may lose that state."
                        color: window.secondaryTextColor
                        wrapMode: Text.WordWrap
                    }
                    RowLayout {
                        Layout.alignment: Qt.AlignRight
                        spacing: 8
                        Button {
                            text: "Keep window open"
                            Accessible.name: "Keep window open"
                            onClicked: {
                                secondaryWindow.cancelPageStateProbe()
                                window.abortApplicationShutdown()
                                secondaryUi.status_text = "Shutdown cancelled"
                            }
                        }
                        Button {
                            text: "Close anyway"
                            Accessible.name: "Close anyway despite page state"
                            onClicked: secondaryWindow.finalizeQuit()
                        }
                    }
                }
            }

            Rectangle {
                anchors.centerIn: parent
                width: Math.min(560, secondaryWindow.width - 80)
                height: Math.min(220 * window.chromeScale, secondaryWindow.height - 32)
                z: 101
                visible: secondaryWindow.windowShutdownStoragePromptVisible
                color: window.panelColor
                border.color: window.errorColor
                border.width: 2

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 16
                    spacing: 12

                    Label {
                        Layout.fillWidth: true
                        text: "Durable profile state could not be flushed"
                        color: window.errorColor
                        font.bold: true
                    }
                    Label {
                        Layout.fillWidth: true
                        text: "The window remains open so its session and profile data are not abandoned. Retry the close after checking storage availability, or keep it open."
                        color: window.secondaryTextColor
                        wrapMode: Text.WordWrap
                    }
                    RowLayout {
                        Layout.alignment: Qt.AlignRight
                        spacing: 8
                        Button {
                            text: "Keep window open"
                            Accessible.name: "Keep window open after storage flush failure"
                            onClicked: {
                                secondaryWindow.windowShutdownStoragePromptVisible = false
                                window.abortApplicationShutdown()
                                secondaryUi.status_text = "Shutdown cancelled; durable state was retained"
                            }
                        }
                        Button {
                            text: "Retry close"
                            Accessible.name: "Retry close after storage flush failure"
                            onClicked: secondaryWindow.finalizeQuit()
                        }
                    }
                }
            }

            Connections {
                target: browserUi
                function onConfig_jsonChanged() {
                    secondaryUi.config_json = browserUi.config_json
                    secondaryUi.config_base_json = browserUi.config_base_json
                    secondaryUi.cli_overrides_json = browserUi.cli_overrides_json
                    secondaryUi.config_path = browserUi.config_path
                }
                function onConfig_base_jsonChanged() {
                    secondaryUi.config_base_json = browserUi.config_base_json
                }
                function onContexts_jsonChanged() {
                    secondaryUi.contexts_json = browserUi.contexts_json
                }
            }

            Connections {
                target: secondaryUi
        function onContext_route_jsonChanged() {
            window.showContextRoute(secondaryUi)
        }
        function onContext_workspaceChanged() {
            window.routeContextWorkspace(secondaryUi)
        }
                function onExternal_navigation_visibleChanged() {
                    if (secondaryUi.external_navigation_visible) {
                        window.captureOverlayFocus(secondaryWindow, secondaryWindow.activeView)
                    } else {
                        window.restoreOverlayFocus()
                    }
                }
            }

            Popup {
                id: secondaryExternalNavigationPopup
                parent: Overlay.overlay
                modal: true
                focus: true
                closePolicy: Popup.NoAutoClose
                visible: secondaryUi.external_navigation_visible
                width: Math.min(620, secondaryWindow.width - 48)
                padding: 14
                x: Math.round((secondaryWindow.width - width) / 2)
                y: Math.round((secondaryWindow.height - height) / 2)

                background: Rectangle {
                    color: window.panelColor
                    border.color: window.warningColor
                    radius: 4
                }

                contentItem: ColumnLayout {
                    focus: true
                    Accessible.role: Accessible.Dialog
                    Accessible.name: "External URI confirmation"
                    spacing: 10

                    Keys.onPressed: function(event) {
                        if (event.key === Qt.Key_Escape) {
                            secondaryUi.cancel_external_navigation()
                            event.accepted = true
                        } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                            if (secondaryUi.confirm_external_navigation()) {
                                secondaryWindow.applySwitcherEngineAction()
                            }
                            event.accepted = true
                        }
                    }

                    Label {
                        Layout.fillWidth: true
                        text: "Open with system handler?"
                        color: window.primaryTextColor
                        font.bold: true
                    }
                    Label {
                        Layout.fillWidth: true
                        text: "This URI will leave Ferric Browser and may launch another application."
                        color: window.warningColor
                        wrapMode: Text.WordWrap
                    }
                    Label {
                        Layout.fillWidth: true
                        text: "Scheme: " + secondaryUi.external_navigation_scheme
                        color: window.mutedTextColor
                    }
                    Label {
                        Layout.fillWidth: true
                        text: secondaryUi.external_navigation_uri
                        color: window.primaryTextColor
                        wrapMode: Text.WrapAnywhere
                        maximumLineCount: 8
                        elide: Text.ElideRight
                        Accessible.name: "External URI"
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        Item { Layout.fillWidth: true }
                        Button {
                            text: "Cancel"
                            Accessible.name: "Cancel external URI"
                            onClicked: secondaryUi.cancel_external_navigation()
                        }
                        Button {
                            text: "Open with system handler"
                            Accessible.name: "Confirm external URI"
                            onClicked: {
                                if (secondaryUi.confirm_external_navigation()) {
                                    secondaryWindow.applySwitcherEngineAction()
                                }
                            }
                        }
                    }
                }
            }

            RequestInterceptor {
                id: secondaryRequestInterceptor
                enabled: secondaryUi.blocking_enabled || securityDenyHosts.length > 0
                blockedHosts: {
                    try {
                        var hosts = JSON.parse(secondaryUi.blocking_hosts)
                        return Array.isArray(hosts) ? hosts : []
                    } catch (error) {
                        return []
                    }
                }
                exceptionHosts: {
                    try {
                        var hosts = JSON.parse(secondaryUi.blocking_exceptions)
                        return Array.isArray(hosts) ? hosts : []
                    } catch (error) {
                        return []
                    }
                }
                blockedRuleLists: {
                    try {
                        var lists = JSON.parse(secondaryUi.blocking_rule_lists)
                        return lists && typeof lists === "object" && !Array.isArray(lists) ? lists : ({})
                    } catch (error) {
                        return ({})
                    }
                }
                exceptionRuleLists: {
                    try {
                        var lists = JSON.parse(secondaryUi.blocking_exception_rule_lists)
                        return lists && typeof lists === "object" && !Array.isArray(lists) ? lists : ({})
                    } catch (error) {
                        return ({})
                    }
                }
                adblockEngineHandle: secondaryUi.blocking_adblock_handle
                bypassSites: {
                    try {
                        var sites = JSON.parse(secondaryUi.blocking_bypass_sites)
                        return Array.isArray(sites) ? sites : []
                    } catch (error) {
                        return []
                    }
                }
                securityDenyHosts: {
                    try {
                        var hosts = JSON.parse(secondaryUi.blocking_security_deny_hosts)
                        return Array.isArray(hosts) ? hosts : []
                    } catch (error) {
                        return []
                    }
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
                var urls = secondaryFileChooser.fileMode === FileDialog.OpenFiles
                        ? secondaryFileChooser.selectedFiles
                        : [secondaryFileChooser.selectedFile]
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
                var url = secondaryFolderChooser.selectedFolder
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
                if (secondaryFileChooser.visible) {
                    secondaryFileChooser.close()
                }
                if (secondaryFolderChooser.visible) {
                    secondaryFolderChooser.close()
                }
                if (hadRequest) {
                    window.restoreOverlayFocus()
                }
                if (cancelRequest && request) {
                    window.resolveQtRequest(secondaryUi, request, "file-dialog", "dialogReject", [])
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
                        window.resolveQtRequest(secondaryUi, request, "file-dialog", "dialogReject", [])
                    }
                    if (request) {
                        window.restoreOverlayFocus()
                    }
                    secondaryUi.status_text = "File selection returned no local paths"
                    return
                }
                window.resolveQtRequest(secondaryUi, request, "file-dialog", "dialogAccept", [paths])
                window.restoreOverlayFocus()
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
                        window.resolveQtRequest(secondaryUi, request, "file-dialog", "dialogReject", [])
                    }
                    if (request) {
                        window.restoreOverlayFocus()
                    }
                    secondaryUi.status_text = "Folder selection returned no local path"
                    return
                }
                window.resolveQtRequest(secondaryUi, request, "file-dialog", "dialogAccept", [paths])
                window.restoreOverlayFocus()
                secondaryUi.status_text = "Folder selected"
            }

            function rejectFileDialog() {
                var request = secondaryWindow.pendingFileDialogRequest
                if (!request) {
                    return
                }
                secondaryWindow.pendingFileDialogRequest = null
                secondaryWindow.pendingFileDialogView = null
                window.resolveQtRequest(secondaryUi, request, "file-dialog", "dialogReject", [])
                window.restoreOverlayFocus()
                secondaryUi.status_text = "File selection cancelled"
            }

            function handleFileDialogRequested(request) {
                if (secondaryWindow.pendingFileDialogRequest) {
                    secondaryWindow.clearSecondaryFileDialog(true,
                            "Previous file selection cancelled")
                    window.resolveQtRequest(
                        secondaryUi, request, "file-dialog", "dialogReject", [])
                    secondaryUi.status_text = "Another file selection is already open"
                    return
                }
                window.captureOverlayFocus(secondaryWindow, secondaryWindow.activeView)
                secondaryWindow.pendingFileDialogRequest = request
                secondaryWindow.pendingFileDialogView = secondaryWindow.activeView
                var requestUi = secondaryUi
                if (window.desktopPortalMode(requestUi) === "required") {
                    var status = window.desktopPortalCapabilityStatus(requestUi, "file_chooser")
                    if (status === "not-probed") {
                        browserUi.probe_desktop_portals()
                        status = window.desktopPortalCapabilityStatus(requestUi, "file_chooser")
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
                    secondaryFolderChooser.open()
                    return
                }
                secondaryFileChooser.fileMode = request.mode === FileDialogRequest.FileModeOpenMultiple
                        ? FileDialog.OpenFiles
                        : request.mode === FileDialogRequest.FileModeSave
                            ? FileDialog.SaveFile
                            : FileDialog.OpenFile
                secondaryFileChooser.nameFilters = window.fileDialogNameFilters(request)
                secondaryFileChooser.currentFile = request.defaultFileName.length > 0
                        ? request.defaultFileName
                        : ""
                secondaryFileChooser.title = request.mode === FileDialogRequest.FileModeSave
                        ? "Save file"
                        : request.mode === FileDialogRequest.FileModeOpenMultiple
                            ? "Choose files"
                            : "Choose file"
                secondaryFileChooser.open()
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
                var status = window.desktopPortalCapabilityStatus(ui, "file_chooser")
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
                    secondaryFolderChooser.open()
                    return
                }
                secondaryFileChooser.fileMode = request.mode === FileDialogRequest.FileModeOpenMultiple
                        ? FileDialog.OpenFiles
                        : request.mode === FileDialogRequest.FileModeSave
                            ? FileDialog.SaveFile
                            : FileDialog.OpenFile
                secondaryFileChooser.nameFilters = window.fileDialogNameFilters(request)
                secondaryFileChooser.currentFile = request.defaultFileName.length > 0
                        ? request.defaultFileName : ""
                secondaryFileChooser.title = request.mode === FileDialogRequest.FileModeSave
                        ? "Save file"
                        : request.mode === FileDialogRequest.FileModeOpenMultiple
                            ? "Choose files"
                            : "Choose file"
                secondaryFileChooser.open()
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
                    secondaryWindow.activeView.forceActiveFocus()
                    return
                }
                if (action.indexOf("command-prefill\t") === 0) {
                    secondaryCommandLine.text = action.slice("command-prefill\t".length)
                    secondaryCommandLine.cursorPosition = secondaryCommandLine.text.length
                    secondaryCommandLine.forceActiveFocus()
                    return
                }
                if (action === "quit-request") {
                    window.beginQuitRequest()
                    return
                }
                if (action === "window-close-request") {
                    secondaryWindow.beginQuitRequest()
                    return
                }
                if (action === "tab-detach" || action.indexOf("tab-detach\t") === 0
                        || action.indexOf("tab-give\t") === 0) {
                    window.executeBrowserTransferAction(secondaryUi, secondaryWindow, action)
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
                if (action.indexOf("reopen-window\t") === 0) {
                    window.openReopenedWindow(secondaryUi, secondaryWindow, action)
                    secondaryWindow.activeView.forceActiveFocus()
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
                        var secondaryFindFlags = window.searchFindFlags(
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
                    var secondaryFindFlags = window.searchFindFlags(
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
                        window.runBrowserScript(secondaryWindow.activeView, window.scrollScript(
                            "scroll", secondaryScrollParts[1], false,
                            Number(secondaryScrollParts[2])))
                    } else if (secondaryScrollParts[0] === "scroll-page"
                               && secondaryScrollParts.length >= 4) {
                        window.runBrowserScript(secondaryWindow.activeView, window.scrollScript(
                            "scroll-page", secondaryScrollParts[1],
                            secondaryScrollParts[2] === "true",
                            Number(secondaryScrollParts[3])))
                    } else if (secondaryScrollParts[0] === "scroll-to"
                               && secondaryScrollParts.length >= 2) {
                        window.runBrowserScript(secondaryWindow.activeView, window.scrollScript(
                            "scroll-to", secondaryScrollParts[1], false, 1))
                    }
                    secondaryWindow.activeView.forceActiveFocus()
                    return
                }
                if (action.indexOf("download-open\t") === 0) {
                    window.openExternalUri(secondaryUi,
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
                if (action.indexOf("jseval\t") === 0) {
                    var secondaryEvalPayload = null
                    try {
                        secondaryEvalPayload = JSON.parse(
                            action.split("\t").slice(1).join("\t"))
                    } catch (error) {
                        secondaryUi.status_text = "JavaScript evaluation request was invalid"
                        return
                    }
                    var secondaryEvalId = String(secondaryEvalPayload.tab_id || "")
                    var secondaryEvalIndex = secondaryUi.tab_index_for_id(secondaryEvalId)
                    var secondaryEvalView = secondaryEvalIndex >= 0
                            && secondaryEvalIndex === secondaryUi.active_tab_index
                            ? secondaryWindow.activeView : null
                    var secondaryEvalScript = String(secondaryEvalPayload.script || "")
                    var secondaryEvalWorld = secondaryEvalPayload.world === "page"
                            ? WebEngineScript.MainWorld : window.browserScriptWorld
                    if (!secondaryEvalView || secondaryEvalScript.length === 0
                            || secondaryEvalScript.length > 65536) {
                        secondaryUi.status_text =
                                "JavaScript evaluation target is stale or invalid"
                        return
                    }
                    window.runBrowserScript(secondaryEvalView, secondaryEvalScript,
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
                        var secondaryDevtools = devToolsWindowComponent.createObject(null, {
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
                if (action.indexOf("external-open\t") === 0) {
                    window.openExternalUri(secondaryUi,
                                           action.split("\t").slice(1).join("\t"))
                }
                secondaryWindow.activeView.forceActiveFocus()
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
                                text = window.addressPresentation(
                                    secondaryUi.display_url,
                                    width / Math.max(1, font.pixelSize * 0.56))
                            }
                        }
                        text: window.addressPresentation(
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
                        color: window.primaryTextColor
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
                visible: secondaryWindow.windowTransferMode
            }

            Item {
                id: fallbackViewHost
                anchors.fill: parent
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
                property var viewTabs: null
                property var viewModel: ({})
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
                    effectiveSiteSettings = window.siteRuleSettingsFor(
                        viewUi, secondaryView.url.toString())
                }
                Connections {
                    target: viewUi
                    function onSite_experiment_jsonChanged() {
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
                visible: !secondaryWindow.windowTransferMode
                profile: viewProfile
                url: secondaryWindow.windowTransferMode ? "about:blank" : secondaryUi.initial_url
                settings.javascriptEnabled: !!window.siteRuleValue(
                    effectiveSiteSettings, "content.javascript", true)
                settings.autoLoadImages: !!window.siteRuleValue(
                    effectiveSiteSettings, "content.images", true)
                settings.forceDarkMode: !!window.siteRuleValue(
                    effectiveSiteSettings, "content.force_dark", false)
                settings.playbackRequiresUserGesture:
                    window.siteRuleValue(effectiveSiteSettings, "content.autoplay", "engine-default")
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
                    if (viewUi === browserUi
                            && tabIndex === viewUi.active_tab_index) {
                        window.updateMprisForPrimaryView(webView)
                    }
                }
                onAudioMutedChanged: {
                    if (viewUi === browserUi
                            && tabIndex === viewUi.active_tab_index) {
                        window.updateMprisForPrimaryView(webView)
                    }
                }
                onLoadingChanged: function(loadRequest) {
                    if (loadRequest.status === WebEngineView.LoadStartedStatus) {
                        window.clearPageDialogForView(secondaryView)
                        window.clearClientCertificateForView(secondaryView)
                        window.clearCertificateErrorForView(secondaryView)
                        window.clearWebAuthForView(secondaryView)
                        window.clearContextMenuForView(secondaryView)
                        window.clearDesktopMediaForView(secondaryView)
                        window.clearSiteDataClearForView(secondaryView)
                        window.noteCaptureNavigation(secondaryView)
                        secondaryWindow.clearFileDialogForView(secondaryView)
                        window.clearRendererFailureForView(secondaryView)
                        window.resetPageDialogBudget(secondaryView)
                        viewInterceptor.clearSiteEvidence(secondaryView.url.host)
                        window.cancelPermissionForUi(viewUi)
                        window.installPageUserscripts(
                            viewUi, secondaryView, secondaryView.url.toString(),
                            viewTransientProfile)
                        window.injectPageUserscripts(
                            viewUi, secondaryView, secondaryView.url.toString(),
                            viewTransientProfile, "document_start")
                        window.injectCosmeticRules(viewUi, secondaryView)
                        if (viewTabs === tabs) {
                            viewUi.navigation_started_for(tabIndex, loadRequest.url.toString())
                        } else {
                            viewUi.navigation_started(loadRequest.url.toString())
                        }
                    } else if (loadRequest.status === WebEngineView.LoadSucceededStatus) {
                        if (viewTabs === tabs) {
                            viewUi.navigation_committed_for(
                                tabIndex, secondaryView.url.toString(), secondaryView.title)
                        } else {
                            viewUi.navigation_committed(secondaryView.url.toString(), secondaryView.title)
                        }
                        window.injectPageUserscripts(
                            viewUi, secondaryView, secondaryView.url.toString(),
                            viewTransientProfile, "document_end")
                        window.injectCosmeticRules(viewUi, secondaryView)
                        Qt.callLater(function() {
                            window.injectPageUserscripts(
                                viewUi, secondaryView, secondaryView.url.toString(),
                                viewTransientProfile, "document_idle")
                        })
                        if (viewTabs === tabs) {
                            viewUi.navigation_completed_for(tabIndex)
                        } else {
                            viewUi.navigation_completed()
                        }
                    } else if (loadRequest.status === WebEngineView.LoadFailedStatus) {
                        if (viewTabs === tabs) {
                            viewUi.navigation_failed_with_details(
                                tabIndex,
                                loadRequest.url.toString(),
                                window.navigationFailureKind(loadRequest),
                                window.navigationFailureDetail(loadRequest))
                        } else {
                            viewUi.navigation_failed_with_details(
                                -1,
                                loadRequest.url.toString(),
                                window.navigationFailureKind(loadRequest),
                                window.navigationFailureDetail(loadRequest))
                        }
                    }
                }
                onPermissionRequested: function(permissionRequest) {
                    window.handleImmediatePermissionRequested(
                        viewUi, permissionRequest, viewTransientProfile, viewHost)
                }
                onJavaScriptDialogRequested: function(request) {
                    window.handleJavaScriptDialogRequested(
                        viewUi, secondaryView, request, viewTransientProfile)
                }
                onAuthenticationDialogRequested: function(request) {
                    window.handleAuthenticationDialogRequested(
                        viewUi, secondaryView, request, viewTransientProfile)
                }
                onSelectClientCertificate: function(selection) {
                    window.handleClientCertificateRequested(
                        viewUi, secondaryView, selection, viewTransientProfile, viewHost)
                }
                onCertificateError: function(error) {
                    window.handleCertificateError(
                        viewUi, secondaryView, error, viewHost)
                }
                onWebAuthUxRequested: function(request) {
                    window.handleWebAuthRequested(viewUi, secondaryView, request, viewHost)
                }
                onContextMenuRequested: function(request) {
                    window.handleContextMenuRequested(viewUi, secondaryView, request, viewHost)
                }
                onFileDialogRequested: function(request) {
                    if (viewHost === secondaryWindow) {
                        secondaryWindow.handleFileDialogRequested(request)
                    } else {
                        window.handleFileDialogRequested(request, secondaryView, viewUi)
                    }
                }
                onDesktopMediaRequested: function(request) {
                    window.handleDesktopMediaRequested(
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
                    window.handleRendererProcessTerminated(
                        viewUi, secondaryView, viewHost, -1,
                        terminationStatus, exitCode)
                }
                onNewWindowRequested: function(request) {
                    if (!viewUi.popup_allowed(request.requestedUrl.toString(), request.userInitiated)) {
                        return
                    }
                    var popup = popupWindowComponent.createObject(null, {
                        popupRequest: request,
                        popupJourneyToken: viewUi.take_popup_journey_token(
                            request.requestedUrl.toString()),
                        popupProfile: viewProfile,
                        popupRequestInterceptor: viewInterceptor,
                        popupPermissionUi: viewUi,
                        popupContextName: viewUi.context_name,
                        popupContextLabel: viewUi.context_label,
                        popupPrivateProfile: viewTransientProfile,
                        popupEphemeralProfile: secondaryWindow.windowEphemeralProfile,
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
                var fallback = webViewComponent.createObject(fallbackViewHost, {
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
                window.updateBrowserWindowView(secondaryUi, fallback)
                secondaryUi.status_text = "Live tab moved; blank fallback tab created"
                return true
            }

            Rectangle {
                id: secondaryCommandBar
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                height: window.inputBarHeight
                z: 20
                visible: secondaryUi.mode === "command"
                color: window.surfaceColor

                RowLayout {
                    anchors.fill: parent
                    spacing: 0

                    Rectangle {
                        Layout.fillHeight: true
                        Layout.preferredWidth: secondaryCommandPrefix.implicitWidth + 16
                        color: window.accentColor

                        Label {
                            id: secondaryCommandPrefix
                            anchors.centerIn: parent
                            text: ":"
                            color: window.contrastText(parent.color)
                            font.bold: true
                            Accessible.ignored: true
                        }
                    }

                    TextField {
                        id: secondaryCommandLine
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        leftPadding: 8
                        rightPadding: 8
                        topPadding: 0
                        bottomPadding: 0
                        color: window.primaryTextColor
                        selectionColor: window.selectionColor
                        selectedTextColor: window.selectionTextColor
                        placeholderText: "command"
                        placeholderTextColor: window.mutedTextColor
                        background: Rectangle { color: "transparent" }
                        focus: secondaryCommandBar.visible
                        Accessible.name: "Command line"
                        Accessible.role: Accessible.EditableText
                        Accessible.editable: true
                        onVisibleChanged: if (visible) forceActiveFocus()
                        onTextChanged: secondaryUi.update_completion(text, cursorPosition)
                        onCursorPositionChanged:
                            secondaryUi.update_completion(text, cursorPosition)
                        onAccepted: {
                            if (secondaryUi.execute_command(text)) {
                                text = ""
                                secondaryWindow.applySwitcherEngineAction()
                                if (secondaryUi.mode === "command") {
                                    secondaryUi.escape()
                                }
                            } else {
                                selectAll()
                            }
                        }
                        Keys.onPressed: function(event) {
                            if (event.key === Qt.Key_Escape) {
                                secondaryUi.escape()
                                event.accepted = true
                            } else if (event.key === Qt.Key_Tab) {
                                secondaryUi.completion_move(
                                    event.modifiers & Qt.ShiftModifier ? -1 : 1)
                                event.accepted = true
                            }
                        }
                    }
                }
            }

            Rectangle {
                id: secondarySearchBar
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                height: window.inputBarHeight
                z: 20
                visible: secondaryUi.mode === "search"
                color: window.surfaceColor

                Label {
                    id: secondarySearchPrefix
                    anchors.left: parent.left
                    anchors.verticalCenter: parent.verticalCenter
                    width: window.inputBarHeight
                    text: secondaryUi.search_backward ? "?" : "/"
                    color: window.warningColor
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
                    color: window.primaryTextColor
                    selectionColor: window.selectionColor
                    selectedTextColor: window.selectionTextColor
                    placeholderText: "search"
                    placeholderTextColor: window.mutedTextColor
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

            Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                height: window.statusBarHeight
                z: 10
                visible: window.statusbarMode === "always"
                         && secondaryUi.mode !== "command"
                         && secondaryUi.mode !== "search"
                color: window.surfaceColor
                opacity: window.chromeOpacity

                Accessible.name: "Browser status bar. " + secondaryUi.mode + ". "
                                 + secondaryUi.display_url + ". "
                                 + secondaryUi.status_text
                                 + window.statusDetails(
                                     secondaryUi, secondaryWindow,
                                     secondaryWindow.activeView,
                                     secondaryWindow.windowPrivateProfile,
                                     secondaryWindow.windowProfileName,
                                     secondaryWindow.windowEphemeralProfile)
                Accessible.role: Accessible.StatusBar

                RowLayout {
                    anchors.fill: parent
                    spacing: 0

                    Rectangle {
                        Layout.fillHeight: true
                        Layout.preferredWidth: secondaryModeLabel.implicitWidth + 16
                        color: secondaryUi.mode === "insert"
                               ? window.modeInsertColor : window.panelColor

                        Label {
                            id: secondaryModeLabel
                            anchors.centerIn: parent
                            text: secondaryUi.mode.toUpperCase()
                            color: secondaryUi.mode === "normal"
                                   ? window.primaryTextColor : window.backgroundColor
                            font.bold: true
                            Accessible.ignored: true
                        }
                    }

                    Label {
                        Layout.fillWidth: true
                        Layout.leftMargin: 8
                        Layout.rightMargin: 8
                        text: window.addressPresentation(
                                  secondaryUi.display_url,
                                  width / Math.max(1, font.pixelSize * 0.56))
                        color: /^https:/i.test(secondaryUi.display_url)
                               ? window.successColor : window.primaryTextColor
                        elide: Text.ElideMiddle
                        Accessible.ignored: true
                    }

                    Label {
                        Layout.maximumWidth: Math.max(120, parent.width * 0.32)
                        Layout.rightMargin: 8
                        text: secondaryUi.status_text
                        color: window.contextStatusColor(
                                   secondaryUi, window.mutedTextColor)
                        elide: Text.ElideRight
                        horizontalAlignment: Text.AlignRight
                        Accessible.ignored: true
                    }

                    Label {
                        Layout.rightMargin: 8
                        text: secondaryWindow.windowEphemeralProfile ? "EPHEMERAL"
                              : secondaryWindow.windowPrivateProfile ? "PRIVATE"
                              : secondaryWindow.windowProfileName
                        color: window.contextStatusColor(
                                   secondaryUi, window.secondaryTextColor)
                        font.bold: true
                        Accessible.ignored: true
                    }
                }
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
                window.updateBrowserWindowView(secondaryUi, view)
                if (!window.completeDetachedSource(sourceUi, sourceHost, payload)) {
                    view.parent = null
                    view.visible = false
                    secondaryUi.rollback_tab_transfer(adoptedTabId)
                    window.restoreSourceView(sourceUi, sourceHost, view)
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

            Component.onCompleted: {
                secondaryWindow.permissionPromptSurface =
                        permissionPromptSurfaceComponent.createObject(
                            secondaryWindow.contentItem, { hostWindow: secondaryWindow })
                secondaryWindow.desktopMediaSurface =
                        desktopMediaSurfaceComponent.createObject(
                            secondaryWindow.contentItem, { hostWindow: secondaryWindow })
                secondaryWindow.captureIndicatorSurface =
                        captureIndicatorComponent.createObject(
                            secondaryWindow.contentItem, { hostWindow: secondaryWindow })
                secondaryWindow.rendererFailureSurface =
                        rendererFailureSurfaceComponent.createObject(
                            secondaryWindow.contentItem, { hostWindow: secondaryWindow })
                window.installFocusObserver(secondaryWindow.activeView)
                secondaryUi.config_json = browserUi.config_json
                secondaryUi.config_base_json = browserUi.config_base_json
                secondaryUi.cli_overrides_json = browserUi.cli_overrides_json
                secondaryUi.config_path = browserUi.config_path
                secondaryUi.config_source = browserUi.config_source
                secondaryUi.contexts_json = browserUi.contexts_json
                secondaryUi.configure_profile(
                                              secondaryWindow.windowPrivateProfile,
                                              secondaryWindow.windowEphemeralProfile,
                                              secondaryWindow.windowProfileLabel,
                                              secondaryWindow.windowProfileName,
                                              window.storageBasePath)
                if (secondaryWindow.windowBookmarkTransferUrl.length > 0) {
                    bookmarkTransferTimer.start()
                }
                secondaryUi.context_entry_force_reuse = secondaryWindow.windowStartupContextRestore
                window.registerBrowserWindow(
                            secondaryWindow, secondaryUi, secondaryWindow.activeView,
                            secondaryWindow.windowWebEngineProfile,
                            secondaryWindow.windowProfileName,
                            secondaryWindow.windowEphemeralInvocationToken,
                            secondaryWindow.windowEphemeralProfile)
                if (!secondaryWindow.windowSharedProfile
                        && !secondaryRequestInterceptor.attach(secondaryProfile)) {
                    secondaryUi.status_text = "Request interceptor unavailable"
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
                } else if (secondaryWindow.windowStartupContext.length > 0) {
                    secondaryUi.execute_command(
                                "context-enter " + secondaryWindow.windowStartupContext)
                    window.routeContextWorkspace(secondaryUi)
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
            }
            Component.onDestruction: {
                if (secondaryWindow.devToolsWindow) {
                    secondaryWindow.devToolsWindow.close()
                    secondaryWindow.devToolsWindow = null
                }
                window.unregisterBrowserWindow(secondaryUi)
                    window.clearPageDialogForView(secondaryView)
                    window.clearClientCertificateForView(secondaryView)
                    window.clearCertificateErrorForView(secondaryView)
                    window.clearWebAuthForView(secondaryView)
                    window.clearContextMenuForView(secondaryView)
                    window.clearDesktopMediaForView(secondaryView)
                    window.clearCaptureSessionForView(secondaryView)
        window.clearCaptureSessionsForHost(secondaryWindow)
                secondaryWindow.clearFileDialogForView(secondaryView)
                window.clearSiteDataClearForView(secondaryView)
                window.cancelPermissionForUi(secondaryUi)
                if (window.pendingDesktopMediaHost === secondaryWindow) {
                    window.clearDesktopMediaRequest(true)
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
                secondaryUi.release_transient_resources()
            }
        }
    }

    header: ToolBar {
        id: browserHeader
        height: window.tabPosition === "top" && window.tabStripVisible
                ? window.tabBarHeight : 0
        visible: height > 0

        background: Rectangle {
            color: window.panelColor

            Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                height: 1
                color: window.borderColor
            }
        }

        GridLayout {
            id: headerColumn
            columns: 1
            anchors.fill: parent
            rowSpacing: 0

            GridLayout {
                id: compactTabStrip
                property bool vertical: window.sideTabs
                parent: window.tabPosition === "top" ? headerColumn : window.contentItem
                columns: vertical ? 1 : Math.max(1, tabs.count + 1)
                rows: vertical ? Math.max(1, tabs.count + 1) : 1
                x: parent === headerColumn ? 0
                   : (window.tabPosition === "right" ? parent.width - width : 0)
                y: parent === headerColumn || vertical ? 0
                   : parent.height - window.bottomChromeHeight
                width: parent === headerColumn ? headerColumn.width
                       : (vertical ? window.sideTabWidth : parent.width)
                height: parent === headerColumn ? window.tabBarHeight
                        : (vertical
                           ? parent.height
                             - window.bottomChromeHeight
                           : window.tabBarHeight)
                z: 9
                Accessible.role: Accessible.PageTabList
                Accessible.name: "Browser tabs"
                Accessible.description: tabs.count + " browser tabs"
                Layout.row: 0
                Layout.fillWidth: parent === headerColumn
                Layout.preferredHeight: window.tabBarHeight
                Layout.maximumHeight: visible && parent === headerColumn
                                      ? window.tabBarHeight : 0
                rowSpacing: 1
                columnSpacing: 1
                visible: window.tabStripVisible

                Repeater {
                    model: tabs
                    delegate: Rectangle {
                        property int tabIndex: index
                        Layout.fillWidth: true
                        Layout.minimumWidth: compactTabStrip.vertical
                                             ? compactTabStrip.width : 48
                        Layout.maximumWidth: compactTabStrip.vertical
                                             ? compactTabStrip.width : Number.POSITIVE_INFINITY
                        Layout.fillHeight: !compactTabStrip.vertical
                        Layout.preferredHeight: window.tabBarHeight
                        Layout.maximumHeight: window.tabBarHeight
                        color: tabIndex === browserUi.active_tab_index
                               ? window.surfaceColor : window.backgroundColor

                        Accessible.role: Accessible.PageTab
                        Accessible.name: "Tab " + (tabIndex + 1) + ": "
                                         + (model.title || "New tab")
                        Accessible.selected: tabIndex === browserUi.active_tab_index

                        Text {
                            id: tabTitle
                            anchors.left: parent.left
                            anchors.leftMargin: 9
                            anchors.right: tabClose.left
                            anchors.rightMargin: 4
                            anchors.verticalCenter: parent.verticalCenter
                            color: tabIndex === browserUi.active_tab_index
                                   ? window.primaryTextColor : window.mutedTextColor
                            text: (tabIndex + 1) + "  "
                                  + (model.pinned ? "◆ " : "")
                                  + (model.muted ? "[M] " : "")
                                  + (model.title || "New tab")
                            elide: Text.ElideRight
                        }

                        ToolButton {
                            id: tabClose
                            anchors.right: parent.right
                            anchors.verticalCenter: parent.verticalCenter
                            width: visible ? window.tabBarHeight : 0
                            height: parent.height
                            text: "×"
                            padding: 0
                            visible: tabIndex === browserUi.active_tab_index
                                     || tabMouse.containsMouse
                            background: Item {}
                            contentItem: Text {
                                text: tabClose.text
                                color: window.mutedTextColor
                                horizontalAlignment: Text.AlignHCenter
                                verticalAlignment: Text.AlignVCenter
                            }
                            onClicked: {
                                if (window.closeTabAtIndex(tabIndex)) {
                                    window.executePendingEngineAction()
                                }
                            }
                        }

                        MouseArea {
                            id: tabMouse
                            anchors.fill: parent
                            anchors.rightMargin: tabClose.visible ? tabClose.width : 0
                            hoverEnabled: true
                            onClicked: {
                                var tabId = browserUi.tab_id_for_index(tabIndex)
                                if (tabId.length > 0
                                        && browserUi.execute_ui_action(
                                            "browser.tab.select", tabId)) {
                                    tabs.setProperty(tabIndex, "loaded", true)
                                    window.executePendingEngineAction()
                                }
                            }
                        }

                        Rectangle {
                            width: compactTabStrip.vertical ? 2 : parent.width
                            height: compactTabStrip.vertical ? parent.height : 2
                            anchors.left: parent.left
                            anchors.bottom: parent.bottom
                            color: window.accentColor
                            visible: tabIndex === browserUi.active_tab_index
                        }
                    }
                }

                ToolButton {
                    text: "+"
                    Layout.fillWidth: compactTabStrip.vertical
                    Layout.preferredWidth: compactTabStrip.vertical
                                           ? compactTabStrip.width : window.tabBarHeight
                    Layout.maximumWidth: compactTabStrip.vertical
                                         ? compactTabStrip.width : window.tabBarHeight
                    Layout.preferredHeight: window.tabBarHeight
                    padding: 0
                    background: Rectangle {
                        color: parent.hovered ? window.surfaceColor : window.backgroundColor
                    }
                    contentItem: Text {
                        text: parent.text
                        color: window.mutedTextColor
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }
                    Accessible.name: "New tab"
                    onClicked: {
                        var index = browserUi.new_tab()
                        if (index >= 0) {
                            tabs.append({ url: "about:blank", title: "New tab",
                                loaded: true, pinned: false, muted: false, zoom: 1.0,
                                suspended: false, discarded: false })
                            window.syncTabModel()
                        }
                    }
                }
            }

            RowLayout {
                Layout.row: 1
                Layout.fillWidth: true
                Layout.preferredHeight: 0
                Layout.maximumHeight: 0
                spacing: 8
                visible: false

                ToolButton {
                    text: "‹"
                    Accessible.name: "Back"
                    onClicked: {
                        var historyView = window.activeWebView()
                        if (historyView && historyView.canGoBack) {
                            browserUi.back()
                            historyView.goBack()
                        } else {
                            browserUi.status_text = "History boundary reached"
                        }
                    }
                }
                ToolButton {
                    text: "›"
                    Accessible.name: "Forward"
                    onClicked: {
                        var historyView = window.activeWebView()
                        if (historyView && historyView.canGoForward) {
                            browserUi.forward()
                            historyView.goForward()
                        } else {
                            browserUi.status_text = "History boundary reached"
                        }
                    }
                }
                TextField {
                    id: address
                    Layout.fillWidth: true
                    property bool addressEditing: false
                    function refreshAddressPresentation() {
                        if (!addressEditing) {
                            text = window.addressPresentation(
                                browserUi.display_url,
                                width / Math.max(1, font.pixelSize * 0.56))
                        }
                    }
                    text: window.addressPresentation(
                        browserUi.display_url,
                        width / Math.max(1, font.pixelSize * 0.56))
                    placeholderText: "Enter a URL"
                    Accessible.name: "Address"
                    Accessible.description: browserUi.display_url
                    Accessible.role: Accessible.EditableText
                    Accessible.editable: true
                    selectByMouse: true
                    onActiveFocusChanged: {
                        addressEditing = activeFocus
                        if (addressEditing) {
                            text = browserUi.display_url
                            cursorPosition = text.length
                        } else {
                            refreshAddressPresentation()
                        }
                    }
                    onWidthChanged: refreshAddressPresentation()
                    Connections {
                        target: browserUi
                        function onDisplay_urlChanged() {
                            address.refreshAddressPresentation()
                        }
                    }
                    onAccepted: {
                        browserUi.navigate(text)
                        window.updateActiveTabUrl()
                    }
                }
                ToolButton {
                    text: "⟳"
                    Accessible.name: "Reload"
                    onClicked: {
                        browserUi.reload()
                        window.activeWebView()?.reload()
                    }
                }
                Label {
            text: browserUi.mode + " · " + browserUi.status_text
                  + window.contextStatus(
                      browserUi, window.temporaryProfile, window.profileName,
                      window.ephemeralProfile)
                  + window.siteDoctorBadge(browserUi)
                  + (window.recoveryAvailable ? " · recovery available" : "")
                    Accessible.name: "Browser status"
                    color: window.contextStatusColor(
                               browserUi, window.primaryTextColor)
                }
                ToolButton {
                    text: "Window"
                    visible: false
                    Accessible.name: "New profile window"
                    onClicked: {
                        browserWindowComponent.createObject(null, {
                            windowStartupUrl: browserUi.current_url,
                            windowProfileName: "secondary",
                            windowPrivateProfile: false
                        })
                    }
                }
                ToolButton {
                    text: "Private"
                    visible: false
                    contentItem: Text {
                        text: parent.text
                        color: window.privateColor
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                        elide: Text.ElideRight
                    }
                    Accessible.name: "New private window"
                    onClicked: {
                        browserWindowComponent.createObject(null, {
                            windowStartupUrl: "about:blank",
                            windowProfileName: "private",
                            windowPrivateProfile: true
                        })
                    }
                }
                ToolButton {
                    text: "Sessions"
                    visible: false
                    Accessible.name: "Open session manager"
                    onClicked: {
                        window.refreshSessions()
                        window.openInternalSurface()
                        window.sessionManagerVisible = true
                    }
                }
                ToolButton {
                    text: "Profiles"
                    visible: false
                    Accessible.name: "Open profile manager"
                    onClicked: {
                        window.refreshProfiles()
                        window.openInternalSurface()
                        window.profileManagerVisible = true
                    }
                }
                ToolButton {
                    text: "Downloads"
                    visible: false
                    Accessible.name: "Open downloads manager"
                    onClicked: {
                        window.refreshDownloads()
                        window.openInternalSurface()
                        window.downloadManagerVisible = true
                    }
                }
                ToolButton {
                    text: "Diagnostics"
                    visible: false
                    Accessible.name: "Open diagnostics"
                    onClicked: window.showDiagnostics()
                }
                ToolButton {
                    text: "Help"
                    visible: false
                    Accessible.name: "Open binding help"
                    onClicked: window.showBindingHelp()
                }
                ToolButton {
                    text: "Settings"
                    visible: false
                    Accessible.name: "Open settings"
                    onClicked: window.showSettings()
                }
                ToolButton {
                    text: "Undo tab"
                    visible: false
                    Accessible.name: "Reopen most recently closed tab"
                    onClicked: {
                        if (browserUi.execute_ui_action("browser.tab.undo", "")) {
                            window.syncTabModel()
                            window.executePendingEngineAction()
                        }
                    }
                }
                ToolButton {
                    text: "Clone tab"
                    visible: false
                    Accessible.name: "Clone current tab"
                    onClicked: {
                        if (browserUi.execute_command(":tab-clone")) {
                            window.syncTabModel()
                            window.executePendingEngineAction()
                        }
                    }
                }
                ToolButton {
                    text: "Reopen window"
                    visible: false
                    Accessible.name: "Reopen tab in same-profile window; live state will be lost"
                    onClicked: {
                        if (browserUi.execute_ui_action("browser.tab.reopen-window", "")) {
                            window.executePendingEngineAction()
                        }
                    }
                }
                ToolButton {
                    text: "−"
                    visible: false
                    Accessible.name: "Zoom out"
                    onClicked: {
                        if (browserUi.execute_ui_action("browser.tab.zoom", "out")) {
                            window.executePendingEngineAction()
                        }
                    }
                }
                ToolButton {
                    text: "100%"
                    visible: false
                    Accessible.name: "Reset page zoom"
                    onClicked: {
                        if (browserUi.execute_ui_action("browser.tab.zoom", "reset")) {
                            window.executePendingEngineAction()
                        }
                    }
                }
                ToolButton {
                    text: "+"
                    visible: false
                    Accessible.name: "Zoom in"
                    onClicked: {
                        if (browserUi.execute_ui_action("browser.tab.zoom", "in")) {
                            window.executePendingEngineAction()
                        }
                    }
                }
            }

            RowLayout {
                Layout.row: 2
                Accessible.role: Accessible.PageTabList
                Accessible.name: "Browser tabs"
                Layout.fillWidth: true
                spacing: 4
                visible: false

                Repeater {
                    id: tabButtons
                    model: tabs
                    delegate: RowLayout {
                        property int tabIndex: index
                        spacing: 0
                        Layout.preferredWidth: Math.min(400, Math.max(150, tabLabel.implicitWidth + pinTab.implicitWidth + muteTab.implicitWidth + lifecycleTab.implicitWidth + discardTab.implicitWidth + moveLeft.implicitWidth + moveRight.implicitWidth + closeTab.implicitWidth + 24))

                        ToolButton {
                            id: tabLabel
                            Layout.fillWidth: true
                            text: (title || "New tab")
                                  + (window.permissionPendingForTab(tabIndex)
                                     ? " · permission"
                                     : "")
                                  + (tabViewAt(tabIndex)
                                     && tabViewAt(tabIndex).rendererFailed
                                     ? " · renderer failed"
                                     : "")
                            highlighted: tabIndex === browserUi.active_tab_index
                            Accessible.role: Accessible.PageTab
                            Accessible.name: "Tab " + (tabIndex + 1) + ": " + text
                            Accessible.selected: tabIndex === browserUi.active_tab_index
                            Accessible.description: model.pinned
                                  ? "Pinned tab"
                                  : "Browser tab"
                            onClicked: {
                                var tabId = browserUi.tab_id_for_index(tabIndex)
                                if (tabId.length === 0
                                        || !browserUi.execute_ui_action(
                                            "browser.tab.select", tabId)) {
                                    return
                                }
                                if (browserUi.active_tab_index === tabIndex) {
                                    tabs.setProperty(tabIndex, "loaded", true)
                                    window.executePendingEngineAction()
                                }
                                window.showRendererFailureForTab(tabIndex)
                            }
                        }
                        ToolButton {
                            id: pinTab
                            text: model.pinned ? "📌" : "pin"
                            Accessible.name: model.pinned ? "Unpin tab " + (tabIndex + 1) : "Pin tab " + (tabIndex + 1)
                            onClicked: {
                                var nextPinned = !model.pinned
                                var tabId = browserUi.tab_id_for_index(tabIndex)
                                if (tabId.length === 0
                                        || !browserUi.execute_ui_action("browser.tab.pin", tabId)) {
                                    return
                                }
                                var newIndex = browserUi.tab_index_for_id(tabId)
                                if (newIndex >= 0 && newIndex !== tabIndex) {
                                    tabs.move(tabIndex, newIndex, 1)
                                }
                                if (newIndex >= 0) {
                                    tabs.setProperty(newIndex, "pinned", nextPinned)
                                }
                                window.syncTabModel()
                            }
                        }
                        ToolButton {
                            id: muteTab
                            text: model.muted ? "🔇" : "mute"
                            Accessible.name: model.muted ? "Unmute tab " + (tabIndex + 1) : "Mute tab " + (tabIndex + 1)
                            onClicked: {
                                var nextMuted = !model.muted
                                var tabId = browserUi.tab_id_for_index(tabIndex)
                                if (tabId.length === 0
                                        || !browserUi.execute_ui_action("browser.tab.mute", tabId)) {
                                    return
                                }
                                var newIndex = browserUi.tab_index_for_id(tabId)
                                if (newIndex >= 0) {
                                    tabs.setProperty(newIndex, "muted", nextMuted)
                                }
                            }
                        }
                        ToolButton {
                            id: lifecycleTab
                            text: model.suspended || model.discarded ? "resume" : "freeze"
                            enabled: model.suspended || model.discarded || tabIndex !== browserUi.active_tab_index
                            Accessible.name: model.suspended || model.discarded
                                              ? "Resume tab " + (tabIndex + 1)
                                              : "Freeze hidden tab " + (tabIndex + 1)
                            onClicked: {
                                var tabId = browserUi.tab_id_for_index(tabIndex)
                                var action = model.suspended || model.discarded
                                        ? "browser.tab.resume" : "browser.tab.suspend"
                                if (tabId.length === 0
                                        || !browserUi.execute_ui_action(action, tabId)) {
                                    return
                                }
                                window.executePendingEngineAction()
                            }
                        }
                        ToolButton {
                            id: discardTab
                            text: "discard"
                            enabled: !model.suspended && !model.discarded
                                     && tabIndex !== browserUi.active_tab_index
                            Accessible.name: "Discard hidden tab " + (tabIndex + 1)
                            onClicked: {
                                var tabId = browserUi.tab_id_for_index(tabIndex)
                                if (tabId.length === 0
                                        || !browserUi.execute_ui_action("browser.tab.discard", tabId)) {
                                    return
                                }
                                window.executePendingEngineAction()
                            }
                        }
                        ToolButton {
                            id: moveContext
                            text: "context"
                            Accessible.name: "Move tab " + (tabIndex + 1) + " to another context window"
                            enabled: !model.suspended && !model.discarded
                            onClicked: {
                                var tabId = browserUi.tab_id_for_index(tabIndex)
                                if (tabId.length > 0) {
                                    window.showContextMovePicker(tabId)
                                }
                            }
                        }
                        ToolButton {
                            id: moveLeft
                            text: "←"
                            Accessible.name: "Move tab left"
                            onClicked: {
                                var tabId = browserUi.tab_id_for_index(tabIndex)
                                if (tabId.length === 0
                                        || !browserUi.execute_ui_action(
                                            "browser.tab.move", tabId + "\tleft")) {
                                    return
                                }
                                var newIndex = browserUi.tab_index_for_id(tabId)
                                if (newIndex >= 0 && newIndex !== tabIndex) {
                                    tabs.move(tabIndex, newIndex, 1)
                                }
                                window.syncTabModel()
                            }
                        }
                        ToolButton {
                            id: moveRight
                            text: "→"
                            Accessible.name: "Move tab right"
                            onClicked: {
                                var tabId = browserUi.tab_id_for_index(tabIndex)
                                if (tabId.length === 0
                                        || !browserUi.execute_ui_action(
                                            "browser.tab.move", tabId + "\tright")) {
                                    return
                                }
                                var newIndex = browserUi.tab_index_for_id(tabId)
                                if (newIndex >= 0 && newIndex !== tabIndex) {
                                    tabs.move(tabIndex, newIndex, 1)
                                }
                                window.syncTabModel()
                            }
                        }
                        ToolButton {
                            id: closeTab
                            text: "×"
                            Accessible.name: "Close tab " + (tabIndex + 1)
                            onClicked: {
                                if (window.closeTabAtIndex(tabIndex)) {
                                    window.executePendingEngineAction()
                                }
                            }
                        }
                    }
                }

                ToolButton {
                    text: "+"
                    Accessible.name: "New tab"
                    onClicked: {
                        var index = browserUi.new_tab()
                        if (index >= 0) {
                            tabs.append({ url: "about:blank", title: "New tab", loaded: true, pinned: false, muted: false, zoom: 1.0, suspended: false, discarded: false })
                            window.syncTabModel()
                        }
                    }
                }
            }
        }
    }

    FerricContextMoveDialog {
        id: contextMovePopup
        browserWindow: window
    }

    FerricExternalNavigationDialog {
        id: externalNavigationPopup
        browserWindow: window
        browserUi: browserUi
    }

    Popup {
        id: contextRoutePopup
        parent: Overlay.overlay
        modal: true
        focus: true
        closePolicy: Popup.NoAutoClose
        visible: window.contextRouteVisible
        width: Math.min(680 * window.chromeScale, window.width - 48)
        padding: 14
        x: Math.round((window.width - width) / 2)
        y: Math.round((window.height - height) / 2)

        background: Rectangle {
            color: window.panelColor
            border.color: window.accentColor
            radius: 4
        }

        contentItem: ColumnLayout {
            focus: true
            Accessible.role: Accessible.Dialog
            Accessible.name: "Context route confirmation"
            spacing: 10

            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    window.dismissContextRoute()
                    event.accepted = true
                } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                    window.acceptContextRoute()
                    event.accepted = true
                }
            }

            Label {
                Layout.fillWidth: true
                text: "Use browsing context " + (window.contextRouteData.context || "") + "?"
                color: window.primaryTextColor
                font.bold: true
                Accessible.name: "Context route title"
            }

            Label {
                Layout.fillWidth: true
                text: "Route " + (window.contextRouteData.route_id || "")
                      + " (" + (window.contextRouteData.behavior || "prompt") + ")"
                color: window.mutedTextColor
                elide: Text.ElideRight
            }

            Label {
                Layout.fillWidth: true
                text: "Target profile: " + (window.contextRouteData.profile || "unavailable")
                color: window.secondaryTextColor
                elide: Text.ElideMiddle
            }

            Label {
                Layout.fillWidth: true
                text: "Address: " + (window.contextRouteData.url || "unavailable")
                color: window.primaryTextColor
                wrapMode: Text.WrapAnywhere
                maximumLineCount: 6
                elide: Text.ElideRight
                Accessible.name: "Context route address"
            }

            Label {
                Layout.fillWidth: true
                text: "This choice applies before navigation. Existing redirects, popups, forms, permissions, and authentication chains are never moved automatically."
                color: window.warningColor
                wrapMode: Text.WordWrap
            }

            RowLayout {
                Layout.fillWidth: true
                Item { Layout.fillWidth: true }
                Button {
                    text: "Open normally"
                    Accessible.name: "Open without context route"
                    onClicked: window.dismissContextRoute()
                }
                Button {
                    text: "Use context"
                    Accessible.name: "Accept context route"
                    onClicked: window.acceptContextRoute()
                }
            }
        }

    }

    FerricNavigationFailure {
        id: navigationFailureSurface
        browserWindow: window
        browserUi: browserUi
    }

    Rectangle {
        id: recoveryBanner
        anchors.top: parent.top
        anchors.topMargin: 76
        anchors.left: parent.left
        anchors.right: parent.right
        height: Math.max(42, window.chromeRowHeight * 2.75)
        z: 20
        visible: window.recoveryAvailable
        color: Qt.darker(window.warningColor, 2.2)

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 10
            anchors.rightMargin: 10
            spacing: 10

            Label {
                Layout.fillWidth: true
                text: "The previous browser run ended unexpectedly. Recover the last safe session?"
                color: window.primaryTextColor
                Accessible.name: "Session recovery notice"
            }
            Button {
                text: "Recover"
                Accessible.name: "Recover last session"
                onClicked: {
                    var restored = browserUi.recover_session()
                    if (restored === "QUEUED") {
                        window.sessionPreviewLoading = true
                        return
                    }
                    if (restored.length === 0) {
                        return
                    }
                    tabs.clear()
                    window.applyRestorePayload(restored, false)
                    window.recoveryAvailable = false
                    window.syncTabModel()
                    window.executePendingEngineAction()
                }
            }
            Button {
                text: "Dismiss"
                Accessible.name: "Dismiss session recovery"
                onClicked: window.recoveryAvailable = false
            }
        }
    }

    Rectangle {
        id: siteLedger
        anchors.centerIn: parent
        width: Math.min(900, parent.width - 80)
        height: Math.min(600, parent.height - 100)
        z: 70
        visible: window.siteLedgerVisible
        focus: visible
        Accessible.role: Accessible.Dialog
        Accessible.name: "Site Ledger"
        onVisibleChanged: if (visible) forceActiveFocus()
        color: window.panelColor
        border.color: window.accentColor
        border.width: 1

        Keys.onPressed: function(event) {
            if (event.key === Qt.Key_Escape) {
                window.siteLedgerVisible = false
                window.closeInternalSurface()
                event.accepted = true
            }
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 14
            spacing: 8

            RowLayout {
                Layout.fillWidth: true
                Label {
                    Layout.fillWidth: true
                    text: "Site Ledger"
                    color: window.primaryTextColor
                    font.bold: true
                    Accessible.name: "Site Ledger"
                }
                Button {
                    text: "Refresh"
                    Accessible.name: "Refresh Site Ledger"
                    onClicked: window.showSiteLedger()
                }
                Button {
                    text: "Close"
                    Accessible.name: "Close Site Ledger"
                    onClicked: {
                        window.siteLedgerVisible = false
                        window.closeInternalSurface()
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                visible: !!(!window.siteLedgerData.private && window.siteLedgerData.origin)
                CheckBox {
                    id: siteDataClearConfirmation
                    text: "Clear supported site data"
                    checked: false
                    Accessible.name: "Confirm supported site data clearing"
                    Accessible.description: "Clears page-origin local storage, Cache Storage, service-worker registrations, and visible cookies only; HTTP cache and other cookie-store entries remain"
                }
                Button {
                    text: "Clear active-origin data"
                    enabled: siteDataClearConfirmation.checked && !window.siteDataClearPending
                    Accessible.name: "Clear supported active-origin site data"
                    onClicked: {
                        var origin = window.siteLedgerData.origin
                        if (browserUi.site_data_clear(origin, true)) {
                            siteDataClearConfirmation.checked = false
                            window.executePendingEngineAction()
                        }
                    }
                }
                Label {
                    Layout.fillWidth: true
                    text: "Per-origin: local storage, Cache Storage, service workers; cookies are page-visible-only; HTTP cache is profile-wide and excluded"
                    color: window.secondaryTextColor
                    wrapMode: Text.WordWrap
                }
            }

            Label {
                Layout.fillWidth: true
                text: window.siteLedgerData.private
                      ? "Private session · site identity is withheld"
                      : (window.siteLedgerData.origin || "No normalized HTTP(S) origin")
                color: window.secondaryTextColor
                elide: Text.ElideMiddle
                Accessible.name: "Ledger origin"
            }

            RowLayout {
                Layout.fillWidth: true
                CheckBox {
                    id: siteReportHost
                    text: "Include current site host"
                    checked: false
                    Accessible.name: "Include current site host in report"
                    Accessible.description: "Host disclosure is opt-in; paths and query strings are never included"
                }
                Button {
                    text: "Copy sanitized report"
                    Accessible.name: "Copy sanitized site report"
                    onClicked: {
                        var report = browserUi.site_report(siteReportHost.checked)
                        if (report && report.length > 0) {
                            window.copyToClipboard(report, true)
                            browserUi.status_text = "Sanitized site report copied"
                        }
                    }
                }
                Label {
                    Layout.fillWidth: true
                    text: "URLs, cookies, tokens, DOM, and account identifiers are excluded"
                    color: window.secondaryTextColor
                    wrapMode: Text.WordWrap
                }
            }

            Label {
                Layout.fillWidth: true
                text: "Tab " + (window.siteLedgerData.capture && window.siteLedgerData.capture.tab_id || "unknown")
                      + " · document " + (window.siteLedgerData.capture && window.siteLedgerData.capture.document_id || "unknown")
                      + " · " + (window.siteLedgerData.renderer || "renderer unknown")
                color: window.mutedTextColor
                elide: Text.ElideRight
            }

            ListView {
                id: siteLedgerFacts
                Layout.fillWidth: true
                Layout.preferredHeight: Math.min(270, Math.max(66, siteLedgerFacts.contentHeight + 6))
                clip: true
                spacing: 6
                model: window.siteLedgerData.facts || []
                delegate: Rectangle {
                    width: siteLedgerFacts.width
                    height: Math.max(66, window.chromeRowHeight * 5)
                    color: window.surfaceColor
                    radius: 3

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: 8
                        spacing: 2
                        Label {
                            Layout.fillWidth: true
                            text: modelData.id + " · " + modelData.state + " · " + modelData.capability
                            color: window.primaryTextColor
                            font.bold: true
                            elide: Text.ElideRight
                            Accessible.name: modelData.id + " fact"
                        }
                        Label {
                            Layout.fillWidth: true
                            text: modelData.provenance + " · " + modelData.scope + " · " + modelData.apply_time
                            color: window.mutedTextColor
                            elide: Text.ElideRight
                        }
                        Label {
                            Layout.fillWidth: true
                            text: JSON.stringify(modelData.value)
                            color: window.secondaryTextColor
                            elide: Text.ElideRight
                        }
                    }
                }
            }

            Label {
                Layout.fillWidth: true
                text: "Recent request decisions"
                color: window.primaryTextColor
                font.bold: true
            }

            ListView {
                id: siteLedgerDecisions
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 3
                model: window.siteLedgerData.blocking
                      ? (window.siteLedgerData.blocking.active_site_decisions || [])
                      : []
                delegate: Label {
                    width: siteLedgerDecisions.width
                    text: (modelData.decision || "unknown") + " · "
                          + (modelData.resource_host || "unknown host") + " · "
                          + "list " + (modelData.list_id || "unknown") + " · "
                          + (modelData.exception_list_id
                             ? "exception " + modelData.exception_list_id + " · " : "")
                          + (modelData.reason || "no reason")
                    color: modelData.decision === "blocked" ? window.errorColor : window.successColor
                    elide: Text.ElideRight
                    Accessible.name: "Request decision"
                }
            }

            Label {
                Layout.fillWidth: true
                text: "Safe actions: " + ((window.siteLedgerData.safe_remediation_actions || []).join(", ") || "none")
                color: window.warningColor
                wrapMode: Text.WordWrap
            }

            Label {
                Layout.fillWidth: true
                visible: window.siteLedgerData.site_doctor
                         && window.siteLedgerData.site_doctor.active
                         && window.siteLedgerData.site_doctor.active.kind
                text: window.siteLedgerData.site_doctor
                      && window.siteLedgerData.site_doctor.active
                      ? "Active experiment: "
                        + window.siteLedgerData.site_doctor.active.kind
                        + " (temporary; "
                        + (window.siteLedgerData.site_doctor.active.remaining_seconds || 0)
                        + "s remaining; reload/navigation ends it)"
                      : ""
                color: window.accentColor
                wrapMode: Text.WordWrap
            }

            Label {
                Layout.fillWidth: true
                visible: !!(window.siteLedgerData.site_doctor
                         && window.siteLedgerData.site_doctor.last_result
                         && window.siteLedgerData.site_doctor.last_result.kind)
                text: window.siteLedgerData.site_doctor
                      && window.siteLedgerData.site_doctor.last_result
                      ? "Site Doctor proposal: "
                        + window.siteLedgerData.site_doctor.last_result.kind
                        + " · "
                        + window.siteLedgerData.site_doctor.last_result.security_effect
                      : ""
                color: window.warningColor
                wrapMode: Text.WordWrap
                Accessible.name: "Site Doctor durable fix proposal"
            }

            RowLayout {
                Layout.fillWidth: true
                visible: !!(window.siteLedgerData.site_doctor
                         && window.siteLedgerData.site_doctor.last_result
                         && window.siteLedgerData.site_doctor.last_result.kind === "blocker-exception")
                CheckBox {
                    id: siteDoctorProposalConfirmation
                    text: "I reviewed this host-scoped exception"
                    checked: false
                    Accessible.name: "Confirm Site Doctor blocker exception"
                    Accessible.description: "This saves a generated runtime override for the current host and does not change permissions or TLS"
                }
                Button {
                    text: "Apply reviewed fix"
                    enabled: siteDoctorProposalConfirmation.checked
                             && window.siteLedgerData.site_doctor.last_result.state === "pending-confirmation"
                    Accessible.name: "Apply reviewed Site Doctor fix"
                    onClicked: {
                        var proposal = window.siteLedgerData.site_doctor.last_result
                        if (browserUi.apply_site_doctor_proposal(proposal.id, true)) {
                            siteDoctorProposalConfirmation.checked = false
                            window.showSiteLedger()
                        }
                    }
                }
            }

            Button {
                Layout.alignment: Qt.AlignLeft
                visible: window.siteLedgerData.site_doctor
                         && window.siteLedgerData.site_doctor.experiments
                         && window.siteLedgerData.site_doctor.experiments.indexOf("blocking-bypass") >= 0
                enabled: browserUi.site_experiment_json.length === 0
                         || browserUi.site_experiment_json === "{}"
                text: "Try blocker bypass once"
                Accessible.name: "Run one-shot blocker bypass experiment"
                onClicked: {
                    var started = browserUi.begin_site_doctor_experiment("blocking-bypass")
                    if (started.length > 0) {
                        window.executePendingEngineAction()
                    }
                }
            }

            Button {
                Layout.alignment: Qt.AlignLeft
                visible: window.siteLedgerData.site_doctor
                         && window.siteLedgerData.site_doctor.experiments
                         && window.siteLedgerData.site_doctor.experiments.indexOf("compiled-defaults") >= 0
                enabled: browserUi.site_experiment_json.length === 0
                         || browserUi.site_experiment_json === "{}"
                text: "Try compiled-default site settings once"
                Accessible.name: "Run one-shot compiled-default site settings experiment"
                onClicked: {
                    var started = browserUi.begin_site_doctor_experiment("compiled-defaults")
                    if (started.length > 0) {
                        window.executePendingEngineAction()
                    }
                }
            }

            Button {
                Layout.alignment: Qt.AlignLeft
                visible: window.siteLedgerData.site_doctor
                         && window.siteLedgerData.site_doctor.experiments
                         && window.siteLedgerData.site_doctor.experiments.indexOf("userscripts-off") >= 0
                enabled: browserUi.site_experiment_json.length === 0
                         || browserUi.site_experiment_json === "{}"
                text: "Try without matching userscripts once"
                Accessible.name: "Run one-shot userscript-free experiment"
                onClicked: {
                    var started = browserUi.begin_site_doctor_experiment("userscripts-off")
                    if (started.length > 0) {
                        window.executePendingEngineAction()
                    }
                }
            }

            Button {
                Layout.alignment: Qt.AlignLeft
                visible: window.siteLedgerData.site_doctor
                         && window.siteLedgerData.site_doctor.experiments
                         && window.siteLedgerData.site_doctor.experiments.indexOf("fresh-view") >= 0
                enabled: browserUi.site_experiment_json.length === 0
                         || browserUi.site_experiment_json === "{}"
                text: "Open fresh same-profile view once"
                Accessible.name: "Run one-shot fresh same-profile view experiment"
                onClicked: {
                    var started = browserUi.begin_site_doctor_experiment("fresh-view")
                    if (started.length > 0) {
                        window.executePendingEngineAction()
                    }
                }
            }
        }
    }

    Rectangle {
        id: diagnosticsSurface
        anchors.centerIn: parent
        width: Math.min(900, parent.width - 80)
        height: Math.min(620, parent.height - 100)
        z: 72
        visible: window.diagnosticsVisible
        focus: visible
        Accessible.role: Accessible.Dialog
        Accessible.name: "Diagnostics"
        onVisibleChanged: if (visible) forceActiveFocus()
        color: window.panelColor
        border.color: window.accentColor
        border.width: 1

        Keys.onPressed: function(event) {
            if (event.key === Qt.Key_Escape) {
                window.closeDiagnostics()
                event.accepted = true
            }
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 14
            spacing: 8

            RowLayout {
                Layout.fillWidth: true
                Label {
                    Layout.fillWidth: true
                    text: "Diagnostics"
                    color: window.primaryTextColor
                    font.bold: true
                    Accessible.name: "Diagnostics"
                }
                Button {
                    text: "Refresh"
                    Accessible.name: "Refresh diagnostics"
                    onClicked: window.refreshDiagnostics()
                }
                Button {
                    text: "Copy preview"
                    Accessible.name: "Copy diagnostics preview"
                    onClicked: window.copyToClipboard(window.diagnosticsText, true)
                }
                Button {
                    text: "Save preview"
                    Accessible.name: "Save diagnostics preview"
                    onClicked: {
                        window.refreshDiagnostics()
                        diagnosticsExportChooser.currentFile = "ferric-browser-diagnostics.json"
                        diagnosticsExportChooser.open()
                    }
                }
                Button {
                    text: "Close"
                    Accessible.name: "Close diagnostics"
                    onClicked: window.closeDiagnostics()
                }
            }

            Label {
                Layout.fillWidth: true
                text: "Read-only, privacy-safe runtime snapshot. Probe values are bounded and may be marked unavailable or not tested."
                color: window.secondaryTextColor
                wrapMode: Text.WordWrap
            }

            Label {
                Layout.fillWidth: true
                text: "Qt logical-unit layout · current screen scale: " + window.displayScaleLabel
                color: window.mutedTextColor
                Accessible.name: "Current screen scale"
            }

            Label {
                Layout.fillWidth: true
                text: "Window activation: " + window.activationStatus
                color: window.activationStatus === "unknown"
                      ? window.warningColor : window.mutedTextColor
                Accessible.name: "Window activation status"
            }

            Label {
                Layout.fillWidth: true
                visible: window.themeContrastWarning.length > 0
                text: "Theme contrast warning: " + window.themeContrastWarning
                color: window.warningColor
                wrapMode: Text.WordWrap
                Accessible.name: "Theme contrast warning"
                Accessible.description: "The imported theme remains enabled, but one or more assessed chrome color pairs are below WCAG AA."
            }

            Label {
                Layout.fillWidth: true
                text: window.reducedMotionActive
                      ? "Optional interface motion is disabled (reduced-motion setting: system or on)."
                      : "Optional interface motion is enabled by the reduced-motion setting."
                color: window.mutedTextColor
                wrapMode: Text.WordWrap
                Accessible.name: "Reduced motion status"
            }

            ScrollView {
                id: diagnosticsScroll
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true

                TextArea {
                    id: diagnosticsTextArea
                    width: diagnosticsScroll.availableWidth
                    height: Math.max(diagnosticsScroll.availableHeight, contentHeight + 16)
                    text: window.diagnosticsText
                    readOnly: true
                    selectByMouse: true
                    wrapMode: TextEdit.NoWrap
                    color: window.primaryTextColor
                    selectionColor: window.accentColor
                    selectedTextColor: window.backgroundColor
                    font.family: window.chromeFontFamily
                    Accessible.name: "Read-only diagnostics snapshot"
                    Accessible.description: "Privacy-safe runtime diagnostics in JSON format"
                }
            }
        }
    }

    Rectangle {
        id: bindingHelpSurface
        anchors.centerIn: parent
        width: Math.min(980, parent.width - 70)
        height: Math.min(650, parent.height - 90)
        z: 74
        visible: window.bindingHelpVisible
        focus: visible
        Accessible.role: Accessible.Dialog
        Accessible.name: "Binding help"
        onVisibleChanged: if (visible) forceActiveFocus()
        color: window.panelColor
        border.color: window.accentColor
        border.width: 1

        Keys.onPressed: function(event) {
            if (event.key === Qt.Key_Escape) {
                window.closeBindingHelp()
                event.accepted = true
            }
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 14
            spacing: 8

            RowLayout {
                Layout.fillWidth: true
                Label {
                    Layout.fillWidth: true
                    text: "Keyboard help"
                    color: window.primaryTextColor
                    font.bold: true
                    Accessible.name: "Keyboard help"
                }
                Button {
                    text: "Refresh"
                    Accessible.name: "Refresh keyboard help"
                    onClicked: window.refreshBindingHelp()
                }
                Button {
                    text: "Close"
                    Accessible.name: "Close keyboard help"
                    onClicked: window.closeBindingHelp()
                }
            }

            TextField {
                id: helpSearchInput
                Layout.fillWidth: true
                text: window.bindingHelpSearch
                placeholderText: "Search commands, keys, modes, or descriptions"
                Accessible.name: "Search keyboard help"
                Accessible.role: Accessible.EditableText
                onTextChanged: {
                    window.bindingHelpSearch = text
                    window.rebuildBindingHelpRows()
                }
                Keys.onPressed: function(event) {
                    if (event.key === Qt.Key_Escape) {
                        window.closeBindingHelp()
                        event.accepted = true
                    }
                }
            }

            Label {
                Layout.fillWidth: true
                text: "Effective bindings are generated from the active validated trie. ‘unbound’ commands remain available through the command line."
                color: window.secondaryTextColor
                wrapMode: Text.WordWrap
            }

            ScrollView {
                id: bindingHelpScroll
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true

                ListView {
                    id: bindingHelpList
                    width: bindingHelpScroll.availableWidth
                    height: bindingHelpScroll.availableHeight
                    clip: true
                    model: window.bindingHelpRows
                    spacing: 4
                    delegate: Rectangle {
                        width: bindingHelpList.width
                        height: modelData.kind === "heading" ? 30 : 76
                        color: modelData.kind === "heading"
                               ? window.surfaceColor
                               : modelData.kind === "conflict"
                                   ? Qt.darker(window.errorColor, 2.0)
                                   : window.surfaceColor
                        radius: 3
                        Accessible.name: modelData.kind === "heading"
                                         ? modelData.title
                                         : modelData.mode + " " + modelData.command
                                           + " " + modelData.keys

                        Label {
                            anchors.fill: parent
                            anchors.margins: 8
                            visible: modelData.kind === "heading"
                            text: modelData.title
                            color: window.accentColor
                            font.bold: true
                            verticalAlignment: Text.AlignVCenter
                        }

                        RowLayout {
                            anchors.fill: parent
                            anchors.margins: 8
                            visible: modelData.kind !== "heading"
                            spacing: 8
                            Label {
                                Layout.preferredWidth: 100
                                text: modelData.mode
                                color: modelData.kind === "conflict"
                                       ? window.warningColor : window.mutedTextColor
                                elide: Text.ElideRight
                            }
                            Label {
                                Layout.preferredWidth: 180
                                text: modelData.command
                                color: window.primaryTextColor
                                font.bold: true
                                elide: Text.ElideRight
                            }
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 2
                                Label {
                                    Layout.fillWidth: true
                                    text: modelData.keys
                                    color: modelData.kind === "conflict"
                                           ? window.warningColor : window.accentColor
                                    elide: Text.ElideRight
                                }
                                Label {
                                    Layout.fillWidth: true
                                    text: modelData.description
                                    color: window.secondaryTextColor
                                    elide: Text.ElideRight
                                }
                            }
                            Label {
                                Layout.preferredWidth: 120
                                text: modelData.source + " · " + modelData.count
                                color: window.mutedTextColor
                                elide: Text.ElideRight
                            }
                        }
                    }
                }
            }
        }
    }

    Rectangle {
        id: settingsSurface
        anchors.centerIn: parent
        width: Math.min(980, parent.width - 70)
        height: Math.min(680, parent.height - 80)
        z: 76
        visible: window.settingsVisible
        focus: visible
        Accessible.role: Accessible.Dialog
        Accessible.name: "Settings"
        onVisibleChanged: if (visible) forceActiveFocus()
        color: window.panelColor
        border.color: window.accentColor
        border.width: 1

        Keys.onPressed: function(event) {
            if (event.key === Qt.Key_Escape) {
                window.closeSettings()
                event.accepted = true
            }
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 14
            spacing: 8

            RowLayout {
                Layout.fillWidth: true
                Label {
                    Layout.fillWidth: true
                    text: "Settings"
                    color: window.primaryTextColor
                    font.bold: true
                    Accessible.name: "Settings"
                }
                Button {
                    text: "Refresh"
                    Accessible.name: "Refresh settings"
                    onClicked: window.refreshSettings()
                }
                Button {
                    text: "Install userscript"
                    Accessible.name: "Install userscript manifest"
                    onClicked: window.chooseUserscriptManifest()
                }
                Button {
                    text: "Close"
                    Accessible.name: "Close settings"
                    onClicked: window.closeSettings()
                }
            }

            RowLayout {
                Layout.fillWidth: true
                CheckBox {
                    id: settingsTemporaryToggle
                    text: "Temporary (memory only)"
                    checked: window.settingsTemporary
                    Accessible.name: "Apply settings temporarily"
                    onToggled: {
                        window.settingsTemporary = checked
                        window.settingsNotice = checked
                                ? "Changes apply only to this window and session"
                                : "Changes use the profile runtime override layer"
                    }
                }
                Label {
                    Layout.fillWidth: true
                    text: "Persistent edits never rewrite the authored config.toml."
                    color: window.secondaryTextColor
                    wrapMode: Text.WordWrap
                }
            }

            TextField {
                id: settingsSearchInput
                Layout.fillWidth: true
                text: window.settingsSearch
                placeholderText: "Filter settings by key, description, scope, or apply time"
                Accessible.name: "Search settings"
                Accessible.role: Accessible.EditableText
                onTextChanged: {
                    window.settingsSearch = text
                    window.refreshSettings()
                }
                Keys.onPressed: function(event) {
                    if (event.key === Qt.Key_Escape) {
                        window.closeSettings()
                        event.accepted = true
                    }
                }
            }

            Label {
                Layout.fillWidth: true
                text: "Supported fields below use the validated typed schema. Site-scoped values remain available through explicit site-rule commands; this manager edits the active global/profile runtime layer."
                color: window.secondaryTextColor
                wrapMode: Text.WordWrap
            }

            Label {
                Layout.fillWidth: true
                visible: window.settingsNotice.length > 0
                text: window.settingsNotice
                color: window.settingsNotice.indexOf("error") >= 0
                       ? window.errorColor : window.successColor
                wrapMode: Text.WordWrap
                Accessible.name: "Settings status"
            }

            Label {
                Layout.fillWidth: true
                text: window.spellcheckStatusText(browserUi)
                color: window.spellcheckDictionaryStatus(browserUi).missing.length > 0
                       ? window.warningColor : window.mutedTextColor
                wrapMode: Text.WordWrap
                Accessible.name: "Spellcheck dictionary status"
            }

            Label {
                Layout.fillWidth: true
                text: "Installed userscripts"
                color: window.primaryTextColor
                font.bold: true
                Accessible.name: "Installed userscripts"
            }

            ListView {
                id: userscriptList
                Layout.fillWidth: true
                Layout.preferredHeight: Math.min(150, Math.max(42, contentHeight))
                visible: !window.temporaryProfile
                clip: true
                spacing: 3
                model: window.userscriptInventoryRows()
                delegate: Rectangle {
                    width: userscriptList.width
                    height: 42
                    color: window.surfaceColor
                    radius: 3
                    Accessible.name: String(modelData.name || "userscript")

                    RowLayout {
                        anchors.fill: parent
                        anchors.margins: 6
                        spacing: 8
                        CheckBox {
                            checked: !!modelData.enabled
                            text: checked ? "Enabled" : "Disabled"
                            Accessible.name: "Enable userscript " + String(modelData.name || "")
                            onToggled: {
                                if (!browserUi.set_userscript_enabled(
                                            String(modelData.name || ""), checked)) {
                                    checked = !checked
                                }
                            }
                        }
                        Label {
                            Layout.fillWidth: true
                            text: String(modelData.name || "")
                                  + " · " + Number(modelData.actions || 0) + " action(s)"
                                  + (modelData.page_world ? " · page world" : "")
                            color: window.secondaryTextColor
                            elide: Text.ElideRight
                        }
                        Button {
                            text: "Remove"
                            Accessible.name: "Remove userscript " + String(modelData.name || "")
                            onClicked: window.confirmRemoveUserscript(String(modelData.name || ""))
                        }
                    }
                }
            }

            Label {
                Layout.fillWidth: true
                visible: window.temporaryProfile
                text: "Userscript installation and enable changes are unavailable in private or ephemeral profiles."
                color: window.mutedTextColor
                wrapMode: Text.WordWrap
            }

            ListView {
                id: settingsList
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 5
                model: window.settingsRows
                delegate: Rectangle {
                    property var rowData: modelData
                    width: settingsList.width
                    height: Math.max(76, window.chromeRowHeight * 5)
                    color: window.surfaceColor
                    radius: 3
                    Accessible.name: rowData.label + " " + rowData.key

                    RowLayout {
                        anchors.fill: parent
                        anchors.margins: 8
                        spacing: 8

                        ColumnLayout {
                            Layout.preferredWidth: 300
                            Layout.minimumWidth: 210
                            spacing: 2
                            Label {
                                Layout.fillWidth: true
                                text: rowData.label
                                color: window.primaryTextColor
                                font.bold: true
                                elide: Text.ElideRight
                            }
                            Label {
                                Layout.fillWidth: true
                                text: rowData.key + " · " + rowData.scope + " · " + rowData.apply
                                color: window.mutedTextColor
                                elide: Text.ElideRight
                            }
                        }

                        CheckBox {
                            id: settingBooleanEditor
                            visible: rowData.type === "bool"
                            Layout.fillWidth: true
                            text: checked ? "On" : "Off"
                            checked: !!rowData.value
                            Accessible.name: rowData.label
                            onToggled: window.applySetting(rowData, checked)
                        }

                        ComboBox {
                            id: settingEnumEditor
                            visible: rowData.type === "enum"
                            Layout.fillWidth: true
                            model: rowData.options || []
                            currentIndex: Math.max(0, (rowData.options || []).indexOf(String(rowData.value)))
                            Accessible.name: rowData.label
                            onActivated: window.applySetting(rowData, currentText)
                        }

                        TextField {
                            id: settingTextEditor
                            visible: rowData.type === "text" || rowData.type === "number"
                                     || rowData.type === "languages"
                            Layout.fillWidth: true
                            text: rowData.type === "languages"
                                  ? (rowData.value || []).join(", ") : String(rowData.value)
                            Accessible.name: rowData.label
                            Accessible.role: Accessible.EditableText
                            onAccepted: window.applySetting(rowData, text)
                        }

                        Button {
                            visible: rowData.type === "text" || rowData.type === "number"
                                     || rowData.type === "languages"
                            text: "Apply"
                            Accessible.name: "Apply " + rowData.label
                            onClicked: window.applySetting(rowData, settingTextEditor.text)
                        }
                        Button {
                            text: "Reset"
                            Accessible.name: "Reset " + rowData.label
                            onClicked: window.resetSetting(rowData)
                        }
                    }
                }
            }
        }
    }

    Rectangle {
        id: libraryManager
        anchors.centerIn: parent
        width: Math.min(820, parent.width - 100)
        height: Math.min(520, parent.height - 120)
        z: 55
        visible: window.libraryManagerVisible
        focus: visible
        Accessible.role: Accessible.Dialog
        Accessible.name: "Library manager"
        onVisibleChanged: if (visible) forceActiveFocus()
        color: window.panelColor
        border.color: window.borderColor

        Keys.onPressed: function(event) {
            if (event.key === Qt.Key_Escape) {
                window.libraryManagerVisible = false
                window.closeInternalSurface()
                event.accepted = true
            }
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 12
            spacing: 8

            RowLayout {
                Layout.fillWidth: true
                Label {
                    Layout.fillWidth: true
                    text: browserUi.library_kind
                    color: window.primaryTextColor
                    font.bold: true
                }
                Button {
                    visible: browserUi.library_kind === "journey"
                    text: window.libraryJourneyGraphMode ? "Outline" : "Relationships"
                    Accessible.name: text + " view"
                    onClicked: window.libraryJourneyGraphMode = !window.libraryJourneyGraphMode
                }
                Button {
                    visible: browserUi.library_kind === "journey"
                    text: "Export"
                    Accessible.name: "Export journey records"
                    onClicked: window.beginJourneyExport()
                }
                Button {
                    visible: browserUi.library_kind !== "journey"
                             && window.libraryTotalEntries > window.libraryPageSize
                    text: "Previous"
                    enabled: window.libraryPage > 0
                    Accessible.name: "Previous library page"
                    onClicked: window.changeLibraryPage(-1)
                }
                Label {
                    visible: browserUi.library_kind !== "journey"
                             && window.libraryTotalEntries > window.libraryPageSize
                    text: "Page " + (window.libraryPage + 1) + " / " + window.libraryPageCount()
                    color: window.mutedTextColor
                    Accessible.name: text
                }
                Button {
                    visible: browserUi.library_kind !== "journey"
                             && window.libraryTotalEntries > window.libraryPageSize
                    text: "Next"
                    enabled: window.libraryPage + 1 < window.libraryPageCount()
                    Accessible.name: "Next library page"
                    onClicked: window.changeLibraryPage(1)
                }
                Button {
                    text: "Close"
                    Accessible.name: "Close library manager"
                    onClicked: {
                        window.libraryManagerVisible = false
                        window.closeInternalSurface()
                    }
                }
            }

            Label {
                Layout.fillWidth: true
                text: "Profile-local records; URLs are displayed in their sanitized form."
                color: window.mutedTextColor
            }

            RowLayout {
                Layout.fillWidth: true
                visible: browserUi.library_kind === "journey"
                TextField {
                    id: journeySearchField
                    Layout.fillWidth: true
                    placeholderText: "Search title, URL, transition, or source"
                    Accessible.name: "Search journey records"
                    text: window.libraryJourneySearchText
                    onAccepted: {
                        window.libraryJourneySearchText = text
                        window.libraryJourneyExpandedNode = ""
                        window.libraryJourneyCurrentOnly = false
                        window.runJourneyQuery(window.journeySearchArgument(text))
                    }
                }
                Button {
                    text: "Search"
                    Accessible.name: "Search journey records"
                    onClicked: {
                        window.libraryJourneySearchText = journeySearchField.text
                        window.libraryJourneyExpandedNode = ""
                        window.libraryJourneyCurrentOnly = false
                        window.runJourneyQuery(
                            window.journeySearchArgument(journeySearchField.text))
                    }
                }
                Button {
                    text: "Clear"
                    Accessible.name: "Clear journey search"
                    onClicked: {
                        journeySearchField.text = ""
                        window.libraryJourneySearchText = ""
                        window.libraryJourneyExpandedNode = ""
                        window.libraryJourneyCurrentOnly = false
                        window.runJourneyQuery("")
                    }
                }
                Button {
                    text: "Current"
                    checkable: true
                    checked: window.libraryJourneyCurrentOnly
                    Accessible.name: "Show current journey node only"
                    onClicked: window.runJourneyCurrentQuery()
                }
                Button {
                    text: "All"
                    enabled: window.libraryJourneyCurrentOnly
                    Accessible.name: "Show all journey records"
                    onClicked: window.runJourneyAllQuery()
                }
            }

            ListView {
                id: libraryList
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                visible: !window.libraryJourneyGraphMode || browserUi.library_kind !== "journey"
                model: libraryEntries
                delegate: ColumnLayout {
                    width: libraryList.width
                    spacing: 2
                    property bool editing: false
                    property bool journeyTargetPickerVisible: false
                    property string editValue: model.entryKind === "quickmark"
                            ? model.secondary : model.label
                    RowLayout {
                        Layout.fillWidth: true
                        Label {
                            Layout.fillWidth: !editing
                            text: model.label
                            color: window.primaryTextColor
                            elide: Text.ElideRight
                        }
                        TextField {
                            visible: editing
                            Layout.fillWidth: true
                            text: editValue
                            Accessible.name: "Edit " + model.entryKind + " " + model.entryId
                            onTextChanged: editValue = text
                            onAccepted: {
                                if (window.editLibraryEntry(model.entryKind, model.entryId, text)) {
                                    editing = false
                                }
                            }
                        }
                        Button {
                            visible: model.entryKind === "history"
                                     || model.entryKind === "bookmark"
                                     || model.entryKind === "quickmark"
                            text: "Open"
                            Accessible.name: "Open " + model.entryKind + " " + model.entryId
                            onClicked: window.openLibraryEntry(model.entryKind, model.entryId)
                        }
                        Button {
                            visible: window.temporaryProfile && model.entryKind === "history"
                            text: "Reopen in profile"
                            Accessible.name: "Reopen private history entry in a named profile"
                            onClicked: window.showPrivateHistoryTransfer(
                                        model.entryId, model.label, model.secondary)
                        }
                        Button {
                            visible: model.entryKind === "bookmark"
                                     || model.entryKind === "quickmark"
                            text: editing ? "Save" : "Edit"
                            Accessible.name: (editing ? "Save " : "Edit ")
                                             + model.entryKind + " " + model.entryId
                            onClicked: {
                                if (editing) {
                                    if (window.editLibraryEntry(model.entryKind,
                                                                 model.entryId,
                                                                 editValue)) {
                                        editing = false
                                    }
                                } else {
                                    editValue = model.entryKind === "quickmark"
                                            ? model.secondary : model.label
                                    editing = true
                                }
                            }
                        }
                        Button {
                            visible: (model.entryKind === "bookmark"
                                      || model.entryKind === "quickmark") && !editing
                            text: window.pendingLibraryDelete === (model.entryKind + "\t" + model.entryId)
                                  ? "Confirm delete" : "Delete"
                            Accessible.name: (text === "Delete" ? "Delete " : "Confirm delete ")
                                             + model.entryKind + " " + model.entryId
                            onClicked: window.deleteLibraryEntry(model.entryKind, model.entryId)
                        }
                        Button {
                            visible: browserUi.library_kind === "journey"
                            text: "Reopen target"
                            Accessible.name: "Choose reopen target for journey node " + model.nodeId
                            onClicked: {
                                journeyTargetPickerVisible = !journeyTargetPickerVisible
                            }
                        }
                        ComboBox {
                            id: journeyTargetSelector
                            visible: browserUi.library_kind === "journey"
                                     && journeyTargetPickerVisible
                            model: ["current", "tab", "window"]
                            Accessible.name: "Journey reopen target"
                            ToolTip.visible: hovered
                            ToolTip.text: "Choose where this safe GET will open"
                        }
                        Button {
                            visible: browserUi.library_kind === "journey"
                            text: "Reopen"
                            Accessible.name: "Reopen journey node " + model.nodeId
                                    + " in " + journeyTargetSelector.currentText
                            onClicked: {
                                var target = journeyTargetSelector.currentText
                                var command = ":journey-reopen "
                                        + window.libraryCommandArgument(model.nodeId)
                                        + " --target " + target
                                if (browserUi.execute_command(command)) {
                                    window.syncTabModel()
                                    window.executePendingEngineAction()
                                    window.libraryManagerVisible = false
                                    window.closeInternalSurface()
                                }
                            }
                        }
                        Button {
                            visible: browserUi.library_kind === "journey"
                            text: "Expand"
                            Accessible.name: "Expand journey node " + model.nodeId
                            onClicked: {
                                window.libraryJourneyExpandedNode = model.nodeId
                                window.libraryJourneySearchText = ""
                                window.libraryJourneyCurrentOnly = false
                                journeySearchField.text = ""
                                window.runJourneyQuery(window.journeyExpandArgument(model.nodeId))
                            }
                        }
                    }
                    Label {
                        Layout.fillWidth: true
                        text: model.secondary
                        color: window.mutedTextColor
                        elide: Text.ElideMiddle
                    }
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: window.libraryJourneyGraphMode && browserUi.library_kind === "journey"
                spacing: 6

                Flickable {
                    id: journeyGraphFlickable
                    Layout.fillWidth: true
                    Layout.preferredHeight: 220
                    Layout.minimumHeight: 120
                    clip: true
                    contentWidth: journeyGraphCanvas.width
                    contentHeight: journeyGraphCanvas.height
                    Accessible.name: "Bounded journey graph layout"

                    Item {
                        id: journeyGraphCanvas
                        width: Math.max(journeyGraphFlickable.width, 4 * (170 + 24))
                        height: Math.max(120, Math.ceil(libraryGraphNodes.count / 4) * (48 + 18))

                        Canvas {
                            id: graphCanvas
                            anchors.fill: parent
                            z: 0
                            onPaint: {
                                var context = getContext("2d")
                                context.clearRect(0, 0, width, height)
                                var colors = {
                                    navigate: window.borderColor,
                                    redirect: window.warningColor,
                                    opener: window.accentColor,
                                    popup: window.accentColor,
                                    hint: window.successColor,
                                    "session-restore": window.privateColor,
                                    reopen: window.warningColor,
                                    "branch-after-back": window.errorColor
                                }
                                var lines = window.libraryGraphLineData
                                for (var i = 0; i < lines.length; ++i) {
                                    var line = lines[i]
                                    context.beginPath()
                                    context.moveTo(line.x1, line.y1)
                                    context.lineTo(line.x2, line.y2)
                                    context.strokeStyle = colors[line.transition]
                                            || window.borderColor
                                    context.lineWidth = 2
                                    context.stroke()
                                }
                            }
                        }

                        Repeater {
                            model: libraryGraphNodes
                            delegate: Rectangle {
                                x: model.x
                                y: model.y
                                width: 170
                                height: 48
                                z: 1
                                color: window.surfaceColor
                                border.color: window.accentColor
                                Accessible.role: Accessible.ListItem
                                Accessible.name: model.label + ", transition " + model.transition
                                Accessible.description: "Journey node " + model.nodeId
                                        + (model.source.length > 0 ? ", source " + model.source : "")

                                Column {
                                    anchors.fill: parent
                                    anchors.margins: 5
                                    spacing: 2
                                    Label {
                                        width: parent.width
                                        text: model.label
                                        color: window.primaryTextColor
                                        elide: Text.ElideRight
                                    }
                                    Label {
                                        width: parent.width
                                        text: model.transition
                                                + (model.source.length > 0 ? " · " + model.source : "")
                                        color: window.mutedTextColor
                                        elide: Text.ElideRight
                                    }
                                }
                            }
                        }
                    }
                }

                ListView {
                    id: libraryGraphList
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    model: libraryGraphEntries
                    Accessible.name: "Journey relationships"
                    delegate: ColumnLayout {
                        width: libraryGraphList.width
                        spacing: 2
                        Label {
                            Layout.fillWidth: true
                            text: model.label
                            color: window.primaryTextColor
                            elide: Text.ElideRight
                        }
                        Label {
                            Layout.fillWidth: true
                            text: model.secondary
                            color: window.mutedTextColor
                            elide: Text.ElideMiddle
                        }
                    }
                }
            }
        }

        Rectangle {
            id: journeyExportPreviewSurface
            anchors.fill: parent
            z: 10
            visible: window.journeyExportPreviewVisible
            color: window.panelColor
            border.color: window.warningColor
            Accessible.role: Accessible.Dialog
            Accessible.name: "Journey export preview"

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 18
                spacing: 12

                Label {
                    Layout.fillWidth: true
                    text: "Review journey export"
                    color: window.primaryTextColor
                    font.bold: true
                }
                Text {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    text: window.journeyExportPreviewText
                    color: window.primaryTextColor
                    wrapMode: Text.WordWrap
                    Accessible.name: text
                }
                RowLayout {
                    Layout.fillWidth: true
                    Item { Layout.fillWidth: true }
                    Button {
                        text: "Choose file"
                        Accessible.name: "Choose journey export file"
                        onClicked: {
                            journeyExportChooser.currentFile = "journey-export.json"
                            journeyExportChooser.open()
                        }
                    }
                    Button {
                        text: "Cancel"
                        Accessible.name: "Cancel journey export"
                        onClicked: window.journeyExportPreviewVisible = false
                    }
                }
            }
        }

        Rectangle {
            id: privateHistoryTransferSurface
            anchors.fill: parent
            z: 12
            visible: window.privateHistoryTransferVisible
            color: window.panelColor
            border.color: window.warningColor
            Accessible.role: Accessible.Dialog
            Accessible.name: "Private history transfer preview"

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 18
                spacing: 10

                Label {
                    Layout.fillWidth: true
                    text: "Reopen private history in a named profile"
                    color: window.primaryTextColor
                    font.bold: true
                }
                Label {
                    Layout.fillWidth: true
                    text: "Preview only: this transfers the safe URL below. Private history, title, permissions, cookies, sessions, and marks remain in the transient profile."
                    color: window.warningColor
                    wrapMode: Text.WordWrap
                }
                Label {
                    Layout.fillWidth: true
                    text: "Title: " + window.privateHistoryTransferTitle
                    color: window.primaryTextColor
                    elide: Text.ElideRight
                }
                Label {
                    Layout.fillWidth: true
                    text: "URL: " + window.privateHistoryTransferUrl
                    color: window.secondaryTextColor
                    wrapMode: Text.WrapAnywhere
                }
                Label {
                    Layout.fillWidth: true
                    text: "Choose a destination profile:"
                    color: window.primaryTextColor
                }
                ListView {
                    id: privateHistoryProfileList
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    model: profiles
                    delegate: RowLayout {
                        width: privateHistoryProfileList.width
                        spacing: 8
                        Label {
                            Layout.fillWidth: true
                            text: model.name + " — " + model.label
                            color: window.primaryTextColor
                            elide: Text.ElideRight
                        }
                        Button {
                            text: "Confirm reopen"
                            Accessible.name: "Confirm private history reopen in " + model.label
                            onClicked: window.confirmPrivateHistoryTransfer(model.name)
                        }
                        Button {
                            text: "Add bookmark"
                            Accessible.name: "Add private history entry as a bookmark in "
                                             + model.label
                            onClicked: window.confirmPrivateHistoryBookmark(model.name,
                                                                              model.label)
                        }
                    }
                    Label {
                        anchors.centerIn: parent
                        visible: privateHistoryProfileList.count === 0
                        text: browserUi.profile_values_pending
                                ? "Loading named profiles…"
                                : "No named profiles available"
                        color: window.mutedTextColor
                    }
                }
                Button {
                    Layout.alignment: Qt.AlignRight
                    text: "Cancel"
                    Accessible.name: "Cancel private history transfer"
                    onClicked: window.cancelPrivateHistoryTransfer()
                }
            }
        }
    }

    Rectangle {
        id: switcher
        anchors.centerIn: parent
        width: Math.min(860, parent.width - Math.max(40, window.chromeRowHeight * 4))
        height: Math.min(520, parent.height - Math.max(90, window.chromeRowHeight * 6))
        z: 60
        visible: window.switcherVisible
        focus: visible
        Accessible.role: Accessible.Dialog
        Accessible.name: "Universal switcher"
        Accessible.description: "Search up to " + window.switcherMaxResults()
            + " browser items; use arrow keys, Page Up, Page Down, Home, End, and Enter to choose"
        color: window.panelColor
        border.color: window.accentColor

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 12
            spacing: 8

            RowLayout {
                Layout.fillWidth: true
                Label {
                    Layout.fillWidth: true
                    text: "Switcher"
                    color: window.primaryTextColor
                    font.bold: true
                    Accessible.name: "Universal switcher heading"
                }
                Label {
                    text: "Ctrl-P · Esc"
                    color: window.mutedTextColor
                }
                Label {
                    text: window.switcherResults.length + " results"
                    color: window.mutedTextColor
                    Accessible.name: text
                    Accessible.role: Accessible.StatusBar
                    Accessible.description: "Current universal switcher result count"
                }
            }

            TextField {
                id: switcherInput
                Layout.fillWidth: true
                placeholderText: "Search tabs, windows, contexts, commands, history, marks, sessions, downloads"
                Accessible.name: "Universal switcher search"
                Accessible.role: Accessible.EditableText
                Accessible.editable: true
                onTextChanged: {
                    if (window.switcherVisible) {
                        window.cancelSwitcherBatch()
                        switcherRefreshTimer.restart()
                    }
                }
                Keys.onPressed: function(event) {
                    if (event.key === Qt.Key_Escape) {
                        window.closeSwitcher()
                        event.accepted = true
                    } else if (event.key === Qt.Key_Down) {
                        switcherList.incrementCurrentIndex()
                        event.accepted = true
                    } else if (event.key === Qt.Key_Up) {
                        switcherList.decrementCurrentIndex()
                        event.accepted = true
                    } else if (event.key === Qt.Key_PageDown) {
                        if (switcherList.count > 0) {
                            switcherList.positionViewAtIndex(
                                        Math.min(switcherList.count - 1,
                                                 switcherList.currentIndex + 8),
                                        ListView.Beginning)
                            switcherList.currentIndex = Math.min(
                                        switcherList.count - 1,
                                        switcherList.currentIndex + 8)
                        }
                        event.accepted = true
                    } else if (event.key === Qt.Key_PageUp) {
                        if (switcherList.count > 0) {
                            switcherList.positionViewAtIndex(
                                        Math.max(0, switcherList.currentIndex - 8),
                                        ListView.Beginning)
                            switcherList.currentIndex = Math.max(
                                        0, switcherList.currentIndex - 8)
                        }
                        event.accepted = true
                    } else if (event.key === Qt.Key_Home) {
                        if (switcherList.count > 0) {
                            switcherList.currentIndex = 0
                            switcherList.positionViewAtBeginning()
                        }
                        event.accepted = true
                    } else if (event.key === Qt.Key_End) {
                        if (switcherList.count > 0) {
                            switcherList.currentIndex = switcherList.count - 1
                            switcherList.positionViewAtEnd()
                        }
                        event.accepted = true
                    } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                        window.activateSwitcher(switcherList.currentIndex)
                        event.accepted = true
                    }
                }
            }

            ListView {
                id: switcherList
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                focus: true
                model: window.switcherResults
                currentIndex: count > 0 ? 0 : -1
                Accessible.role: Accessible.List
                Accessible.name: "Switcher results"
                Accessible.description: count + " results; selected result is announced with its kind and label"
                delegate: Rectangle {
                    width: switcherList.width
                    height: Math.max(window.chromeRowHeight * 5,
                                     resultColumn.implicitHeight + window.chromeRowHeight)
                    property var resultData: modelData
                    property int resultIndex: index
                    color: index === switcherList.currentIndex ? window.selectionColor : "transparent"
                    Accessible.role: Accessible.ListItem
                    Accessible.selected: index === switcherList.currentIndex
                    Accessible.name: resultData.kind + " " + resultData.label
                    Accessible.description: (resultData.profile || "")
                        + (resultData.secondary ? " · " + resultData.secondary : "")
                        + (index === switcherList.currentIndex ? " · selected" : "")

                    ColumnLayout {
                        id: resultColumn
                        anchors.fill: parent
                        anchors.margins: 6
                        spacing: 2
                        Label {
                            Layout.fillWidth: true
                            color: window.primaryTextColor
                            text: "[" + resultData.kind + "] " + resultData.label
                            elide: Text.ElideRight
                        }
                        Label {
                            Layout.fillWidth: true
                            color: window.mutedTextColor
                            text: (resultData.profile || "")
                                  + (resultData.secondary ? " · " + resultData.secondary : "")
                            elide: Text.ElideRight
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 4
                            Repeater {
                                model: resultData.actions || []
                                delegate: Button {
                                    text: modelData
                                    Accessible.name: modelData + " " + resultData.kind
                                    onClicked: window.activateSwitcherAction(resultIndex, modelData)
                                }
                            }
                            Item { Layout.fillWidth: true }
                        }
                    }

                    MouseArea {
                        anchors.top: parent.top
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.bottom: parent.bottom
                        onClicked: window.activateSwitcher(index)
                    }
                }
            }
        }
    }

    Rectangle {
        id: linkPreview
        anchors.centerIn: parent
        width: Math.min(900, parent.width - 80)
        height: Math.min(470, parent.height - 120)
        z: 70
        visible: window.linkPreviewVisible
        focus: visible
        Accessible.role: Accessible.Dialog
        Accessible.name: "Clean-link preview"
        onVisibleChanged: if (visible) forceActiveFocus()
        color: window.panelColor
        border.color: window.warningColor

        Keys.onPressed: function(event) {
            if (event.key === Qt.Key_Escape) {
                window.closeLinkPreview()
                event.accepted = true
            }
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 14
            spacing: 8

            RowLayout {
                Layout.fillWidth: true
                Label {
                    Layout.fillWidth: true
                    text: linkPreviewData.command === "url-explain" ? "URL explanation" : "Clean-link preview"
                    color: window.primaryTextColor
                    font.bold: true
                }
                Button {
                    text: "Confirm navigation"
                    visible: linkPreviewData.requires_confirmation === true
                    Accessible.name: "Confirm cleaned URL navigation"
                    onClicked: {
                        if (browserUi.confirm_link_navigation()) {
                            window.executePendingEngineAction()
                            window.closeLinkPreview()
                            window.syncTabModel()
                        }
                    }
                }
                Button {
                    text: "Close"
                    Accessible.name: "Close URL preview"
                    onClicked: window.closeLinkPreview()
                }
            }

            Label {
                Layout.fillWidth: true
                text: "Preview only; no navigation was performed. Sensitive URL components are masked."
                color: window.warningColor
                wrapMode: Text.WordWrap
            }

            Label {
                Layout.fillWidth: true
                text: "Original: " + (linkPreviewData.original || "")
                color: window.primaryTextColor
                wrapMode: Text.WrapAnywhere
            }
            Label {
                Layout.fillWidth: true
                text: "Result: " + (linkPreviewData.cleaned || "")
                color: window.primaryTextColor
                wrapMode: Text.WrapAnywhere
            }
            Label {
                Layout.fillWidth: true
                text: "Rules: " + ((linkPreviewData.applied_rules || []).join(", ") || "none")
                color: window.mutedTextColor
                wrapMode: Text.WordWrap
            }
            Label {
                Layout.fillWidth: true
                text: "Removed: " + ((linkPreviewData.removed_parameters || []).join(", ") || "none")
                color: window.mutedTextColor
                wrapMode: Text.WordWrap
            }
            Label {
                Layout.fillWidth: true
                text: "Retained: " + ((linkPreviewData.retained_parameters || []).join(", ") || "none")
                color: window.mutedTextColor
                wrapMode: Text.WordWrap
            }
            Label {
                Layout.fillWidth: true
                Layout.fillHeight: true
                text: linkPreviewData.explanation || ""
                color: window.secondaryTextColor
                wrapMode: Text.WordWrap
                verticalAlignment: Text.AlignTop
            }
        }
    }

    Rectangle {
        id: downloadManager
        anchors.centerIn: parent
        width: Math.min(760 * window.chromeScale, parent.width - 32)
        height: Math.min(430 * window.chromeScale, parent.height - 32)
        z: 40
        visible: window.downloadManagerVisible
        focus: visible
        Accessible.role: Accessible.Dialog
        Accessible.name: "Download manager"
        onVisibleChanged: if (visible) forceActiveFocus()
        color: window.panelColor
        border.color: window.borderColor

        Keys.onPressed: function(event) {
            if (event.key === Qt.Key_Escape) {
                window.downloadManagerVisible = false
                window.closeInternalSurface()
                event.accepted = true
            }
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 12
            spacing: 8

            RowLayout {
                Layout.fillWidth: true
                Label {
                    Layout.fillWidth: true
                    text: "Downloads"
                    color: window.primaryTextColor
                    font.bold: true
                }
                Button {
                    text: "Close"
                    Accessible.name: "Close download manager"
                    onClicked: {
                        window.downloadManagerVisible = false
                        window.closeInternalSurface()
                    }
                }
            }

            Label {
                Layout.fillWidth: true
                text: "Downloads are saved to the user Downloads directory with validated names and collision-safe renaming."
                color: window.secondaryTextColor
                wrapMode: Text.WordWrap
            }

            ListView {
                id: downloadList
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: downloads
                delegate: ColumnLayout {
                    width: downloadList.width
                    spacing: 2
                    Label {
                        Layout.fillWidth: true
                        text: model.state + " · " + model.bytes + " bytes"
                              + (Number(model.total) >= 0
                                 ? " / " + model.total + " bytes" : "")
                              + (window.formatDownloadRate(model.speed).length > 0
                                 ? " · " + window.formatDownloadRate(model.speed) : "")
                              + " · " + model.id
                        color: window.primaryTextColor
                    }
                    Label {
                        Layout.fillWidth: true
                        text: model.destination.length > 0 ? model.destination : "Selecting destination"
                        color: window.mutedTextColor
                        elide: Text.ElideMiddle
                    }
                    Label {
                        Layout.fillWidth: true
                        visible: model.reason.length > 0
                        text: "Failure reason: " + model.reason
                        color: window.warningColor
                        wrapMode: Text.WordWrap
                    }
                    RowLayout {
                        visible: model.state === "completed"
                        Button {
                            text: "Open"
                            Accessible.name: "Open completed download"
                            onClicked: window.openDownload(model.id, false)
                        }
                        Button {
                            text: "Show"
                            Accessible.name: "Reveal completed download"
                            onClicked: window.openDownload(model.id, true)
                        }
                    }
                    RowLayout {
                        visible: model.state === "in-progress" || model.state === "paused"
                        Button {
                            text: model.state === "paused" ? "Resume" : "Pause"
                            Accessible.name: model.state === "paused" ? "Resume download" : "Pause download"
                            onClicked: window.requestDownloadAction(model.id, model.state === "paused" ? "resume" : "pause")
                        }
                        Button {
                            text: "Cancel"
                            Accessible.name: "Cancel download"
                            onClicked: window.requestDownloadAction(model.id, "cancel")
                        }
                    }
                    RowLayout {
                        visible: model.state === "interrupted" || model.state === "cancelled"
                        Button {
                            text: "Retry"
                            Accessible.name: "Retry download"
                            onClicked: window.requestDownloadAction(model.id, "retry")
                        }
                    }
                }
            }
        }
    }

    Timer {
        id: downloadMetricsTimer
        interval: 1000
        repeat: true
        running: window.downloadManagerVisible
        onTriggered: window.refreshDownloads()
    }

    Timer {
        id: profileRefreshTimer
        interval: 100
        repeat: true
        onTriggered: {
            if (browserUi.profile_values_pending) {
                profileRefreshAttempts += 1
                if (profileRefreshAttempts >= 20) {
                    stop()
                }
                return
            }
            window.applyProfileValues()
            profileRefreshAttempts += 1
            stop()
        }
    }

    Timer {
        id: profilePreviewTimer
        interval: 100
        repeat: true
        onTriggered: {
            if (browserUi.profile_preview_pending) {
                return
            }
            var preview = browserUi.profile_preview_text
            if (preview.length === 0) {
                profileDeletePreviewVisible = false
                window.closeInternalSurface()
                stop()
                return
            }
            profileDeletePreviewText = preview
            stop()
        }
    }

    Rectangle {
        id: profileManager
        anchors.centerIn: parent
        width: Math.min(620 * window.chromeScale, parent.width - 32)
        height: Math.min(430 * window.chromeScale, parent.height - 32)
        z: 40
        visible: window.profileManagerVisible
        focus: visible
        Accessible.role: Accessible.Dialog
        Accessible.name: "Profile manager"
        onVisibleChanged: if (visible) forceActiveFocus()
        color: window.panelColor
        border.color: window.borderColor

        Keys.onPressed: function(event) {
            if (event.key === Qt.Key_Escape) {
                window.profileManagerVisible = false
                window.closeInternalSurface()
                event.accepted = true
            }
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 12
            spacing: 8

            RowLayout {
                Layout.fillWidth: true
                Label {
                    Layout.fillWidth: true
                    text: "Profiles"
                    color: window.primaryTextColor
                    font.bold: true
                }
                Button {
                    text: "Close"
                    Accessible.name: "Close profile manager"
                    onClicked: {
                        window.profileManagerVisible = false
                        window.closeInternalSurface()
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                TextField {
                    id: profileNameInput
                    Layout.fillWidth: true
                    placeholderText: "new-profile-name"
                    Accessible.name: "New profile name"
                }
                TextField {
                    id: profileLabelInput
                    Layout.fillWidth: true
                    placeholderText: "Display label"
                    Accessible.name: "New profile label"
                }
                Button {
                    text: "Create"
                    enabled: profileNameInput.text.length > 0 && profileLabelInput.text.length > 0
                    onClicked: {
                        if (browserUi.create_profile(profileNameInput.text, profileLabelInput.text)) {
                            profileNameInput.text = ""
                            profileLabelInput.text = ""
                            window.scheduleProfileRefresh()
                        }
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                TextField {
                    Layout.fillWidth: true
                    text: window.profileRenameName
                    readOnly: true
                    placeholderText: "Select a profile to rename"
                    Accessible.name: "Profile being renamed"
                }
                TextField {
                    id: profileRenameInput
                    Layout.fillWidth: true
                    text: window.profileRenameLabel
                    placeholderText: "New display label"
                    Accessible.name: "New profile display label"
                }
                Button {
                    text: "Rename label"
                    enabled: window.profileRenameName.length > 0 && profileRenameInput.text.length > 0
                    onClicked: {
                        if (browserUi.rename_profile(window.profileRenameName, profileRenameInput.text)) {
                            window.profileRenameName = ""
                            window.profileRenameLabel = ""
                            profileRenameInput.text = ""
                            window.scheduleProfileRefresh()
                        }
                    }
                }
            }

            ListView {
                id: profileList
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: profiles
                delegate: RowLayout {
                    width: profileList.width
                    spacing: 6
                    Label {
                        Layout.fillWidth: true
                        text: model.name + " — " + model.label
                        color: window.primaryTextColor
                    }
                    Button {
                        text: "Open window"
                        onClicked: window.openProfile(model.name, model.label)
                    }
                    Button {
                        text: "Edit label"
                        onClicked: {
                            window.profileRenameName = model.name
                            window.profileRenameLabel = model.label
                            profileRenameInput.text = model.label
                        }
                    }
                    Button {
                        text: "Delete"
                        onClicked: window.showProfileDeletePreview(model.name)
                    }
                }
            }
        }
    }

    Rectangle {
        id: profileDeletePreview
        anchors.centerIn: parent
        width: Math.min(660 * window.chromeScale, parent.width - 32)
        height: Math.min(390 * window.chromeScale, parent.height - 32)
        z: 50
        visible: window.profileDeletePreviewVisible
        focus: visible
        Accessible.role: Accessible.Dialog
        Accessible.name: "Profile deletion preview"
        onVisibleChanged: if (visible) forceActiveFocus()
        color: window.backgroundColor
        border.color: window.errorColor

        Keys.onPressed: function(event) {
            if (event.key === Qt.Key_Escape) {
                window.profileDeletePreviewVisible = false
                window.closeInternalSurface()
                event.accepted = true
            }
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 14
            spacing: 10

            Label {
                Layout.fillWidth: true
                text: "Delete profile: " + window.profileDeleteName
                color: window.primaryTextColor
                font.bold: true
            }
            Label {
                Layout.fillWidth: true
                text: "This removes the exact Ferric Browser metadata roots below. QtWebEngine storage is not removed."
                color: window.errorColor
                wrapMode: Text.WordWrap
            }
            Text {
                Layout.fillWidth: true
                Layout.fillHeight: true
                text: window.profileDeletePreviewText
                color: window.primaryTextColor
                wrapMode: Text.Wrap
                elide: Text.ElideRight
            }
            RowLayout {
                Layout.fillWidth: true
                Item { Layout.fillWidth: true }
                Button {
                    text: "Confirm delete"
                    onClicked: {
                        if (browserUi.delete_profile(window.profileDeleteName, true)) {
                            window.profileDeletePreviewVisible = false
                            window.profileManagerVisible = false
                            window.closeInternalSurface()
                            window.closeInternalSurface()
                        }
                    }
                }
                Button {
                    text: "Cancel"
                    onClicked: {
                        window.profileDeletePreviewVisible = false
                        window.closeInternalSurface()
                    }
                }
            }
        }
    }

    Rectangle {
        id: sessionManager
        anchors.centerIn: parent
        width: Math.min(560 * window.chromeScale, parent.width - 32)
        height: Math.min(420 * window.chromeScale, parent.height - 32)
        z: 40
        visible: window.sessionManagerVisible
        focus: visible
        Accessible.role: Accessible.Dialog
        Accessible.name: "Session manager"
        onVisibleChanged: if (visible) forceActiveFocus()
        color: window.panelColor
        border.color: window.borderColor

        Keys.onPressed: function(event) {
            if (event.key === Qt.Key_Escape) {
                window.sessionManagerVisible = false
                window.closeInternalSurface()
                event.accepted = true
            }
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 12
            spacing: 8

            RowLayout {
                Layout.fillWidth: true
                Label {
                    Layout.fillWidth: true
                    text: "Named sessions"
                    color: window.primaryTextColor
                    font.bold: true
                }
                Button {
                    text: "Close"
                    Accessible.name: "Close session manager"
                    onClicked: {
                        window.sessionManagerVisible = false
                        window.closeInternalSurface()
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                TextField {
                    id: sessionNameInput
                    Layout.fillWidth: true
                    placeholderText: "Save current session as..."
                    Accessible.name: "New session name"
                }
                Button {
                    text: "Save"
                    enabled: sessionNameInput.text.length > 0
                    onClicked: {
                        if (browserUi.save_named_session(sessionNameInput.text)) {
                            sessionNameInput.text = ""
                            window.refreshSessions()
                        }
                    }
                }
            }

            ListView {
                id: sessionList
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: namedSessions
                delegate: RowLayout {
                    width: sessionList.width
                    spacing: 6
                    Label {
                        Layout.fillWidth: true
                        text: model.name
                        color: window.primaryTextColor
                        Accessible.name: "Named session " + model.name
                    }
                    Button {
                        text: "Preview"
                        onClicked: window.showSessionPreview(model.name, false)
                    }
                    Button {
                        text: window.pendingDeleteName === model.name ? "Confirm delete" : "Delete"
                        onClicked: {
                            if (window.pendingDeleteName === model.name) {
                                if (browserUi.delete_named_session(model.name, true)) {
                                    window.pendingDeleteName = ""
                                    window.refreshSessions()
                                }
                            } else {
                                window.pendingDeleteName = model.name
                            }
                        }
                    }
                }
            }
        }
    }

    Rectangle {
        id: sessionPreview
        anchors.centerIn: parent
        width: Math.min(620 * window.chromeScale, parent.width - 32)
        height: Math.min(430 * window.chromeScale, parent.height - 32)
        z: 50
        visible: window.sessionPreviewVisible
        focus: visible
        Accessible.role: Accessible.Dialog
        Accessible.name: "Session preview"
        onVisibleChanged: if (visible) forceActiveFocus()
        color: window.backgroundColor
        border.color: window.accentColor

        Keys.onPressed: function(event) {
            if (event.key === Qt.Key_Escape) {
                window.sessionPreviewVisible = false
                window.sessionPreviewLoading = false
                window.sessionPreviewError = false
                window.closeInternalSurface()
                event.accepted = true
            }
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 14
            spacing: 10

            Label {
                Layout.fillWidth: true
                text: "Load session: " + window.sessionPreviewName
                color: window.primaryTextColor
                font.bold: true
            }
            Label {
                Layout.fillWidth: true
                text: window.sessionPreviewAppend
                      ? "Append these validated descriptors to the current tabs?"
                      : "Replace the current tabs with these validated descriptors?"
                color: window.secondaryTextColor
                wrapMode: Text.WordWrap
            }
            Text {
                Layout.fillWidth: true
                Layout.fillHeight: true
                text: window.sessionPreviewText
                color: window.primaryTextColor
                wrapMode: Text.Wrap
                elide: Text.ElideRight
            }
            RowLayout {
                Layout.fillWidth: true
                Item { Layout.fillWidth: true }
                Button {
                    text: "Replace"
                    enabled: !window.sessionPreviewLoading
                             && !window.sessionPreviewAppend
                             && !window.sessionPreviewError
                    onClicked: window.loadPreviewedSession()
                }
                Button {
                    text: "Append"
                    enabled: !window.sessionPreviewLoading && !window.sessionPreviewError
                    onClicked: {
                        window.sessionPreviewAppend = true
                        window.loadPreviewedSession()
                    }
                }
                Button {
                    text: "Cancel"
                    Accessible.name: "Cancel session preview"
                    onClicked: {
                        window.sessionPreviewVisible = false
                        window.sessionPreviewLoading = false
                        window.sessionPreviewError = false
                        window.closeInternalSurface()
                    }
                }
            }
        }
    }

    Rectangle {
        id: reopenWindowConfirmation
        anchors.centerIn: parent
        width: Math.min(560 * window.chromeScale, parent.width - 32)
        height: Math.min(260 * window.chromeScale, parent.height - 32)
        z: 55
        visible: window.reopenWindowConfirmationVisible
        focus: visible
        Accessible.role: Accessible.Dialog
        Accessible.name: "Reopen tab in window confirmation"
        onVisibleChanged: if (visible) forceActiveFocus()
        color: window.panelColor
        border.color: window.warningColor

        Keys.onPressed: function(event) {
            if (event.key === Qt.Key_Escape) {
                window.cancelReopenWindow()
                event.accepted = true
            }
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 14
            spacing: 10

            Label {
                Layout.fillWidth: true
                text: "Reopen tab in a same-profile window?"
                color: window.primaryTextColor
                font.bold: true
            }
            Label {
                Layout.fillWidth: true
                text: "This opens a safe URL descriptor in a new window. Live page state, forms, media, and in-progress engine work will not be preserved."
                color: window.warningColor
                wrapMode: Text.WordWrap
            }
            RowLayout {
                Layout.fillWidth: true
                Item { Layout.fillWidth: true }
                Button {
                    text: "Reopen"
                    Accessible.name: "Confirm reopen tab in window"
                    onClicked: window.confirmReopenWindow()
                }
                Button {
                    text: "Cancel"
                    Accessible.name: "Cancel reopen tab in window"
                    onClicked: window.cancelReopenWindow()
                }
            }
        }
    }

    StackLayout {
        id: webViews
        anchors.fill: parent
        anchors.leftMargin: window.tabStripVisible && window.tabPosition === "left"
                            ? window.sideTabWidth : 0
        anchors.rightMargin: window.tabStripVisible && window.tabPosition === "right"
                             ? window.sideTabWidth : 0
        anchors.bottomMargin: window.bottomChromeHeight
        currentIndex: browserUi.active_tab_index

        Component {
            id: webViewComponent

            WebEngineView {
                id: webView
                property int tabIndex: -1
                property string stableTabId: ""
                property var viewUi: browserUi
                property var viewHost: window
                property var viewTabs: tabs
                property var viewModel: ({})
                property var viewProfile: browserProfile
                property var viewInterceptor: requestInterceptor
                property bool viewTransientProfile: window.temporaryProfile
                property bool viewTransferred: false
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
                property bool scrollPositionRestored: false
                devToolsView: window.devToolsVisible
                        ? (window.devToolsDetached
                           ? window.devToolsExternalView : attachedDevToolsLoader.item)
                        : null
                // Navigation-scoped content settings are snapshotted per
                // document; live config changes apply to the next navigation.
                property var effectiveSiteSettings: ({ values: {}, matched_rules: [] })
                function refreshEffectiveSiteSettings() {
                    effectiveSiteSettings = window.siteRuleSettingsFor(
                        viewUi, webView.url.toString())
                }
                Connections {
                    target: viewUi
                    function onSite_experiment_jsonChanged() {
                        webView.refreshEffectiveSiteSettings()
                    }
                    function onActive_tab_indexChanged() {
                        if (viewUi === browserUi
                                && tabIndex === viewUi.active_tab_index) {
                            window.refreshBlockingEvidence()
                            window.updateMprisForPrimaryView(webView)
                            window.scheduleFocusProbe(2)
                        }
                    }
                }
                property int blockedRequestCount: {
                    var host = webView.url && webView.url.host
                            ? String(webView.url.host) : ""
                    var counts = viewInterceptor.blockedSiteCounts || ({})
                    return host.length > 0 && counts[host] !== undefined ? Number(counts[host]) : 0
                }
                profile: viewProfile
                url: viewModel.loaded ? viewModel.url : "about:blank"
                audioMuted: !!viewModel.muted
                zoomFactor: Number(window.siteRuleValue(
                    effectiveSiteSettings, "content.zoom", viewModel.zoom))
                settings.javascriptEnabled: !!window.siteRuleValue(
                    effectiveSiteSettings, "content.javascript", true)
                settings.autoLoadImages: !!window.siteRuleValue(
                    effectiveSiteSettings, "content.images", true)
                settings.forceDarkMode: !!window.siteRuleValue(
                    effectiveSiteSettings, "content.force_dark", false)
                settings.playbackRequiresUserGesture:
                    window.siteRuleValue(effectiveSiteSettings, "content.autoplay", "engine-default")
                    === "require-gesture"
                Accessible.name: "Web content for tab " + (tabIndex + 1)
                Component.onCompleted: {
                    refreshEffectiveSiteSettings()
                    window.installFocusObserver(webView)
                }
                onActiveFocusChanged: {
                    if (activeFocus && viewUi === browserUi
                            && tabIndex === viewUi.active_tab_index) {
                        window.scheduleFocusProbe(2)
                    }
                }

                onUrlChanged: {
                    refreshEffectiveSiteSettings()
                    if (!viewModel.loaded) {
                        return
                    }
                    var changedUrl = url.toString()
                    if (viewModel.url !== changedUrl && viewTabs === tabs) {
                        tabs.setProperty(tabIndex, "url", changedUrl)
                    }
                    if (viewTabs === tabs) {
                        viewUi.navigation_url_changed_for(tabIndex, changedUrl)
                    } else {
                        viewUi.navigation_url_changed(changedUrl)
                    }
                    if (viewTabs !== tabs || tabIndex === viewUi.active_tab_index) {
                        if (viewUi === browserUi) {
                            address.text = browserUi.display_url
                            window.updateMprisForPrimaryView(webView)
                            window.refreshBlockingEvidence()
                        }
                    }
                }
                onTitleChanged: {
                    if (viewModel.loaded && viewTabs === tabs) {
                        tabs.setProperty(tabIndex, "title", title || "New tab")
                        if (tabIndex === viewUi.active_tab_index && viewUi === browserUi) {
                            window.updateMprisForPrimaryView(webView)
                        }
                    }
                }
                onLoadingChanged: function(loadRequest) {
                    if (loadRequest.status === WebEngineView.LoadStartedStatus) {
                        webView.scrollPositionRestored = false
                        window.clearPageDialogForView(webView)
                        window.clearClientCertificateForView(webView)
                        window.clearCertificateErrorForView(webView)
                        window.clearWebAuthForView(webView)
                        window.clearContextMenuForView(webView)
                        window.clearDesktopMediaForView(webView)
                        window.clearSiteDataClearForView(webView)
                        window.noteCaptureNavigation(webView)
                        window.clearFileDialogForView(webView)
                        window.clearRendererFailureForView(webView)
                        window.resetPageDialogBudget(webView)
                        viewInterceptor.clearSiteEvidence(webView.url.host)
                        viewUi.blocking_active_decisions = "[]"
                        if (viewTabs === tabs) {
                            window.cancelPermissionForTab(tabIndex)
                        } else {
                            window.cancelPermissionForUi(viewUi)
                        }
                        window.installPageUserscripts(
                            viewUi, webView, webView.url.toString(), viewTransientProfile)
                        window.injectPageUserscripts(
                            viewUi, webView, webView.url.toString(), viewTransientProfile,
                            "document_start")
                        window.injectCosmeticRules(viewUi, webView)
                        if (viewTabs === tabs) {
                            viewUi.navigation_started_for(tabIndex, loadRequest.url.toString())
                        } else {
                            viewUi.navigation_started(loadRequest.url.toString())
                        }
                    } else if (loadRequest.status === WebEngineView.LoadSucceededStatus) {
                        if (viewTabs === tabs) {
                            viewUi.navigation_committed_for(tabIndex, webView.url.toString(), webView.title)
                        } else {
                            viewUi.navigation_committed(webView.url.toString(), webView.title)
                        }
                        if (!webView.scrollPositionRestored) {
                            webView.scrollPositionRestored = true
                            window.restoreScrollPosition(webView, viewModel.scrollX, viewModel.scrollY)
                        }
                        window.injectPageUserscripts(
                            viewUi, webView, webView.url.toString(), viewTransientProfile,
                            "document_end")
                        window.injectCosmeticRules(viewUi, webView)
                        Qt.callLater(function() {
                            window.injectPageUserscripts(
                                viewUi, webView, webView.url.toString(), viewTransientProfile,
                                "document_idle")
                        })
                        if (viewTabs === tabs) {
                            viewUi.navigation_completed_for(tabIndex)
                        } else {
                            viewUi.navigation_completed()
                        }
                        if (viewUi === browserUi
                                && tabIndex === viewUi.active_tab_index) {
                            window.scheduleFocusProbe(3)
                        }
                    if (viewTabs !== tabs || tabIndex === viewUi.active_tab_index) {
                            window.finishSiteDoctorAfterLoad(viewUi, true)
                        }
                    } else if (loadRequest.status === WebEngineView.LoadFailedStatus) {
                        if (viewTabs === tabs) {
                            viewUi.navigation_failed_with_details(
                                tabIndex,
                                loadRequest.url.toString(),
                                window.navigationFailureKind(loadRequest),
                                window.navigationFailureDetail(loadRequest))
                        } else {
                            viewUi.navigation_failed()
                        }
                        if (tabIndex === viewUi.active_tab_index) {
                            window.finishSiteDoctorAfterLoad(viewUi, false)
                        }
                    }
                }
                onPdfPrintingFinished: function(filePath, success) {
                    viewUi.finish_print_pdf(filePath, success)
                }
                onFileDialogRequested: function(request) {
                    if (viewTabs !== tabs || tabIndex === viewUi.active_tab_index) {
                        window.handleFileDialogRequested(request, webView, viewUi)
                    } else {
                        window.resolveQtRequest(viewUi, request, "file-dialog", "dialogReject", [])
                    }
                }
                onPermissionRequested: function(permissionRequest) {
                    if (viewTabs !== tabs || tabIndex === viewUi.active_tab_index) {
                        window.handlePermissionRequested(permissionRequest, tabIndex)
                    } else {
                        window.resolveQtRequest(viewUi, permissionRequest, "permission", "deny", [])
                    }
                }
                onJavaScriptDialogRequested: function(request) {
                    window.handleJavaScriptDialogRequested(
                        viewUi, webView, request, viewTransientProfile)
                }
                onAuthenticationDialogRequested: function(request) {
                    window.handleAuthenticationDialogRequested(
                        viewUi, webView, request, viewTransientProfile)
                }
                onSelectClientCertificate: function(selection) {
                    if (viewTabs !== tabs || tabIndex === viewUi.active_tab_index) {
                        window.handleClientCertificateRequested(
                            viewUi, webView, selection, viewTransientProfile, window)
                    } else {
                        window.resolveQtRequest(viewUi, selection, "client-certificate", "selectNone", [])
                    }
                }
                onCertificateError: function(error) {
                    if (viewTabs !== tabs || tabIndex === viewUi.active_tab_index) {
                        window.handleCertificateError(
                            viewUi, webView, error, window)
                    } else {
                        window.resolveQtRequest(viewUi, error, "client-certificate", "rejectCertificate", [])
                    }
                }
                onWebAuthUxRequested: function(request) {
                    if (viewTabs !== tabs || tabIndex === viewUi.active_tab_index) {
                        window.handleWebAuthRequested(
                            viewUi, webView, request, window)
                    } else {
                        window.resolveQtRequest(viewUi, request, "webauth", "cancel", [])
                    }
                }
                onContextMenuRequested: function(request) {
                    if (viewTabs !== tabs || tabIndex === viewUi.active_tab_index) {
                        window.handleContextMenuRequested(
                            viewUi, webView, request, window)
                    } else {
                        request.accepted = true
                    }
                }
                onDesktopMediaRequested: function(request) {
                    window.handleDesktopMediaRequested(
                        viewUi, webView, request, window)
                }
                onFullScreenRequested: function(request) {
                    request.accept()
                    if (request.toggleOn) {
                        window.showFullScreen()
                        viewUi.status_text = "Page fullscreen enabled"
                    } else {
                        window.showNormal()
                        viewUi.status_text = "Page fullscreen ended"
                    }
                }
                onRenderProcessTerminated: function(terminationStatus, exitCode) {
                    window.handleRendererProcessTerminated(
                        viewUi, webView, window, tabIndex,
                        terminationStatus, exitCode)
                }
                Component.onDestruction: {
                    window.clearPageDialogForView(webView)
                    window.clearClientCertificateForView(webView)
                    window.clearCertificateErrorForView(webView)
                    window.clearWebAuthForView(webView)
                    window.clearContextMenuForView(webView)
                    window.clearDesktopMediaForView(webView)
                    window.clearCaptureSessionForView(webView)
                    window.clearFileDialogForView(webView)
                    window.clearRendererFailureForView(webView)
                }
                onNewWindowRequested: function(request) {
                    var popupAllowed = viewTabs === tabs
                            ? viewUi.popup_allowed_for(
                                tabIndex, request.requestedUrl.toString(), request.userInitiated)
                            : viewUi.popup_allowed(
                                request.requestedUrl.toString(), request.userInitiated)
                    if (!popupAllowed) {
                        return
                    }
                    var popup = popupWindowComponent.createObject(null, {
                        popupRequest: request,
                        popupJourneyToken: viewUi.take_popup_journey_token(
                            request.requestedUrl.toString()),
                        popupProfile: viewProfile,
                        popupPermissionUi: viewUi,
                        popupContextName: viewUi.context_name,
                        popupContextLabel: viewUi.context_label,
                        popupPrivateProfile: viewTransientProfile,
                        popupEphemeralProfile: window.ephemeralProfile,
                        popupProfileName: window.profileName
                    })
                    if (!popup) {
                        if (viewTabs === tabs) {
                            viewUi.navigation_failed_for(tabIndex)
                        } else {
                            viewUi.navigation_failed()
                        }
                    }
                }
            }
        }
    }

    Loader {
        id: attachedDevToolsLoader
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: Math.min(window.devToolsPanelHeight, parent.height * 0.55)
        active: window.devToolsVisible && !window.devToolsDetached
        visible: active
        z: 20
        sourceComponent: Component {
            WebEngineView {
                anchors.fill: parent
                profile: browserProfile
                inspectedView: window.activeWebView()
                Accessible.name: "Attached developer tools"
            }
        }
    }

    Item {
        id: hintOverlay
        anchors.fill: webViews
        z: 30
        visible: browserUi.hint_visible && window.hintResults.length > 0
        focus: visible

        Repeater {
            model: window.hintResults
            delegate: Rectangle {
                x: modelData.x
                y: modelData.y
                width: Math.max(24, hintLabel.implicitWidth + 10)
                height: Math.max(22, hintLabel.implicitHeight + 6)
                color: window.warningColor
                border.color: window.backgroundColor
                border.width: 1
                radius: 3

                Text {
                    id: hintLabel
                    anchors.centerIn: parent
                    text: modelData.label
                    color: window.contrastText(parent.color)
                    font.bold: true
                    Accessible.name: "Hint " + modelData.label + " " + modelData.text
                }

                MouseArea {
                    anchors.fill: parent
                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                    onClicked: function(mouse) {
                        if (mouse.button === Qt.RightButton) {
                            window.showHintActions(modelData.label)
                        } else {
                            window.activateHint(modelData.label)
                        }
                    }
                }
            }
        }
    }

    Rectangle {
        id: rapidHintConfirmation
        anchors.centerIn: parent
        width: Math.min(560, parent.width - 80)
        height: Math.min(170 * window.chromeScale, parent.height - 32)
        z: 80
        visible: window.rapidHintConfirmationVisible
        color: window.panelColor
        border.color: window.warningColor
        border.width: 2

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 16
            spacing: 10

            Label {
                Layout.fillWidth: true
                text: "Rapid hint has opened 20 background tabs. Continue?"
                color: window.primaryTextColor
                wrapMode: Text.WordWrap
            }

            Label {
                Layout.fillWidth: true
                text: "Confirming grants one additional bounded batch of 20 tabs."
                color: window.warningColor
                wrapMode: Text.WordWrap
            }

            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 8

                Button {
                    text: "Continue"
                    Accessible.name: "Confirm another rapid hint tab batch"
                    onClicked: {
                        if (browserUi.confirm_rapid_hint_tabs()) {
                            window.rapidHintConfirmationVisible = false
                            Qt.callLater(window.startHintCollection)
                        }
                    }
                }

                Button {
                    text: "Cancel hints"
                    Accessible.name: "Cancel rapid hints"
                    onClicked: {
                        window.rapidHintConfirmationVisible = false
                        window.closeHints()
                    }
                }
            }
        }
    }

    Rectangle {
        id: copyNotice
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: window.bottomChromeHeight + 8
        width: Math.min(parent.width - 32, 720)
        height: Math.max(74, window.chromeRowHeight * 5)
        z: 30
        visible: window.copyNoticeVisible
        color: window.surfaceColor
        border.color: window.accentColor
        radius: 4

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 8
            spacing: 4

            Label {
                Layout.fillWidth: true
                text: window.copyNoticeSensitive ? "Copied selected document text" : "Copied safe URL"
                color: window.primaryTextColor
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                Label {
                    Layout.fillWidth: true
                    text: window.copiedText
                    color: window.secondaryTextColor
                    elide: Text.ElideMiddle
                    Accessible.name: "Copied URL"
                }

                Button {
                    text: "Copy again"
                    onClicked: window.copyToClipboard(window.copiedValue, !window.copyNoticeSensitive)
                }

                ToolButton {
                    text: "×"
                    onClicked: window.copyNoticeVisible = false
                    Accessible.name: "Dismiss copied URL notice"
                }
            }
        }
    }

    Rectangle {
        id: statusBar
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: window.statusBarHeight
        z: 10
        color: window.surfaceColor
        opacity: window.chromeOpacity
        visible: window.normalStatusVisible

        Accessible.name: "Browser status bar. " + browserUi.mode + ". "
                         + browserUi.display_url + ". " + browserUi.status_text
                         + window.statusDetails(
                             browserUi, window, window.activeWebView(),
                             window.temporaryProfile, window.profileName,
                             window.ephemeralProfile)
        Accessible.role: Accessible.StatusBar

        RowLayout {
            anchors.fill: parent
            spacing: 0

            Rectangle {
                Layout.fillHeight: true
                Layout.preferredWidth: statusModeLabel.implicitWidth + 16
                color: browserUi.mode === "insert" ? window.modeInsertColor
                       : browserUi.mode === "hint" ? window.warningColor
                       : browserUi.mode === "caret" ? window.accentColor
                       : window.panelColor

                Label {
                    id: statusModeLabel
                    anchors.centerIn: parent
                    text: browserUi.mode.toUpperCase()
                    color: browserUi.mode === "normal"
                           ? window.primaryTextColor
                           : window.contrastText(parent.color)
                    font.bold: true
                    Accessible.ignored: true
                }
            }

            Label {
                Layout.leftMargin: 8
                Layout.rightMargin: 8
                Layout.maximumWidth: Math.max(90, statusBar.width * 0.18)
                text: (window.ephemeralProfile ? "EPHEMERAL "
                       : window.temporaryProfile ? "PRIVATE " : "")
                      + window.profileName
                      + (browserUi.context_name
                         ? ":" + String(browserUi.context_name) : "")
                color: window.contextStatusColor(browserUi, window.mutedTextColor)
                elide: Text.ElideRight
                Accessible.ignored: true
            }

            Rectangle {
                Layout.fillHeight: true
                Layout.preferredWidth: 1
                color: window.borderColor
            }

            Label {
                id: statusUrl
                Layout.fillWidth: true
                Layout.leftMargin: 8
                Layout.rightMargin: 8
                text: window.addressPresentation(
                          browserUi.display_url,
                          width / Math.max(1, font.pixelSize * 0.56))
                color: /^https:/i.test(browserUi.display_url)
                       ? window.successColor
                       : (/^http:/i.test(browserUi.display_url)
                          ? window.warningColor : window.primaryTextColor)
                elide: Text.ElideMiddle
                Accessible.ignored: true
            }

            Label {
                Layout.maximumWidth: Math.max(120, statusBar.width * 0.32)
                Layout.rightMargin: 10
                visible: text.length > 0
                text: window.engineUpdateNotice.length > 0
                      ? window.engineUpdateNotice
                      : browserUi.status_text
                        + window.macroStatusText(browserUi.macro_status)
                color: window.engineUpdateNotice.length > 0
                       ? window.warningColor : window.mutedTextColor
                elide: Text.ElideRight
                horizontalAlignment: Text.AlignRight
                Accessible.ignored: true
            }

            Label {
                Layout.rightMargin: 8
                text: {
                    var view = window.activeWebView()
                    var load = view && view.loading
                            ? " " + Math.round(Number(view.loadProgress || 0)) + "%" : ""
                    var media = view && view.audioMuted ? " M" : ""
                    return (browserUi.blocking_active_site_count > 0 ? " B" : "")
                            + media + load + "  "
                            + (browserUi.active_tab_index + 1) + "/" + browserUi.tab_count
                }
                color: window.secondaryTextColor
                font.bold: true
                Accessible.ignored: true
            }
        }
    }

    Rectangle {
        id: commandBar
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: window.inputBarHeight
        z: 20
        visible: browserUi.mode === "command"
        color: window.surfaceColor
        opacity: window.chromeOpacity

        RowLayout {
            anchors.fill: parent
            spacing: 0

            Rectangle {
                Layout.fillHeight: true
                Layout.preferredWidth: commandPrefix.implicitWidth + 16
                color: window.accentColor

                Label {
                    id: commandPrefix
                    anchors.centerIn: parent
                    text: ":"
                    color: window.contrastText(parent.color)
                    font.bold: true
                    Accessible.ignored: true
                }
            }

            TextField {
                id: commandLine
                Layout.fillWidth: true
                Layout.fillHeight: true
                leftPadding: 8
                rightPadding: 8
                topPadding: 0
                bottomPadding: 0
                color: window.primaryTextColor
                selectionColor: window.selectionColor
                selectedTextColor: window.selectionTextColor
                placeholderText: "command"
                placeholderTextColor: window.mutedTextColor
                background: Rectangle { color: "transparent" }
                Accessible.name: "Command line"
                Accessible.role: Accessible.EditableText
                Accessible.editable: true
                focus: commandBar.visible

                onVisibleChanged: {
                    if (visible) {
                        forceActiveFocus()
                        browserUi.update_completion(text, cursorPosition)
                    }
                }
                onTextChanged: browserUi.update_completion(text, cursorPosition)
                onCursorPositionChanged: browserUi.update_completion(text, cursorPosition)
                onAccepted: {
                    var focusedContextWindow = window.focusExistingContextWindow(text, browserUi)
                    if (focusedContextWindow || browserUi.execute_command(text)) {
                        var preview = browserUi.take_session_preview()
                        if (preview.length > 0) {
                            window.showCommandSessionPreview(preview)
                        }
                        if (browserUi.library_kind.length > 0) {
                            window.libraryPage = 0
                            window.openInternalSurface()
                            window.libraryManagerVisible = true
                            window.refreshLibraryManager()
                        }
                        if (browserUi.link_preview_visible) {
                            window.showLinkPreview()
                        }
                        if (text.trim().indexOf("context-enter ") === 0) {
                            window.routeContextWorkspace(browserUi)
                        }
                        window.syncTabModel()
                        window.executePendingEngineAction()
                    text = ""
                    if (browserUi.mode === "command") {
                        browserUi.escape()
                    }
                    } else {
                        selectAll()
                    }
                }
                Keys.onPressed: function(event) {
                    if (event.key === Qt.Key_Escape) {
                        browserUi.escape()
                        event.accepted = true
                    } else if (event.key === Qt.Key_Tab) {
                        browserUi.completion_move(event.modifiers & Qt.ShiftModifier ? -1 : 1)
                        event.accepted = true
                    } else if ((event.modifiers & Qt.ControlModifier) && event.key === Qt.Key_N) {
                        browserUi.completion_move(1)
                        event.accepted = true
                    } else if ((event.modifiers & Qt.ControlModifier) && event.key === Qt.Key_P) {
                        browserUi.completion_move(-1)
                        event.accepted = true
                    }
                }
            }
        }
    }

    Rectangle {
        id: completionPopup
        anchors.left: commandBar.left
        anchors.right: commandBar.right
        anchors.bottom: commandBar.top
        height: Math.min(220, completionList.contentHeight + 8)
        z: 19
        visible: commandBar.visible && browserUi.completion_visible
        color: window.panelColor
        opacity: 1.0
        border.color: window.borderColor
        border.width: 1
        Accessible.role: Accessible.PopupMenu
        Accessible.name: "Command completion popup"
        Accessible.description: "Use Tab or Shift-Tab to move through command completion values"

        ListView {
            id: completionList
            anchors.fill: parent
            anchors.margins: 1
            clip: true
            Accessible.role: Accessible.List
            Accessible.name: "Command completion"
            model: browserUi.completion_text.length ? browserUi.completion_text.split("\n") : []
            delegate: Rectangle {
                id: completionRow
                width: completionList.width
                height: window.chromeRowHeight
                color: index === browserUi.completion_selected
                       ? window.selectionColor : window.panelColor
                opacity: 1.0
                Accessible.role: Accessible.ListItem
                Accessible.name: modelData
                Accessible.selected: index === browserUi.completion_selected
                Accessible.focusable: false

                Text {
                    anchors.fill: parent
                    anchors.leftMargin: 8
                    verticalAlignment: Text.AlignVCenter
                    color: index === browserUi.completion_selected
                           ? window.selectionTextColor
                           : window.readableTextColor(window.primaryTextColor,
                                                      completionRow.color)
                    text: modelData
                    elide: Text.ElideRight
                    Accessible.ignored: true
                }

                MouseArea {
                    anchors.fill: parent
                    onClicked: {
                        browserUi.completion_select(index)
                        commandLine.forceActiveFocus()
                    }
                }
            }
        }
    }

    Rectangle {
        id: searchBar
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: window.inputBarHeight
        z: 20
        visible: browserUi.mode === "search"
        color: window.surfaceColor
        opacity: window.chromeOpacity

        Rectangle {
            id: searchPrefixBackground
            anchors.left: parent.left
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            width: searchPrefix.implicitWidth + 16
            color: window.warningColor

            Label {
                id: searchPrefix
                anchors.centerIn: parent
                text: browserUi.search_backward ? "?" : "/"
                color: window.contrastText(parent.color)
                font.bold: true
                Accessible.ignored: true
            }
        }

        TextField {
            id: searchLine
            anchors.left: searchPrefixBackground.right
            anchors.right: searchBackwardButton.left
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            leftPadding: 8
            rightPadding: 8
            topPadding: 0
            bottomPadding: 0
            text: browserUi.search_text
            color: window.primaryTextColor
            selectionColor: window.selectionColor
            selectedTextColor: window.selectionTextColor
            placeholderText: browserUi.search_backward ? "search backward" : "search"
            placeholderTextColor: window.mutedTextColor
            background: Rectangle { color: "transparent" }
            Accessible.name: browserUi.search_backward ? "Search backward" : "Search forward"
            Accessible.role: Accessible.EditableText
            Accessible.editable: true
            focus: searchBar.visible

            onVisibleChanged: {
                if (visible) {
                    forceActiveFocus()
                }
            }
            onTextChanged: browserUi.search_changed(text)
            onAccepted: {
                browserUi.search_next(browserUi.search_backward)
                browserUi.accept_search()
                window.executePendingEngineAction()
            }
            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    browserUi.escape()
                    event.accepted = true
                }
            }
        }

        ToolButton {
            id: searchBackwardButton
            anchors.right: searchForwardButton.left
            anchors.verticalCenter: parent.verticalCenter
            text: "↑"
            width: window.inputBarHeight
            height: window.inputBarHeight
            padding: 0
            background: Rectangle {
                color: parent.hovered ? window.panelColor : "transparent"
            }
            contentItem: Text {
                text: parent.text
                color: window.mutedTextColor
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
            }
            Accessible.name: "Find previous match"
            onClicked: {
                browserUi.execute_ui_action("browser.tab.search-next", "backward")
                window.executePendingEngineAction()
                searchLine.forceActiveFocus()
            }
        }

        ToolButton {
            id: searchForwardButton
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            text: "↓"
            width: window.inputBarHeight
            height: window.inputBarHeight
            padding: 0
            background: Rectangle {
                color: parent.hovered ? window.panelColor : "transparent"
            }
            contentItem: Text {
                text: parent.text
                color: window.mutedTextColor
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
            }
            Accessible.name: "Find next match"
            onClicked: {
                browserUi.execute_ui_action("browser.tab.search-next", "forward")
                window.executePendingEngineAction()
                searchLine.forceActiveFocus()
            }
        }
    }

    function configuredBlocklistIds() {
        try {
            var config = JSON.parse(browserUi.config_json)
            if (config.blocking && Array.isArray(config.blocking.lists)) {
                return config.blocking.lists
            }
        } catch (error) {
            browserUi.status_text = "Blocklist configuration was invalid"
        }
        return []
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
        try {
            var config = JSON.parse(browserUi.config_json)
            var cleaning = config.links && config.links.cleaning
            return {
                source: cleaning && typeof cleaning.update_source === "string"
                        ? cleaning.update_source : "",
                checksum: cleaning && typeof cleaning.update_sha256 === "string"
                        ? cleaning.update_sha256 : ""
            }
        } catch (error) {
            browserUi.status_text = "Clean-link update configuration was invalid"
        }
        return { source: "", checksum: "" }
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
        var intervalHours = 24
        try {
            var config = JSON.parse(browserUi.config_json)
            if (config.blocking && Number.isFinite(Number(config.blocking.update_interval_hours))) {
                intervalHours = Math.max(1, Math.min(168, Math.floor(Number(config.blocking.update_interval_hours))))
            }
        } catch (error) {
            browserUi.status_text = "Blocklist configuration was invalid"
        }
        window.blocklistUpdateIntervalHours = intervalHours
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
            commandLine.text = action.slice("command-prefill\t".length)
            commandLine.cursorPosition = commandLine.text.length
            commandLine.forceActiveFocus()
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
        if (action.indexOf("jseval\t") === 0) {
            var evalPayload = null
            try {
                evalPayload = JSON.parse(action.split("\t").slice(1).join("\t"))
            } catch (error) {
                browserUi.status_text = "JavaScript evaluation request was invalid"
                return
            }
            var evalIndex = browserUi.tab_index_for_id(String(evalPayload.tab_id || ""))
            var evalView = tabViewAt(evalIndex)
            var evalScript = String(evalPayload.script || "")
            var evalWorld = evalPayload.world === "page"
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
        if (action.indexOf("show-switcher\t") === 0) {
            var switcherPayload = null
            try {
                switcherPayload = JSON.parse(action.split("\t").slice(1).join("\t"))
            } catch (error) {
                browserUi.status_text = "Switcher request was invalid"
                return
            }
            window.showSwitcherWith(
                String(switcherPayload.scope || "all"),
                String(switcherPayload.query || ""))
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
                browserWindowComponent.createObject(null, {
                    windowStartupUrl: windowAction.slice(4).join("\t"),
                    windowProfileName: windowAction[2],
                    windowProfileLabel: windowAction[2],
                    windowStartupContext: windowAction[3],
                    windowPrivateProfile: windowAction[1] === "true"
                })
            } else if (windowAction.length >= 4) {
                browserWindowComponent.createObject(null, {
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
                browserWindowComponent.createObject(null, {
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
                view = webViewComponent.createObject(webViews, {
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
            var caretRequest = browserUi.take_caret_request()
            if (caretRequest.length > 0) {
                var caretData = JSON.parse(caretRequest)
                var caretView = window.activeWebView()
                if (caretView) {
                    window.runBrowserScript(caretView, window.caretScript(caretData.operation, window.caretSelecting), function(value) {
                        var response = value || {error: "caret script returned no result"}
                        if (response.selecting !== undefined) {
                            window.caretSelecting = response.selecting
                        }
                        browserUi.deliver_caret(caretData.token, JSON.stringify(response))
                    })
                } else {
                    browserUi.deliver_caret(caretData.token, JSON.stringify({error: "document view unavailable"}))
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
            var editorCompletion = browserUi.take_editor_completion()
            if (editorCompletion.length > 0) {
                var editorData = JSON.parse(editorCompletion)
                var applyView = window.activeWebView()
                if (editorData.error) {
                    var editorError = editorData.error
                    if (editorData.stderr) {
                        editorError += " (stderr: " + editorData.stderr + ")"
                    }
                    browserUi.deliver_editor_apply(editorData.token, JSON.stringify({error: editorError}))
                } else if (applyView) {
                    window.runBrowserScript(applyView, window.editorApplyScript(editorData.original, editorData.updated), function(value) {
                        browserUi.deliver_editor_apply(editorData.token, JSON.stringify(value || {error: "editor apply returned no result"}))
                    })
                } else {
                    browserUi.deliver_editor_apply(editorData.token, JSON.stringify({error: "document view unavailable"}))
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
            var downloadRequest = browserUi.take_download_request()
            if (downloadRequest.length > 0) {
                var downloadData = JSON.parse(downloadRequest)
                var downloadView = window.activeWebView()
                if (downloadView) {
                    window.runBrowserScript(downloadView, window.downloadLinkScript(downloadData.url), function() {
                        browserUi.complete_download_request(downloadData.token, true)
                    })
                } else {
                    browserUi.complete_download_request(downloadData.token, false)
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

    Rectangle {
        id: permissionPrompt
        anchors.centerIn: parent
        width: Math.min(600, parent.width - 80)
        height: Math.min(280 * window.chromeScale, parent.height - 32)
        z: 90
        visible: window.permissionPromptVisible
                 && window.pendingPermissionHost === window
        focus: visible
        Accessible.role: Accessible.Dialog
        Accessible.name: "Permission request"
        onVisibleChanged: if (visible) forceActiveFocus()
        color: window.panelColor
        border.color: window.accentColor
        border.width: 2

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 16
            spacing: 10

            Label {
                Layout.fillWidth: true
                text: "Permission request"
                color: window.primaryTextColor
                font.bold: true
                Accessible.name: "Permission request"
            }
            Label {
                Layout.fillWidth: true
                text: window.pendingPermissionOrigin + " wants to "
                      + window.permissionDisplayName(window.pendingPermissionName) + "."
                color: window.secondaryTextColor
                wrapMode: Text.WordWrap
                Accessible.name: "Permission request for " + window.pendingPermissionOrigin
            }
            Label {
                Layout.fillWidth: true
                text: (window.pendingPermissionPrivate ? "Private profile · " : "")
                      + "Scope: current document"
                      + (window.pendingPermissionGroupCount > 1
                         ? " · " + window.pendingPermissionGroupCount
                           + " identical requests grouped"
                         : "")
                color: window.mutedTextColor
                wrapMode: Text.WordWrap
                Accessible.name: "Permission scope and grouped request count"
            }
            Label {
                Layout.fillWidth: true
                text: window.permissionCanRemember(
                          window.pendingPermissionOrigin, window.pendingPermissionName)
                      ? "Allow once, or remember an allow rule for this exact site."
                      : "This decision applies to this request only."
                color: window.warningColor
                wrapMode: Text.WordWrap
            }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 8
                Button {
                    text: "Deny"
                    Accessible.name: "Deny permission request"
                    onClicked: window.decidePermission(false, "")
                }
                Button {
                    text: "Allow for session"
                    Accessible.name: "Allow permission for this session"
                    visible: window.pendingPermissionName !== "screen-capture"
                    onClicked: window.decidePermission(true, "session")
                }
                Button {
                    text: "Allow once"
                    Accessible.name: "Allow permission request once"
                    onClicked: window.decidePermission(true, "")
                }
                Button {
                    visible: window.permissionCanRememberForSite(
                        window.pendingPermissionOrigin, window.pendingPermissionName)
                    text: "Allow for site"
                    Accessible.name: "Allow permission for this site"
                    onClicked: window.decidePermission(true, "site")
                }
            }
        }
        Keys.onPressed: function(event) {
            if (event.key === Qt.Key_Escape) {
                window.decidePermission(false, "")
                event.accepted = true
            } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                event.accepted = true
            }
        }
    }

    Rectangle {
        id: shutdownPrompt
        anchors.centerIn: parent
        width: Math.min(560, parent.width - 80)
        height: Math.min(190 * window.chromeScale, parent.height - 32)
        z: 100
        visible: window.shutdownPromptVisible
        color: window.panelColor
        border.color: window.warningColor
        border.width: 2

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 16
            spacing: 12

            Label {
                Layout.fillWidth: true
                text: "Active downloads are still running"
                color: window.primaryTextColor
                font.bold: true
            }
            Label {
                Layout.fillWidth: true
                text: "Cancel active browser work before quitting, or keep the browser open."
                color: window.secondaryTextColor
                wrapMode: Text.WordWrap
            }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 8
                Button {
                    text: "Keep browser open"
                    Accessible.name: "Keep browser open"
                    onClicked: {
                        window.shutdownPromptVisible = false
                        browserUi.status_text = "Shutdown cancelled"
                    }
                }
                Button {
                    text: "Cancel active work and quit"
                    Accessible.name: "Cancel active work and quit"
                    onClicked: window.cancelDownloadsAndQuit()
                }
            }
        }
    }

    Rectangle {
        id: shutdownPagePrompt
        anchors.centerIn: parent
        width: Math.min(560, parent.width - 80)
        height: Math.min(220 * window.chromeScale, parent.height - 32)
        z: 100
        visible: window.shutdownPagePromptVisible
        color: window.panelColor
        border.color: window.warningColor
        border.width: 2

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 16
            spacing: 12

            Label {
                Layout.fillWidth: true
                text: "Page state may be lost"
                color: window.primaryTextColor
                font.bold: true
            }
            Label {
                Layout.fillWidth: true
                text: window.shutdownPagePromptReason
                      + " Close anyway may lose that state."
                color: window.secondaryTextColor
                wrapMode: Text.WordWrap
            }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 8
                Button {
                    text: "Keep browser open"
                    Accessible.name: "Keep browser open"
                    onClicked: {
                        window.shutdownPageProbeGeneration += 1
                        window.shutdownPagePromptVisible = false
                        browserUi.status_text = "Shutdown cancelled"
                    }
                }
                Button {
                    text: "Close anyway"
                    Accessible.name: "Close anyway despite page state"
                    onClicked: window.finalizeQuit()
                }
            }
        }
    }

    Rectangle {
        id: applicationShutdownForcePrompt
        anchors.centerIn: parent
        width: Math.min(600, parent.width - 80)
        height: Math.min(250 * window.chromeScale, parent.height - 32)
        z: 110
        visible: window.applicationShutdownForcePromptVisible
        color: window.panelColor
        border.color: window.errorColor
        border.width: 2

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 16
            spacing: 12

            Label {
                Layout.fillWidth: true
                text: "Browser shutdown is taking longer than expected"
                color: window.primaryTextColor
                font.bold: true
            }
            Label {
                Layout.fillWidth: true
                text: window.applicationShutdownStage
                      + ". Continue waiting, or force quit? Force quit leaves the "
                      + "unclean-exit marker for recovery on the next launch."
                color: window.secondaryTextColor
                wrapMode: Text.WordWrap
            }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 8
                Button {
                    text: "Continue waiting"
                    Accessible.name: "Continue waiting for shutdown"
                    onClicked: {
                        window.applicationShutdownForcePromptVisible = false
                        applicationShutdownTimer.restart()
                    }
                }
                Button {
                    text: "Force quit (unclean)"
                    Accessible.name: "Force quit and preserve the unclean marker"
                    onClicked: window.forceApplicationShutdown()
                }
            }
        }
    }

    Component {
        id: permissionPromptSurfaceComponent

        Rectangle {
            property var hostWindow
            width: Math.min(600, hostWindow ? hostWindow.width - 80 : 520)
            height: Math.min(280 * window.chromeScale, hostWindow.height - 32)
            anchors.centerIn: parent
            z: 90
            visible: window.permissionPromptVisible
                     && window.pendingPermissionHost === hostWindow
            focus: visible
            Accessible.role: Accessible.Dialog
            Accessible.name: "Permission request"
            onVisibleChanged: if (visible) forceActiveFocus()
            color: window.panelColor
            border.color: window.accentColor
            border.width: 2

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 16
                spacing: 10

                Label {
                    Layout.fillWidth: true
                    text: "Permission request"
                    color: window.primaryTextColor
                    font.bold: true
                    Accessible.name: "Permission request"
                }
                Label {
                    Layout.fillWidth: true
                    text: window.pendingPermissionOrigin + " wants to "
                          + window.permissionDisplayName(window.pendingPermissionName) + "."
                    color: window.secondaryTextColor
                    wrapMode: Text.WordWrap
                    Accessible.name: "Permission request for " + window.pendingPermissionOrigin
                }
                Label {
                    Layout.fillWidth: true
                    text: (window.pendingPermissionPrivate ? "Private profile · " : "")
                          + "Scope: current document"
                          + (window.pendingPermissionGroupCount > 1
                             ? " · " + window.pendingPermissionGroupCount
                               + " identical requests grouped"
                             : "")
                    color: window.mutedTextColor
                    wrapMode: Text.WordWrap
                    Accessible.name: "Permission scope and grouped request count"
                }
                Label {
                    Layout.fillWidth: true
                    text: window.permissionCanRemember(
                              window.pendingPermissionOrigin, window.pendingPermissionName)
                          ? "Allow once, or remember an allow rule for this exact site."
                          : "This decision applies to this request only."
                    color: window.warningColor
                    wrapMode: Text.WordWrap
                }
                RowLayout {
                    Layout.alignment: Qt.AlignRight
                    spacing: 8
                    Button {
                        text: "Deny"
                        Accessible.name: "Deny permission request"
                        onClicked: window.decidePermission(false, "")
                    }
                    Button {
                        text: "Allow for session"
                        Accessible.name: "Allow permission for this session"
                        visible: window.pendingPermissionName !== "screen-capture"
                        onClicked: window.decidePermission(true, "session")
                    }
                    Button {
                        text: "Allow once"
                        Accessible.name: "Allow permission request once"
                        onClicked: window.decidePermission(true, "")
                    }
                    Button {
                        visible: window.permissionCanRememberForSite(
                            window.pendingPermissionOrigin, window.pendingPermissionName)
                        text: "Allow for site"
                        Accessible.name: "Allow permission for this site"
                        onClicked: window.decidePermission(true, "site")
                    }
                }
            }

            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    window.decidePermission(false, "")
                    event.accepted = true
                } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                    event.accepted = true
                }
            }
        }
    }

    Component {
        id: captureIndicatorComponent

        Rectangle {
            property var hostWindow
            width: Math.min(520 * window.chromeScale,
                            hostWindow ? hostWindow.width - 32 : 488)
            height: Math.min(90 * Math.max(1, window.chromeScale)
                             + (window.captureSessions.length * 42),
                             hostWindow ? hostWindow.height - 32 : 560)
            anchors.top: parent.top
            anchors.right: parent.right
            anchors.topMargin: 8
            anchors.rightMargin: 8
            z: 120
            visible: hostWindow !== null && window.captureSessions.some(function(session) {
                return session && session.host === hostWindow
            })
            color: window.panelColor
            border.color: window.warningColor
            border.width: 2
            radius: 4
            Accessible.role: Accessible.Dialog
            Accessible.name: "Active capture indicator"
            focus: visible

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 8
                spacing: 4

                Label {
                    Layout.fillWidth: true
                    text: "Capture indicator"
                    color: window.warningColor
                    font.bold: true
                    Accessible.name: "Capture indicator title"
                }

                ListView {
                    id: captureIndicatorList
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    model: hostWindow
                           ? window.captureSessions.filter(function(session) {
                               return session && session.host === hostWindow
                           }) : []
                    delegate: RowLayout {
                        required property var modelData
                        Layout.fillWidth: true
                        spacing: 6

                        Label {
                            Layout.fillWidth: true
                            text: modelData.origin + " · "
                                  + (modelData.status === "active"
                                     ? "capture active"
                                     : modelData.status === "stop-requested"
                                       ? "stop requested"
                                       : "ended by navigation")
                            color: window.primaryTextColor
                            elide: Text.ElideMiddle
                            Accessible.name: "Capture origin and state"
                        }

                        Button {
                            text: modelData.status === "ended-by-navigation"
                                  ? "Dismiss" : "Stop"
                            Accessible.name: text + " capture for " + modelData.origin
                            onClicked: {
                                if (modelData.status === "ended-by-navigation") {
                                    window.dismissCaptureSession(modelData.id)
                                } else {
                                    window.stopCaptureSession(modelData.id)
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Component {
        id: desktopMediaSurfaceComponent

        Rectangle {
            property var hostWindow
            property bool showingWindows: false
            width: Math.min(680, hostWindow ? hostWindow.width - 80 : 600)
            height: Math.min(520, hostWindow ? hostWindow.height - 100 : 420)
            anchors.centerIn: parent
            z: 95
            visible: window.desktopMediaPromptVisible
                     && window.pendingDesktopMediaHost === hostWindow
            focus: visible
            Accessible.role: Accessible.Dialog
            Accessible.name: "Screen sharing source chooser"
            color: window.panelColor
            border.color: window.warningColor
            border.width: 2
            onVisibleChanged: {
                if (visible) {
                    showingWindows = false
                    forceActiveFocus()
                }
            }

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 16
                spacing: 10

                Label {
                    Layout.fillWidth: true
                    text: "Choose what to share"
                    color: window.primaryTextColor
                    font.bold: true
                    Accessible.name: "Screen sharing source chooser"
                }
                Label {
                    Layout.fillWidth: true
                    text: "Origin: " + window.desktopMediaOrigin
                    color: window.mutedTextColor
                    elide: Text.ElideMiddle
                    Accessible.name: "Screen sharing requesting origin"
                }
                Label {
                    Layout.fillWidth: true
                    text: "Select one source for this request. Ferric Browser will not reuse a previous source."
                    color: window.secondaryTextColor
                    wrapMode: Text.WordWrap
                }
                RowLayout {
                    Layout.fillWidth: true
                    Button {
                        text: "Screens"
                        checked: !showingWindows
                        checkable: true
                        onClicked: showingWindows = false
                        Accessible.name: "Show screens"
                    }
                    Button {
                        text: "Windows"
                        checked: showingWindows
                        checkable: true
                        onClicked: showingWindows = true
                        Accessible.name: "Show windows"
                    }
                    Item { Layout.fillWidth: true }
                }
                ListView {
                    id: desktopMediaSourceList
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    spacing: 6
                    model: window.pendingDesktopMediaRequest
                           ? (showingWindows
                              ? window.pendingDesktopMediaRequest.windowsModel
                              : window.pendingDesktopMediaRequest.screensModel)
                           : null
                    delegate: Button {
                        width: desktopMediaSourceList.width
                        text: (showingWindows ? "Window " : "Screen ") + (index + 1)
                        Accessible.name: text
                        onClicked: {
                            if (showingWindows) {
                                window.selectDesktopWindow(index)
                            } else {
                                window.selectDesktopScreen(index)
                            }
                        }
                    }
                }
                Label {
                    Layout.fillWidth: true
                    visible: desktopMediaSourceList.count === 0
                    text: showingWindows ? "No windows are available." : "No screens are available."
                    color: window.warningColor
                    horizontalAlignment: Text.AlignHCenter
                }
                RowLayout {
                    Layout.fillWidth: true
                    Item { Layout.fillWidth: true }
                    Button {
                        text: "Cancel"
                        Accessible.name: "Cancel screen sharing source selection"
                        onClicked: window.clearDesktopMediaRequest(true)
                    }
                }
            }

            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    window.clearDesktopMediaRequest(true)
                    event.accepted = true
                } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                    event.accepted = true
                }
            }
        }
    }

    Component {
        id: rendererFailureSurfaceComponent

        Rectangle {
            property var hostWindow
            width: Math.min(680, hostWindow ? hostWindow.width - 80 : 600)
            height: Math.min(260 * window.chromeScale, hostWindow.height - 32)
            anchors.centerIn: parent
            z: 85
            visible: window.rendererFailureVisible
                     && window.rendererFailureHost === hostWindow
                     && (hostWindow !== window
                         || window.rendererFailureView === window.activeWebView())
            focus: visible
            Accessible.role: Accessible.Dialog
            Accessible.name: "Page renderer failure"
            onVisibleChanged: if (visible) forceActiveFocus()
            color: window.panelColor
            border.color: window.errorColor
            border.width: 2

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 16
                spacing: 10

                Label {
                    Layout.fillWidth: true
                    text: "Page renderer stopped"
                    color: window.errorColor
                    font.bold: true
                    Accessible.name: "Page renderer failure"
                }
                Label {
                    Layout.fillWidth: true
                    text: "Safe URL: " + window.rendererFailureSafeUrl
                    color: window.primaryTextColor
                    elide: Text.ElideMiddle
                    Accessible.name: "Safe URL for failed page"
                }
                Label {
                    Layout.fillWidth: true
                    text: "Reason: " + window.rendererFailureReason
                          + " · exit code " + window.rendererFailureExitCode
                    color: window.secondaryTextColor
                    wrapMode: Text.WordWrap
                }
                Label {
                    Layout.fillWidth: true
                    text: window.rendererFailureCount > 1
                          ? "This renderer has failed repeatedly. Ferric Browser will not auto-reload it into a loop."
                          : "Other tabs remain usable. Choose an explicit recovery action."
                    color: window.warningColor
                    wrapMode: Text.WordWrap
                }
                RowLayout {
                    Layout.fillWidth: true
                    Item { Layout.fillWidth: true }
                    Button {
                        text: "Reload"
                        Accessible.name: "Reload failed page"
                        onClicked: window.reloadRendererFailure()
                    }
                    Button {
                        text: hostWindow === window ? "Close tab" : "Close window"
                        Accessible.name: text
                        onClicked: window.closeRendererFailure()
                    }
                    Button {
                        text: "Copy safe URL"
                        Accessible.name: "Copy safe URL from renderer failure"
                        onClicked: window.copyToClipboard(window.rendererFailureSafeUrl, true)
                    }
                    Button {
                        text: "Diagnostics"
                        Accessible.name: "Open renderer diagnostics"
                        onClicked: window.showRendererFailureDiagnostics()
                    }
                    Button {
                        text: "Restart software"
                        Accessible.name: "Restart with software rendering"
                        enabled: !window.softwareRendering
                        onClicked: window.restartSoftwareRendering()
                    }
                }
            }

            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    window.rendererFailureVisible = false
                    window.restoreOverlayFocus()
                    event.accepted = true
                }
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
            browserUi.context_entry_force_reuse = true
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
        var urls
        try {
            urls = JSON.parse(window.startupAdditionalUrlsJson || "[]")
        } catch (error) {
            browserUi.status_text = "Additional startup URLs were invalid"
            return
        }
        if (!Array.isArray(urls)) {
            browserUi.status_text = "Additional startup URLs were invalid"
            return
        }
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
        window.browserProfile = browserProfilePrototype.instance()
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
        browserUi.config_json = window.startupConfigJson
        browserUi.config_base_json = window.startupConfigBaseJson
        browserUi.cli_overrides_json = window.startupCliOverridesJson
        browserUi.profile_overrides_json = window.startupProfileOverridesJson
        browserUi.config_path = window.startupConfigPath
        browserUi.config_source = window.startupConfigSource
        browserUi.contexts_json = window.startupContextsJson
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

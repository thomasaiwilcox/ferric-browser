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

ApplicationWindow {
    id: window
    function executeCapturedPdf(ui, view, path) { return runtimeServices.executeCapturedPdf(ui, view, path) }
    function executeCapturedSave(ui, view, path) { return runtimeServices.executeCapturedSave(ui, view, path) }
    function executeCapturedPrint(ui, view) { return runtimeServices.executeCapturedPrint(ui, view) }
    function executeCapturedScript(ui, view, script, world) { return runtimeServices.executeCapturedScript(ui, view, script, world) }
    FerricWindowLifecycleController { id: runtimeBase; host: window }
    FerricPageServices { id: runtimeServices; host: window }
    FerricWindowPresentationController { id: runtimePresentation; host: window }
    FerricWindowRequestsController { id: runtimeRequests; host: window }
    FerricCommandEntryController { id: runtimeChrome; host: window }
    FerricRuntimeController { id: runtimeSurface; host: window }
    readonly property alias runtime_commandNoticeTimer: commandNoticeTimer
    readonly property alias runtime_focusOverlayController: focusOverlayController
    readonly property alias runtime_chromePresentationController: chromePresentationController
    readonly property alias runtime_windowRegistryController: windowRegistryController
    readonly property alias runtime_permissionScopeBridge: permissionScopeBridge
    readonly property alias runtime_webNotificationController: webNotificationController
    readonly property alias runtime_tabSwitchingTimer: tabSwitchingTimer
    readonly property alias runtime_selectionSourceFactory: selectionSourceFactory
    readonly property alias runtime_copyNoticeTimer: copyNoticeTimer
    readonly property alias runtime_permissionPolicyRefresh: permissionPolicyRefresh
    readonly property alias runtime_permissionPromptExpiry: permissionPromptExpiry
    readonly property alias runtime_pageDialogPopup: pageDialogPopup
    readonly property alias runtime_browserWindowComponent: browserWindowComponent
    readonly property alias runtime_keyboardHelpSurface: keyboardHelpSurface
    readonly property alias runtime_settingsWorkspace: settingsWorkspace
    readonly property alias runtime_libraryManager: libraryManager
    readonly property alias runtime_switcherSurface: switcherSurface
    readonly property alias runtime_webViews: webViews
    readonly property alias runtime_webViewComponent: webViewComponent
    readonly property alias runtime_runtimeWorkTimer: runtimeWorkTimer
    readonly property alias runtime_focusProbeTimer: focusProbeTimer
    readonly property alias runtime_scrollCaptureTimer: scrollCaptureTimer

    readonly property var modalInputScope: FerricModalInputController { hostWindow: window }
    property bool runtimeDisposing: false
    property bool profileBootstrapComplete: false
    property var primaryProfileResources: null
    readonly property alias profileResourceRegistry: profileResources
    FerricProfileResourceRegistry { id: profileResources; rootWindow: window }
    readonly property alias scriptRunnerObject: scriptRunner
    FerricScriptRunner { id: scriptRunner }
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
    property var permissionPromptSurfaceComponentObject: permissionPromptSurfaceComponent
    property var captureIndicatorComponentObject: captureIndicatorComponent
    property var desktopMediaSurfaceComponentObject: desktopMediaSurfaceComponent
    property var rendererFailureSurfaceComponentObject: rendererFailureSurfaceComponent
    property var browserWindowComponentObject: browserWindowComponent
    property var popupWindowComponentObject: popupWindowComponent
    property var webViewComponentObject: webViewComponent
    property var webViewsObject: webViews
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
    property string statusbarMode: "in-mode"
    property bool commandNoticeVisible: false
    property string commandNoticeText: ""
    property bool commandNoticeError: false
    property string pendingInteractiveCommand: ""
    property string tabsMode: "multiple"
    property string tabPosition: "top"
    property bool tabSwitchingVisible: false
    readonly property bool tabStripVisible:
        tabsMode === "always"
        || (tabsMode === "multiple" && browserUi.tab_count > 1)
        || (tabsMode === "switching" && tabSwitchingVisible)
    readonly property bool sideTabs: tabPosition === "left" || tabPosition === "right"
    readonly property real sideTabWidth: Math.max(160, Math.min(280, width * 0.22))
    readonly property bool inputBarActive: browserUi.mode === "command"
                                           || browserUi.mode === "search"
    readonly property bool statusBarVisible:
        (window.commandNoticeVisible && !window.inputBarActive)
        || statusBarVisibleForMode(browserUi.mode)
    readonly property real bottomControlsHeight:
        inputBarActive ? inputBarHeight : (statusBarVisible ? statusBarHeight : 0)
    property real profileNoticeHeight: profileNotice.height
    property bool profileNoticeInputActive: profileNotice.inputActive
    readonly property real bottomChromeHeight:
        bottomControlsHeight + profileNoticeHeight
        + (tabStripVisible && tabPosition === "bottom"
           ? tabBarHeight : 0)
    readonly property real chromeOpacity: 1.0
    property var browserWindowRegistry: []
    property var popupWindowRegistry: []
    property bool applicationShutdownInProgress: false
    property bool applicationShutdownForcePromptVisible: false
    property var attachedDevToolsView: null
    readonly property alias devToolsWorkspace: primaryDevTools
    readonly property bool devToolsVisible: primaryDevTools.visible
    readonly property bool devToolsDetached: primaryDevTools.detached
    readonly property var devToolsExternalView: primaryDevTools.detached ? primaryDevTools.inspector : null
    readonly property var devToolsDetachedWindow: primaryDevTools.detachedWindow
    FerricDevToolsWorkspace {
        id: primaryDevTools
        browserWindow: window
        hostWindow: window
        browserUi: primaryBrowserUi
        activeView: window.activeWebView()
        windowFactory: window.devToolsWindowFactory
        attachedInspector: window.attachedDevToolsView
    }
    property real devToolsPanelHeight: 280
    property string applicationShutdownStage: ""
    property double applicationShutdownStartedAt: 0
    property int applicationShutdownGeneration: 0
    property alias ephemeralProfileOwners: windowRegistryController.ephemeralProfileOwners
    readonly property alias switcherRegistry: windowRegistryController
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
    readonly property string displayScaleLabel: ChromePresentation.displayScaleLabel(
                                                   window.displayScale, window.logicalPixelDensity)
    FontMetrics {
        id: chromeFontMetrics
        font: window.font
    }

    Timer {
        id: commandNoticeTimer
        interval: 3500
        repeat: false
        onTriggered: window.commandNoticeVisible = false
    }

    function statusBarVisibleForMode(mode) { return runtimeBase.statusBarVisibleForMode(mode) }

    function showCommandNotice(text, isError) { return runtimeBase.showCommandNotice(text, isError) }

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

    function contrastText(background) { return runtimeBase.contrastText(background) }

    function readableTextColor(candidate, background) { return runtimeBase.readableTextColor(candidate, background) }

    function renderedContrastReport() { return runtimeBase.renderedContrastReport() }

    function refreshChromeAppearance() { return runtimeBase.refreshChromeAppearance() }

    function noteTabActivity() { return runtimeBase.noteTabActivity() }

    function spellcheckEnabled(ui) { return runtimeBase.spellcheckEnabled(ui) }

    function spellcheckDictionaryStatus(ui) { return runtimeBase.spellcheckDictionaryStatus(ui) }

    function spellcheckLanguages(ui) { return runtimeBase.spellcheckLanguages(ui) }

    function refreshSpellcheckInventory(ui) { return runtimeBase.refreshSpellcheckInventory(ui) }

    function spellcheckStatusText(ui) { return runtimeBase.spellcheckStatusText(ui) }

    function desktopNotificationsEnabled(ui) { return runtimeBase.desktopNotificationsEnabled(ui) }

    function pushServiceEnabled(ui, privateProfile) { return runtimeBase.pushServiceEnabled(ui, privateProfile) }

    function mediaKeysEnabled(ui) { return runtimeBase.mediaKeysEnabled(ui) }

    function handleMediaKey(event) { return runtimeBase.handleMediaKey(event) }

    function triggerMediaToggle(view) { return runtimeBase.triggerMediaToggle(view) }

    function updateMprisForPrimaryView(view) { return runtimeBase.updateMprisForPrimaryView(view) }

    function safeNotificationOrigin(notification) { return runtimeBase.safeNotificationOrigin(notification) }

    function addressPresentation(value, maxCharacters) { return runtimeBase.addressPresentation(value, maxCharacters) }

    function closeWebNotifications() { return runtimeBase.closeWebNotifications() }

    function removePendingPortalNotification(notification) { return runtimeBase.removePendingPortalNotification(notification) }

    function notificationOwnerFor(notification, requireKnown) { return runtimeBase.notificationOwnerFor(notification, requireKnown) }

    function rememberNotificationOwner(notification, ui) { return runtimeBase.rememberNotificationOwner(notification, ui) }

    function forgetNotificationOwner(notification) { return runtimeBase.forgetNotificationOwner(notification) }

    function deferredNotificationOwnerValid(entry) { return runtimeBase.deferredNotificationOwnerValid(entry) }

    function deferNotificationUntilPortal(ui, notification, privateProfile, previous) { return runtimeBase.deferNotificationUntilPortal(ui, notification, privateProfile, previous) }

    function processPendingPortalNotifications() { return runtimeBase.processPendingPortalNotifications() }

    function cancelPendingExternalUris(message) { return runtimeBase.cancelPendingExternalUris(message) }

    function processPendingExternalUris() { return runtimeBase.processPendingExternalUris() }

    FerricPopupOwnerBridge { id: permissionScopeBridge }

    function sharesPermissionSession(left, right) { return runtimeBase.sharesPermissionSession(left, right) }

    function closeWebNotificationsForOrigin(origin, sourceUi) { return runtimeBase.closeWebNotificationsForOrigin(origin, sourceUi) }

    function activeWebNotification(notification) { return runtimeBase.activeWebNotification(notification) }

    function notificationProfileScope(ui, privateProfile) { return runtimeBase.notificationProfileScope(ui, privateProfile) }

    function presentWebNotification(ui, notification, privateProfile, previous) { return runtimeBase.presentWebNotification(ui, notification, privateProfile, previous) }

    function focusTargetAvailable(target) { return runtimeBase.focusTargetAvailable(target) }

    function beginNativePromptLease(ui, view, category, tabIndex) { return runtimeBase.beginNativePromptLease(ui, view, category, tabIndex) }

    function completeNativePromptLease(ui, token, view) { return runtimeBase.completeNativePromptLease(ui, token, view) }

    function nativePromptActiveFor(ui) { return runtimeBase.nativePromptActiveFor(ui) }

    // Bindings need observable presentation properties. The opaque runtime
    // lease query remains in nativePromptActiveFor for each dispatched key.
    function presentedPromptActiveFor(ui) { return runtimeBase.presentedPromptActiveFor(ui) }

    function captureOverlayFocus(hostWindow, fallbackTarget) { return runtimeBase.captureOverlayFocus(hostWindow, fallbackTarget) }

    function restoreOverlayFocus(hostWindow) { return runtimeBase.restoreOverlayFocus(hostWindow) }

    function discardOverlayFocus(hostWindow) { return runtimeBase.discardOverlayFocus(hostWindow) }

    function captureModeFocus() { return runtimeBase.captureModeFocus() }

    function restoreModeFocus() { return runtimeBase.restoreModeFocus() }

    function openInternalSurface() { return runtimeBase.openInternalSurface() }

    function closeInternalSurface() { return runtimeBase.closeInternalSurface() }

    function showContextRoute(ui) { return runtimeBase.showContextRoute(ui) }

    function closeContextRoutePrompt() { return runtimeBase.closeContextRoutePrompt() }

    function showContextMovePicker(tabId) { return runtimeBase.showContextMovePicker(tabId) }

    function chooseContextMove(name) { return runtimeBase.chooseContextMove(name) }

    function executeNewWindowAction(ui, action) { return runtimeBase.executeNewWindowAction(ui, action) }

    function executeContextRouteWindowAction(ui) { return runtimeBase.executeContextRouteWindowAction(ui) }

    function acceptContextRoute() { return runtimeBase.acceptContextRoute() }

    function dismissContextRoute() { return runtimeBase.dismissContextRoute() }

    function registerBrowserWindow(hostWindow, ui, view, profile, profileName, invocationToken, isEphemeral) { return runtimeBase.registerBrowserWindow(hostWindow, ui, view, profile, profileName, invocationToken, isEphemeral) }

    function liveWindowEntries() { return runtimeBase.liveWindowEntries() }

    function publishBrowserWindowRegistry() { return runtimeBase.publishBrowserWindowRegistry() }

    function updateBrowserWindowView(ui, view) { return runtimeBase.updateBrowserWindowView(ui, view) }

    function browserWindowEntryForTarget(target) { return runtimeBase.browserWindowEntryForTarget(target) }

    function browserWindowEntryForContext(contextName, sourceEntry) { return runtimeBase.browserWindowEntryForContext(contextName, sourceEntry) }

    function browserWindowEntryForUi(ui) { return runtimeBase.browserWindowEntryForUi(ui) }

    function focusedBrowserWindowEntry(preferredUi, preferredHost) { return runtimeBase.focusedBrowserWindowEntry(preferredUi, preferredHost) }

    function unregisterBrowserWindow(ui) { return runtimeBase.unregisterBrowserWindow(ui) }

    function registerPopupWindow(hostWindow, ui, view) { return runtimeBase.registerPopupWindow(hostWindow, ui, view) }

    function unregisterPopupWindow(hostWindow) { return runtimeBase.unregisterPopupWindow(hostWindow) }

    function shutdownParticipantCount() { return runtimeBase.shutdownParticipantCount() }

    function abortApplicationShutdown() { return runtimeBase.abortApplicationShutdown() }

    function finishApplicationShutdown() { return runtimeBase.finishApplicationShutdown() }

    function advanceApplicationShutdown() { return runtimeBase.advanceApplicationShutdown() }

    function beginApplicationShutdown() { return runtimeBase.beginApplicationShutdown() }

    function forceApplicationShutdown() { return runtimeBase.forceApplicationShutdown() }

    function retainEphemeralProfileOwner(profile, token) { return runtimeBase.retainEphemeralProfileOwner(profile, token) }

    function releaseEphemeralProfileOwner(profile, token) { return runtimeBase.releaseEphemeralProfileOwner(profile, token) }

    function switcherOwner(result) { return runtimeBase.switcherOwner(result) }

    function ephemeralProfileForToken(token) { return runtimeBase.ephemeralProfileForToken(token) }

    function ephemeralTokenForUi(ui) { return runtimeBase.ephemeralTokenForUi(ui) }

    function executeWindowFocusAction(sourceUi, action) { return runtimeBase.executeWindowFocusAction(sourceUi, action) }

    function activateBrowserWindow(hostWindow, targetUi, operationId) { return runtimeBase.activateBrowserWindow(hostWindow, targetUi, operationId) }

    function focusNotificationOrigin(profileScope, origin) { return runtimeBase.focusNotificationOrigin(profileScope, origin) }

    function focusExistingContextWindow(commandText, sourceUi) { return runtimeBase.focusExistingContextWindow(commandText, sourceUi) }

    function contextStatus(ui, privateProfile, profileName, ephemeralProfile) { return runtimeBase.contextStatus(ui, privateProfile, profileName, ephemeralProfile) }

    function contextStatusColor(ui, fallback) { return runtimeBase.contextStatusColor(ui, fallback) }

    function statusTransport(view) { return runtimeBase.statusTransport(view) }

    function statusLoad(view) { return runtimeBase.statusLoad(view) }

    function statusMedia(view) { return runtimeBase.statusMedia(view) }

    function statusPermission(ui) { return runtimeBase.statusPermission(ui) }

    function statusCapture(hostWindow) { return runtimeBase.statusCapture(hostWindow) }

    function statusDownloads(hostWindow) { return runtimeBase.statusDownloads(hostWindow) }

    function statusTabBlocking(view) { return runtimeBase.statusTabBlocking(view) }

    function statusDetails(ui, hostWindow, view, privateProfile, profileName, ephemeralProfile) { return runtimeBase.statusDetails(ui, hostWindow, view, privateProfile, profileName, ephemeralProfile) }

    function routeContextWorkspace(ui) { return runtimeBase.routeContextWorkspace(ui) }

    onClosing: function(close) {
        if (window.shutdownApproved) {
            return
        }
        close.accepted = false
        window.beginQuitRequest(":window-close")
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
    property bool nativeWayland: false
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
    property var contextRouteHost: null
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
    property bool profileManagerVisible: false
    property bool profileDeletePreviewVisible: false
    property string profileDeleteName: ""
    property bool downloadManagerVisible: false
    property var spellcheckInventory: []
    property alias activeWebNotifications: webNotificationController.activeWebNotifications
    property alias webNotificationSequence: webNotificationController.webNotificationSequence
    property var activeDownloads: ({})
    property var pendingDownloadRequests: ({})
    property alias notificationOwners: webNotificationController.notificationOwners
    property alias pendingPortalNotifications: webNotificationController.pendingPortalNotifications
    property var pendingExternalUris: []
    property string pendingDownloadId: ""
    property string pendingDownloadSuggestedName: ""
    property var pendingFileDialogRequest: null
    property var pendingFileDialogView: null
    property var pendingFileDialogUi: null
    property string pendingFileDialogLeaseToken: ""
    property var pendingPermissionRequest: null
    property var pendingPermissionUi: null
    property string pendingPermissionLeaseToken: ""
    property var pendingPermissionView: null
    property var permissionLeaseUi: null
    property var permissionLeaseView: null
    property int pendingPermissionTabIndex: -1
    property string pendingPermissionOrigin: ""
    property string pendingPermissionName: ""
    property string pendingPermissionPolicyNotice: ""
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
    property string pendingDesktopMediaLeaseToken: ""
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
    property string pendingPageDialogLeaseToken: ""
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
    property string pendingClientCertificateLeaseToken: ""
    property var pendingClientCertificateOptions: []
    property string pendingClientCertificateHost: ""
    property var pendingCertificateError: null
    property var pendingCertificateErrorUi: null
    property var pendingCertificateErrorView: null
    property string pendingCertificateErrorLeaseToken: ""
    property string pendingCertificateErrorHost: ""
    property string pendingCertificateErrorDescription: ""
    property var pendingWebAuthRequest: null
    property var pendingWebAuthUi: null
    property var pendingWebAuthView: null
    property string pendingWebAuthLeaseToken: ""
    property var pendingWebAuthUserNames: []
    property var pendingWebAuthState: null
    property string pendingWebAuthRelyingParty: ""
    property string webAuthStatusText: ""
    property var pendingContextMenuRequest: null
    property var pendingContextMenuView: null
    property var pendingContextMenuSelection: null
    property var pendingContextMenuUi: null
    property string pendingContextMenuLeaseToken: ""
    property var pendingContextMenuHost: null
    property var contextMenuItems: []
    property bool shutdownConfirmationVisible: false
    property string shutdownConfirmationCommand: ":window-close"
    property bool shutdownPromptVisible: false
    property bool shutdownPagePromptVisible: false
    property string shutdownPagePromptReason: ""
    property int shutdownPageProbeGeneration: 0
    property bool shutdownApproved: false
    property bool switcherVisible: false
    property var switcherUi: null
    property var switcherHost: null
    property var switcherProfile: null
    property string switcherOwnerToken: ""
    property var switcherResults: []
    property var switcherResultContext: null
    property string switcherScope: "all"
    property var switcherBatchEntries: []
    property var switcherBatchMerged: []
    property var switcherBatchSources: []
    property var switcherBatchPrevious: []
    property var switcherSourceState: ({pending: false, failed: false, retryable: false, notice: "", detail: ""})
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
        site_doctor: ({ active: ({ kind: "" }), experiments: [] }),
        interaction: ({})
    })
    property var diagnosticsUi: null
    property var diagnosticsHost: null
    readonly property var diagnosticsAppearance: diagnosticsHost && diagnosticsHost !== window
                                               ? diagnosticsHost.chromeAppearance : window
    property var diagnosticsExportUi: null
    property string diagnosticsStartupDetails: ""
    property bool diagnosticsVisible: false
    property string diagnosticsText: ""
    property string diagnosticsExportNotice: ""
    property string diagnosticsExportDetail: ""
    property string engineUpdateStatus: ""
    property string engineUpdateNotice: ""
    property var resolvedQtRequests: []
    property var qtResolutionCounts: ({})
    property bool bindingHelpVisible: false
    property string bindingHelpSearch: ""

    function noteQtResolution(ui, kind, outcome) { return runtimeBase.noteQtResolution(ui, kind, outcome) }

    function resolveQtRequest(ui, request, kind, method, args) { return runtimeBase.resolveQtRequest(ui, request, kind, method, args) }
    property var bindingHelpRows: []
    property bool bindingOverlayVisible: false
    property bool settingsVisible: false
    property bool settingsTemporary: false
    property string settingsSearch: ""
    property string settingsNotice: ""
    property string pendingUserscriptRemoval: ""
    property bool libraryManagerVisible: false
    property string pendingLibraryDelete: ""
    property var privateBookmarkTransferUi: null
    property var privateBookmarkTransferWindow: null
    property string privateBookmarkTransferReceipt: ""
    property string privateBookmarkTransferLabel: ""
    property string privateBookmarkTransferMessage: ""
    property string privateBookmarkTransferDetail: ""
    property bool privateBookmarkTransferPending: false
    property bool privateBookmarkTransferFailed: false
    onPrivateBookmarkTransferUiChanged: if (!privateBookmarkTransferUi && privateBookmarkTransferPending && privateBookmarkTransferReceipt.length > 0) finishPrivateBookmarkTransfer(false, "The destination closed before the save could be confirmed. Check its bookmarks before trying again.", "")
    onPrivateBookmarkTransferWindowChanged: if (!privateBookmarkTransferWindow && privateBookmarkTransferPending && privateBookmarkTransferReceipt.length === 0) finishPrivateBookmarkTransfer(false, "The destination closed before the save could be confirmed. Check its bookmarks before trying again.", "")
    Connections {
        target: window.privateBookmarkTransferUi
        function onBookmark_transfer_stateChanged() { window.checkPrivateBookmarkTransfer() }
    }
    Connections {
        target: window.privateBookmarkTransferWindow
        function onBookmarkTransferStarted(ui, queued) {
            if (!window.privateBookmarkTransferPending) return
            if (!queued) { window.finishPrivateBookmarkTransfer(false, "The bookmark couldn't be saved. Check the destination profile and try again.", ui.status_text); return }
            window.privateBookmarkTransferUi = ui
            window.privateBookmarkTransferReceipt = String(ui.bookmark_transfer_id)
            window.checkPrivateBookmarkTransfer()
        }
    }
    property bool privateHistoryTransferVisible: false
    property string privateHistoryTransferId: ""
    property string privateHistoryTransferTitle: ""
    property string privateHistoryTransferUrl: ""
    property int libraryPage: 0
    property int libraryPageSize: 100
    property int libraryTotalEntries: 0
    readonly property bool linkPreviewVisible: primaryBrowserUi.link_preview_visible
    property alias hintResults: primaryHintWorkspace.hintResults
    property alias hintViewport: primaryHintWorkspace.hintViewport
    property alias hintInput: primaryHintWorkspace.hintInput
    property alias hintState: primaryHintWorkspace.hintState
    property alias targetSearchSelectedIdentity: primaryHintWorkspace.targetSearchSelectedIdentity
    property alias targetSearchSelectedMeaning: primaryHintWorkspace.targetSearchSelectedMeaning
    property alias targetSearchSelectionInitialized: primaryHintWorkspace.targetSearchSelectionInitialized
    property alias hintScopeHandoffLeaseId: primaryHintWorkspace.hintScopeHandoffLeaseId
    property alias scopedGridHandoffView: primaryHintWorkspace.scopedGridHandoffView
    property alias scopedGridHandoffLeaseId: primaryHintWorkspace.scopedGridHandoffLeaseId
    property alias hintCollectionTruncated: primaryHintWorkspace.hintCollectionTruncated
    property alias hintDirtyRevision: primaryHintWorkspace.hintDirtyRevision
    property alias hintRefreshPending: primaryHintWorkspace.hintRefreshPending
    property alias hintCollectionEpoch: primaryHintWorkspace.hintCollectionEpoch
    property alias hintTrackingView: primaryHintWorkspace.hintTrackingView
    property alias rapidHintConfirmationVisible: primaryHintWorkspace.rapidHintConfirmationVisible
    property alias activeHintScopeLeaseId: primaryHintWorkspace.activeHintScopeLeaseId
    property alias activeSpatialScopeLeaseId: primaryHintWorkspace.activeSpatialScopeLeaseId
    property alias activeSpatialScopeView: primaryHintWorkspace.activeSpatialScopeView
    property alias spatialAdapterEnabled: primaryHintWorkspace.spatialAdapterEnabled
    property alias spatialAdapterTargetItem: primaryHintWorkspace.spatialAdapterTargetItem
    property alias spatialAdapterSurfaceSerial: primaryHintWorkspace.spatialAdapterSurfaceSerial
    property alias spatialAdapterSurfaceRevision: primaryHintWorkspace.spatialAdapterSurfaceRevision
    readonly property alias searchWorkspace: primaryPageSearch
    FerricPageSearch {
        id: primaryPageSearch
        appearance: window
        browserUi: window.browserUi
        hostWindow: window
        activeView: window.activeWebView()
    }
    readonly property alias hintWorkspace: primaryHintWorkspace
    readonly property alias hintFrameProvider: primaryHintWorkspace.hintFrameProvider
    FerricHintWorkspace {
        id: primaryHintWorkspace
        hostWindow: window
        rootWindow: window
        browserUi: window.primaryBrowserUi
        activeView: window.activeWebView()
        appearance: window
        anchors.fill: window.webViewsObject
    }
    property string copiedText: ""
    property string copiedValue: ""
    property bool copyNoticeSensitive: false
    property bool copyNoticeVisible: false
    property bool copyNoticePrimary: false
    property var copyNoticeUi: null
    property var copyNoticeHost: null
    property var copyNoticeView: null
    property string copyNoticeWindowId: ""
    property var copyNoticeSurfaceObject: copyNoticeSurface
    readonly property bool copyNoticeInputActive: !!copyNoticeSurfaceObject && copyNoticeSurfaceObject.inputActive
    readonly property var copyNoticeAppearance: copyNoticeHost && copyNoticeHost.chromeAppearance
                                               ? copyNoticeHost.chromeAppearance : window
    readonly property bool copyNoticeOwnerCurrent: {
        if (!copyNoticeUi || !copyNoticeHost || runtimeDisposing
                || copyNoticeHost.runtimeReleased === true || copyNoticeHost.runtimeDisposing === true
                || String(copyNoticeUi.core_window_id) !== copyNoticeWindowId
                || copyNoticeHost.activeWebView() !== copyNoticeView) return false
        var owner = window.browserWindowEntryForUi(copyNoticeUi)
        return !!owner && owner.host === copyNoticeHost
    }
    onCopyNoticeOwnerCurrentChanged: {
        if (copyNoticeVisible && !copyNoticeOwnerCurrent) Qt.callLater(function() {
            // Clear outside the binding evaluation that detected invalidation.
            if (window.copyNoticeVisible && !window.copyNoticeOwnerCurrent) window.copyNoticeVisible = false
        })
    }
    onCopyNoticeVisibleChanged: {
        if (!copyNoticeVisible) {
            copiedValue = ""; copiedText = ""; copyNoticeSensitive = false
            copyNoticeUi = null; copyNoticeHost = null; copyNoticeView = null
            copyNoticeWindowId = ""; copyNoticePrimary = false
        }
    }
    readonly property bool browserChromeInputActive:
        window.profileNoticeInputActive
        || !!window.modalInputScope.activeSurface
        || (window.copyNoticeInputActive && window.copyNoticeHost === window)
        || window.inputBarActive
        || (window.activeWebView() && window.activeWebView().nativeUrl === "rb://start" && window.activeWebView().startPage && window.activeWebView().startPage.inputActive)
        || window.applicationShutdownForcePromptVisible
        || primaryDevTools.inputActive
        || window.journeyExportPreviewVisible
        || (window.rendererFailureVisible && (window.rendererFailureHost || window) === window)
        || window.recoveryAvailable
        || window.rapidHintConfirmationVisible
        || browserUi.target_search_active
        || browserUi.navigation_failure_visible
        || (window.contextRouteVisible && (window.contextRouteHost || window) === window)
        || window.contextMoveVisible
        || window.sessionManagerVisible
        || window.sessionPreviewVisible
        || window.reopenWindowConfirmationVisible
        || window.profileManagerVisible
        || window.profileDeletePreviewVisible
        || window.downloadManagerVisible
        || window.shutdownConfirmationVisible
        || window.shutdownPromptVisible
        || window.shutdownPagePromptVisible
        || (window.switcherVisible && window.switcherHost === window)
        || window.siteLedgerVisible
        || (window.diagnosticsVisible && (window.diagnosticsHost || window) === window)
        || window.bindingHelpVisible
        || window.settingsVisible
        || window.libraryManagerVisible
        || window.privateHistoryTransferVisible
        || window.linkPreviewVisible
        || window.siteDataClearPending
        || browserUi.external_navigation_visible
        || window.presentedPromptActiveFor(browserUi)
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

    function refreshBlockingEvidence() { return runtimeBase.refreshBlockingEvidence() }

    NotificationPresenter {
        id: notificationPresenter
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

    FerricWebNotificationController {
        id: webNotificationController
        rootWindow: window
        notificationPresenter: window.notificationPresenterObject
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
        // Completion belongs to every durable window, including destinations
        // opened from a private primary window.
        running: !window.temporaryProfile || window.liveWindowEntries().some(function(entry) {
            return entry && entry.ui && !entry.privateProfile && !entry.ephemeralProfile
        })
        onTriggered: {
            var entries = window.liveWindowEntries() || []
            if (entries.length === 0) {
                entries = [{ ui: browserUi }]
            }
            var consumed = false
            for (var i = 0; i < entries.length; ++i) {
                var entry = entries[i]
                if (entry && entry.ui && !entry.privateProfile && !entry.ephemeralProfile
                        && entry.ui.poll_storage_library()) {
                    consumed = true
                }
            }
            if (consumed && window.switcherVisible) {
                window.refreshSwitcher()
            }
            if (consumed && window.downloadManagerVisible) {
                window.refreshDownloads()
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
            window.runBrowserScript(view,
                        BrowserScripts.siteDataClearResult(),
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
                        }, window.browserScriptWorld)
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
            if (window.pendingInteractiveCommand.length > 0
                    && commandSurface.commandVisible
                    && commandSurface.commandText === window.pendingInteractiveCommand) {
                var command = window.pendingInteractiveCommand
                window.pendingInteractiveCommand = ""
                Qt.callLater(function() {
                    if (commandSurface.commandVisible
                            && commandSurface.commandText === command) {
                        window.submitInteractiveCommand(command)
                    }
                })
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

        FerricDevToolsWindow { browserWindow: window }
    }

    BrowserUi {
        id: browserUi
        status_text: "Ready"
        onRuntime_work_available: window.scheduleRuntimeWork(0)
        onStatus_textChanged: {
            window.scheduleRuntimeWork(0)
            if (window.commandNoticeVisible && browserUi.mode !== "command"
                    && browserUi.status_text !== "Normal mode") {
                var loweredStatus = browserUi.status_text.toLowerCase()
                window.showCommandNotice(
                    browserUi.status_text,
                    loweredStatus.indexOf("failed") >= 0
                    || loweredStatus.indexOf("error") >= 0
                    || loweredStatus.indexOf("rejected") >= 0)
            }
        }
    }

    BrowserKeyRouter {
        id: browserKeyRouter
        transientInsert: browserUi.mode === "insert"
        passNextKey: browserUi.mode === "pass-through-once"
        passThrough: browserUi.mode === "pass-through"
        inputEpoch: browserUi.input_epoch
        onPassNextKeyDispatched: function(epoch) {
            if (browserUi.mode === "pass-through-once" && browserUi.input_epoch === epoch) {
                window.cancelFocusProbes()
                browserUi.escape()
            }
        }
        onTransientEnterDispatched: function(epoch) {
            if (browserUi.mode === "insert" && browserUi.input_epoch === epoch) {
                // Cancel pre-entry retries as well as invalidating in-flight queries.
                window.cancelFocusProbes()
                browserUi.escape()
            }
        }
        targetWindow: window
        enabled: window.active && window.browserKeyFocusActive
                 && (browserUi.mode === "normal"
                     || browserUi.mode === "hint"
                     || browserUi.mode === "grid"
                     || browserUi.mode === "caret"
                     || browserUi.mode === "insert"
                     || browserUi.mode === "insert-locked"
                     || browserUi.mode === "pass-through"
                     || browserUi.mode === "pass-through-once")
        onClipboardShortcutOverride: function(key, modifiers) {
            var logical = window.logicalNormalKeyText({key: key, modifiers: modifiers, text: ""})
            if (!window.nativePromptActiveFor(browserUi) && browserUi.clipboard_shortcut_browser_owned(logical)) browserKeyRouter.acceptCurrentEvent()
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
            if (window.handleBrowserKey(browserUi, window, event)) {
                browserKeyRouter.acceptCurrentEvent()
            }
        }
        onKeyReleased: function(key, modifiers) {
            if (window.handleBrowserKeyRelease(browserUi, window, key, modifiers)) {
                browserKeyRouter.acceptCurrentEvent()
            }
        }
    }

    onActiveChanged: {
        browserUi.set_ipc_window_active(active)
        if (!active && window.copyNoticeHost === window) window.copyNoticeVisible = false
        if (!active && browserUi.hint_pointer_mode) {
            browserUi.hint_pointer_invalidated("window-inactive")
        }
        if (!active && browserUi.mode === "pass-through-once") {
            window.cancelFocusProbes()
            browserUi.escape()
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

    function runBrowserScript(view, script, callback, world) { return runtimeServices.runBrowserScript(view, script, callback, world) }

    function runBrowserFrameScript(view, frame, script, world, callback) { return runtimeServices.runBrowserFrameScript(view, frame, script, world, callback) }

    function cancelScriptsForView(view) { return runtimeServices.cancelScriptsForView(view) }

    function closeDevTools() { return runtimeServices.closeDevTools() }

    function toggleDevTools(detach) { return runtimeServices.toggleDevTools(detach) }

    function scrollScript(kind, direction, half, count) { return runtimeServices.scrollScript(kind, direction, half, count) }

    function scrollTargetPolicyScript(policy) { return runtimeServices.scrollTargetPolicyScript(policy) }

    function scrollTargetStatusScript() { return runtimeServices.scrollTargetStatusScript() }

    function applyScrollTargetAction(ui, view, action, feedbackHost) { return runtimeServices.applyScrollTargetAction(ui, view, action, feedbackHost) }

    function searchFindFlags(query, caseMode, backward) { return runtimeServices.searchFindFlags(query, caseMode, backward) }

    function captureScrollPosition(ui, index, view) { return runtimeServices.captureScrollPosition(ui, index, view) }

    function restoreScrollPosition(view, x, y) { return runtimeServices.restoreScrollPosition(view, x, y) }

    function siteDataClearScript() { return runtimeServices.siteDataClearScript() }

    function clearSiteDataClearForView(view) { return runtimeServices.clearSiteDataClearForView(view) }

    function siteRuleSettingsFor(ui, url) { return runtimeServices.siteRuleSettingsFor(ui, url) }

    function siteRuleValue(settings, key, fallback) { return runtimeServices.siteRuleValue(settings, key, fallback) }

    Component { id: selectionSourceFactory; FerricSelectionSource {} }

    function selectionProviderFor(view) { return runtimeServices.selectionProviderFor(view) }
    function readSelection(ui, view, token, captured) { return runtimeServices.readSelection(ui, view, token, captured) }

    function focusObserverSource() { return runtimeServices.focusObserverSource() }

    function installFocusObserver(view) { return runtimeServices.installFocusObserver(view) }

    function focusProbeScript() { return runtimeServices.focusProbeScript() }

    function probeFocusFrame(ui, index, frame, framePath, url, view) { return runtimeServices.probeFocusFrame(ui, index, frame, framePath, url, view) }

    function probeNativePromptFocus(ui, view, leaseId) { return runtimeServices.probeNativePromptFocus(ui, view, leaseId) }

    function probeFocusNotification(ui, host, view, index, message) { return runtimeServices.probeFocusNotification(ui, host, view, index, message) }

    function probeActiveFocus(ui, index, view) { return runtimeServices.probeActiveFocus(ui, index, view) }

    function cosmeticHostMatches(host, pattern) { return runtimeServices.cosmeticHostMatches(host, pattern) }

    function injectCosmeticRules(ui, view) { return runtimeServices.injectCosmeticRules(ui, view) }

    function matchingPageUserscriptsFor(ui, url, privateProfile) { return runtimeServices.matchingPageUserscriptsFor(ui, url, privateProfile) }

    function injectPageUserscripts(ui, view, url, privateProfile, phase) { return runtimeServices.injectPageUserscripts(ui, view, url, privateProfile, phase) }

    function clearInstalledPageUserscripts(view) { return runtimeServices.clearInstalledPageUserscripts(view) }

    function installPageUserscripts(ui, view, url, privateProfile) { return runtimeServices.installPageUserscripts(ui, view, url, privateProfile) }

    FerricTextField {
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
        onTriggered: {
            var surface = window.copyNoticeSurfaceObject
            if (surface && (surface.inputActive || surface.hovered)) restart()
            else window.copyNoticeVisible = false
        }
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
        engineHostWindow: window
        diagnosticsHostWindow: window.diagnosticsHost || window
        portalRequired: window.desktopPortalMode(browserUi) === "required"
        enginePortalRequired: window.desktopPortalMode(window.pendingFileDialogUi || browserUi) === "required"
        diagnosticsPortalRequired: window.desktopPortalMode(window.diagnosticsUi || browserUi) === "required"
        onDownloadAccepted: window.acceptPendingDownload()
        onDownloadRejected: window.cancelPendingDownload()
        onDownloadFailed: function(message) { window.failPendingDownload(message) }
        onJourneyExportAccepted: window.finishJourneyExport()
        onJourneyExportRejected: window.journeyExportPreviewVisible = true
        onJourneyExportFailed: function(message) {
            window.journeyExportPreviewVisible = true
            browserUi.status_text = message
        }
        onDiagnosticsExportAccepted: window.finishDiagnosticsExport()
        onDiagnosticsExportRejected: if (window.diagnosticsUi) window.diagnosticsVisible = true
        onDiagnosticsExportFailed: function(message) {
            if (!window.diagnosticsUi) return
            window.diagnosticsVisible = true
            window.diagnosticsExportNotice = message
            window.diagnosticsExportDetail = ""
        }
        onEngineFileAccepted: window.acceptEngineFileDialog()
        onEngineFileRejected: window.rejectEngineFileDialog()
        onEngineFileFailed: function(message) { window.clearEngineFileDialog(true, message) }
        onEngineChooserOpened: function(backend) {
            if (!window.pendingFileDialogRequest) return
            var ui = window.pendingFileDialogUi || browserUi
            ui.status_text = backend === "system"
                    ? "The system file chooser is open"
                    : "The browser file chooser is open"
        }
        onEngineFolderAccepted: window.acceptEngineFolderDialog()
        onEngineFolderRejected: window.rejectEngineFileDialog()
        onEngineFolderFailed: function(message) { window.clearEngineFileDialog(true, message) }
        onUserscriptManifestAccepted: window.installUserscriptManifest()
        onUserscriptManifestFailed: function(message) { browserUi.status_text = message }
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
        onDismissed: {
            // Qt can close a Menu before its MenuItem emits triggered. Give
            // that selection a chance to consume its captured permit, while
            // never clearing a replacement request from a later menu.
            var dismissedRequest = window.pendingContextMenuRequest
            Qt.callLater(function() {
                if (window.pendingContextMenuRequest === dismissedRequest) {
                    window.clearContextMenuRequest()
                }
            })
        }
    }

    function applyNativeEditAction(ui, view, action, source) { return runtimeServices.applyNativeEditAction(ui, view, action, source) }

    function requestNativeEdit(ui, view, operation, source) { return runtimeServices.requestNativeEdit(ui, view, operation, source) }

    function copyToClipboard(value, exposeValue, primary, ownerUi) { return runtimeServices.copyToClipboard(value, exposeValue, primary, ownerUi) }

    function repeatClipboardCopy() { return runtimeServices.repeatClipboardCopy() }

    function dismissCopyNotice() { return runtimeServices.dismissCopyNotice() }

    function fileDialogNameFilters(request) { return runtimeServices.fileDialogNameFilters(request) }

    function fileDialogPaths() { return runtimeServices.fileDialogPaths() }

    function fileUrlForPath(path) { return runtimeServices.fileUrlForPath(path) }

    function folderDialogPaths() { return runtimeServices.folderDialogPaths() }

    function desktopPortalMode(ui) { return runtimeServices.desktopPortalMode(ui) }

    function desktopPortalCapabilityStatus(ui, capability) { return runtimeServices.desktopPortalCapabilityStatus(ui, capability) }

    function presentEngineFileDialog(surfaces, request, ui) { return runtimeServices.presentEngineFileDialog(surfaces, request, ui) }

    function openPendingEngineFileDialog() { return runtimeServices.openPendingEngineFileDialog() }

    function clearEngineFileDialog(cancelRequest, message) { return runtimeServices.clearEngineFileDialog(cancelRequest, message) }

    function acceptEngineFileDialog() { return runtimeServices.acceptEngineFileDialog() }

    function acceptEngineFolderDialog() { return runtimeServices.acceptEngineFolderDialog() }

    function rejectEngineFileDialog() { return runtimeServices.rejectEngineFileDialog() }

    function handleFileDialogRequested(request, view, ui) { return runtimeServices.handleFileDialogRequested(request, view, ui) }

    function clearFileDialogForView(view) { return runtimeServices.clearFileDialogForView(view) }

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

    readonly property alias tabViewControllerObject: primaryTabController
    property alias tabViews: primaryTabController.tabViews
    FerricTabViewController {
        id: primaryTabController
        ready: window.profileBootstrapComplete && !window.runtimeDisposing
        browserUi: window.browserUi
        browserHost: window
        tabs: window.tabsModel
        viewFactory: window.webViewFactory
        viewParent: window.webViewsObject
        profile: window.browserProfile
        interceptor: window.primaryRequestInterceptor
        transientProfile: window.temporaryProfile
        onActivity: window.noteTabActivity()
        onRestored: {
            if (window.profileSessionWorkspaceObject) window.profileSessionWorkspaceObject.finishSessionRestore(window.recoveryAvailable)
            window.recoveryAvailable = false
        }
    }
    property var preparedTransferFallback: null

    function tabViewAt(index) { return runtimeServices.tabViewAt(index) }

    function removeTabViewAt(index, transferParent) { return runtimeServices.removeTabViewAt(index, transferParent) }

    function detachTabViewAt(index) { return runtimeServices.detachTabViewAt(index) }

    function prepareTransferFallback() { return runtimeServices.prepareTransferFallback() }

    function discardPreparedTransferFallback() { return runtimeServices.discardPreparedTransferFallback() }

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

    function refreshSessions() { return runtimeServices.refreshSessions() }

    function applySessionValues() { return runtimeServices.applySessionValues() }

    function showSessionPreview(name, append) { return runtimeServices.showSessionPreview(name, append) }


    function loadPreviewedSession() { return runtimeServices.loadPreviewedSession() }

    function showReopenWindowConfirmation(sourceUi, sourceHost, action) { return runtimeServices.showReopenWindowConfirmation(sourceUi, sourceHost, action) }

    function confirmReopenWindow() { return runtimeServices.confirmReopenWindow() }

    function cancelReopenWindow() { return runtimeServices.cancelReopenWindow() }

    function refreshProfiles() { return runtimeServices.refreshProfiles() }

    function applyProfileValues() { return runtimeServices.applyProfileValues() }

    function showSiteLedger() { return runtimeServices.showSiteLedger() }

    function siteLedgerSnapshotIsCurrent() { return runtimeServices.siteLedgerSnapshotIsCurrent() }

    function refreshVisibleSiteLedger() { return runtimeServices.refreshVisibleSiteLedger() }

    // Reading a failed startup's own diagnostics does not require an admitted
    // page. Validate the QObject/host pair without granting page authority.
    function diagnosticsOwner(ui, host) { return runtimeServices.diagnosticsOwner(ui, host) }

    function refreshDiagnostics() { return runtimeServices.refreshDiagnostics() }

    Connections {
        target: window.diagnosticsUi
        function onDiagnostics_previewChanged() {
            window.diagnosticsText = window.diagnosticsUi ? String(window.diagnosticsUi.diagnostics_preview) : ""
        }
    }

    function refreshEngineUpdateNotice() { return runtimeServices.refreshEngineUpdateNotice() }

    function showDiagnostics(ui, host) { return runtimeServices.showDiagnostics(ui, host) }

    function closeDiagnostics() { return runtimeServices.closeDiagnostics() }

    function rebuildBindingHelpRows() { return runtimeServices.rebuildBindingHelpRows() }

    function refreshBindingHelp() { return runtimeServices.refreshBindingHelp() }

    function showBindingHelp(search) { return runtimeServices.showBindingHelp(search) }

    function closeBindingHelp() { return runtimeServices.closeBindingHelp() }

    function refreshSettings() { return runtimeServices.refreshSettings() }
    function settingLiteral(row, value) { return runtimeServices.settingLiteral(row, value) }
    function applySetting(row, value) { return runtimeServices.applySetting(row, value) }
    function resetSetting(row) { return runtimeServices.resetSetting(row) }
    function showSettings(search) { return runtimeServices.showSettings(search) }
    function closeSettings() { return runtimeServices.closeSettings() }
    function chooseUserscriptManifest() { return runtimeServices.chooseUserscriptManifest() }
    function installUserscriptManifest() { return runtimeServices.installUserscriptManifest() }
    function confirmRemoveUserscript(name) { return runtimeServices.confirmRemoveUserscript(name) }
    function refreshUserscriptInventory() { return runtimeServices.refreshUserscriptInventory() }
    function userscriptInventoryRows() { return runtimeServices.userscriptInventoryRows() }

    function siteDoctorUserscriptsDisabled(ui) { return runtimeServices.siteDoctorUserscriptsDisabled(ui) }

    function siteDoctorBadge(ui) { return runtimeServices.siteDoctorBadge(ui) }

    function finishSiteDoctorAfterLoad(ui, succeeded) { return runtimeServices.finishSiteDoctorAfterLoad(ui, succeeded) }

    function openProfile(name, label, requestedUrl) { return runtimeServices.openProfile(name, label, requestedUrl) }
    function openProfileWindow(name, label, requestedUrl, sourceUrl) { return runtimeServices.openProfileWindow(name, label, requestedUrl, sourceUrl) }

    function showProfileDeletePreview(name) { return runtimeServices.showProfileDeletePreview(name) }



    readonly property var clipboardBridge: window.clipboardBridgeObject
    readonly property var activationOutcomeTimer: window.activationOutcomeTimerObject
    readonly property var fileDialogSurfaces: window.fileDialogSurfacesObject
    readonly property var certificatePrompts: window.certificatePromptsObject
    readonly property var webAuthPrompt: window.webAuthPromptObject
    readonly property var contextMenu: window.contextMenuObject
    readonly property var tabs: window.tabsModel
    readonly property var namedSessions: window.namedSessionsModel
    readonly property var profiles: window.profilesModel
    readonly property var downloads: window.downloadsModel
    readonly property var libraryEntries: window.libraryEntriesModel
    readonly property var libraryGraphEntries: window.libraryGraphEntriesModel
    readonly property var libraryGraphNodes: window.libraryGraphNodesModel

    readonly property alias switcherRefreshTimerObject: switcherRefreshTimer
    readonly property alias switcherBatchTimerObject: switcherBatchTimer

    function refreshDownloads() { return runtimePresentation.refreshDownloads() }

    function externalUriAllowed(uri) { return runtimePresentation.externalUriAllowed(uri) }

    function openExternalUri(ui, uri) { return runtimePresentation.openExternalUri(ui, uri) }

    function hasActiveDownloads() { return runtimePresentation.hasActiveDownloads() }

    function hasActiveShutdownRequestsFor(ui, hostWindow) { return runtimePresentation.hasActiveShutdownRequestsFor(ui, hostWindow) }

    function cancelShutdownRequestsFor(ui, hostWindow) { return runtimePresentation.cancelShutdownRequestsFor(ui, hostWindow) }

    function tabSuspensionBlockReason(index, id, view) { return runtimePresentation.tabSuspensionBlockReason(index, id, view) }

    function requestTabSuspension(id) { return runtimePresentation.requestTabSuspension(id) }

    function tabDiscardBlockReason(index, id, view) { return runtimePresentation.tabDiscardBlockReason(index, id, view) }

    function requestTabDiscard(id) { return runtimePresentation.requestTabDiscard(id) }

    function shutdownPageProbeScript() { return runtimePresentation.shutdownPageProbeScript() }

    function checkPageStateBeforeQuit() { return runtimePresentation.checkPageStateBeforeQuit() }

    function beginQuitRequest(commandText) { return runtimePresentation.beginQuitRequest(commandText) }

    function continueQuitRequest() { return runtimePresentation.continueQuitRequest() }

    function finalizeQuit() { return runtimePresentation.finalizeQuit() }

    function cancelDownloadsAndQuit() { return runtimePresentation.cancelDownloadsAndQuit() }

    Timer {
        id: libraryPublicationTimer
        interval: 0
        onTriggered: window.refreshLibraryManager()
    }
    Connections {
        target: browserUi
        function onLibrary_valuesChanged() { libraryPublicationTimer.restart() }
        function onLibrary_pendingChanged() {
            if (!browserUi.library_pending) Qt.callLater(function() {
                if (window.downloadManagerVisible && !browserUi.library_pending
                        && browserUi.library_error.length === 0) window.refreshDownloads()
            })
        }
        function onLibrary_kindChanged() { libraryPublicationTimer.restart() }
        function onLibrary_graph_edge_sourcesChanged() { libraryPublicationTimer.restart() }
        function onLibrary_graph_edge_targetsChanged() { libraryPublicationTimer.restart() }
        function onLibrary_graph_edge_transitionsChanged() { libraryPublicationTimer.restart() }
    }
    function updateLibraryRows(rows) { return runtimePresentation.updateLibraryRows(rows) }
    function refreshActiveLibrary() { return runtimePresentation.refreshActiveLibrary() }
    function refreshLibraryManager() { return runtimePresentation.refreshLibraryManager() }

    function libraryPageCount() { return runtimePresentation.libraryPageCount() }

    function changeLibraryPage(delta) { return runtimePresentation.changeLibraryPage(delta) }

    function runJourneyQuery(argument) { return runtimePresentation.runJourneyQuery(argument) }

    function runJourneyCurrentQuery() { return runtimePresentation.runJourneyCurrentQuery() }

    function runJourneyAllQuery() { return runtimePresentation.runJourneyAllQuery() }

    function libraryCommandArgument(value) { return runtimePresentation.libraryCommandArgument(value) }

    function openLibraryEntry(entryKind, entryId) { return runtimePresentation.openLibraryEntry(entryKind, entryId) }

    function showPrivateHistoryTransfer(entryId, title, url) { return runtimePresentation.showPrivateHistoryTransfer(entryId, title, url) }

    function cancelPrivateHistoryTransfer() { return runtimePresentation.cancelPrivateHistoryTransfer() }

    function confirmPrivateHistoryTransfer(profileName) { return runtimePresentation.confirmPrivateHistoryTransfer(profileName) }

    function checkPrivateBookmarkTransfer() { return runtimePresentation.checkPrivateBookmarkTransfer() }

    function finishPrivateBookmarkTransfer(succeeded, message, detail) { return runtimePresentation.finishPrivateBookmarkTransfer(succeeded, message, detail) }

    function confirmPrivateHistoryBookmark(profileName, profileLabel) { return runtimePresentation.confirmPrivateHistoryBookmark(profileName, profileLabel) }

    function deleteLibraryEntry(entryKind, entryId) { return runtimePresentation.deleteLibraryEntry(entryKind, entryId) }

    function editLibraryEntry(entryKind, entryId, value) { return runtimePresentation.editLibraryEntry(entryKind, entryId, value) }

    function beginJourneyExport() { return runtimePresentation.beginJourneyExport() }

    function finishJourneyExport() { return runtimePresentation.finishJourneyExport() }

    function finishDiagnosticsExport() { return runtimePresentation.finishDiagnosticsExport() }

    function journeySearchArgument(value) { return runtimePresentation.journeySearchArgument(value) }

    function journeyExpandArgument(nodeId) { return runtimePresentation.journeyExpandArgument(nodeId) }

    function cancelSwitcherBatch() { return runtimePresentation.cancelSwitcherBatch() }

    // A query edit invalidates the old rows before the debounce timer runs.
    // Guard activation at the owner boundary as well as in the presentation.
    function switcherResultsCurrent() { return runtimePresentation.switcherResultsCurrent() }

    function switcherMaxResults() { return runtimePresentation.switcherMaxResults() }

    function processSwitcherBatch() { return runtimePresentation.processSwitcherBatch() }

    function publishedSwitcherBatch() { return runtimePresentation.publishedSwitcherBatch() }

    function retrySwitcherSources() { return runtimePresentation.retrySwitcherSources() }

    function refreshSwitcher() { return runtimePresentation.refreshSwitcher() }

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

    function showSwitcherWith(scope, query, ownerUi, ownerHost) { return runtimePresentation.showSwitcherWith(scope, query, ownerUi, ownerHost) }

    function showSwitcher() { return runtimePresentation.showSwitcher() }

    function closeSwitcher() { return runtimePresentation.closeSwitcher() }

    function setSwitcherScope(scope) { return runtimePresentation.setSwitcherScope(scope) }

    function hintSelectorPolicySnapshot(family) { return runtimePresentation.hintSelectorPolicySnapshot(family) }

    function hintCollectorScript(family, selectorSnapshot, scopeLeaseId, includeFacets) { return runtimePresentation.hintCollectorScript(family, selectorSnapshot, scopeLeaseId, includeFacets) }

    function targetSearchIdentity(candidate) { return runtimePresentation.targetSearchIdentity(candidate) }

    function targetSearchMeaning(candidate) { return runtimePresentation.targetSearchMeaning(candidate) }

    function targetSearchRows() { return runtimePresentation.targetSearchRows() }

    function reconcileTargetSearchSelection() { return runtimePresentation.reconcileTargetSearchSelection() }

    function applyTargetSearchQuery(query) { return runtimePresentation.applyTargetSearchQuery(query) }

    function selectTargetSearchIdentity(identity) { return runtimePresentation.selectTargetSearchIdentity(identity) }

    function moveTargetSearchSelection(delta) { return runtimePresentation.moveTargetSearchSelection(delta) }

    function confirmTargetSearchSelection() { return runtimePresentation.confirmTargetSearchSelection() }

    function hintDirtyRevisionScript() { return runtimePresentation.hintDirtyRevisionScript() }

    function hintStopTrackingScript(scopeLeaseId) { return runtimePresentation.hintStopTrackingScript(scopeLeaseId) }

    function spatialScopeGeometryScript(scopeLeaseId) { return runtimePresentation.spatialScopeGeometryScript(scopeLeaseId) }

    function selectionScript() { return runtimePresentation.selectionScript() }

    function downloadLinkScript(url) { return runtimePresentation.downloadLinkScript(url) }

    function editorScript() { return runtimePresentation.editorScript() }

    function editorApplyScript(original, updated) { return runtimePresentation.editorApplyScript(original, updated) }

    function caretScript(operation, selecting) { return runtimePresentation.caretScript(operation, selecting) }

    function hintFreshScript(candidate) { return runtimePresentation.hintFreshScript(candidate) }

    function completeHintScopeHandoff(view, request, candidate, actionEpoch) { return runtimePresentation.completeHintScopeHandoff(view, request, candidate, actionEpoch) }

    function completeGridScopeHandoff(view, request, candidate, actionEpoch) { return runtimePresentation.completeGridScopeHandoff(view, request, candidate, actionEpoch) }

    function hintPointerPreflightScript(candidate, viewWidth, viewHeight) { return runtimePresentation.hintPointerPreflightScript(candidate, viewWidth, viewHeight) }

    function hintFocusScript(candidate) { return runtimePresentation.hintFocusScript(candidate) }

    function hintClickScript(candidate) { return runtimePresentation.hintClickScript(candidate) }

    function hintSetScrollTargetScript(elementId, framePath) { return runtimePresentation.hintSetScrollTargetScript(elementId, framePath) }

    function startHintCollection() { return runtimePresentation.startHintCollection() }

    function pollHintRefresh() { return runtimePresentation.pollHintRefresh() }

    function applyHintInteraction(action, text) { return runtimePresentation.applyHintInteraction(action, text) }

    function beginHintPointerConfirmation() { return runtimePresentation.beginHintPointerConfirmation() }

    function rotateHintCollision() { return runtimePresentation.rotateHintCollision() }

    function closeHints() { return runtimePresentation.closeHints() }

    function stopHintTracking() { return runtimePresentation.stopHintTracking() }

    function openEphemeralWindow(url, requestedToken) { return runtimePresentation.openEphemeralWindow(url, requestedToken) }

    function openReopenedWindow(ui, hostWindow, action) { return runtimePresentation.openReopenedWindow(ui, hostWindow, action) }

    function transferPayloadForUi(sourceUi, sourceTabId) { return runtimePresentation.transferPayloadForUi(sourceUi, sourceTabId) }

    function detachSourceView(sourceUi, sourceHost, sourceTabId) { return runtimePresentation.detachSourceView(sourceUi, sourceHost, sourceTabId) }

    function restoreSourceView(sourceUi, sourceHost, view) { return runtimePresentation.restoreSourceView(sourceUi, sourceHost, view) }

    function sameProfileWindow(sourceEntry, targetEntry) { return runtimePresentation.sameProfileWindow(sourceEntry, targetEntry) }

    function attachTransferredView(view, payload, sourceUi, sourceHost) { return runtimePresentation.attachTransferredView(view, payload, sourceUi, sourceHost) }

    function executeBrowserTransferAction(sourceUi, sourceHost, action) { return runtimePresentation.executeBrowserTransferAction(sourceUi, sourceHost, action) }

    function openDetachedWindow(sourceUi, hostWindow, action, view) { return runtimePresentation.openDetachedWindow(sourceUi, hostWindow, action, view) }

    function completeDetachedSource(sourceUi, sourceHost, payload) { return runtimePresentation.completeDetachedSource(sourceUi, sourceHost, payload) }

    function showHintActions(label) { return runtimePresentation.showHintActions(label) }

    function openHintActionPalette(label, capabilities, workspace) { return runtimePresentation.openHintActionPalette(label, capabilities, workspace) }

    function activateHint(label, actionId, localSelection) { return runtimePresentation.activateHint(label, actionId, localSelection) }



    readonly property var switcherRefreshTimer: window.switcherRefreshTimerObject
    readonly property var switcherBatchTimer: window.switcherBatchTimerObject
    readonly property var requestPresentationController: window.requestPresentationControllerObject

    Connections {
        target: browserUi
        function onActive_tab_indexChanged() {
            if (window.copyNoticeHost === window) window.copyNoticeVisible = false
            window.noteTabActivity()
        }
        function onTab_countChanged() {
            window.noteTabActivity()
        }
        function onContext_route_idChanged() {
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
                        commandSurface.focusInput()
                    } else if (browserUi.mode === "search") {
                        searchSurface.focusInput()
                    }
                })
            } else {
                window.restoreModeFocus()
            }
            if (browserUi.mode !== "hint" && browserUi.mode !== "caret") window.caretSelecting = false
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
        function onTheme_background_colorChanged() {
            window.refreshChromeAppearance()
        }
        function onSystem_font_scaleChanged() {
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

    function activateSwitcher(index) { return runtimeRequests.activateSwitcher(index) }

    function activateSwitcherAction(index, action) { return runtimeRequests.activateSwitcherAction(index, action) }

    Shortcut {
        sequence: "Ctrl+P"
        onActivated: window.showSwitcher()
    }

    function downloadStateName(download) { return runtimeRequests.downloadStateName(download) }

    function navigationFailureKind(loadRequest) { return runtimeRequests.navigationFailureKind(loadRequest) }

    function navigationFailureDetail(loadRequest) { return runtimeRequests.navigationFailureDetail(loadRequest) }

    function downloadsAskDestination(ui) { return runtimeRequests.downloadsAskDestination(ui) }

    function permissionTypeName(permissionType) { return runtimeRequests.permissionTypeName(permissionType) }

    function permissionDisplayName(permissionName) { return runtimeRequests.permissionDisplayName(permissionName) }

    function permissionCanRemember(origin, permissionName) { return runtimeRequests.permissionCanRemember(origin, permissionName) }

    function permissionCanRememberForSite(origin, permissionName) { return runtimeRequests.permissionCanRememberForSite(origin, permissionName) }

    function boundedPageDialogText(value) { return runtimeRequests.boundedPageDialogText(value) }

    function pageDialogType(request) { return runtimeRequests.pageDialogType(request) }

    function pageDialogKind() { return runtimeRequests.pageDialogKind() }

    function clearPageDialog(rejectRequest) { return runtimeRequests.clearPageDialog(rejectRequest) }

    function clearPageDialogForView(view) { return runtimeRequests.clearPageDialogForView(view) }

    function resetPageDialogBudget(view) { return runtimeRequests.resetPageDialogBudget(view) }

    function pageDialogDocumentKey(view) { return runtimeRequests.pageDialogDocumentKey(view) }

    function handleJavaScriptDialogRequested(ui, view, request, privateProfile, hostWindow) { return runtimeRequests.handleJavaScriptDialogRequested(ui, view, request, privateProfile, hostWindow) }

    function handleAuthenticationDialogRequested(ui, view, request, privateProfile, hostWindow) { return runtimeRequests.handleAuthenticationDialogRequested(ui, view, request, privateProfile, hostWindow) }

    function clearClientCertificateRequest(selectNone) { return runtimeRequests.clearClientCertificateRequest(selectNone) }

    function clearClientCertificateForView(view) { return runtimeRequests.clearClientCertificateForView(view) }

    function handleClientCertificateRequested(ui, view, selection, privateProfile, hostWindow) { return runtimeRequests.handleClientCertificateRequested(ui, view, selection, privateProfile, hostWindow) }

    function acceptClientCertificate(index) { return runtimeRequests.acceptClientCertificate(index) }

    function rejectClientCertificate() { return runtimeRequests.rejectClientCertificate() }

    function googleCertificateHost(host) { return runtimeRequests.googleCertificateHost(host) }

    function clearCertificateErrorRequest(rejectRequest) { return runtimeRequests.clearCertificateErrorRequest(rejectRequest) }

    function clearCertificateErrorForView(view) { return runtimeRequests.clearCertificateErrorForView(view) }

    function handleCertificateError(ui, view, error, hostWindow) { return runtimeRequests.handleCertificateError(ui, view, error, hostWindow) }

    function acceptCertificateError() { return runtimeRequests.acceptCertificateError() }

    function rejectCertificateError() { return runtimeRequests.rejectCertificateError() }

    function webAuthStateLabel(state) { return runtimeRequests.webAuthStateLabel(state) }

    function updateWebAuthState(request, state) { return runtimeRequests.updateWebAuthState(request, state) }

    function clearWebAuthRequest(cancelRequest) { return runtimeRequests.clearWebAuthRequest(cancelRequest) }

    function clearWebAuthForView(view) { return runtimeRequests.clearWebAuthForView(view) }

    function handleWebAuthRequested(ui, view, request, hostWindow) { return runtimeRequests.handleWebAuthRequested(ui, view, request, hostWindow) }

    function selectWebAuthAccount(account) { return runtimeRequests.selectWebAuthAccount(account) }

    function submitWebAuthPin() { return runtimeRequests.submitWebAuthPin() }

    function retryWebAuth() { return runtimeRequests.retryWebAuth() }

    function cancelWebAuth() { return runtimeRequests.cancelWebAuth() }

    function safeContextUrl(value) { return runtimeRequests.safeContextUrl(value) }

    function clearContextMenuRequest(preserveHintPalettePermit) { return runtimeRequests.clearContextMenuRequest(preserveHintPalettePermit) }

    function clearContextMenuForView(view) { return runtimeRequests.clearContextMenuForView(view) }

    function contextMenuItem(label, action, value, actionId, enabled) { return runtimeRequests.contextMenuItem(label, action, value, actionId, enabled) }

    function appendExternalActions(items, ui, subject, value) { return runtimeRequests.appendExternalActions(items, ui, subject, value) }

    function appendUserscriptActions(items, ui, subject, value) { return runtimeRequests.appendUserscriptActions(items, ui, subject, value) }

    function handleContextMenuRequested(ui, view, request, hostWindow) { return runtimeRequests.handleContextMenuRequested(ui, view, request, hostWindow) }

    function activateContextMenuItem(item) { return runtimeRequests.activateContextMenuItem(item) }

    function acceptPageDialog() { return runtimeRequests.acceptPageDialog() }

    function rejectPageDialog() { return runtimeRequests.rejectPageDialog() }

    function permissionQueueKey(origin, permissionName, privateProfile, ui, view, tabIndex) { return runtimeRequests.permissionQueueKey(origin, permissionName, privateProfile, ui, view, tabIndex) }

    function permissionPolicyOwnerValid(group) { return runtimeRequests.permissionPolicyOwnerValid(group) }

    function refreshPendingPermissionPolicies() { return runtimeRequests.refreshPendingPermissionPolicies() }

    function denyPermissionGroup(group) { return runtimeRequests.denyPermissionGroup(group) }

    function permissionQueueRequestCount(queue) { return runtimeRequests.permissionQueueRequestCount(queue) }

    function clearDisplayedPermissionPrompt() { return runtimeRequests.clearDisplayedPermissionPrompt() }

    function showNextPermissionPrompt() { return runtimeRequests.showNextPermissionPrompt() }

    function clearPermissionPrompt(denyRequest) { return runtimeRequests.clearPermissionPrompt(denyRequest) }

    function removePermissionGroups(predicate, denyRequest) { return runtimeRequests.removePermissionGroups(predicate, denyRequest) }

    function queuePermissionPrompt(ui, request, tabIndex, privateProfile, hostWindow, view, policyPending) { return runtimeRequests.queuePermissionPrompt(ui, request, tabIndex, privateProfile, hostWindow, view, policyPending) }

    function handlePermissionRequested(request, tabIndex, ui, view, privateProfile, hostWindow) { return runtimeRequests.handlePermissionRequested(request, tabIndex, ui, view, privateProfile, hostWindow) }

    function decidePermission(allow, lifetime) { return runtimeRequests.decidePermission(allow, lifetime) }

    function handleImmediatePermissionRequested(ui, request, privateProfile, hostWindow, view) { return runtimeRequests.handleImmediatePermissionRequested(ui, request, privateProfile, hostWindow, view) }

    function cancelPermissionForTab(tabIndex) { return runtimeRequests.cancelPermissionForTab(tabIndex) }

    function cancelPermissionForUi(ui) { return runtimeRequests.cancelPermissionForUi(ui) }

    function permissionPendingForTab(tabIndex) { return runtimeRequests.permissionPendingForTab(tabIndex) }

    function clearDesktopMediaRequest(cancelRequest) { return runtimeRequests.clearDesktopMediaRequest(cancelRequest) }

    function maybeOpenPendingDesktopMediaRequest() { return runtimeRequests.maybeOpenPendingDesktopMediaRequest() }

    function handleDesktopMediaRequested(ui, view, request, hostWindow) { return runtimeRequests.handleDesktopMediaRequested(ui, view, request, hostWindow) }

    function selectDesktopScreen(index) { return runtimeRequests.selectDesktopScreen(index) }

    function selectDesktopWindow(index) { return runtimeRequests.selectDesktopWindow(index) }

    function clearDesktopMediaForView(view) { return runtimeRequests.clearDesktopMediaForView(view) }

    function recordCaptureSession(ui, view, hostWindow, origin) { return runtimeRequests.recordCaptureSession(ui, view, hostWindow, origin) }

    function captureSessionForView(view) { return runtimeRequests.captureSessionForView(view) }

    function noteCaptureNavigation(view) { return runtimeRequests.noteCaptureNavigation(view) }

    function stopCaptureSession(id) { return runtimeRequests.stopCaptureSession(id) }

    function dismissCaptureSession(id) { return runtimeRequests.dismissCaptureSession(id) }

    function clearCaptureSessionForView(view) { return runtimeRequests.clearCaptureSessionForView(view) }

    function clearCaptureSessionsForHost(hostWindow) { return runtimeRequests.clearCaptureSessionsForHost(hostWindow) }

    function permissionOriginForView(view) { return runtimeRequests.permissionOriginForView(view) }

    function applyPermissionReset(ui, origin, permission) { return runtimeRequests.applyPermissionReset(ui, origin, permission) }

    function reloadViewsForPermission(origin, sourceUi) { return runtimeRequests.reloadViewsForPermission(origin, sourceUi) }

    function rendererTerminationName(status) { return runtimeRequests.rendererTerminationName(status) }

    function handleRendererProcessTerminated(ui, view, hostWindow, tabIndex, terminationStatus, exitCode) { return runtimeRequests.handleRendererProcessTerminated(ui, view, hostWindow, tabIndex, terminationStatus, exitCode) }

    function showRendererFailureForTab(tabIndex) { return runtimeRequests.showRendererFailureForTab(tabIndex) }

    function clearRendererFailureForView(view) { return runtimeRequests.clearRendererFailureForView(view) }

    function reloadRendererFailure() { return runtimeRequests.reloadRendererFailure() }

    function closeRendererFailure() { return runtimeRequests.closeRendererFailure() }

    function showRendererFailureDiagnostics() { return runtimeRequests.showRendererFailureDiagnostics() }

    function restartSoftwareRendering() { return runtimeRequests.restartSoftwareRendering() }

    Timer {
        id: permissionPolicyRefresh
        interval: 50
        repeat: true
        onTriggered: {
            window.refreshPendingPermissionPolicies()
            if (!(window.permissionPromptQueue || []).some(function(group) { return group.policyPending })) stop()
        }
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
        objectName: "pageDialogPopup"
        browserWindow: window
    }

    function dispatchDownloadRequest(ui) { return runtimeRequests.dispatchDownloadRequest(ui) }

    function attachDownloadStateUpdates(download, id, ui, host) { return runtimeRequests.attachDownloadStateUpdates(download, id, ui, host) }

    function downloadStagingDirectory(ui, id) { return runtimeRequests.downloadStagingDirectory(ui, id) }

    function acceptDownloadRequest(download, id, directory, suggestedName) { return runtimeRequests.acceptDownloadRequest(download, id, directory, suggestedName) }

    function acceptPendingDownload() { return runtimeRequests.acceptPendingDownload() }

    function failPendingDownload(message) { return runtimeRequests.failPendingDownload(message) }

    function cancelPendingDownload() { return runtimeRequests.cancelPendingDownload() }

    function handleDownloadRequested(download, id, safeName) { return runtimeRequests.handleDownloadRequested(download, id, safeName) }

    function handleDownloadFinished(download, selectionFailed, id) { return runtimeRequests.handleDownloadFinished(download, selectionFailed, id) }

    function completeFinishedDownload(id) { return runtimeRequests.completeFinishedDownload(id) }




    readonly property alias commandSurfaceObject: commandSurface
    readonly property alias searchSurfaceObject: searchSurface
    readonly property alias attachedDevToolsLoaderObject: attachedDevToolsLoader

    Component {
        id: popupWindowComponent

        FerricPopupWindow {
            rootWindow: window
        }
    }
    Component {
        id: browserWindowComponent

        FerricBrowserWindow {
            rootWindow: window
        }
    }

    FerricWebEngineSurfaceRecovery {
        hostWindow: window
        enabled: window.nativeWayland && !window.softwareRendering
        views: [
            window.activeWebView(),
            attachedDevToolsLoader.active ? attachedDevToolsLoader.item : null
        ]
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

    FerricTabStrip {
        id: compactTabStrip
        browserWindow: window
        tooltipsEnabled: window.active && !window.browserChromeInputActive
                         && browserUi.mode !== "hint" && browserUi.mode !== "grid"
        tabsModel: tabs
        activeTabIndex: browserUi.active_tab_index
        vertical: window.sideTabs
        parent: window.tabPosition === "top" ? headerColumn : window.contentItem
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
        Layout.row: 0
        Layout.fillWidth: parent === headerColumn
        Layout.preferredHeight: window.tabBarHeight
        Layout.maximumHeight: visible && parent === headerColumn
                              ? window.tabBarHeight : 0
        visible: window.tabStripVisible

        onSelectRequested: function(tabIndex) {
            var tabId = browserUi.tab_id_for_index(tabIndex)
            if (tabId.length > 0
                    && browserUi.execute_ui_action("browser.tab.select", tabId)) {
                tabs.setProperty(tabIndex, "loaded", true)
                window.executePendingEngineAction()
            }
        }
        onCloseRequested: function(tabIndex) {
            if (window.closeTabAtIndex(tabIndex)) {
                window.executePendingEngineAction()
            }
        }
        onNewRequested: {
            if (browserUi.execute_command(":tab-new")) window.executePendingEngineAction()
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
                if (historyView && historyView.contentCanGoBack) {
                    browserUi.back()
                    window.executePendingEngineAction()
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
                if (historyView && historyView.contentCanGoForward) {
                    browserUi.forward()
                    window.executePendingEngineAction()
                } else {
                    browserUi.status_text = "History boundary reached"
                }
            }
        }
        FerricTextField {
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
                window.executePendingEngineAction()
            }
        }
        Label {
            textFormat: Text.PlainText
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
                window.browserWindowFactory.createObject(null, {
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
                window.browserWindowFactory.createObject(null, {
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
                if (browserUi.execute_command(":tab-new")) window.executePendingEngineAction()
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
        browserUi: window.browserUi
    }

    FerricContextRouteDialog {
        id: contextRoutePopup
        browserWindow: window
    }

    FerricNavigationFailure {
        id: navigationFailureSurface
        browserWindow: window
        browserUi: window.browserUi
    }

    FerricRecoveryBanner {
        browserWindow: window
        onRecoveryRequested: {
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
        onDismissalRequested: window.recoveryAvailable = false
    }

    Connections {
        target: browserUi
        function onSite_experiment_activeChanged() {
            Qt.callLater(window.refreshVisibleSiteLedger)
        }
    }

    FerricSiteLedger {
        id: siteLedgerSurface
        browserWindow: window
        siteExperimentAvailable: !browserUi.site_experiment_active
        experimentRemainingSeconds: browserUi.site_experiment_remaining_seconds
        onCloseRequested: {
            window.siteLedgerVisible = false
            window.closeInternalSurface()
        }
        onRefreshRequested: window.showSiteLedger()
        onActiveOriginDataClearRequested: {
            if (!window.siteLedgerSnapshotIsCurrent()) {
                siteLedgerSurface.showNotice(browserUi.status_text, true)
                return
            }
            var accepted = browserUi.site_data_clear(window.siteLedgerData.origin, true)
            if (accepted) window.executePendingEngineAction()
            siteLedgerSurface.showNotice(browserUi.status_text, !accepted)
        }
        onSanitizedReportCopyRequested: function(includeHost) {
            if (!window.siteLedgerSnapshotIsCurrent()) {
                siteLedgerSurface.showNotice(browserUi.status_text, true)
                return
            }
            var report = browserUi.site_report(includeHost)
            if (report && report.length > 0) {
                window.copyToClipboard(report, true)
                browserUi.status_text = "Sanitized site report copied"
            }
            siteLedgerSurface.showNotice(browserUi.status_text, !report || report.length === 0)
        }
        onSiteDoctorProposalApplyRequested: function(proposalId) {
            if (!window.siteLedgerSnapshotIsCurrent()) {
                siteLedgerSurface.showNotice(browserUi.status_text, true)
                return
            }
            var accepted = browserUi.apply_site_doctor_proposal(proposalId, true)
            if (accepted) window.showSiteLedger()
            siteLedgerSurface.showNotice(browserUi.status_text, !accepted)
        }
        onSiteDoctorExperimentRequested: function(kind) {
            if (!window.siteLedgerSnapshotIsCurrent()) {
                siteLedgerSurface.showNotice(browserUi.status_text, true)
                return
            }
            var started = browserUi.begin_site_doctor_experiment(kind)
            if (started.length > 0) {
                window.executePendingEngineAction()
            }
            if (window.siteLedgerVisible) window.showSiteLedger()
            siteLedgerSurface.showNotice(browserUi.status_text, started.length === 0)
        }
    }

    FerricProfileNotice {
        id: profileNotice
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.bottomMargin: window.bottomControlsHeight
        browserWindow: window
        hostWindow: window
        noticeText: browserUi.profile_startup_notice
        onDetailsRequested: window.showDiagnostics(browserUi, window)
    }

    FerricDiagnostics {
        hostWindow: window.diagnosticsHost || window
        parent: hostWindow && hostWindow.contentItem ? hostWindow.contentItem : null
        startupDetails: window.diagnosticsStartupDetails
        previewText: window.diagnosticsText
        summaryRows: (window.diagnosticsUi ? window.diagnosticsUi.diagnostics_summary : [])
        reportAvailable: (window.diagnosticsUi ? window.diagnosticsUi.diagnostics_available : false)
        reportPending: (window.diagnosticsUi ? window.diagnosticsUi.diagnostics_pending : false)
        reportError: (window.diagnosticsUi ? window.diagnosticsUi.diagnostics_error : "")
        exportNotice: window.diagnosticsExportNotice
        exportDetail: window.diagnosticsExportDetail
        visible: window.diagnosticsVisible
        browserWindow: window.diagnosticsAppearance
        onCloseRequested: window.closeDiagnostics()
        onRefreshRequested: window.refreshDiagnostics()
        onCopyRequested: {
            var ui = window.diagnosticsUi
            if (!ui || !ui.diagnostics_available) return
            if (ui.write_clipboard(window.diagnosticsText, false)) {
                window.diagnosticsExportNotice = "Report preview copied"
                window.diagnosticsExportDetail = ""
            } else {
                window.diagnosticsExportNotice = "The report preview couldn't be copied."
                window.diagnosticsExportDetail = String(ui.status_text)
            }
        }
        onSaveRequested: {
            if (!(window.diagnosticsUi ? window.diagnosticsUi.diagnostics_available : false)) return
            window.diagnosticsExportUi = window.diagnosticsUi
            fileDialogSurfaces.diagnosticsExportCurrentFile = "ferric-browser-diagnostics.json"
            fileDialogSurfaces.openDiagnosticsExport()
        }
    }

    FerricBindingHelp {
        id: keyboardHelpSurface
        browserWindow: window
        onCloseRequested: window.closeBindingHelp()
        onRefreshRequested: window.refreshBindingHelp()
        onSearchChanged: function(text) {
            window.bindingHelpSearch = text
            window.refreshBindingHelp()
        }
    }

    FerricSettingsWorkspace {
        id: settingsWorkspace
        objectName: "primarySettingsWorkspace"
        browserWindow: window
        browserUi: window.browserUi
        state: window
        profileLabel: window.profileLabel
        transientProfile: window.temporaryProfile || window.ephemeralProfile
        spellcheckStatus: window.spellcheckStatusText(browserUi)
        spellcheckDictionariesMissing: window.spellcheckDictionaryStatus(browserUi).missing.length > 0
        onOpening: window.openInternalSurface()
        onClosed: window.closeInternalSurface()
        onInstallManifestRequested: window.chooseUserscriptManifest()
    }

    FerricLibraryManager {
        id: libraryManager
        browserWindow: window
        managerVisible: window.libraryManagerVisible
        listPending: ["actions", "context-routes"].indexOf(browserUi.library_kind) < 0 && browserUi.library_pending
        listError: ["actions", "context-routes"].indexOf(browserUi.library_kind) < 0 ? browserUi.library_error : ""
        feedbackId: ["bookmarks", "quickmarks"].indexOf(browserUi.library_kind) >= 0 ? browserUi.mark_action_id : ""
        feedbackState: ["bookmarks", "quickmarks"].indexOf(browserUi.library_kind) >= 0 ? browserUi.mark_action_state : "idle"
        feedbackMessage: ["bookmarks", "quickmarks"].indexOf(browserUi.library_kind) >= 0 ? browserUi.mark_action_message : ""
        feedbackDetail: ["bookmarks", "quickmarks"].indexOf(browserUi.library_kind) >= 0 ? browserUi.mark_action_detail : ""
        onRefreshRequested: {
            if (window.refreshActiveLibrary() === false)
                libraryManager.requestRejected("This view couldn't be refreshed. Try again.", browserUi.status_text)
        }
        onDeleteCancelled: window.pendingLibraryDelete = ""
        onGraphLayoutRequested: if (window.libraryManagerVisible) window.refreshLibraryManager()
        libraryKind: browserUi.library_kind
        graphMode: window.libraryJourneyGraphMode
        totalEntries: window.libraryTotalEntries
        page: window.libraryPage
        pageCount: window.libraryPageCount()
        pageSize: window.libraryPageSize
        journeySearchText: window.libraryJourneySearchText
        journeyCurrentOnly: window.libraryJourneyCurrentOnly
        pendingDelete: window.pendingLibraryDelete
        entries: libraryEntries
        graphNodes: libraryGraphNodes
        graphEntries: libraryGraphEntries
        graphLineData: window.libraryGraphLineData
        profilesModel: profiles
        profilesLoading: browserUi.profile_values_pending
        onCloseRequested: {
            window.libraryManagerVisible = false
            window.closeInternalSurface()
        }
        onGraphToggleRequested: window.libraryJourneyGraphMode = !window.libraryJourneyGraphMode
        onExportRequested: window.beginJourneyExport()
        onPageChangeRequested: function(delta) { window.changeLibraryPage(delta) }
        onJourneySearchRequested: function(text) {
            window.libraryJourneySearchText = text
            window.libraryJourneyExpandedNode = ""
            window.libraryJourneyCurrentOnly = false
            window.runJourneyQuery(window.journeySearchArgument(text))
        }
        onJourneyClearRequested: {
            window.libraryJourneySearchText = ""
            window.libraryJourneyExpandedNode = ""
            window.libraryJourneyCurrentOnly = false
            window.runJourneyQuery("")
        }
        onJourneyCurrentRequested: window.runJourneyCurrentQuery()
        onJourneyAllRequested: window.runJourneyAllQuery()
        onEntryOpenRequested: function(entryKind, entryId) {
            if (!window.openLibraryEntry(entryKind, entryId))
                libraryManager.requestRejected("This saved page couldn't be opened. Refresh and try again.", browserUi.status_text)
        }
        onPrivateHistoryTransferRequested: function(entryId, title, url) {
            window.showPrivateHistoryTransfer(entryId, title, url)
        }
        onEntryEditRequested: function(entryKind, entryId, value) {
            if (window.editLibraryEntry(entryKind, entryId, value)) {
                libraryManager.mutationAccepted(browserUi.mark_action_id, entryKind, entryId, String(value), false)
            } else {
                libraryManager.requestRejected("This edit couldn't be saved. Review the value and try again.", browserUi.status_text)
            }
        }
        onEntryDeleteRequested: function(entryKind, entryId) {
            var confirmed = window.pendingLibraryDelete === entryKind + "\t" + entryId
            if (window.deleteLibraryEntry(entryKind, entryId))
                libraryManager.mutationAccepted(browserUi.mark_action_id, entryKind, entryId, "", true)
            else if (confirmed)
                libraryManager.requestRejected("This item couldn't be deleted. Try again.", browserUi.status_text)
        }
        onJourneyReopenRequested: function(nodeId, target) {
            var command = ":journey-reopen " + window.libraryCommandArgument(nodeId)
                    + " --target " + target
            if (browserUi.execute_command(command)) {
                window.syncTabModel()
                window.executePendingEngineAction()
                window.libraryManagerVisible = false
                window.closeInternalSurface()
            } else {
                libraryManager.requestRejected("This page couldn't be reopened. Refresh and try again.", browserUi.status_text)
            }
        }
        onJourneyExpandRequested: function(nodeId) {
            window.libraryJourneyExpandedNode = nodeId
            window.libraryJourneySearchText = ""
            window.libraryJourneyCurrentOnly = false
            window.runJourneyQuery(window.journeyExpandArgument(nodeId))
        }
        onJourneyExportFileRequested: {
            fileDialogSurfaces.journeyExportCurrentFile = "journey-export.json"
            fileDialogSurfaces.openJourneyExport()
        }
        onJourneyExportCancelRequested: window.journeyExportPreviewVisible = false
        onPrivateHistoryReopenRequested: function(profileName) {
            window.confirmPrivateHistoryTransfer(profileName)
        }
        onPrivateHistoryBookmarkRequested: function(profileName, profileLabel) {
            window.confirmPrivateHistoryBookmark(profileName, profileLabel)
        }
        onPrivateHistoryCancelRequested: window.cancelPrivateHistoryTransfer()
    }

    FerricSwitcher {
        id: switcherSurface
        parent: window.switcherHost ? window.switcherHost.contentItem : window.contentItem
        browserWindow: window.switcherHost && window.switcherHost.chromeAppearance
            ? window.switcherHost.chromeAppearance : window
        hostWindow: window.switcherHost || window
        maximumResults: window.switcherMaxResults()
        switcherVisible: window.switcherVisible
        scope: window.switcherScope
        results: window.switcherResults
        resultsCurrent: window.switcherResultsCurrent()
        sourcesPending: window.switcherSourceState.pending
        sourcesFailed: window.switcherSourceState.failed
        sourcesRetryable: window.switcherSourceState.retryable
        sourceNotice: window.switcherSourceState.notice
        sourceDetail: window.switcherSourceState.detail
        onRefreshRequested: window.retrySwitcherSources()
        onQueryChangeRequested: function(query) {
            if (window.switcherVisible) {
                window.cancelSwitcherBatch()
                window.switcherResultContext = null
                window.switcherResults = []
                switcherRefreshTimer.restart()
            }
        }
        onCloseRequested: window.closeSwitcher()
        onActivationRequested: function(index) { window.activateSwitcher(index) }
        onActionRequested: function(index, action) {
            window.activateSwitcherAction(index, action)
        }
    }

    Connections {
        target: window
        function onBrowserWindowRegistryChanged() {
            if (window.switcherVisible && !window.switcherRegistry.switcherInvocation()) window.closeSwitcher()
        }
        function onPopupWindowRegistryChanged() {
            if (window.switcherVisible && !window.switcherRegistry.switcherInvocation()) window.closeSwitcher()
        }
    }
    Connections {
        target: window.switcherHost
        ignoreUnknownSignals: true
        function onRuntimeReleasedChanged() {
            if (window.switcherHost && window.switcherHost.runtimeReleased) window.closeSwitcher()
        }
        function onRuntimeDisposingChanged() {
            if (window.switcherHost && window.switcherHost.runtimeDisposing) window.closeSwitcher()
        }
    }

    Connections {
        target: window.switcherUi
        ignoreUnknownSignals: true
        function onProfile_bootstrap_pendingChanged() {
            if (window.switcherUi && window.switcherUi.profile_bootstrap_pending) window.closeSwitcher()
        }
        function onWindow_tokenChanged() {
            if (window.switcherUi && String(window.switcherUi.window_token) !== window.switcherOwnerToken) window.closeSwitcher()
        }
    }

    FerricLinkPreview {
        browserWindow: window
        hostWindow: window
        previewUi: browserUi
        onNavigationConfirmed: {
            window.syncTabModel()
            window.executePendingEngineAction()
        }
    }

    readonly property alias downloadWorkspaceObject: downloadWorkspace
    FerricDownloadWorkspace {
        id: downloadWorkspace
        browserWindow: window
        hostWindow: window
        browserUi: window.primaryBrowserUi
        rootWindow: window
        state: window
        downloadsModel: downloads
        onOpening: window.openInternalSurface()
        onClosed: window.closeInternalSurface()
    }

    readonly property alias profileSessionWorkspaceObject: profileSessionWorkspace
    FerricProfileSessionWorkspace {
        id: profileSessionWorkspace
        browserWindow: window
        hostWindow: window
        browserUi: window.primaryBrowserUi
        state: window
        profilesModel: profiles
        namedSessionsModel: namedSessions
        transientProfile: window.temporaryProfile
        onOpening: window.openInternalSurface()
        onClosed: window.closeInternalSurface()
    }

    StackLayout {
        id: webViews
        anchors.fill: parent
        anchors.leftMargin: window.tabStripVisible && window.tabPosition === "left"
                            ? window.sideTabWidth : 0
        anchors.rightMargin: window.tabStripVisible && window.tabPosition === "right"
                             ? window.sideTabWidth : 0
        anchors.bottomMargin: window.bottomChromeHeight
        currentIndex: {
            var selected = window.activeWebView()
            return selected ? selected.StackLayout.index : -1
        }

        Component {
            id: webViewComponent

            WebEngineView {
                id: webView
                readonly property string promptLeaseIdentity: browserUi.new_prompt_view_identity()
                property var promptIdentityUi: null
                property int promptIdentityTabIndex: -1
                function refreshPromptViewIdentity() {
                    if (promptIdentityUi && promptIdentityTabIndex >= 0
                            && promptIdentityUi.unregister_prompt_view_identity) {
                        promptIdentityUi.unregister_prompt_view_identity(
                            promptIdentityTabIndex, promptLeaseIdentity)
                    }
                    promptIdentityUi = viewUi
                    promptIdentityTabIndex = tabIndex
                    if (promptIdentityUi && promptIdentityTabIndex >= 0
                            && promptIdentityUi.register_prompt_view_identity) {
                        promptIdentityUi.register_prompt_view_identity(
                            promptIdentityTabIndex, promptLeaseIdentity)
                    }
                }
                onTabIndexChanged: refreshPromptViewIdentity()
                onViewUiChanged: {
                    documentRequestOwner = ({})
                    refreshPromptViewIdentity()
                    if (resourceTracked) window.profileResourceRegistry.trackView(webView,viewHost,viewUi)
                }
                onViewHostChanged: {
                    documentRequestOwner = ({})
                    if (resourceTracked) window.profileResourceRegistry.trackView(webView,viewHost,viewUi)
                }
                property bool resourceTracked: false
                property alias nativeUrl: nativeContent.nativeUrl
                onNativeUrlChanged: documentRequestOwner = ({})
                property alias contentTraversalPending: nativeContent.traversalPending
                readonly property var startPage: nativeContent.startPage
                readonly property int engineHistoryIndex: nativeContent.engineHistoryIndex
                readonly property bool contentCanGoBack: nativeContent.canNavigateBack
                readonly property bool contentCanGoForward: nativeContent.canNavigateForward
                property bool contentInitialized: false
                readonly property string modelContentUrl: viewModel.loaded ? viewModel.url : ""
                function setContentUrl(destination, traversal, engineMoves) { contentInitialized = true; nativeContent.setContentUrl(destination, traversal, engineMoves) }
                function contentEngineOffset(destination, expected, backwards) { return nativeContent.engineOffset(destination, expected, backwards) }
                function synchronizeContentHistory() { nativeContent.synchronizeHistory() }
                onModelContentUrlChanged: {
                    if (modelContentUrl.length > 0 && (modelContentUrl === "rb://start" || modelContentUrl === "rb://blank" || nativeUrl.length > 0 || !contentInitialized)) setContentUrl(modelContentUrl, false)
                }
                FerricNativeTabContent {
                    id: nativeContent
                    anchors.fill: parent
                    webView: webView
                    browserUi: webView.viewUi
                    browserWindow: window
                    browserHost: webView.viewHost
                    tabsModel: webView.viewTabs
                }
                property int tabIndex: -1
                property string stableTabId: ""
                property var viewUi: browserUi
        property var selectionSource: null
                property var viewHost: window
                property var viewTabs: tabs
                property var viewModel: ({})
                property var viewProfile: browserProfile
                property var viewInterceptor: window.primaryRequestInterceptor
                property bool viewTransientProfile: window.temporaryProfile
                property bool viewTransferred: false
                property bool viewRetired: false
                property var documentRequestOwner: ({})
                function ownsTab() {
                    return !viewRetired && !!viewUi && tabIndex >= 0 && stableTabId.length > 0
                        && viewUi.tab_id_for_index(tabIndex) === stableTabId
                }
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
                devToolsView: viewHost && viewHost.devToolsWorkspace
                    ? viewHost.devToolsWorkspace.inspectorFor(webView) : null
                // Navigation-scoped content settings are snapshotted per
                // document; live config changes apply to the next navigation.
                property var effectiveSiteSettings: ({ values: {}, matched_rules: [] })
                function refreshEffectiveSiteSettings() {
                    effectiveSiteSettings = window.siteRuleSettingsFor(
                        viewUi, webView.url.toString())
                }
                Connections {
                    target: viewUi
                    function onSite_experiment_kindChanged() {
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
                url: "about:blank"
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
                    resourceTracked=window.profileResourceRegistry.trackView(webView,viewHost,viewUi)
                    if (!resourceTracked) {
                        viewRetired=true
                        viewUi.status_text="This page couldn't be initialized. Try reopening the window."
                        return
                    }
                    refreshPromptViewIdentity()
                    if (viewModel.loaded && !contentInitialized) setContentUrl(viewModel.url || "about:blank", false)
                    refreshEffectiveSiteSettings()
                    if (nativeUrl.length > 0) {
                        viewUi.content_navigated(tabIndex, nativeUrl, engineHistoryIndex, false)
                        if (nativeUrl === "rb://start") Qt.callLater(nativeContent.focusContent)
                    } else if (viewModel.loaded) window.installFocusObserver(webView)
                }
                onJavaScriptConsoleMessage: function(level, message, lineNumber, sourceID) {
                    window.probeFocusNotification(viewUi, viewHost, webView, tabIndex, message)
                }
                onActiveFocusChanged: {
                    if (activeFocus && nativeUrl === "rb://start") { Qt.callLater(nativeContent.focusContent); return }
                    if (activeFocus && viewUi === browserUi
                            && tabIndex === viewUi.active_tab_index) {
                        window.scheduleFocusProbe(2)
                    }
                }
                onRecentlyAudibleChanged: {
                    if (ownsTab() && viewUi === browserUi && tabIndex === viewUi.active_tab_index)
                        window.updateMprisForPrimaryView(webView)
                }
                onAudioMutedChanged: {
                    if (ownsTab() && viewUi === browserUi && tabIndex === viewUi.active_tab_index)
                        window.updateMprisForPrimaryView(webView)
                }
                onZoomFactorChanged: {
                    if (viewUi === browserUi && browserUi.mode === "grid") {
                        browserUi.spatial_invalidated("geometry-changed")
                    }
                }

                onNavigationRequested: function(request) {
                    if (request.isMainFrame) nativeContent.beginNavigation(request.navigationType === WebEngineView.BackForwardNavigation)
                }
                onUrlChanged: {
                    if (nativeUrl.length > 0) return
                    if (!ownsTab()) return
                    nativeContent.noteSameDocument(url.toString())
                    refreshEffectiveSiteSettings()
                    if (!viewModel.loaded) {
                        return
                    }
                    var changedUrl = url.toString()
                    if (viewModel.url !== changedUrl && viewTabs && tabIndex >= 0) {
                        viewTabs.setProperty(tabIndex, "url", changedUrl)
                    }
                    if (!viewTabs || tabIndex < 0 || tabIndex === viewUi.active_tab_index) {
                        if (viewUi === browserUi) {
                            address.text = browserUi.display_url
                            window.updateMprisForPrimaryView(webView)
                            window.refreshBlockingEvidence()
                        }
                    }
                }
                onTitleChanged: {
                    if (nativeUrl.length > 0) return
                    if (!ownsTab()) return
                    if (viewModel.loaded && viewTabs && tabIndex >= 0) {
                        viewTabs.setProperty(tabIndex, "title", title || "New tab")
                        if (tabIndex === viewUi.active_tab_index && viewUi === browserUi) {
                            window.updateMprisForPrimaryView(webView)
                        }
                    }
                }
                onLoadingChanged: function(loadRequest) {
                    if (loadRequest.status === WebEngineView.LoadStartedStatus) documentRequestOwner = ({})
                    if (!viewModel.loaded || nativeUrl.length > 0 || !ownsTab()) return
                    if (loadRequest.status === WebEngineView.LoadStartedStatus) {
                        viewUi.invalidate_native_prompt_leases()
                        webView.scrollPositionRestored = false
                        window.clearPageDialogForView(webView)
                        window.clearClientCertificateForView(webView)
                        window.clearCertificateErrorForView(webView)
                        window.clearWebAuthForView(webView)
                        window.clearContextMenuForView(webView)
                        window.clearDesktopMediaForView(webView)
                        window.clearSiteDataClearForView(webView)
                        window.noteCaptureNavigation(webView)
                        if (viewHost && viewHost.clearFileDialogForView) viewHost.clearFileDialogForView(webView)
                        window.clearRendererFailureForView(webView)
                        window.resetPageDialogBudget(webView)
                        viewInterceptor.clearSiteEvidence(webView.url.host)
                        viewUi.clear_blocking_active_evidence()
                        if (viewTabs && tabIndex >= 0) {
                            window.removePermissionGroups(function(group) {
                                return group.ui === webView.viewUi && group.tabIndex === webView.tabIndex
                            }, true)
                        } else {
                            window.cancelPermissionForUi(viewUi)
                        }
                        window.installPageUserscripts(
                            viewUi, webView, webView.url.toString(), viewTransientProfile)
                        window.injectPageUserscripts(
                            viewUi, webView, webView.url.toString(), viewTransientProfile,
                            "document_start")
                        window.injectCosmeticRules(viewUi, webView)
                        if (viewTabs && tabIndex >= 0) {
                            viewUi.navigation_started_for(tabIndex, loadRequest.url.toString())
                        } else {
                            viewUi.navigation_started(loadRequest.url.toString())
                        }
                    } else if (loadRequest.status === WebEngineView.LoadSucceededStatus) {
                        nativeContent.noteCommit()
                        if (viewTabs && tabIndex >= 0) {
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
                        if (viewTabs && tabIndex >= 0) {
                            viewUi.navigation_completed_for(tabIndex)
                        } else {
                            viewUi.navigation_completed()
                        }
                        if (viewUi === browserUi
                                && tabIndex === viewUi.active_tab_index) {
                            window.scheduleFocusProbe(3)
                        }
                    if (!viewTabs || tabIndex < 0 || tabIndex === viewUi.active_tab_index) {
                            window.finishSiteDoctorAfterLoad(viewUi, true)
                        }
                    } else if (loadRequest.status === WebEngineView.LoadFailedStatus) {
                        nativeContent.noteCommit()
                        if (viewTabs && tabIndex >= 0) {
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
                    if (!request) return
                    request.accepted = true
                    var host = viewHost || window
                    if ((!viewTabs || tabIndex < 0 || tabIndex === viewUi.active_tab_index)
                            && !host.runtimeReleased && !host.runtimeDisposing
                            && host.handleFileDialogRequested) {
                        host.handleFileDialogRequested(request, webView, viewUi)
                    } else {
                        window.resolveQtRequest(viewUi, request, "file-dialog", "dialogReject", [])
                    }
                }
                onPermissionRequested: function(permissionRequest) {
                    if (!viewTabs || tabIndex < 0 || tabIndex === viewUi.active_tab_index) {
                        window.handlePermissionRequested(permissionRequest, tabIndex,
                            viewUi, webView, viewTransientProfile, viewHost)
                    } else {
                        window.resolveQtRequest(viewUi, permissionRequest, "permission", "deny", [])
                    }
                }
                onJavaScriptDialogRequested: function(request) {
                    window.handleJavaScriptDialogRequested(
                        viewUi, webView, request, viewTransientProfile, viewHost)
                }
                onAuthenticationDialogRequested: function(request) {
                    window.handleAuthenticationDialogRequested(
                        viewUi, webView, request, viewTransientProfile, viewHost)
                }
                onSelectClientCertificate: function(selection) {
                    if (!viewTabs || tabIndex < 0 || tabIndex === viewUi.active_tab_index) {
                        window.handleClientCertificateRequested(
                            viewUi, webView, selection, viewTransientProfile, viewHost)
                    } else {
                        window.resolveQtRequest(viewUi, selection, "client-certificate", "selectNone", [])
                    }
                }
                onCertificateError: function(error) {
                    if (!viewTabs || tabIndex < 0 || tabIndex === viewUi.active_tab_index) {
                        window.handleCertificateError(
                            viewUi, webView, error, viewHost)
                    } else {
                        window.resolveQtRequest(viewUi, error, "client-certificate", "rejectCertificate", [])
                    }
                }
                onWebAuthUxRequested: function(request) {
                    if (!viewTabs || tabIndex < 0 || tabIndex === viewUi.active_tab_index) {
                        window.handleWebAuthRequested(
                            viewUi, webView, request, viewHost)
                    } else {
                        window.resolveQtRequest(viewUi, request, "webauth", "cancel", [])
                    }
                }
                onContextMenuRequested: function(request) {
                    if (!viewTabs || tabIndex < 0 || tabIndex === viewUi.active_tab_index) {
                        window.handleContextMenuRequested(
                            viewUi, webView, request, viewHost)
                    } else {
                        request.accepted = true
                    }
                }
                onDesktopMediaRequested: function(request) {
                    window.handleDesktopMediaRequested(
                        viewUi, webView, request, viewHost)
                }
                onFullScreenRequested: function(request) {
                    if (viewUi === browserUi && browserUi.mode === "grid") {
                        browserUi.spatial_invalidated("view-changed")
                    }
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
                    if (nativeUrl.length > 0) return
                    window.handleRendererProcessTerminated(
                        viewUi, webView, viewHost, tabIndex,
                        terminationStatus, exitCode)
                }
                Component.onDestruction: {
                    if (!(viewHost && viewHost.runtimeReleased) && !window.runtimeDisposing) {
                        window.cancelScriptsForView(webView)
                        window.clearPageDialogForView(webView)
                        window.clearClientCertificateForView(webView)
                        window.clearCertificateErrorForView(webView)
                        window.clearWebAuthForView(webView)
                        window.clearContextMenuForView(webView)
                        window.clearDesktopMediaForView(webView)
                        window.clearCaptureSessionForView(webView)
                        if (viewHost && viewHost.clearFileDialogForView) viewHost.clearFileDialogForView(webView)
                        window.clearRendererFailureForView(webView)
                    }
                    if (promptIdentityUi && promptIdentityTabIndex >= 0
                            && promptIdentityUi.unregister_prompt_view_identity) {
                        promptIdentityUi.unregister_prompt_view_identity(
                            promptIdentityTabIndex, promptLeaseIdentity)
                    }
                }
                onNewWindowRequested: function(request) {
                    var pageSearch = viewHost && viewHost.searchWorkspace
                    if (pageSearch && pageSearch.acceptNewTabRequest(webView, request)) return
                    var popupAllowed = viewTabs && tabIndex >= 0
                            ? viewUi.popup_allowed_for(
                                tabIndex, request.requestedUrl.toString(), request.userInitiated)
                            : viewUi.popup_allowed(
                                request.requestedUrl.toString(), request.userInitiated)
                    if (!popupAllowed) {
                        return
                    }
                    var popupToken=viewUi.take_popup_journey_token(request.requestedUrl.toString())
                    var popup = popupWindowComponent.createObject(null, {
                        popupRequest: request,
                        popupJourneyToken: popupToken,
                        popupProfile: viewProfile,
                        popupOpenerUi: viewUi,
                        popupContextName: viewUi.context_name,
                        popupContextLabel: viewUi.context_label,
                        popupPrivateProfile: viewTransientProfile,
                        popupEphemeralProfile: viewHost === window ? window.ephemeralProfile : viewHost.windowEphemeralProfile,
                        popupEphemeralInvocationToken: viewHost === window ? window.ephemeralInvocationToken : viewHost.windowEphemeralInvocationToken,
                        popupProfileName: viewHost === window ? window.profileName : viewHost.windowProfileName,
                        popupStorageBasePath: viewHost === window ? window.storageBasePath : viewHost.windowStorageBasePath
                    })
                    if (!popup || !popup.profileBootstrapComplete) {
                        viewUi.close_popup_tab(popupToken)
                        viewUi.status_text="The popup could not be opened. Try opening the link again."
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
        active: window.devToolsWorkspace.visible && !window.devToolsWorkspace.detached
                && window.devToolsWorkspace.sourceCurrent
        visible: active && window.devToolsWorkspace.attachedVisible
        z: 20
        onItemChanged: window.attachedDevToolsView = item
        sourceComponent: Component {
            WebEngineView {
                id: primaryInspector
                property var viewHost: window
                property var viewUi: browserUi
                anchors.fill: parent
                profile: window.devToolsWorkspace.sourceProfile
                inspectedView: window.devToolsWorkspace.sourceView
                Accessible.name: "Attached developer tools"
                Component.onCompleted: window.profileResourceRegistry.trackView(primaryInspector,viewHost,viewUi)
            }
        }
    }

    FerricCopyNotice {
        id: copyNoticeSurface
        parent: window.copyNoticeView || (window.copyNoticeHost || window).contentItem
        browserWindow: window.copyNoticeAppearance
        hostWindow: window.copyNoticeHost || window
        noticeVisible: window.copyNoticeVisible && window.copyNoticeOwnerCurrent
        copiedText: window.copiedText
        sensitive: window.copyNoticeSensitive
        primarySelection: window.copyNoticePrimary
        onCopyAgainRequested: window.repeatClipboardCopy()
        onDismissalRequested: window.dismissCopyNotice()
    }

    FerricStatusBar {
        onCopyUrlRequested: browserUi.execute_ui_action("browser.url.copy", "")
        browserWindow: window
        statusVisible: window.statusBarVisible
        mode: browserUi.mode
        displayUrl: browserUi.display_url
        hintPreviewText: browserUi.mode === "hint" && !browserUi.target_search_active ? window.hintWorkspace.previewText : primaryPageSearch.previewText
        statusText: window.commandNoticeVisible
                    ? window.commandNoticeText : browserUi.status_text
        statusError: window.commandNoticeVisible && window.commandNoticeError
        contextName: browserUi.context_name
        contextColor: window.contextStatusColor(browserUi, window.mutedTextColor)
        profileName: window.profileName
        temporaryProfile: window.temporaryProfile
        ephemeralProfile: window.ephemeralProfile
        blockingSiteCount: browserUi.blocking_active_site_count
        activeTabIndex: browserUi.active_tab_index
        tabCount: browserUi.tab_count
        engineUpdateNotice: window.engineUpdateNotice
        macroStatusText: browserUi.macro_status_text
        activeView: window.activeWebView()
        accessibleDetails: window.statusDetails(
                               browserUi, window, window.activeWebView(),
                               window.temporaryProfile, window.profileName,
                               window.ephemeralProfile)
    }

    function submitInteractiveCommand(text) { return runtimeChrome.submitInteractiveCommand(text) }

    FerricCommandLine {
        id: commandSurface
        browserWindow: window
        commandVisible: browserUi.mode === "command"
        completionVisible: browserUi.completion_visible
        completionText: browserUi.completion_text
        completionValues: browserUi.completion_values
        completionStart: browserUi.completion_start
        completionEnd: browserUi.completion_end
        completionSelected: browserUi.completion_selected
        onCompletionUpdateRequested: function(text, cursorPosition) {
            browserUi.update_completion(text, cursorPosition)
        }
        onSubmitted: function(text) { window.submitInteractiveCommand(text) }
        onInputEdited: window.pendingInteractiveCommand = ""
        onEscapeRequested: {
            window.pendingInteractiveCommand = ""
            browserUi.escape()
        }
        onCompletionMoveRequested: function(delta) { browserUi.completion_move(delta) }
        onCompletionSelectRequested: function(index) { browserUi.completion_select(index) }
        onHistoryMoveRequested: function(delta, current) {
            commandSurface.commandText = delta < 0
                ? browserUi.command_history_previous(current)
                : browserUi.command_history_next(current)
            commandSurface.cursorPosition = commandSurface.commandText.length
        }
    }

    FerricSearchBar {
        id: searchSurface
        browserWindow: window
        searchVisible: browserUi.mode === "search"
        searchBackward: browserUi.search_backward
        searchText: browserUi.search_text
        onSearchChanged: function(text) { browserUi.search_changed(text) }
        onAccepted: {
            primaryPageSearch.accept()
            browserUi.accept_search()
            window.executePendingEngineAction()
        }
        onEscapeRequested: browserUi.escape()
        onNextRequested: function(backward) {
            browserUi.search_next(backward)
            window.executePendingEngineAction()
        }
    }




    readonly property var commandSurface: window.commandSurfaceObject
    readonly property var searchSurface: window.searchSurfaceObject
    readonly property var attachedDevToolsLoader: window.attachedDevToolsLoaderObject

    function configuredBlocklistIds() { return runtimeSurface.configuredBlocklistIds() }

    function requestBlocklistUpdate() { return runtimeSurface.requestBlocklistUpdate() }

    function configuredLinkCleaningUpdate() { return runtimeSurface.configuredLinkCleaningUpdate() }

    function requestLinkCleaningUpdate(automatic) { return runtimeSurface.requestLinkCleaningUpdate(automatic) }

    function configureLinkCleaningUpdateTimer() { return runtimeSurface.configureLinkCleaningUpdateTimer() }

    function configureBlocklistUpdateTimer() { return runtimeSurface.configureBlocklistUpdateTimer() }

    function closeTabAtIndex(index) { return runtimeSurface.closeTabAtIndex(index) }

    function executeHistoryTraversal(view, backwards, requestedCount) { return runtimeSurface.executeHistoryTraversal(view, backwards, requestedCount) }

    function executePendingEngineAction() { return runtimeSurface.executePendingEngineAction() }

    function activeWebView() { return runtimeSurface.activeWebView() }

    function updateActiveTabUrl() { return runtimeSurface.updateActiveTabUrl() }

    function applyRestorePayload(payload, hasModeLine) { return runtimeSurface.applyRestorePayload(payload, hasModeLine) }

    function syncTabModel() { return runtimeSurface.syncTabModel() }

    function scheduleRuntimeWork(delay) { return runtimeSurface.scheduleRuntimeWork(delay) }

    function processRuntimeWork() { return runtimeSurface.processRuntimeWork() }

    Timer {
        id: runtimeWorkTimer
        interval: 1
        repeat: false
        running: false
        onTriggered: window.processRuntimeWork()
    }

    property int focusProbeRetries: 0

    function scheduleFocusProbe(retries) { return runtimeSurface.scheduleFocusProbe(retries) }

    function cancelFocusProbes() { return runtimeSurface.cancelFocusProbes() }

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

    function scheduleScrollCapture() { return runtimeSurface.scheduleScrollCapture() }

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

    function logicalNormalKeyText(event) { return runtimeSurface.logicalNormalKeyText(event) }

    function completeBrowserKeyAction(ui, host) { return runtimeSurface.completeBrowserKeyAction(ui, host) }

    function logicalGridKeyText(event) { return runtimeSurface.logicalGridKeyText(event) }

    function handleBrowserKey(ui, host, event) { return runtimeSurface.handleBrowserKey(ui, host, event) }

    function handleBrowserKeyRelease(ui, host, key, modifiers) { return runtimeSurface.handleBrowserKeyRelease(ui, host, key, modifiers) }

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
        title: "Active downloads are still running"
        message: "Cancel active browser work before quitting, or keep the browser open."
        keepLabel: "Keep browser open"
        proceedLabel: "Cancel active work and quit"
        dialogHeight: 190 * window.chromeScale
        onKeepRequested: {
            window.shutdownPromptVisible = false
            browserUi.status_text = "Shutdown cancelled"
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
            browserUi.status_text = "Shutdown cancelled"
        }
        onProceedRequested: {
            window.shutdownPagePromptVisible = false
            window.finalizeQuit()
        }
    }

    FerricShutdownDecisionDialog {
        hostWindow: window
        promptVisible: window.applicationShutdownForcePromptVisible
        commandText: ":quit"
        title: "Browser shutdown is taking longer than expected"
        message: window.applicationShutdownStage
                 + ". Continue waiting, or force quit? Recent changes may not be saved if you force quit."
        keepLabel: "Continue waiting"
        proceedLabel: "Force quit"
        keepAccessibleName: "Continue waiting for shutdown"
        proceedAccessibleName: "Force quit; recent changes may not be saved"
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

    function completeProfileBootstrap() { return runtimeSurface.completeProfileBootstrap() }

    function openAdditionalStartupUrls() { return runtimeSurface.openAdditionalStartupUrls() }

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

    FerricSelectorValidationSurface {
        browserWindow: window
        browserUi: window.primaryBrowserUi
    }

    Component.onCompleted: {
        window.browserProfile = window.browserProfilePrototypeObject.instance()
        if (!window.browserProfile) {
            console.error("WebEngine profile instance was unavailable")
            Qt.quit()
            return
        }
        window.primaryProfileResources=window.profileResourceRegistry.ensure(
                    window.profileName,window.temporaryProfile,window.storageBasePath,
                    window.browserProfile,window.primaryRequestInterceptor,
                    window.browserProfilePrototypeObject)
        if (!window.profileResourceRegistry.retainWindow(window.primaryProfileResources,window,browserUi)) {
            console.error("The primary profile resources could not be initialized")
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
        // Resolve live child requests while this creation context is callable.
        // Descendant destructors must not call back after it has been released.
        window.runtimeDisposing = true
        window.downloadWorkspaceObject.shutdown()
        window.scriptRunnerObject.cancelAll()
        var children = (window.browserWindowRegistry || []).concat(window.popupWindowRegistry || [])
        for (var i = 0; i < children.length; ++i) {
            var host = children[i] && children[i].host
            if (host && host !== window && host.releaseWindowResources) host.releaseWindowResources()
        }
        window.clearPageDialog(true)
        window.clearSiteDataClearForView(window.siteDataClearView)
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
        window.profileResourceRegistry.releaseWindow(window)
        window.profileResourceRegistry.shutdown()
        browserUi.view_closed()
        browserUi.release_transient_resources()
    }

}

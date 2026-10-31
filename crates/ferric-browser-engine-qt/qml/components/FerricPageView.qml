import QtQuick
import QtWebEngine
import io.github.ferricbrowser 1.0

WebEngineView {
    id: webView
    property var factory: null
    enabled: !!factory && !factory.rootWindow.applicationShutdownInProgress
    readonly property bool awaitingPreparedNavigation: !!(viewHost && viewHost.windowPreparedPageMode
        && !viewHost.windowPreparedNavigationStarted)

                readonly property string promptLeaseIdentity: factory.primaryUi.new_prompt_view_identity()
                property var promptIdentityUi: null
                property int promptIdentityTabIndex: -1
                property bool viewTransferPending: false
                property var liveTransferSource: null
                function detachPromptIdentityForTransfer() {
                    if (promptIdentityUi && promptIdentityTabIndex >= 0)
                        promptIdentityUi.unregister_prompt_view_identity(promptIdentityTabIndex,promptLeaseIdentity)
                    promptIdentityUi = null
                    promptIdentityTabIndex = -1
                }
                function lifecycleOwner() {
                    var saved = liveTransferSource
                    if (viewTransferPending && saved && saved.ui) {
                        var index = saved.ui.tab_index_for_id(stableTabId)
                        if (index >= 0) return { ui:saved.ui, host:saved.host, tabs:saved.tabs, index:index }
                    }
                    return { ui:viewUi, host:viewHost, tabs:viewTabs, index:tabIndex }
                }
                function refreshDocumentOwnerAfterTransfer(target) {
                    if (!target || !ownsTab() || loading) return
                    if (nativeUrl.length === 0 && nativeContent.engineDocumentReady) {
                        // The transferred document owns its captured Target;
                        // a newly queued navigation must never donate authority.
                        documentTitleTarget = target
                        publishPageTitle()
                    }
                    synchronizeContentHistory()
                }
                function ownsLifecycleTab() {
                    var owner = lifecycleOwner()
                    return !viewRetired && owner.ui && owner.index >= 0
                        && owner.ui.tab_id_for_index(owner.index) === stableTabId
                }
                function refreshPromptViewIdentity() {
                    // Initial-property callbacks run before createObject has
                    // supplied the final Ui/index. A not-yet-adopted child
                    // must never overwrite its opener's registration.
                    if (!resourceTracked || viewTransferPending) return
                    if (viewRetired || !viewUi || tabIndex < 0 || stableTabId.length === 0) {
                        if (promptIdentityUi && promptIdentityTabIndex >= 0)
                            promptIdentityUi.unregister_prompt_view_identity(promptIdentityTabIndex, promptLeaseIdentity)
                        promptIdentityUi = null
                        promptIdentityTabIndex = -1
                        return
                    }
                    if ((promptIdentityUi !== viewUi || promptIdentityTabIndex !== tabIndex)
                            && promptIdentityUi && promptIdentityTabIndex >= 0
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
                    if (resourceTracked) factory.rootWindow.profileResourceRegistry.trackView(webView,viewHost,viewUi)
                }
                onTabIndexChanged: refreshPromptViewIdentity()
                onViewUiChanged: {
                    documentRequestOwner = ({})
                    documentTitleTarget = null
                    refreshPromptViewIdentity()
                    if (resourceTracked) factory.rootWindow.profileResourceRegistry.trackView(webView,viewHost,viewUi)
                }
                onViewHostChanged: {
                    documentRequestOwner = ({})
                    if (resourceTracked) factory.rootWindow.profileResourceRegistry.trackView(webView,viewHost,viewUi)
                }
                property bool resourceTracked: false
                property alias nativeUrl: nativeContent.nativeUrl
                onNativeUrlChanged: {
                    documentRequestOwner = ({})
                    documentTitleTarget = null
                }
                property alias contentTraversalPending: nativeContent.traversalPending
                readonly property var startPage: nativeContent.startPage
                readonly property int engineHistoryIndex: nativeContent.engineHistoryIndex
                readonly property bool contentCanGoBack: nativeContent.canNavigateBack
                readonly property bool contentCanGoForward: nativeContent.canNavigateForward
                property bool contentInitialized: false
                readonly property string modelContentUrl: viewModel.loaded ? viewModel.url : ""
                function setContentUrl(destination, traversal, engineMoves, freshRequest, historyIndex) { contentInitialized = true; nativeContent.setContentUrl(destination, traversal, engineMoves, freshRequest, historyIndex) }
                function requestContentReload(request) { nativeContent.requestReload(request) }
                function cancelPendingContentReload() { nativeContent.cancelReloadAttempt() }
                function noteBeforeUnloadRejected() { nativeContent.noteBeforeUnloadRejected() }
                readonly property bool formResubmissionPending: !!nativeContent.formReloadRequest
                function contentEngineOffset(destination, expected, backwards) { return nativeContent.engineOffset(destination, expected, backwards) }
                function synchronizeContentHistory() { nativeContent.synchronizeHistory() }
                function requestContentHistoryTraversal(backwards, count, perform) {
                    nativeContent.requestHistoryTraversal(backwards, count, perform)
                }
                // Bootstrap can publish a temporary blank tab before choosing
                // its startup content. Initialize from the settled model, or
                // let an explicit navigation take ownership first.
                Timer {
                    id: initializeContentTimer
                    interval: 0
                    onTriggered: if (webView.ownsTab() && webView.viewModel.loaded && !webView.contentInitialized
                            && !webView.awaitingPreparedNavigation)
                        webView.setContentUrl(webView.modelContentUrl || "about:blank", false)
                }
                onModelContentUrlChanged: {
                    if (awaitingPreparedNavigation) return
                    if (!contentInitialized) { initializeContentTimer.restart(); return }
                    if (!viewTransferPending && modelContentUrl.length > 0 && (modelContentUrl === "rb://start" || modelContentUrl === "rb://blank" || nativeUrl.length > 0)) setContentUrl(modelContentUrl, false)
                }
                FerricNativeTabContent {
                    id: nativeContent
                    anchors.fill: parent
                    webView: webView
                    browserUi: webView.viewUi
                    browserWindow: factory.rootWindow
                    browserHost: webView.viewHost
                    tabsModel: webView.viewTabs
                }
                property int tabIndex: -1
                property string stableTabId: ""
                onStableTabIdChanged: {
                    documentTitleOwnerTimer.restart()
                    if (resourceTracked) refreshPromptViewIdentity()
                }
                Timer {
                    id: documentTitleOwnerTimer
                    interval: 0
                    // The view owns this deferred work, so destroying a
                    // provisional view during transfer cancels the callback.
                    onTriggered: {
                        if (!webView.ownsTab() || webView.loading || !nativeContent.engineDocumentReady) return
                        webView.documentTitleTarget = webView.viewUi.navigation_target_for_index(webView.tabIndex)
                        webView.publishPageTitle()
                    }
                }
                property var viewUi: factory.primaryUi
        property var selectionSource: null
                property var viewHost: factory.rootWindow
                property var viewTabs: factory.rootWindow.tabs
                property var viewModel: ({})
                property var viewProfile: factory.rootWindow.browserProfile
                property var viewInterceptor: factory.rootWindow.primaryRequestInterceptor
                property bool viewTransientProfile: factory.rootWindow.temporaryProfile
                property bool viewTransferred: false
                property bool viewRetired: false
                onViewRetiredChanged: { if (resourceTracked) refreshPromptViewIdentity() }
                property var documentRequestOwner: ({})
                property var documentTitleTarget: null
                property string creationLoadReceipt: ""
                property var creationLoadProfile: null
                function publishPageTitle() {
                    var target = documentTitleTarget
                    if (!target || !ownsTab() || nativeUrl.length > 0 || loading
                            || !nativeContent.engineDocumentReady) return
                    viewUi.document_title_changed(target.tabId, target.generation,
                            target.documentId, title)
                }
                function ownsTab() {
                    return !viewRetired && !viewTransferPending && !!viewUi && tabIndex >= 0 && stableTabId.length > 0
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
                    effectiveSiteSettings = factory.rootWindow.siteRuleSettingsFor(
                        viewUi, webView.url.toString())
                }
                Connections {
                    target: viewUi
                    function onSite_experiment_kindChanged() {
                        webView.refreshEffectiveSiteSettings()
                    }
                    function onActive_tab_indexChanged() {
                        if (viewUi === factory.primaryUi
                                && tabIndex === viewUi.active_tab_index) {
                            factory.rootWindow.refreshBlockingEvidence()
                            factory.rootWindow.updateMprisForPrimaryView(webView)
                            factory.rootWindow.scheduleFocusProbe(2)
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
                // Native pages supply their own content; the model or an
                // explicit request initializes the first engine navigation.
                url: ""
                audioMuted: !!viewModel.muted
                zoomFactor: Number(factory.rootWindow.siteRuleValue(
                    effectiveSiteSettings, "content.zoom", viewModel.zoom))
                settings.javascriptEnabled: !!factory.rootWindow.siteRuleValue(
                    effectiveSiteSettings, "content.javascript", true)
                settings.autoLoadImages: !!factory.rootWindow.siteRuleValue(
                    effectiveSiteSettings, "content.images", true)
                settings.forceDarkMode: !!factory.rootWindow.siteRuleValue(
                    effectiveSiteSettings, "content.force_dark", false)
                settings.playbackRequiresUserGesture:
                    factory.rootWindow.siteRuleValue(effectiveSiteSettings, "content.autoplay", "engine-default")
                    === "require-gesture"
                Accessible.name: "Web content for tab " + (tabIndex + 1)
                Component.onCompleted: {
                    resourceTracked=factory.rootWindow.profileResourceRegistry.trackView(webView,viewHost,viewUi)
                    if (!resourceTracked) {
                        viewRetired=true
                        viewUi.status_text="This page couldn't be initialized. Try reopening the window."
                        return
                    }
                    refreshPromptViewIdentity()
                    if (viewModel.loaded && !contentInitialized) initializeContentTimer.restart()
                    refreshEffectiveSiteSettings()
                    if (nativeUrl.length > 0) {
                        viewUi.content_navigated(tabIndex, nativeUrl, engineHistoryIndex, false)
                        if (nativeUrl === "rb://start") Qt.callLater(nativeContent.focusContent)
                    } else if (viewModel.loaded) factory.rootWindow.installFocusObserver(webView)
                }
                onJavaScriptConsoleMessage: function(level, message, lineNumber, sourceID) {
                    factory.rootWindow.probeFocusNotification(viewUi, viewHost, webView, tabIndex, message)
                }
                onActiveFocusChanged: {
                    if (activeFocus && nativeUrl === "rb://start") { Qt.callLater(nativeContent.focusContent); return }
                    if (activeFocus && viewUi === factory.primaryUi
                            && tabIndex === viewUi.active_tab_index) {
                        factory.rootWindow.scheduleFocusProbe(2)
                    }
                }
                onRecentlyAudibleChanged: {
                    if (ownsTab() && viewUi === factory.primaryUi && tabIndex === viewUi.active_tab_index)
                        factory.rootWindow.updateMprisForPrimaryView(webView)
                }
                onAudioMutedChanged: {
                    if (ownsTab() && viewUi === factory.primaryUi && tabIndex === viewUi.active_tab_index)
                        factory.rootWindow.updateMprisForPrimaryView(webView)
                }
                onZoomFactorChanged: {
                    if (viewUi === factory.primaryUi && factory.primaryUi.mode === "grid") {
                        factory.primaryUi.spatial_invalidated("geometry-changed")
                    }
                }

                onNavigationRequested: function(request) {
                    if (request.isMainFrame && request.navigationType === WebEngineView.ReloadNavigation
                            && nativeContent.interceptReloadNavigation(request)) return
                    if (request.isMainFrame) nativeContent.beginNavigation(
                        request.navigationType === WebEngineView.BackForwardNavigation, request.hasFormData,
                        request.navigationType === WebEngineView.ReloadNavigation)
                }
                onUrlChanged: {
                    var lifecycle = lifecycleOwner()
                    var viewUi = lifecycle.ui
                    var viewHost = lifecycle.host
                    var viewTabs = lifecycle.tabs
                    var tabIndex = lifecycle.index

                    if (nativeUrl.length > 0) return
                    if (!ownsLifecycleTab()) return
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
                        if (viewUi === factory.primaryUi) {
                            factory.addressField.text = factory.primaryUi.display_url
                            factory.rootWindow.updateMprisForPrimaryView(webView)
                            factory.rootWindow.refreshBlockingEvidence()
                        }
                    }
                }
                onTitleChanged: {
                    var lifecycle = lifecycleOwner()
                    var viewUi = lifecycle.ui
                    var viewHost = lifecycle.host
                    var viewTabs = lifecycle.tabs
                    var tabIndex = lifecycle.index

                    if (nativeUrl.length > 0) return
                    if (!ownsLifecycleTab()) return
                    publishPageTitle()
                    if (viewModel.loaded && viewTabs && tabIndex >= 0) {
                        viewTabs.setProperty(tabIndex, "title", viewUi.display_page_title(title) || "New tab")
                        if (tabIndex === viewUi.active_tab_index && viewUi === factory.primaryUi) {
                            factory.rootWindow.updateMprisForPrimaryView(webView)
                        }
                    }
                }
                onLoadingChanged: function(loadRequest) {
                    var lifecycle = lifecycleOwner()
                    var viewUi = lifecycle.ui
                    var viewHost = lifecycle.host
                    var viewTabs = lifecycle.tabs
                    var tabIndex = lifecycle.index

                    var stage = loadRequest.status === WebEngineView.LoadStartedStatus ? "started"
                        : loadRequest.status === WebEngineView.LoadStoppedStatus ? "stopped"
                        : loadRequest.status === WebEngineView.LoadSucceededStatus ? "succeeded"
                        : loadRequest.status === WebEngineView.LoadFailedStatus ? "failed" : ""
                    if (nativeContent.settleBeforeUnloadLoading(stage, loadRequest)) return
                    if (nativeContent.blockedReloadLoading(stage)) return
                    if (loadRequest.status === WebEngineView.LoadStartedStatus) {
                        documentRequestOwner = ({})
                        documentTitleTarget = null
                    }
                    if (!viewModel.loaded || nativeUrl.length > 0 || !ownsLifecycleTab()) return
                    if (loadRequest.status === WebEngineView.LoadStartedStatus) {
                        nativeContent.engineLoadState = "loading"
                        viewUi.invalidate_native_prompt_leases()
                        webView.scrollPositionRestored = false
                        factory.rootWindow.clearPageDialogForView(webView)
                        factory.rootWindow.clearClientCertificateForView(webView)
                        factory.rootWindow.clearCertificateErrorForView(webView)
                        factory.rootWindow.clearWebAuthForView(webView)
                        factory.rootWindow.clearContextMenuForView(webView)
                        factory.rootWindow.clearDesktopMediaForView(webView)
                        factory.rootWindow.clearSiteDataClearForView(webView)
                        factory.rootWindow.noteCaptureNavigation(webView)
                        if (viewHost && viewHost.clearFileDialogForView) viewHost.clearFileDialogForView(webView)
                        factory.rootWindow.clearRendererFailureForView(webView)
                        factory.rootWindow.resetPageDialogBudget(webView)
                        viewInterceptor.clearSiteEvidence(webView.url.host)
                        viewUi.clear_blocking_active_evidence()
                        if (viewTabs && tabIndex >= 0) {
                            factory.rootWindow.removePermissionGroups(function(group) {
                                return group.ui === webView.viewUi && group.tabIndex === webView.tabIndex
                            }, true)
                        } else {
                            factory.rootWindow.cancelPermissionForUi(viewUi)
                        }
                        factory.rootWindow.installPageUserscripts(
                            viewUi, webView, webView.url.toString(), viewTransientProfile)
                        factory.rootWindow.injectPageUserscripts(
                            viewUi, webView, webView.url.toString(), viewTransientProfile,
                            "document_start")
                        factory.rootWindow.injectCosmeticRules(viewUi, webView)
                        if (viewTabs && tabIndex >= 0) {
                            viewUi.navigation_started_for(tabIndex, loadRequest.url.toString())
                        } else {
                            viewUi.navigation_started(loadRequest.url.toString())
                        }
                        creationLoadProfile = webView.profile
                        creationLoadReceipt = viewUi.begin_page_creation_load(tabIndex,
                                promptLeaseIdentity, loadRequest.url.toString())
                    } else if (loadRequest.status === WebEngineView.LoadSucceededStatus) {
                        nativeContent.noteCommit(true)
                        if (viewTabs && tabIndex >= 0) {
                            viewUi.navigation_committed_for(tabIndex, webView.url.toString(), webView.title)
                        } else {
                            viewUi.navigation_committed(webView.url.toString(), webView.title)
                        }
                        if (!webView.scrollPositionRestored) {
                            webView.scrollPositionRestored = true
                            factory.rootWindow.restoreScrollPosition(webView, viewModel.scrollX, viewModel.scrollY)
                        }
                        factory.rootWindow.injectPageUserscripts(
                            viewUi, webView, webView.url.toString(), viewTransientProfile,
                            "document_end")
                        factory.rootWindow.injectCosmeticRules(viewUi, webView)
                        Qt.callLater(function() {
                            factory.rootWindow.injectPageUserscripts(
                                viewUi, webView, webView.url.toString(), viewTransientProfile,
                                "document_idle")
                        })
                        if (viewTabs && tabIndex >= 0) {
                            viewUi.navigation_completed_for(tabIndex)
                        } else {
                            viewUi.navigation_completed()
                        }
                        viewUi.complete_page_creation_load(creationLoadReceipt, promptLeaseIdentity,
                                creationLoadProfile === webView.profile && webView.profile === viewProfile)
                        creationLoadReceipt = ""
                        documentTitleTarget = viewUi.navigation_target_for_index(tabIndex)
                        publishPageTitle()
                        if (viewUi === factory.primaryUi
                                && tabIndex === viewUi.active_tab_index) {
                            factory.rootWindow.scheduleFocusProbe(3)
                        }
                    if (!viewTabs || tabIndex < 0 || tabIndex === viewUi.active_tab_index) {
                            factory.rootWindow.finishSiteDoctorAfterLoad(viewUi, true)
                        }
                    } else if (loadRequest.status === WebEngineView.LoadStoppedStatus) {
                        nativeContent.noteStopped()
                        viewUi.complete_page_creation_load(creationLoadReceipt, promptLeaseIdentity, false)
                        creationLoadReceipt = ""
                    } else if (loadRequest.status === WebEngineView.LoadFailedStatus) {
                        nativeContent.noteCommit(false)
                        viewUi.complete_page_creation_load(creationLoadReceipt, promptLeaseIdentity, false)
                        creationLoadReceipt = ""
                        if (viewTabs && tabIndex >= 0) {
                            viewUi.navigation_failed_with_details(
                                tabIndex,
                                loadRequest.url.toString(),
                                factory.rootWindow.navigationFailureKind(loadRequest),
                                factory.rootWindow.navigationFailureDetail(loadRequest))
                        } else {
                            viewUi.navigation_failed()
                        }
                        if (tabIndex === viewUi.active_tab_index) {
                            factory.rootWindow.finishSiteDoctorAfterLoad(viewUi, false)
                        }
                    }
                }
                onPdfPrintingFinished: function(filePath, success) {
                    viewUi.finish_print_pdf(filePath, success)
                }
                onFileDialogRequested: function(request) {
                    if (!request) return
                    request.accepted = true
                    var host = viewHost || factory.rootWindow
                    if ((!viewTabs || tabIndex < 0 || tabIndex === viewUi.active_tab_index)
                            && !host.runtimeReleased && !host.runtimeDisposing
                            && host.handleFileDialogRequested) {
                        host.handleFileDialogRequested(request, webView, viewUi)
                    } else {
                        factory.rootWindow.resolveQtRequest(viewUi, request, "file-dialog", "dialogReject", [])
                    }
                }
                onPermissionRequested: function(permissionRequest) {
                    if (!viewTabs || tabIndex < 0 || tabIndex === viewUi.active_tab_index) {
                        factory.rootWindow.handlePermissionRequested(permissionRequest, tabIndex,
                            viewUi, webView, viewTransientProfile, viewHost)
                    } else {
                        factory.rootWindow.resolveQtRequest(viewUi, permissionRequest, "permission", "deny", [])
                    }
                }
                onJavaScriptDialogRequested: function(request) {
                    factory.rootWindow.handleJavaScriptDialogRequested(
                        viewUi, webView, request, viewTransientProfile, viewHost)
                }
                onAuthenticationDialogRequested: function(request) {
                    factory.rootWindow.handleAuthenticationDialogRequested(
                        viewUi, webView, request, viewTransientProfile, viewHost)
                }
                onSelectClientCertificate: function(selection) {
                    if (!viewTabs || tabIndex < 0 || tabIndex === viewUi.active_tab_index) {
                        factory.rootWindow.handleClientCertificateRequested(
                            viewUi, webView, selection, viewTransientProfile, viewHost)
                    } else {
                        factory.rootWindow.resolveQtRequest(viewUi, selection, "client-certificate", "selectNone", [])
                    }
                }
                onCertificateError: function(error) {
                    if (!viewTabs || tabIndex < 0 || tabIndex === viewUi.active_tab_index) {
                        factory.rootWindow.handleCertificateError(
                            viewUi, webView, error, viewHost)
                    } else {
                        factory.rootWindow.resolveQtRequest(viewUi, error, "client-certificate", "rejectCertificate", [])
                    }
                }
                onWebAuthUxRequested: function(request) {
                    if (!viewTabs || tabIndex < 0 || tabIndex === viewUi.active_tab_index) {
                        factory.rootWindow.handleWebAuthRequested(
                            viewUi, webView, request, viewHost)
                    } else {
                        factory.rootWindow.resolveQtRequest(viewUi, request, "webauth", "cancel", [])
                    }
                }
                onContextMenuRequested: function(request) {
                    if (!viewTabs || tabIndex < 0 || tabIndex === viewUi.active_tab_index) {
                        factory.rootWindow.handleContextMenuRequested(
                            viewUi, webView, request, viewHost)
                    } else {
                        request.accepted = true
                    }
                }
                onDesktopMediaRequested: function(request) {
                    factory.rootWindow.handleDesktopMediaRequested(
                        viewUi, webView, request, viewHost)
                }
                onFullScreenRequested: function(request) {
                    if (viewUi === factory.primaryUi && factory.primaryUi.mode === "grid") {
                        factory.primaryUi.spatial_invalidated("view-changed")
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
                    var lifecycle = lifecycleOwner()
                    if (!ownsLifecycleTab()) return

                    if (nativeUrl.length > 0) return
                    factory.rootWindow.handleRendererProcessTerminated(
                        lifecycle.ui, webView, lifecycle.host, lifecycle.index,
                        terminationStatus, exitCode)
                }
                Component.onDestruction: {
                    if (!(viewHost && viewHost.runtimeReleased) && !factory.rootWindow.runtimeDisposing) {
                        factory.rootWindow.cancelScriptsForView(webView)
                        factory.rootWindow.clearPageDialogForView(webView)
                        factory.rootWindow.clearClientCertificateForView(webView)
                        factory.rootWindow.clearCertificateErrorForView(webView)
                        factory.rootWindow.clearWebAuthForView(webView)
                        factory.rootWindow.clearContextMenuForView(webView)
                        factory.rootWindow.clearDesktopMediaForView(webView)
                        factory.rootWindow.clearCaptureSessionForView(webView)
                        if (viewHost && viewHost.clearFileDialogForView) viewHost.clearFileDialogForView(webView)
                        factory.rootWindow.clearRendererFailureForView(webView)
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
                    var popupProperties = {
                        popupRequest: request,
                        popupOrdinaryPage: request.destination !== WebEngineView.NewViewInDialog,
                        popupJourneyToken: popupToken,
                        popupProfile: viewProfile,
                        popupOpenerUi: viewUi,
                        popupContextName: viewUi.context_name,
                        popupContextLabel: viewUi.context_label,
                        popupPrivateProfile: viewTransientProfile,
                        popupEphemeralProfile: viewHost === factory.rootWindow ? factory.rootWindow.ephemeralProfile : viewHost.windowEphemeralProfile,
                        popupEphemeralInvocationToken: viewHost === factory.rootWindow ? factory.rootWindow.ephemeralInvocationToken : viewHost.windowEphemeralInvocationToken,
                        popupProfileName: viewHost === factory.rootWindow ? factory.rootWindow.profileName : viewHost.windowProfileName,
                        popupStorageBasePath: viewHost === factory.rootWindow ? factory.rootWindow.storageBasePath : viewHost.windowStorageBasePath
                    }
                    if (viewUi.feature_tabs_presentation_effective === "windows") {
                        if (request.destination === WebEngineView.NewViewInTab
                                || request.destination === WebEngineView.NewViewInBackgroundTab)
                            popupProperties.pageGroupId = viewHost.pageGroupId
                        if (request.destination === WebEngineView.NewViewInBackgroundTab)
                            popupProperties.windowInitialVisible = false
                    }
                    var popup = factory.popupFactory.createObject(null, popupProperties)
                    if (!popup || !popup.profileBootstrapComplete) {
                        viewUi.close_popup_tab(popupToken)
                        viewUi.status_text="The popup could not be opened. Try opening the link again."
                    }
                }
}

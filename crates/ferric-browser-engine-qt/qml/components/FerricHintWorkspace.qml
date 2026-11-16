import QtQuick
import QtQuick.Controls
import io.github.ferricbrowser 1.0
import "../scripts/BrowserScripts.js" as BrowserScripts
import "../scripts/TargetSearchPresentation.js" as TargetSearchPresentation
import "../scripts/HintPresentation.js" as HintPresentation

// One page-interaction workspace per native window. Rust retains selection,
// permits and action authority; this object owns its view leases and presentation.
Item {
    id: window
    required property var rootWindow
    required property var hostWindow
    required property var browserUi
    required property var activeView
    required property var appearance
    readonly property bool inputBlocked: hostWindow === rootWindow
        ? rootWindow.browserChromeInputActive : !hostWindow.browserKeyFocusActive
    readonly property bool hintInputOwnerActive: hostWindow.active && hostWindow.browserKeyFocusActive
    readonly property var pendingContextMenuRequest:
        rootWindow.pendingContextMenuUi === browserUi ? rootWindow.pendingContextMenuRequest : null
    readonly property int browserScriptWorld: rootWindow.browserScriptWorld
    readonly property string previewText: hintOverlaySurface.previewText
    property alias spatialAdapterEnabled: spatialPointerAdapter.enabled
    property alias spatialAdapterTargetItem: spatialPointerAdapter.targetItem
    property alias spatialAdapterSurfaceSerial: spatialPointerAdapter.surfaceSerial
    property alias spatialAdapterSurfaceRevision: spatialPointerAdapter.surfaceRevision
    z: 30

    function activeWebView() { return activeView }
    function runBrowserScript(view, script, callback, world) {
        return rootWindow.runBrowserScript(view, script, callback, world)
    }
    function showCommandNotice(text, error) { hostWindow.showCommandNotice(text, error) }
    function syncTabModel() { hostWindow.syncTabModel() }
    function executePendingEngineAction() {
        if (hostWindow === rootWindow) rootWindow.executePendingEngineAction()
        else { hostWindow.syncTabModel(); hostWindow.applySwitcherEngineAction() }
    }
    function openEphemeralWindow(url, token) { return rootWindow.openEphemeralWindow(url, token) }
    function ephemeralTokenForUi(ui) { return rootWindow.ephemeralTokenForUi(ui) }
    function openHintActionPalette(label, capabilities) {
        return rootWindow.openHintActionPalette(label, capabilities, window)
    }
    function clearContextMenuRequest() {
        if (rootWindow.pendingContextMenuUi === browserUi) rootWindow.clearContextMenuRequest()
    }

    property var hintResults: []
    property string hintPresentationSessionId: ""
    property var hintViewport: ({ width: 0, height: 0 })
    property string hintInput: ""
    property var hintState: ({ mode: "label", prefix: "", query: "", matching_labels: [], active_label: "", remaining: 0, total: 0 })
    property string targetSearchSelectedIdentity: ""
    property string targetSearchSelectedMeaning: ""
    property bool targetSearchSelectionInitialized: false
    property string hintScopeHandoffLeaseId: ""
    property var scopedGridHandoffView: null
    property string scopedGridHandoffLeaseId: ""
    property bool hintCollectionTruncated: false
    property int hintDirtyRevision: 0
    property bool hintRefreshPending: false
    property int hintCollectionEpoch: 0
    property var hintTrackingView: null
    property var hintCollectionRequest: null
    property var hintCollectionWorlds: []
    property string hintCollectionAnnotation: ""
    property int hintCollectionRetryCount: 0
    // Only input rejected because the initial Rust session does not yet exist.
    // These are gestures, never guessed labels or retained selection authority.
    property var pendingHintInputs: []
    property var pendingHintInputView: null
    property string pendingHintInputContext: ""
    property bool pendingHintProvisional: false
    property bool hintDomActivationPending: false
    readonly property alias hintFrameProvider: hintFrameProviderObject
    property bool rapidHintConfirmationVisible: false
    property string activeHintScopeLeaseId: ""
    property string activeSpatialScopeLeaseId: ""
    property var activeSpatialScopeView: null
    FerricHintFrameProvider { id: hintFrameProviderObject; browserWindow: window }
    FerricHintRequestAdapter { id: hintRequestAdapter }

    onHintInputOwnerActiveChanged: if (!hintInputOwnerActive) clearPendingHintInput()

    Connections {
        target: window.browserUi
        function onHint_invocationChanged() {
            targetSearchRefreshTimer.stop()
            window.hintCollectionEpoch += 1
            window.hintRefreshPending = false
            window.hintDomActivationPending = false
            window.clearPendingHintInput()
            window.hintPresentationSessionId = ""
            window.hintResults = []
            window.hintState = ({mode:"label",prefix:"",query:"",active_label:"",matching_labels:[],remaining:0,total:0})
            if(browserUi.mode === "hint")Qt.callLater(function(){if(browserUi.mode === "hint")window.startHintCollection()})
        }
        function onModeChanged() {
            window.clearPendingHintInput()
            if (browserUi.mode === "hint") {
                Qt.callLater(function() {
                    if (browserUi.mode === "hint") window.startHintCollection()
                })
            } else {
                targetSearchRefreshTimer.stop()
                // Navigation can retire the page callback while collection or
                // activation is pending. No pending token may block a new session.
                window.hintCollectionEpoch += 1
                window.hintRefreshPending = false
                window.hintDomActivationPending = false
                window.hintCollectionTruncated = false
                window.activeHintScopeLeaseId = ""
                window.stopHintTracking()
                window.rapidHintConfirmationVisible = false
                window.hintResults = []
                window.hintInput = ""
            }
        }
        function onHint_visibleChanged() {
            if (!browserUi.hint_visible) window.stopHintTracking()
        }
    }
    onActiveViewChanged: {
        clearPendingHintInput()
        if (hintTrackingView && activeView !== hintTrackingView) closeHints()
    }
    Component.onDestruction: {
        ++hintCollectionEpoch
        hintFrameProviderObject.stop()
    }

    function hintSelectorPolicySnapshot(family) {
        return browserUi.hint_selector_policy(family)
    }

    function hintCollectorScript(family, selectorSnapshot, scopeLeaseId, includeFacets, interactionRequest, deferInteraction, workBudget) {
        return BrowserScripts.hintCollector(
                    family, browserUi.hint_proxy_targets_enabled, selectorSnapshot,
                    scopeLeaseId, includeFacets, workBudget, interactionRequest, deferInteraction)
    }
    function hintFinishInteractionScript(interactionRequest, pendingId, workBudget) {
        return BrowserScripts.hintFinishInteraction(interactionRequest, pendingId, workBudget)
    }

    function targetSearchIdentity(candidate) {
        return TargetSearchPresentation.identity(candidate)
    }

    function targetSearchMeaning(candidate) {
        return TargetSearchPresentation.meaning(candidate)
    }

    function targetSearchRows() {
        return TargetSearchPresentation.rows(
                    window.hintResults,
                    window.hintState && window.hintState.matching_labels || [])
    }

    function reconcileTargetSearchSelection() {
        if (!browserUi.target_search_active) return
        var next = TargetSearchPresentation.reconcile(window.targetSearchRows(), {
            identity: window.targetSearchSelectedIdentity,
            meaning: window.targetSearchSelectedMeaning,
            initialized: window.targetSearchSelectionInitialized
        })
        window.targetSearchSelectedIdentity = next.identity
        window.targetSearchSelectedMeaning = next.meaning
        window.targetSearchSelectionInitialized = next.initialized
    }

    function applyTargetSearchQuery(query) {
        window.applyHintInteraction("set-query", String(query || ""))
        window.reconcileTargetSearchSelection()
    }

    function selectTargetSearchIdentity(identity) {
        var next = TargetSearchPresentation.select(window.targetSearchRows(), identity)
        if (!next) return
        window.targetSearchSelectedIdentity = next.identity
        window.targetSearchSelectedMeaning = next.meaning
        window.targetSearchSelectionInitialized = next.initialized
    }

    function moveTargetSearchSelection(delta) {
        var rows = window.targetSearchRows()
        if (rows.length === 0) return
        var next = TargetSearchPresentation.move(rows, {
            identity: window.targetSearchSelectedIdentity,
            meaning: window.targetSearchSelectedMeaning
        }, delta)
        if (!next) return
        window.targetSearchSelectedIdentity = next.identity
        window.targetSearchSelectedMeaning = next.meaning
        window.targetSearchSelectionInitialized = next.initialized
    }

    function confirmTargetSearchSelection() {
        var selected = TargetSearchPresentation.selectedRow(window.targetSearchRows(), {
            identity: window.targetSearchSelectedIdentity,
            meaning: window.targetSearchSelectedMeaning
        })
        if (!selected) return false
        window.activateHint(selected.label, "", true)
        return true
    }

    function hintDirtyRevisionScript() {
        return BrowserScripts.hintDirtyRevision()
    }

    function hintStopTrackingScript(scopeLeaseId) {
        return BrowserScripts.hintStopTracking(scopeLeaseId)
    }

    function spatialScopeGeometryScript(scopeLeaseId) {
        return BrowserScripts.spatialScopeGeometry(scopeLeaseId)
    }

    function hintFreshScript(candidate) {
        return BrowserScripts.hintFresh(
                    candidate, hintSelectorPolicySnapshot(browserUi.hint_family))
    }

    function completeHintScopeHandoff(view, request, candidate, actionEpoch) {
        if (!request || typeof request.lease_id !== "string"
                || window.activeWebView() !== view
                || window.hintTrackingView !== view
                || browserUi.mode !== "hint") {
            window.closeHints()
            return
        }
        window.hintScopeHandoffLeaseId = request.lease_id
        var finishHandoff = function() {
            if (window.hintScopeHandoffLeaseId === request.lease_id) window.hintScopeHandoffLeaseId = ""
        }
        var selectorSnapshot = window.hintSelectorPolicySnapshot(browserUi.hint_family)
        var acquireStarted = window.runBrowserScript(
                    view,
                    "JSON.stringify(" + BrowserScripts.hintAcquireScopeLease(request, candidate, selectorSnapshot) + ")",
                    function(receiptJson) {
            // Preserve explicit null fields across WebEngine's QVariant bridge.
            var receipt = null
            try { receipt = JSON.parse(receiptJson) } catch (error) {}
            if (window.hintCollectionEpoch !== actionEpoch
                    || window.activeWebView() !== view
                    || window.hintTrackingView !== view
                    || browserUi.mode !== "hint") {
                if (receipt && receipt.ok === true) {
                    window.runBrowserScript(
                                view, BrowserScripts.hintClearScopeLease(request.lease_id),
                                function(value) {})
                }
                finishHandoff()
                return
            }
            if (!receipt || receipt.ok !== true) {
                var reason = receipt && receipt.error === "weak-reference-unavailable"
                        ? "Hinting within this region isn't supported on this page. Try full-page Grid."
                        : "The selected region changed. Start hints again."
                window.closeHints()
                browserUi.status_text = reason
                window.showCommandNotice(reason, true)
                return
            }
            var stopStarted = window.runBrowserScript(
                        view, window.hintStopTrackingScript(request.lease_id), function(stopped) {
                if (window.hintCollectionEpoch !== actionEpoch
                        || window.activeWebView() !== view
                        || window.hintTrackingView !== view
                        || browserUi.mode !== "hint") {
                    window.runBrowserScript(
                                view, BrowserScripts.hintClearScopeLease(request.lease_id),
                                function(value) {})
                    finishHandoff()
                    return
                }
                if (stopped !== true
                        || !browserUi.complete_hint_scope_handoff(
                            request.lease_id, JSON.stringify(receipt))) {
                    window.runBrowserScript(
                                view, BrowserScripts.hintClearScopeLease(request.lease_id),
                                function(value) {})
                    window.closeHints()
                    browserUi.status_text = "The selected region changed. Start hints again."
                    window.showCommandNotice(browserUi.status_text, true)
                    return
                }
                window.activeHintScopeLeaseId = request.lease_id
                finishHandoff()
                window.hintResults = []
                window.hintInput = ""
                window.hintRefreshPending = false
                window.startHintCollection()
            })
            if (!stopStarted && window.hintCollectionEpoch === actionEpoch) {
                window.closeHints()
                browserUi.status_text = "Region hints couldn't reset. Start hints again."
                window.showCommandNotice(browserUi.status_text, true)
            }
        })
        if (!acquireStarted && window.hintCollectionEpoch === actionEpoch) {
            window.closeHints()
            browserUi.status_text = "Region hints couldn't start. Select the region again."
            window.showCommandNotice(browserUi.status_text, true)
        }
    }

    function completeGridScopeHandoff(view, request, candidate, actionEpoch) {
        if (!request || typeof request.lease_id !== "string"
                || window.activeWebView() !== view
                || window.hintTrackingView !== view
                || browserUi.mode !== "hint") {
            window.closeHints()
            return
        }
        window.scopedGridHandoffView = view
        window.scopedGridHandoffLeaseId = request.lease_id
        var finishSurfaceHandoff = function() {
            if (window.scopedGridHandoffLeaseId === request.lease_id) {
                window.scopedGridHandoffView = null
                window.scopedGridHandoffLeaseId = ""
            }
        }
        var selectorSnapshot = window.hintSelectorPolicySnapshot(browserUi.hint_family)
        var acquireStarted = window.runBrowserScript(
                    view,
                    "JSON.stringify(" + BrowserScripts.hintAcquireScopeLease(
                        request, candidate, selectorSnapshot) + ")", function(receiptJson) {
            var receipt = null
            try { receipt = JSON.parse(receiptJson) } catch (error) {}
            var clearLease = function() {
                finishSurfaceHandoff()
                window.runBrowserScript(
                            view, BrowserScripts.hintClearScopeLease(request.lease_id),
                            function(value) {})
            }
            if (window.hintCollectionEpoch !== actionEpoch
                    || window.activeWebView() !== view
                    || window.hintTrackingView !== view
                    || browserUi.mode !== "hint") {
                if (receipt && receipt.ok === true) clearLease()
                else finishSurfaceHandoff()
                return
            }
            if (!receipt || receipt.ok !== true) {
                finishSurfaceHandoff()
                window.closeHints()
                browserUi.status_text = receipt && receipt.error === "weak-reference-unavailable"
                        ? "Grid within this region isn't supported on this page. Try full-page Grid."
                        : "The selected region changed. Start hints again."
                window.showCommandNotice(browserUi.status_text, true)
                return
            }
            var stopStarted = window.runBrowserScript(
                        view, window.hintStopTrackingScript(request.lease_id), function(stopped) {
                if (window.hintCollectionEpoch !== actionEpoch
                        || window.activeWebView() !== view
                        || window.hintTrackingView !== view
                        || browserUi.mode !== "hint") {
                    clearLease()
                    return
                }
                if (stopped !== true || !window.spatialAdapterEnabled
                        || window.spatialAdapterTargetItem !== view
                        || view.width <= 0 || view.height <= 0) {
                    clearLease()
                    window.closeHints()
                    browserUi.status_text = "The page moved or changed. Select the region again."
                    window.showCommandNotice(browserUi.status_text, true)
                    return
                }
                var zoomFactor = Number(view.zoomFactor)
                var started = window.runBrowserScript(
                            view, window.spatialScopeGeometryScript(request.lease_id), function(value) {
                    if (window.hintCollectionEpoch !== actionEpoch
                            || window.activeWebView() !== view
                            || window.hintTrackingView !== view
                            || browserUi.mode !== "hint"
                            || window.spatialAdapterTargetItem !== view
                            || Number(view.zoomFactor) !== zoomFactor) {
                        clearLease()
                        return
                    }
                    var accepted = false
                    try {
                        accepted = browserUi.complete_grid_scope_handoff(
                                    request.lease_id, JSON.stringify(receipt),
                                    JSON.stringify(value || {}),
                                    window.spatialAdapterSurfaceSerial,
                                    window.spatialAdapterSurfaceRevision,
                                    view.width, view.height, zoomFactor)
                    } catch (error) {
                        accepted = false
                    }
                    if (!accepted) {
                        clearLease()
                        window.closeHints()
                        browserUi.status_text = "The selected region moved. Select it again."
                        window.showCommandNotice(browserUi.status_text, true)
                        return
                    }
                    finishSurfaceHandoff()
                    window.activeSpatialScopeLeaseId = request.lease_id
                    window.activeSpatialScopeView = view
                    window.activeHintScopeLeaseId = ""
                    window.hintTrackingView = null
                    window.hintCollectionEpoch += 1
                    window.hintResults = []
                    window.hintInput = ""
                    window.hintRefreshPending = false
                })
                if (!started) {
                    clearLease()
                    window.closeHints()
                    browserUi.status_text = "The page isn't responding. Try selecting the region again."
                    window.showCommandNotice(browserUi.status_text, true)
                }
            })
            if (!stopStarted && window.hintCollectionEpoch === actionEpoch) {
                clearLease()
                window.closeHints()
                browserUi.status_text = "Region Grid couldn't reset. Start hints again."
                window.showCommandNotice(browserUi.status_text, true)
            }
        })
        if (!acquireStarted && window.hintCollectionEpoch === actionEpoch) {
            finishSurfaceHandoff()
            window.closeHints()
            browserUi.status_text = "Region Grid couldn't start. Select the region again."
            window.showCommandNotice(browserUi.status_text, true)
        }
    }

    function hintPointerPreflightScript(candidate, viewWidth, viewHeight) {
        return BrowserScripts.hintPointerPreflight(
                    candidate, viewWidth, viewHeight,
                    hintSelectorPolicySnapshot(browserUi.hint_family))
    }

    function hintFocusScript(candidate) {
        return BrowserScripts.hintFocus(
                    candidate, hintSelectorPolicySnapshot(browserUi.hint_family))
    }

    function hintClickScript(candidate) {
        return BrowserScripts.hintClick(
                    candidate, hintSelectorPolicySnapshot(browserUi.hint_family))
    }

    function hintSetScrollTargetScript(elementId, framePath) {
        return BrowserScripts.hintSetScrollTarget(elementId, framePath)
    }

    function startHintCollection() {
        if (window.pendingContextMenuRequest && window.pendingContextMenuRequest.hintAction) return
        if (browserUi.hint_pointer_mode
                && browserUi.hint_pointer_state !== "selecting") {
            return
        }
        var view = window.activeWebView()
        if (!view) {
            window.closeHints()
            return
        }
        if (window.hintRefreshPending || targetSearchRefreshTimer.running) {
            return
        }
        if (window.hintTrackingView && window.hintTrackingView !== view) {
            window.closeHints()
            return
        }
        window.hintTrackingView = view
        var collectionEpoch = window.hintCollectionEpoch + 1
        window.hintCollectionEpoch = collectionEpoch
        window.hintRefreshPending = true
        var selectorSnapshot = window.hintSelectorPolicySnapshot(browserUi.hint_family)
        if (selectorSnapshot === '{"error":"selector-policy-pending"}') {
            // Reuse the existing collection timer and abstract loading input.
            // No page work or fallback policy is allowed before preparation.
            window.hintRefreshPending = false
            targetSearchRefreshTimer.interval = 16
            targetSearchRefreshTimer.restart()
            return
        }
        if (!selectorSnapshot) {
            window.hintRefreshPending = false
            window.closeHints()
            var selectorFailure = "Hint selectors unavailable"
            browserUi.status_text = selectorFailure
            window.showCommandNotice(selectorFailure, true)
            return
        }
        if(window.hintFrameProvider.view&&window.hintFrameProvider.view!==view)window.hintFrameProvider.stop()
        var resources=rootWindow.profileResourceRegistry.forProfile(view.profile)
        hintRequestAdapter.lifetime=resources?resources.lifetime:null
        var captured=hintRequestAdapter.beginCollection(view,window.hintFrameProvider.nativeBroker,
            window.hintFrameProvider.scopedOwner)
        window.hintCollectionRequest=null
        window.hintCollectionWorlds=[]
        window.hintCollectionAnnotation=""
        if(captured.state==="pending") {
            window.hintRefreshPending=false
            targetSearchRefreshTimer.interval=16
            targetSearchRefreshTimer.restart()
            return
        }
        var receive=function(value,request) {
            if (window.hintCollectionEpoch !== collectionEpoch) {
                return
            }
            window.hintRefreshPending = false
            if (browserUi.mode !== "hint" || window.hintTrackingView !== view) {
                return
            }
            if (value && value.error) {
                if((value.error==="interaction-collection-stale"||value.error==="frame-timeout"||value.retryable===true)
                        &&window.hintCollectionRetryCount===0) {
                    // Initial observers, navigation or a busy renderer can
                    // invalidate the first attempt. Capture one NEW request;
                    // the old inventory, receipts and grants are never reused.
                    window.hintCollectionRetryCount=1
                    if (value.cause === "frame-timeout" || value.error === "frame-timeout") {
                        // Expiry abandons the result, not the renderer script.
                        // Keep collection gated until that work settles, with a
                        // bounded wait owned by this invocation and document.
                        window.hintRefreshPending = true
                        window.hintFrameProvider.nativeBroker.waitForPendingScripts(function(settled) {
                            if (window.hintCollectionEpoch !== collectionEpoch
                                    || browserUi.mode !== "hint" || window.hintTrackingView !== view) return
                            window.hintRefreshPending = false
                            if (!settled) { receive(value, request); return }
                            targetSearchRefreshTimer.interval = 16
                            targetSearchRefreshTimer.restart()
                        })
                    } else {
                        targetSearchRefreshTimer.interval=16
                        targetSearchRefreshTimer.restart()
                    }
                    return
                }
                if(value.error==="interaction-collection-stale" && browserUi.hint_visible
                        && window.hintResults.length>0 && window.hintPresentationSessionId.length>0
                        && !browserUi.target_search_active && !browserUi.hint_pointer_mode) {
                    // Keep the last published labels while a changing page
                    // prevents passive refresh. Selection still requires fresh
                    // proof; no result from this failed request is published.
                    targetSearchRefreshTimer.interval=1000
                    targetSearchRefreshTimer.restart()
                    return
                }
                window.hintResults = []
                window.closeHints()
                // Keep the technical category in local logs, outside ordinary
                // browsing feedback. Never echo page-provided detail as a toast.
                console.warn("Hint collection:", String(value.error).slice(0, 256).replace(/[\x00-\x1f\x7f]/g, " "),
                    String(value.cause||"").slice(0,128).replace(/[\x00-\x1f\x7f]/g," "))
                browserUi.status_text = "Hints couldn't be collected. Try again, or use Grid."
                window.showCommandNotice(browserUi.status_text, true)
                return
            }
            window.hintCollectionRequest=request||null
            window.hintCollectionWorlds=value&&value.interaction_worlds||[]
            window.hintCollectionAnnotation=value&&value.interaction_annotation||""
            window.hintCollectionRetryCount=0
            window.hintDirtyRevision = Number(value && value.revision || 0)
            var wasTruncated = window.hintCollectionTruncated
            window.hintCollectionTruncated = !!(value && value.truncated)
            var collectedViewport = value && value.viewport ? value.viewport : ({})
            var viewportSnapshot = HintPresentation.viewportSnapshot(collectedViewport, window.hintViewport)
            if (viewportSnapshot !== window.hintViewport) window.hintViewport = viewportSnapshot
            var payload = value && value.candidates !== undefined
                    ? {candidates: value.candidates,
                       selector_revision: value.selector_revision}
                    : {candidates: value || []}
            if (window.activeHintScopeLeaseId.length > 0) {
                payload.scope_lease_id = window.activeHintScopeLeaseId
            }
            if(value&&typeof value.interaction_annotation==="string")payload.interaction_annotation=value.interaction_annotation
            var response = {}
            try {
                response = JSON.parse(request&&typeof value.interaction_annotation==="string"
                    ?hintRequestAdapter.publishCollection(request)
                    :browserUi.begin_hint_session(JSON.stringify(payload)))
            } catch (error) {
                response = {}
            }
            if(!response.error)window.pendingHintProvisional = false
            if (browserUi.target_search_active && !response.error) {
                window.hintState = response.state || window.hintState
                window.hintInput = window.hintState.query || ""
                window.hintResults = response.hints || []
                window.reconcileTargetSearchSelection()
                if (window.hintResults.length === 0) {
                    browserUi.status_text = window.hintCollectionTruncated
                            ? "Visible target collection reached its inspection limit"
                            : "No visible targets"
                }
                return
            }
            var reservedOnly = response.state && response.state.kind === "unavailable-only"
            if (!response.hints || response.hints.length === 0 && !reservedOnly) {
                var hintFailure = response.state && response.state.refresh_required
                        ? "New controls available—restart hints"
                        : window.hintCollectionTruncated
                        ? "No targets found in the inspected area. Narrow hints or use Grid."
                        : response.error
                        ? browserUi.status_text : "No visible hint targets"
                window.hintResults = []
                window.closeHints()
                browserUi.status_text = hintFailure
                window.showCommandNotice(hintFailure, true)
                return
            }
            var requiredRestart = !!(window.hintState && window.hintState.refresh_required)
            window.hintState = response.state || window.hintState
            window.hintInput = window.hintState.mode === "text"
                    ? "/" + window.hintState.query : window.hintState.prefix
            window.hintPresentationSessionId = String(response.session_id || "")
            window.hintResults = response.hints
            if (reservedOnly) {
                browserUi.status_text = "Policy codes are unavailable · start hints again, or Escape to cancel"
            } else if (window.hintState.refresh_required) {
                browserUi.status_text = "New controls available—restart hints"
                if (!requiredRestart) window.showCommandNotice(browserUi.status_text,false)
            } else if (window.hintCollectionTruncated) {
                var truncationNotice = "Some targets may be missing · narrow hints or select a region"
                browserUi.status_text = truncationNotice
                if (!wasTruncated) window.showCommandNotice(truncationNotice, false)
            }
            var replayedInput = window.replayPendingHintInput(collectionEpoch, view)
            if (response.initial_selection && !replayedInput) {
                var initial = response.initial_selection
                Qt.callLater(function(){
                    if(window.hintCollectionEpoch === collectionEpoch && browserUi.mode === "hint"
                        && window.activeWebView() === view && window.hintInputOwnerActive
                        && window.hintPresentationSessionId === initial.session_id)
                        window.activateHint("", "", false, initial)
                })
            } else if (response.first_label && !replayedInput) {
                var sessionId = window.hintPresentationSessionId
                Qt.callLater(function() {
                    if (window.hintCollectionEpoch === collectionEpoch
                            && browserUi.mode === "hint" && window.activeWebView() === view
                            && window.hintInputOwnerActive
                            && window.hintPresentationSessionId === sessionId)
                        window.activateHint(String(response.first_label))
                })
            }
        }
        if(!captured.request||(captured.state!=="ordinary"&&captured.state!=="proof-ready")) {
            receive({error:"interaction-policy-unavailable"})
            return
        }
        var options={family:browserUi.hint_family,proxy:browserUi.hint_proxy_targets_enabled,
            selectors:selectorSnapshot,scope:window.activeHintScopeLeaseId,pointer:browserUi.hint_pointer_mode,
            includeFacets:browserUi.target_search_active&&browserUi.target_search_scope==="all"}
        if(captured.state==="proof-ready") {
            try {options.selectors=JSON.parse(hintRequestAdapter.collectorSelectors(captured.request))}
            catch(error){receive({error:"selector-policy-unavailable"});return}
            window.hintFrameProvider.collectInteraction(view,options,hintRequestAdapter,captured.request,receive)
        } else {
            window.hintFrameProvider.collect(view,window.hintCollectorScript(options.family,
                options.selectors,options.scope,options.includeFacets),options,receive)
        }
    }

    function pollHintRefresh() {
        var view = window.activeWebView()
        if (!view || !browserUi.hint_visible || window.hintRefreshPending || targetSearchRefreshTimer.running
                || (browserUi.hint_pointer_mode
                    && browserUi.hint_pointer_state !== "selecting")) {
            return
        }
        window.hintFrameProvider.dirty(function(value) {
            // A palette captures one session. Refreshing underneath it would
            // invalidate its permit; final selection still checks fresh DOM.
            if (window.pendingContextMenuRequest && window.pendingContextMenuRequest.hintAction) return
            if (value && value.dirty === true
                    && Number(value.revision || 0) !== window.hintDirtyRevision) {
                window.startHintCollection()
            }
        })
    }

    function clearPendingHintInput() {
        if(window.pendingHintProvisional) {
            window.hintState = ({mode:"label",prefix:"",query:"",active_label:"",matching_labels:[],remaining:0,total:0})
            window.hintInput = ""
        }
        window.pendingHintProvisional = false
        if(window.pendingHintInputs.length > 0 || window.pendingHintInputView)
            browserUi.update_hint_interaction("discard-loading", "")
        window.pendingHintInputs = []
        window.pendingHintInputView = null
        window.pendingHintInputContext = ""
    }

    // Routing invokes this only after preparing an admitted page command.
    // It clears selection at commit; it never grants native delivery authority.
    function commitHintPageCommand(expectedContext) {
        if(!window.hintInputOwnerActive || browserUi.mode !== "hint"
            || browserUi.target_search_active || window.hintDomActivationPending
            || window.pendingContextMenuRequest || window.hintState.mode !== "label")return false
        var context = browserUi.hint_input_context()
        if (expectedContext !== undefined && expectedContext !== context) return false
        var response = JSON.parse(browserUi.commit_hint_page_command())
        if(!response.ok || context !== browserUi.hint_input_context())return false
        window.clearPendingHintInput()
        window.hintState = response.state || ({mode:"label",prefix:"",query:"",active_label:"",matching_labels:[],remaining:0,total:0})
        window.hintInput = ""
        // The page command can change the DOM while an older passive refresh
        // is in flight. Retire its callbacks and coalesce a fresh snapshot
        // after native delivery; old results must not close the Hint session.
        ++window.hintCollectionEpoch
        window.hintRefreshPending = false
        window.hintCollectionRetryCount = 0
        targetSearchRefreshTimer.interval = 80
        targetSearchRefreshTimer.restart()
        return true
    }

    // Native delivery calls this only after the admitted gesture's real release.
    function finishHintPageCommand(epoch, context) {
        if(!window.hintInputOwnerActive || browserUi.mode !== "hint"
                || browserUi.input_epoch !== epoch || browserUi.hint_input_context() !== context
                || browserUi.target_search_active || window.hintState.mode !== "label"
                || window.hintDomActivationPending || window.pendingContextMenuRequest)return
        window.closeHints()
    }

    function replayPendingHintInput(collectionEpoch, view) {
        var hadInputs = window.pendingHintInputs.length > 0
        var inputs = window.pendingHintInputView === view && window.pendingHintInputContext === browserUi.hint_input_context()
                ? window.pendingHintInputs : []
        window.clearPendingHintInput()
        for (var i = 0; i < inputs.length; ++i) {
            if (window.hintCollectionEpoch !== collectionEpoch || browserUi.mode !== "hint"
                    || window.activeWebView() !== view || window.hintTrackingView !== view
                    || !window.hintInputOwnerActive
                    || window.hintRefreshPending)
                break
            // A confirmation/auto-follow ends replay even if its asynchronous
            // action leaves Hint mode visible until fresh validation finishes.
            if (!window.applyHintInteraction(inputs[i].action, inputs[i].text, true)) break
        }
        return hadInputs
    }

    function applyHintInteraction(action, text, replaying) {
        if(window.hintDomActivationPending)return false
        var context = browserUi.hint_input_context()
        if(window.pendingHintInputContext && window.pendingHintInputContext !== context)window.clearPendingHintInput()
        var originalAction = action
        // Interpret text at dispatch, after any preceding queued edit. In
        // particular, spaces and a second slash belong to an early text query.
        if (action === "character" && window.hintState.mode === "label") {
            if (text === "/") action = "text-mode"
            else if (text === " ") action = "rotate"
        }
        var response = {}
        try {
            response = JSON.parse(browserUi.update_hint_interaction(action, text || ""))
        } catch (error) {
            return false
        }
        if (response.error === "no-session" && !replaying
                && browserUi.mode === "hint" && !browserUi.target_search_active
                && window.hintInputOwnerActive
                && window.activeWebView()
                && ["character", "text-mode", "backspace", "clear", "next", "previous", "activate", "rotate"].indexOf(originalAction) >= 0) {
            if (window.pendingHintInputs.length >= 256) {
                window.closeHints()
                window.showCommandNotice("Too much input while hints were loading. Start hints again.", true)
                return false
            }
            window.pendingHintProvisional = true
            window.pendingHintInputView = window.activeWebView()
            window.pendingHintInputContext = context
            window.pendingHintInputs.push({action:originalAction, text:text || ""})
            if(response.state) {
                window.hintState = response.state
                window.hintInput = response.state.mode === "text" ? "/" + response.state.query : response.state.prefix
            }
            return true
        }
        if (response.error) return false
        if (response.state) {
            window.hintState = response.state
            window.hintInput = response.state.mode === "text"
                    ? "/" + response.state.query : response.state.prefix
        }
        if (browserUi.target_search_active && response.outcome !== "activate") {
            window.reconcileTargetSearchSelection()
        }
        if (response.outcome === "activate") {
            window.activateHint(String(response.label || ""), "", true)
            return !replaying
        } else if (response.outcome === "cancel") {
            window.closeHints()
            return !replaying
        } else if (response.outcome === "rotate") {
            window.rotateHintCollision()
        }
        return true
    }

    function beginHintPointerConfirmation() {
        try {
            var response = JSON.parse(browserUi.hint_pointer_confirmation_pressed())
            if (response.ok !== true) return false
            // Freeze the displayed session for this explicit gesture. An older
            // passive callback must not replace its freshly checked authority.
            // Enter release still performs full DOM and native preflight.
            window.hintCollectionEpoch += 1
            window.hintRefreshPending = false
            return true
        } catch (error) {
            return false
        }
    }

    function rotateHintCollision() {
        var active = String(window.hintState.active_label || "")
        var labels = window.hintCollisionLabels(active)
        if (labels.length < 2) return
        var next = labels[(labels.indexOf(active)+1)%labels.length]
        window.applyHintInteraction("select",String(next))
        window.expandHintStack()
    }

    function closeHints() {
        window.hintScopeHandoffLeaseId = ""
        window.scopedGridHandoffView = null
        window.scopedGridHandoffLeaseId = ""
        if (window.pendingContextMenuRequest && window.pendingContextMenuRequest.hintAction) {
            window.clearContextMenuRequest()
        }
        window.hintCollectionEpoch += 1
        window.activeHintScopeLeaseId = ""
        window.stopHintTracking()
        window.hintRefreshPending = false
        window.hintDomActivationPending = false
        window.hintCollectionTruncated = false
        window.hintInput = ""
        // Clear targets before the viewport change can relayout them.
        window.hintResults = []
        window.hintViewport = ({ width: 0, height: 0 })
        window.hintState = ({ mode: "label", prefix: "", query: "", matching_labels: [], active_label: "", remaining: 0, total: 0 })
        window.targetSearchSelectedIdentity = ""
        window.targetSearchSelectedMeaning = ""
        window.targetSearchSelectionInitialized = false
        window.rapidHintConfirmationVisible = false
        browserUi.cancel_hints()
    }

    function stopHintTracking() {
        window.clearPendingHintInput()
        window.hintCollectionRequest=null
        window.hintCollectionWorlds=[]
        window.hintCollectionAnnotation=""
        window.hintCollectionRetryCount=0
        window.hintFrameProvider.stop()
        var view = window.hintTrackingView
        window.hintTrackingView = null
        if (view) {
            var preservedLease = window.hintScopeHandoffLeaseId || window.scopedGridHandoffLeaseId
            if (!preservedLease && browserUi.spatial_visible
                    && window.activeSpatialScopeView === view) {
                preservedLease = window.activeSpatialScopeLeaseId
            }
            window.runBrowserScript(view, window.hintStopTrackingScript(preservedLease), function(value) {})
        }
    }

    function policyHintFailure(response) {
        window.hintDomActivationPending=false
        window.hintRefreshPending=false
        if(browserUi.hint_pointer_mode)browserUi.hint_pointer_invalidated("preflight-rejected")
        try {window.hintResults=JSON.parse(browserUi.hint_values)}catch(error){}
        if(response&&response.state)window.hintState=response.state
        window.hintInput=""
        browserUi.status_text="That policy Hint changed. Start hints again to use its code."
        window.showCommandNotice(browserUi.status_text,true)
    }

    function preparePolicyHint(label,view,callback) {
        var token=window.hintCollectionEpoch
        var captured=hintRequestAdapter.beginSelection(view,window.hintFrameProvider.nativeBroker,
            String(label),window.hintFrameProvider.scopedOwner)
        if(captured.state==="ordinary"){callback(null);return}
        if(captured.state!=="proof-ready"||!captured.request){window.policyHintFailure({});return}
        view.forceActiveFocus()
        var request=captured.request
        var options={family:browserUi.hint_family,proxy:browserUi.hint_proxy_targets_enabled,
            scope:window.activeHintScopeLeaseId,pointer:browserUi.hint_pointer_mode,
            includeFacets:browserUi.target_search_active&&browserUi.target_search_scope==="all",
            selectionCheck:true}
        try{options.selectors=JSON.parse(hintRequestAdapter.collectorSelectors(request))}
        catch(error){window.policyHintFailure(JSON.parse(hintRequestAdapter.cancelSelection(request)));return}
        window.hintFrameProvider.collectInteraction(view,options,hintRequestAdapter,request,function(value){
            if(window.hintCollectionEpoch!==token||window.activeWebView()!==view
                ||window.hintTrackingView!==view||browserUi.mode!=="hint")return
            if(!value||value.error){
                window.policyHintFailure(JSON.parse(hintRequestAdapter.cancelSelection(request)))
                return
            }
            window.hintCollectionRequest=request
            window.hintCollectionWorlds=value.interaction_worlds||[]
            window.hintCollectionAnnotation=value.interaction_annotation||""
            callback(request)
        })
    }

    function confirmPolicyHint(request,callback) {
        if(!request){callback({ok:true});return}
        hintRequestAdapter.confirmSelection(request,function(raw){
            var response={}
            try{response=JSON.parse(raw)}catch(error){}
            callback(response)
        })
    }

    function showHintActions(label) {
        window.activateHint(label, "hint.choose")
    }

    function activateHint(label, actionId, localSelection, initialSelection) {
        if(window.hintDomActivationPending)return
        if (browserUi.hint_pointer_mode) {
            // Pointer mode treats marker activation as selection only. Enter
            // release owns the separate preflight and native dispatch gesture.
            window.applyHintInteraction("select", String(label))
            return
        }
        var selected = initialSelection ? initialSelection.candidate : null
        for (var i = 0; !initialSelection && i < window.hintResults.length; ++i) {
            if (window.hintResults[i].label === label) {
                selected = window.hintResults[i]
                break
            }
        }
        var view = window.activeWebView()
        var explicitSelection = localSelection === true || !!actionId
        if (!selected || !view || window.hintRefreshPending && !explicitSelection) {
            if (!selected || !view) {
                window.closeHints()
            } else if (actionId && actionId !== "hint.choose") {
                browserUi.cancel_hint_palette()
            }
            return
        }
        // A local confirmation freezes the displayed identity even if a passive
        // collection is pending. Its late result cannot overwrite this request;
        // the exact retained receiver still has to pass fresh validation below.
        if (window.hintRefreshPending) window.hintCollectionEpoch += 1
        // A keyboard/overlay selection is a new local palette entry. An
        // automatic --first completion cannot acquire that authority.
        if (!actionId && localSelection === true
                && (browserUi.hint_target === "choose"
                    || selected.kind === "region" || selected.kind === "scrollable-region"
                    || selected.kind === "surface" || selected.kind === "focusable-surface" || selected.kind === "frame")) {
            actionId = "hint.choose"
        }
        window.hintDomActivationPending = true
        var activationEpoch = window.hintCollectionEpoch
        var frameEntrySelectors = window.hintSelectorPolicySnapshot(browserUi.hint_family)
        window.hintRefreshPending = true
        function commitFresh(value) {
            if (window.hintCollectionEpoch !== activationEpoch
                    || window.activeWebView() !== view
                    || window.hintTrackingView !== view
                    || browserUi.mode !== "hint") {
                if (window.hintCollectionEpoch === activationEpoch) {
                    window.hintRefreshPending = false
                }
                return
            }
            // Consume this callback token before asking Rust to select the label.
            // A duplicate completion from the same page operation cannot dispatch twice.
            window.hintCollectionEpoch += 1
            var actionEpoch = window.hintCollectionEpoch
            var result = {}
            try {
                if(initialSelection) {
                    result = JSON.parse(browserUi.select_initial_hint(initialSelection.session_id,JSON.stringify(value)))
                } else if (actionId && actionId.length > 0) {
                    result = JSON.parse(browserUi.select_hint_action(
                                label, JSON.stringify(value), actionId))
                } else {
                    result = JSON.parse(browserUi.select_hint(label, JSON.stringify(value)))
                }
            } catch (error) {
                result = {}
            }
            if (result.error === "stale-selector-policy") {
                window.closeHints()
                browserUi.status_text = "Hint settings changed. Start hints again."
                window.showCommandNotice(browserUi.status_text, true)
                return
            }
            if(result.action==="yank"||result.action==="clean-yank") {
                var copied=browserUi.take_clipboard_request()
                if(copied.length>0)rootWindow.copyToClipboard(copied,
                    !browserUi.clipboard_request_sensitive,browserUi.clipboard_request_primary,browserUi)
            }
            var pageActionPending = result.action === "hint-inside-frame" || result.action === "focus"
                    || result.action === "click" || result.action === "scroll-target"
                    || result.action === "hint-within"
                    || result.action === "target-search-within"
                    || result.action === "grid-within"
            if (!pageActionPending && browserUi.mode !== "hint") {
                window.activeHintScopeLeaseId = ""
                window.stopHintTracking()
            }
            if (!pageActionPending) {
                window.hintDomActivationPending = false
                window.hintRefreshPending = false
            }
            if ((result.action === "hint-inside-frame" || result.action === "focus" || result.action === "click"
                 || result.action === "scroll-target" || result.action === "hint-within"
                 || result.action === "target-search-within"
                 || result.action === "grid-within")
                    && (window.hintCollectionEpoch !== actionEpoch
                        || window.activeWebView() !== view
                        || window.hintTrackingView !== view
                        || browserUi.mode !== "hint")) {
                if (window.hintCollectionEpoch === actionEpoch
                        && window.hintTrackingView === view) {
                    window.closeHints()
                }
                return
            }
            if (result.action === "hint-within"
                    || result.action === "target-search-within") {
                window.completeHintScopeHandoff(
                            view, result.scope_lease, value, actionEpoch)
            } else if (result.action === "grid-within") {
                window.completeGridScopeHandoff(
                            view, result.scope_lease, value, actionEpoch)
            } else if (result.action === "navigate") {
                window.hintResults = []
                window.hintInput = ""
                window.executePendingEngineAction()
            } else if (result.action === "choose") {
                var capabilities = result.capabilities
                if (selected.frame_owner && capabilities) capabilities = capabilities.filter(function(capability) {return capability.id === "hint-inside-frame" || capability.id === "focus"})
                window.openHintActionPalette(label, capabilities)
            } else if (result.action === "tab") {
                window.hintResults = []
                window.hintInput = ""
                // The engine has already inserted and selected the new tab.
                // Materialize its QML view before applying the queued load.
                window.syncTabModel()
                window.executePendingEngineAction()
            } else if (result.action === "window") {
                window.hintResults = []
                window.hintInput = ""
                window.executePendingEngineAction()
            } else if (result.action === "yank") {
                if (browserUi.mode === "hint") {
                    window.hintResults = window.hintResults.filter(function(candidate) {
                        return String(candidate.label) !== String(label)
                    })
                    Qt.callLater(window.startHintCollection)
                } else {
                    window.hintResults = []
                    window.hintInput = ""
                }
            } else if (result.action === "clean-yank") {
                if (browserUi.mode === "hint") {
                    window.hintResults = window.hintResults.filter(function(candidate) {
                        return String(candidate.label) !== String(label)
                    })
                    Qt.callLater(window.startHintCollection)
                } else {
                    window.hintResults = []
                    window.hintInput = ""
                }
            } else if (result.action === "tab-bg") {
                window.hintResults = []
                window.hintInput = ""
                window.syncTabModel()
                window.executePendingEngineAction()
                // Only rapid background hints remain in Hint mode. A one-shot
                // `;b` selection must return to Normal mode after opening.
                if (browserUi.mode === "hint") {
                    window.hintResults = window.hintResults.filter(function(candidate) {
                        return String(candidate.label) !== String(label)
                    })
                    Qt.callLater(window.startHintCollection)
                }
            } else if (result.action === "userscript") {
                if (browserUi.mode === "hint") {
                    window.hintResults = window.hintResults.filter(function(candidate) {
                        return String(candidate.label) !== String(label)
                    })
                    Qt.callLater(window.startHintCollection)
                } else {
                    window.hintResults = []
                    window.hintInput = ""
                }
            } else if (result.action === "download") {
                if (browserUi.mode === "hint") {
                    window.hintResults = window.hintResults.filter(function(candidate) {
                        return String(candidate.label) !== String(label)
                    })
                    Qt.callLater(window.startHintCollection)
                } else {
                    window.hintResults = []
                    window.hintInput = ""
                }
            } else if (result.action === "ephemeral") {
                window.hintResults = []
                window.hintInput = ""
                window.openEphemeralWindow(
                            result.url, window.ephemeralTokenForUi(browserUi))
            } else if (result.error === "download-request-rejected") {
                // Admission failed before consuming the target. Keep its
                // selection so Enter can retry after the busy request settles.
                window.showCommandNotice(browserUi.status_text, true)
            } else if (result.error === "rapid-tab-limit") {
                window.hintResults = []
                window.hintInput = ""
                window.rapidHintConfirmationVisible = true
            } else if (result.action === "hint-inside-frame") {
                window.hintFrameProvider.enter(value, frameEntrySelectors, function(entered) {
                    if (window.hintCollectionEpoch !== actionEpoch || window.activeWebView() !== view || browserUi.mode !== "hint") return
                    window.hintRefreshPending = false
                    if (!entered || !entered.ok) {
                        window.closeHints()
                        browserUi.status_text = "Cannot hint inside this frame; use Focus then pass-through, or Grid"
                        window.showCommandNotice(browserUi.status_text, true)
                        return
                    }
                    browserUi.cancel_hint_palette()
                    window.hintResults = []
                    window.startHintCollection()
                })
            } else if (result.action === "focus") {
                var focusScript = window.hintFocusScript(value)
                var focusStarted = window.hintFrameProvider.action(value, focusScript, function(value) {
                    if (window.hintCollectionEpoch !== actionEpoch
                            || window.activeWebView() !== view
                            || window.hintTrackingView !== view
                            || browserUi.mode !== "hint") {
                        if (window.hintCollectionEpoch === actionEpoch
                                && window.hintTrackingView === view) {
                            window.closeHints()
                        }
                        return
                    }
                    window.closeHints()
                    if (value !== true) {
                        browserUi.status_text = "Hint focus target is no longer available"
                        window.showCommandNotice(browserUi.status_text, true)
                        return
                    }
                    browserUi.enter_insert()
                })
                if (!focusStarted) {
                    if (window.hintCollectionEpoch !== actionEpoch) {
                        return
                    }
                    window.closeHints()
                    browserUi.status_text = "Hint focus target is no longer available"
                    window.showCommandNotice(browserUi.status_text, true)
                }
            } else if (result.action === "scroll-target") {
                var targetScript = window.hintSetScrollTargetScript(
                            result.element_id, result.frame_path)
                var targetStarted = window.runBrowserScript(view, targetScript, function(value) {
                    if (window.hintCollectionEpoch !== actionEpoch
                            || window.activeWebView() !== view
                            || window.hintTrackingView !== view
                            || browserUi.mode !== "hint") {
                        if (window.hintCollectionEpoch === actionEpoch
                                && window.hintTrackingView === view) {
                            window.closeHints()
                        }
                        return
                    }
                    window.closeHints()
                    var targetSelected = value && typeof value === "object"
                            && value.ok === true && value.policy === "element"
                    browserUi.status_text = targetSelected
                            ? "Scroll target: selected region"
                            : "Scroll target selection failed: region is no longer available"
                    window.showCommandNotice(browserUi.status_text, !targetSelected)
                }, window.browserScriptWorld)
                if (!targetStarted) {
                    if (window.hintCollectionEpoch !== actionEpoch) {
                        return
                    }
                    window.closeHints()
                    browserUi.status_text =
                            "Scroll target selection failed: region is no longer available"
                    window.showCommandNotice(browserUi.status_text, true)
                }
            } else if (result.action === "click") {
                var clickScript = window.hintClickScript(value)
                var clickStarted = window.hintFrameProvider.action(value, clickScript, function(value) {
                    if (window.hintCollectionEpoch !== actionEpoch
                            || window.activeWebView() !== view
                            || window.hintTrackingView !== view
                            || browserUi.mode !== "hint") {
                        if (window.hintCollectionEpoch === actionEpoch
                                && window.hintTrackingView === view) {
                            window.closeHints()
                        }
                        return
                    }
                    window.closeHints()
                    browserUi.status_text = value === true
                            ? "Hint target activated"
                            : value === false ? "Hint target is no longer clickable"
                            : "Activation result unavailable; check the page before trying again"
                    window.showCommandNotice(browserUi.status_text, value !== true)
                })
                if (!clickStarted) {
                    if (window.hintCollectionEpoch !== actionEpoch) {
                        return
                    }
                    window.closeHints()
                    browserUi.status_text = "Hint target is no longer clickable"
                    window.showCommandNotice(browserUi.status_text, true)
                }
            } else {
                window.hintResults = []
                window.hintInput = ""
                var rejectionStatus = browserUi.status_text
                window.closeHints()
                browserUi.status_text = rejectionStatus
            }
        }
        function prepared(request) {
            if(window.hintCollectionEpoch!==activationEpoch||window.activeWebView()!==view||browserUi.mode!=="hint")return
            var activationStarted=window.hintFrameProvider.fresh(selected,window.hintFreshScript(selected),function(value){
                window.confirmPolicyHint(request,function(response){
                    if(window.hintCollectionEpoch!==activationEpoch||window.activeWebView()!==view||browserUi.mode!=="hint")return
                    if(!response.ok){window.policyHintFailure(response);return}
                    commitFresh(value)
                })
            })
            if(!activationStarted&&window.hintCollectionEpoch===activationEpoch){
                if(request)window.policyHintFailure(JSON.parse(hintRequestAdapter.cancelSelection(request)))
                else {window.hintRefreshPending=false;window.closeHints()}
            }
        }
        if(initialSelection)prepared(null)
        else window.preparePolicyHint(label,view,prepared)
    }

    FerricPagePointerAdapter {
        id: spatialPointerAdapter
        enabled: browserUi.spatial_visible || browserUi.hint_pointer_pending
                 || window.scopedGridHandoffView !== null
        targetItem: window.scopedGridHandoffView !== null
                    ? window.scopedGridHandoffView
                    : (browserUi.spatial_visible || browserUi.hint_pointer_mode)
                      ? window.activeWebView() : null
        targetWindow: window.hostWindow
        inputBlocked: window.inputBlocked
        // S19 stays fail-closed until the native Wayland gesture gate passes.
        doubleClickEnabled: false
        onDispatchAcknowledged: function(requestId, sessionId, serial, revision, outcome) {
            if (browserUi.hint_pointer_pending) {
                browserUi.hint_pointer_dispatch_ack(
                            requestId, sessionId, serial, revision, outcome)
            } else {
                browserUi.spatial_dispatch_ack(
                            requestId, sessionId, serial, revision,
                            outcome === "unavailable" ? "rejected" : outcome)
                if (outcome === "unavailable") {
                    browserUi.status_text =
                            "Double-click is unavailable in Grid mode."
                }
            }
        }
        onInvalidated: function(reason) {
            if (browserUi.hint_pointer_pending) {
                browserUi.hint_pointer_invalidated(reason)
                return
            }
            // Initial target binding can advance the adapter stamp before the
            // first geometry observation. Once a root is installed, every
            // later invalidation is authoritative.
            if (browserUi.spatial_visible && browserUi.spatial_root_width > 0) {
                browserUi.spatial_invalidated(reason)
            }
        }
        onPhysicalPointerDetected: function(reason) {
            if (browserUi.hint_pointer_pending) {
                browserUi.hint_pointer_invalidated(reason)
            } else if (browserUi.spatial_visible) {
                browserUi.spatial_invalidated(reason)
            }
        }
        onSurfaceChanged: {
            if (browserUi.hint_pointer_pending) {
                browserUi.hint_pointer_invalidated("geometry-changed")
            } else if (browserUi.spatial_visible && browserUi.spatial_root_width > 0) {
                browserUi.spatial_invalidated("geometry-changed")
            }
        }
    }

    property bool spatialScopeGeometryPending: false

    function releaseSpatialScopeLease() {
        spatialScopeGeometryTimer.stop()
        spatialScopeDispatchTimer.stop()
        spatialScopeGeometryPending = false
        var leaseId = String(window.activeSpatialScopeLeaseId || "")
        var view = window.activeSpatialScopeView
        window.activeSpatialScopeLeaseId = ""
        window.activeSpatialScopeView = null
        if (leaseId.length > 0 && view) {
            window.runBrowserScript(
                        view, BrowserScripts.hintClearScopeLease(leaseId), function(value) {})
        }
    }

    Timer {
        id: spatialScopeGeometryTimer
        interval: 100
        repeat: true
        running: browserUi.spatial_visible
                 && browserUi.spatial_selecting
                 && browserUi.spatial_scope_lease_id.length > 0
        onTriggered: {
            if (window.spatialScopeGeometryPending) return
            var view = window.activeSpatialScopeView
            var leaseId = String(browserUi.spatial_scope_lease_id || "")
            var sessionId = String(browserUi.spatial_session_id || "")
            if (!view || leaseId.length === 0 || view !== window.activeWebView()
                    || spatialPointerAdapter.targetItem !== view) {
                browserUi.spatial_invalidated("target-changed")
                return
            }
            window.spatialScopeGeometryPending = true
            var started = window.runBrowserScript(
                        view, BrowserScripts.spatialScopeGeometry(leaseId), function(value) {
                window.spatialScopeGeometryPending = false
                if (!browserUi.spatial_visible
                        || String(browserUi.spatial_session_id || "") !== sessionId
                        || String(browserUi.spatial_scope_lease_id || "") !== leaseId
                        || window.activeSpatialScopeView !== view) {
                    return
                }
                browserUi.spatial_scope_geometry_validated(
                            "", JSON.stringify(value || {}),
                            spatialPointerAdapter.surfaceSerial,
                            spatialPointerAdapter.surfaceRevision,
                            view.width, view.height, Number(view.zoomFactor))
            })
            if (!started) {
                window.spatialScopeGeometryPending = false
                browserUi.spatial_invalidated("renderer-unavailable")
            }
        }
    }

    Timer {
        id: spatialScopeDispatchTimer
        interval: 250
        repeat: false
        onTriggered: browserUi.spatial_invalidated("renderer-unavailable")
    }

    Connections {
        target: browserUi
        function onSpatial_visibleChanged() {
            if (!browserUi.spatial_visible) {
                window.releaseSpatialScopeLease()
                return
            }
            var sessionId = browserUi.spatial_session_id
            Qt.callLater(function() {
                if (!browserUi.spatial_visible
                        || browserUi.spatial_session_id !== sessionId) {
                    return
                }
                var view = window.activeWebView()
                if (view && view.width > 0 && view.height > 0) {
                    browserUi.spatial_surface_ready(
                                view.width, view.height,
                                spatialPointerAdapter.surfaceSerial,
                                spatialPointerAdapter.surfaceRevision)
                } else {
                    browserUi.spatial_invalidated("surface-unavailable")
                }
            })
        }
        function onSpatial_dispatch_requestChanged() {
            var payload = browserUi.spatial_dispatch_request
            if (!payload || payload.length === 0) {
                return
            }
            var request
            try {
                request = JSON.parse(payload)
            } catch (error) {
                browserUi.spatial_invalidated("dispatch-rejected")
                return
            }
            // Keep `enabled` declaratively bound to the owning Grid or Hint
            // lifecycle so teardown releases the adapter's session. Never
            // assign here: that would replace the QML binding.
            if (!spatialPointerAdapter.enabled) {
                browserUi.spatial_invalidated("dispatch-rejected")
                return
            }
            var scopeLeaseId = String(request.scope_lease_id || "")
            if (scopeLeaseId.length > 0) {
                var view = window.activeSpatialScopeView
                var sessionId = String(request.session_id || "")
                var zoomFactor = view ? Number(view.zoomFactor) : NaN
                if (!view || view !== window.activeWebView()
                        || spatialPointerAdapter.targetItem !== view
                        || scopeLeaseId !== String(browserUi.spatial_scope_lease_id || "")) {
                    browserUi.spatial_invalidated("target-changed")
                    return
                }
                spatialScopeDispatchTimer.restart()
                var started = window.runBrowserScript(
                            view, BrowserScripts.spatialScopeGeometry(scopeLeaseId), function(value) {
                    spatialScopeDispatchTimer.stop()
                    if (!browserUi.spatial_visible
                            || String(browserUi.spatial_session_id || "") !== sessionId
                            || String(browserUi.spatial_scope_lease_id || "") !== scopeLeaseId
                            || window.activeWebView() !== view
                            || window.activeSpatialScopeView !== view
                            || spatialPointerAdapter.targetItem !== view
                            || browserUi.mode !== "grid"
                            || Number(view.zoomFactor) !== zoomFactor) {
                        browserUi.spatial_invalidated("target-changed")
                        return
                    }
                    if (!browserUi.spatial_scope_geometry_validated(
                                request.request_id, JSON.stringify(value || {}),
                                spatialPointerAdapter.surfaceSerial,
                                spatialPointerAdapter.surfaceRevision,
                                view.width, view.height, Number(view.zoomFactor))) {
                        return
                    }
                    spatialPointerAdapter.dispatch(
                                request.request_id, request.session_id,
                                request.serial, request.revision,
                                Number(request.x), Number(request.y), request.action)
                })
                if (!started) {
                    spatialScopeDispatchTimer.stop()
                    browserUi.spatial_invalidated("renderer-unavailable")
                }
                return
            }
            spatialPointerAdapter.dispatch(
                        request.request_id, request.session_id,
                        request.serial, request.revision,
                        Number(request.x), Number(request.y), request.action)
        }
        function onHint_pointer_dispatch_requestChanged() {
            var payload = browserUi.hint_pointer_dispatch_request
            if (!payload || payload.length === 0) return
            var request
            try {
                request = JSON.parse(payload)
            } catch (error) {
                browserUi.hint_pointer_invalidated("dispatch-rejected")
                return
            }
            if (!spatialPointerAdapter.enabled || !browserUi.hint_pointer_pending) {
                browserUi.hint_pointer_invalidated("dispatch-rejected")
                return
            }
            spatialPointerAdapter.dispatch(
                        request.request_id, request.session_id,
                        request.serial, request.revision,
                        Number(request.x), Number(request.y), request.action)
        }
    }

    Timer {
        id: hintPointerPreflightTimer
        interval: 250
        repeat: false
        onTriggered: browserUi.hint_pointer_timeout()
    }

    Timer {
        id: hintPointerDispatchTimer
        interval: 2100
        repeat: false
        onTriggered: browserUi.hint_pointer_timeout()
    }

    Connections {
        target: browserUi
        function onHint_pointer_stateChanged() {
            if (browserUi.hint_pointer_state !== "preflight") {
                hintPointerPreflightTimer.stop()
            }
            if (browserUi.hint_pointer_state !== "dispatch-pending") {
                hintPointerDispatchTimer.stop()
            }
        }
    }

    function releaseHintPointerConfirmation() {
        if (!browserUi.hint_pointer_mode || browserUi.hint_pointer_state !== "awaiting-confirmation-release") {
            return false;
        }
        var view = window.activeWebView();
        var label = String(window.hintState.active_label || "");
        var candidate = null;
        for (var index = 0; index < window.hintResults.length; ++index) {
            if (String(window.hintResults[index].label) === label) {
                candidate = window.hintResults[index];
                break;
            }
        }
        if (!view || !candidate || view !== window.hintTrackingView || spatialPointerAdapter.targetItem !== view || window.hintRefreshPending || view.width <= 0 || view.height <= 0) {
            browserUi.hint_pointer_invalidated("preflight-rejected");
            return true;
        }
        var releaseEpoch = window.hintCollectionEpoch;
        var releaseSerial = String(spatialPointerAdapter.surfaceSerial);
        var releaseRevision = String(spatialPointerAdapter.surfaceRevision);
        var releaseWidth = view.width, releaseHeight = view.height, releaseZoom = Number(view.zoomFactor);
        window.hintRefreshPending = true;
        function beginPreflight(policyRequest) {
            if (window.hintCollectionEpoch !== releaseEpoch || window.activeWebView() !== view || browserUi.hint_pointer_state !== "awaiting-confirmation-release" || String(spatialPointerAdapter.surfaceSerial) !== releaseSerial || String(spatialPointerAdapter.surfaceRevision) !== releaseRevision || view.width !== releaseWidth || view.height !== releaseHeight || Number(view.zoomFactor) !== releaseZoom) {
                window.hintRefreshPending = false;
                browserUi.hint_pointer_invalidated("target-changed");
                return;
            }
            var response = {};
            try {
                response = JSON.parse(browserUi.hint_pointer_confirmation_released(spatialPointerAdapter.surfaceSerial, spatialPointerAdapter.surfaceRevision, view.width, view.height));
            } catch (error) {
                response = {};
            }
            if (!response.ok || !response.request_id) {
                window.hintRefreshPending = false;
                return;
            }
            var requestId = String(response.request_id);
            var viewWidth = view.width;
            var viewHeight = view.height;
            var zoomFactor = Number(view.zoomFactor);
            hintPointerPreflightTimer.restart();
            var started = window.runBrowserScript(view, window.hintPointerPreflightScript(candidate, viewWidth, viewHeight), function (value) {
                window.confirmPolicyHint(policyRequest, function (policyResponse) {
                    if(window.hintCollectionEpoch!==releaseEpoch)return;
                    hintPointerPreflightTimer.stop();
                    window.hintRefreshPending = false;
                    if (browserUi.mode !== "hint" || !browserUi.hint_pointer_mode || browserUi.hint_pointer_state !== "preflight" || window.activeWebView() !== view || window.hintTrackingView !== view || spatialPointerAdapter.targetItem !== view || Number(view.zoomFactor) !== zoomFactor) {
                        if (browserUi.hint_pointer_state === "preflight") {
                            browserUi.hint_pointer_invalidated("target-changed");
                        }
                        return;
                    }
                    if (!policyResponse.ok) {
                        window.policyHintFailure(policyResponse);
                        return;
                    }
                    var accepted = false;
                    try {
                        accepted = browserUi.complete_hint_pointer_preflight(requestId, JSON.stringify(value || {}), spatialPointerAdapter.surfaceSerial, spatialPointerAdapter.surfaceRevision, view.width, view.height);
                    } catch (error) {
                        browserUi.hint_pointer_invalidated("preflight-rejected");
                        return;
                    }
                    if (accepted && browserUi.hint_pointer_state === "dispatch-pending") {
                        hintPointerDispatchTimer.restart();
                    }
                });
            });
            if (!started) {
                hintPointerPreflightTimer.stop();
                window.hintRefreshPending = false;
                browserUi.hint_pointer_invalidated("renderer-unavailable");
            }
        }
        window.preparePolicyHint(label, view, beginPreflight);
        return true;
    }

    FerricSpatialGridOverlay {
        id: spatialGridOverlay
        anchors.fill: parent
        z: 31
        browserUi: window.browserUi
        chromeScale: window.appearance.chromeScale
        labelColor: window.appearance.selectionTextColor
        labelBackground: window.appearance.selectionColor
        crosshairColor: window.appearance.accentColor
        doubleClickEnabled: spatialPointerAdapter.doubleClickEnabled
    }

    Timer {
        interval: 80
        repeat: true
        running: browserUi.hint_visible
        onTriggered: window.pollHintRefresh()
    }

    function hintCollisionLabels(label) { return hintOverlaySurface.collisionLabels(label) }
    function expandHintStack() { hintOverlaySurface.stackExpanded = true }

    FerricHintOverlay {
        id: hintOverlaySurface
        objectName: "hintOverlay"
        anchors.fill: parent
        browserWindow: window.appearance
        hintsVisible: browserUi.hint_visible && !browserUi.hint_pointer_pending
                      && !browserUi.target_search_active
        hintResults: window.hintResults
        sourceViewport: window.hintViewport
        hintState: window.hintState
        unmatchedPolicy: browserUi.hint_unmatched_policy
        markerScale: browserUi.hint_marker_scale
        interactionActive: browserUi.hint_visible
        onSelectionRequested: function(label) { window.applyHintInteraction("select",label) }
        onActivationRequested: function(label) { window.activateHint(label, "", true) }
        onActionsRequested: function(label) { window.showHintActions(label) }
    }

    FerricTargetSearch {
        id: targetSearchPopup
        objectName: "targetSearchPopup"
        browserWindow: window.appearance
        hostWindow: window.hostWindow
        searchVisible: browserUi.target_search_active
        query: String(window.hintState.query || "")
        scope: browserUi.target_search_scope
        rows: window.targetSearchRows()
        selectedIdentity: window.targetSearchSelectedIdentity
        selectedMeaning: window.targetSearchSelectedMeaning
        onOpening: window.rootWindow.captureOverlayFocus(window.hostWindow, window.activeWebView())
        onClosed: window.rootWindow.restoreOverlayFocus(window.hostWindow)
        onQueryEdited: function(value) { window.applyTargetSearchQuery(value) }
        onMoveRequested: function(delta) { window.moveTargetSearchSelection(delta) }
        onSelectionRequested: function(identity) { window.selectTargetSearchIdentity(identity) }
        onConfirmRequested: window.confirmTargetSearchSelection()
        onCancelRequested: browserUi.cancel_target_search()
    }

    Connections {
        target: browserUi
        function onTarget_search_activeChanged() {
            window.clearPendingHintInput()
            var searchActive = browserUi.target_search_active
            window.hintCollectionEpoch += 1
            window.hintRefreshPending = false
            if (browserUi.target_search_active) {
                window.targetSearchSelectedIdentity = ""
                window.targetSearchSelectedMeaning = ""
                window.targetSearchSelectionInitialized = false
            } else {
                window.targetSearchSelectedIdentity = ""
                window.targetSearchSelectedMeaning = ""
                window.targetSearchSelectionInitialized = false
            }
            targetSearchRefreshTimer.restart()
        }
    }

    Timer {
        id: targetSearchRefreshTimer
        interval: 0
        onTriggered: {
            interval = 0
            if (browserUi.mode !== "hint") return
            window.startHintCollection()
            if (browserUi.target_search_active) targetSearchPopup.focusInput()
        }
    }

    FerricRapidHintConfirmation {
        browserWindow: window.appearance
        hostWindow: window.hostWindow
        visible: window.rapidHintConfirmationVisible
        onContinueRequested: {
            if (browserUi.confirm_rapid_hint_tabs()) {
                window.rapidHintConfirmationVisible = false
                Qt.callLater(window.startHintCollection)
            }
        }
        onCancelRequested: {
            window.rapidHintConfirmationVisible = false
            window.closeHints()
        }
    }

}

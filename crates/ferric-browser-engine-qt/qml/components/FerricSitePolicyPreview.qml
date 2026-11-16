import QtQuick
import io.github.ferricbrowser 1.0
import "../scripts/BrowserScripts.js" as Scripts
import "../scripts/SitePolicyEditorSchema.js" as Schema

// Native inspection only. No Hint session, overlay selection or key submission.
Item {
    id: preview
    visible: false
    required property var hostWindow
    required property var browserUi
    required property var rootWindow
    property var activeView: null
    property int hintCollectionEpoch: 0
    readonly property alias hintFrameProvider: provider
    property string requestId: ""
    property int familyIndex: 0
    property bool proxy: false
    property int initialRetry: 0
    property var nativeRequest: null
    property string captureFamily: ""
    property bool captureReady: false
    readonly property var invocationFamilies: captureFamily ? [captureFamily] : Schema.families
    signal failed(string reason)
    signal captureStarted(string id)
    signal captureInventory(var rows)
    signal captureStopped()
    signal inspectionFinished()
    FerricHintFrameProvider { id: provider; browserWindow: preview }
    FerricHintRequestAdapter { id: adapter }
    function activeWebView() { return activeView }
    function runBrowserScript(view, script, callback, world) { return rootWindow.runBrowserScript(view, script, callback, world) }
    function hintFreshScript(candidate) { return Scripts.hintFresh(candidate, adapter.collectorSelectors(nativeRequest)) }
    function stop(cancelPending) {
        var stoppedView = activeView
        var closingId = requestId
        deadline.stop()
        restart.stop()
        hintCollectionEpoch++
        provider.stop()
        if (stoppedView && rootWindow) runBrowserScript(stoppedView, Scripts.hintStopTracking(), function() {})
        nativeRequest = null
        var wasCapture = captureFamily.length > 0
        requestId = ""; activeView = null; initialRetry = 0; captureFamily = ""; captureReady = false
        if (browserUi && (closingId || cancelPending !== false)) browserUi.site_policy_preview_cancel(closingId)
        if (wasCapture) captureStopped()
    }
    function startCapture(id, tab, useProxy, family) { start(id,tab,useProxy,family) }
    function start(id, tab, useProxy, capture) {
        if (!browserUi.site_policy_preview_pending(id)) return
        stop(false)
        if (!browserUi.site_policy_preview_pending(id)) return
        requestId = id; familyIndex = 0; proxy = useProxy; captureFamily = capture || ""
        var index = browserUi.tab_index_for_id(tab)
        activeView = index >= 0 ? hostWindow.tabViewAt(index) : null
        if (!activeView || activeView.stableTabId !== tab) {
            unavailableRemaining("The preview tab has no live view")
            return
        }
        var resources = rootWindow.profileResourceRegistry.forProfile(activeView.profile)
        adapter.lifetime = resources ? resources.lifetime : null
        if (captureFamily) captureStarted(id)
        else if(hostWindow.runtime_policyWorkspace.focusedPreview)
            rootWindow.runtime_focusOverlayController.contentFocusTarget(activeView).forceActiveFocus()
        next()
    }
    function unavailableRemaining(reason) {
        if (captureFamily) { failed(reason); stop(); return }
        var id = requestId, epoch = hintCollectionEpoch
        while (id === requestId && epoch === hintCollectionEpoch && requestId && familyIndex < Schema.families.length) {
            var result = JSON.parse(browserUi.site_policy_preview_failed(requestId, Schema.families[familyIndex], reason))
            if (id !== requestId || epoch !== hintCollectionEpoch) return
            familyIndex++
            if (result.error) break
        }
        failed(reason); stop();inspectionFinished()
    }
    function next() {
        if (!requestId || familyIndex >= invocationFamilies.length) { var complete=!!requestId;stop();if(complete)inspectionFinished();return }
        nativeRequest = null
        var id = requestId, epoch = hintCollectionEpoch, family = invocationFamilies[familyIndex]
        var captured = captureFamily ? adapter.beginCapture(activeView, provider.nativeBroker,id,family) : adapter.beginPreview(activeView, provider.nativeBroker, id, family)
        if (!captured.request || (captured.state !== "ordinary" && captured.state !== "proof-ready")) {
            unavailableRemaining("Native preview owner or document is unavailable")
            return
        }
        nativeRequest = captured.request
        function current() { return id === requestId && epoch === hintCollectionEpoch && family === invocationFamilies[familyIndex] }
        function publish() {
            if (!current()) return
            deadline.stop()
            var result = JSON.parse(adapter.publishPreview(nativeRequest))
            if (!current()) return
            if (result.error) { unavailableRemaining(result.error); return }
            if (captureFamily) {
                captureReady = true; captureInventory(result.rows || []); return
            }
            initialRetry = 0
            familyIndex++
            // Yield between independent invocations without recapturing policy.
            Qt.callLater(function() { if (id === requestId && epoch === hintCollectionEpoch) next() })
        }
        if (captured.state === "ordinary") { publish(); return }
        var selectors
        try { selectors = JSON.parse(adapter.collectorSelectors(nativeRequest)) }
        catch (error) { unavailableRemaining("Preview selector snapshot is unavailable"); return }
        deadline.restart()
        provider.collectInteraction(activeView, {family:family,proxy:proxy,selectors:selectors,scope:"",pointer:false,includeFacets:false},
            adapter, nativeRequest, function(result) {
                if (!current()) return
                if (result.error === "interaction-collection-stale" && initialRetry === 0) {
                    // Match the normal collector's one initial ResizeObserver
                    // restart: new request, receipts and work ledger; same draft.
                    initialRetry = 1
                    deadline.stop()
                    restart.restart()
                    return
                }
                if (result.error) { unavailableRemaining(result.error); return }
                publish()
            })
    }
    function chooseCapture(label, callback) {
        if (!captureFamily || !captureReady || !nativeRequest) { callback({error:"Capture is not ready"}); return }
        var id=requestId,epoch=hintCollectionEpoch,handle=nativeRequest
        var candidate=JSON.parse(adapter.builderCandidate(handle,label))
        if (candidate.error) { unavailableRemaining(candidate.error); callback(candidate); return }
        function current() { return id===requestId && epoch===hintCollectionEpoch && captureReady }
        provider.fresh(candidate,Scripts.hintCaptureFacts(candidate,adapter.collectorSelectors(handle)),function(fresh) {
            if (!current()) return
            if (!fresh || !fresh.visible || !fresh.capture_facts || fresh.error) {
                preview.unavailableRemaining("Capture changed before inspection; request a fresh sample")
                callback({error:"Capture target is stale"}); return
            }
            // collectInteraction already consumed this original request's one
            // native document validation. These are advisory facts, not a new
            // checked action proof or a renewable validation allowance.
            var geometryCurrent=fresh.geometry&&candidate.geometry&&["x","y","width","height"].every(function(key){return Math.abs(fresh.geometry[key]-candidate.geometry[key])<=0.75})
            var facts=adapter.collectionState(handle)==="proof-ready"&&geometryCurrent?fresh.capture_facts:{error:"Native owner or target geometry changed during capture"}
            if (!facts.error) { facts.geometry=fresh.geometry;facts.tab=activeView.stableTabId;facts.capture_id=id;facts.viewport={width:activeView.width,height:activeView.height} }
            preview.stop()
            callback(facts)
        })
    }
    Timer {
        interval: 250; repeat: true; running: preview.captureReady
        onTriggered: {
            var id=preview.requestId,epoch=preview.hintCollectionEpoch
            if (adapter.collectionState(preview.nativeRequest)!=="proof-ready") { preview.unavailableRemaining("Capture owner, tab or document changed"); return }
            provider.dirty(function(value) {
                if(id===preview.requestId && epoch===preview.hintCollectionEpoch && value && value.dirty)
                    preview.unavailableRemaining("Page membership, roots, slots or layout changed; request a fresh capture")
            })
        }
    }
    Connections {
        target: preview.hostWindow
        function onActiveChanged() { if (preview.captureFamily && !preview.hostWindow.active) preview.unavailableRemaining("Capture window lost focus") }
    }
    Timer { id: deadline; interval: 15000; onTriggered: preview.unavailableRemaining("Native preview timed out; request a fresh preview") }
    Timer { id: restart; interval: 16; onTriggered: preview.next() }
    Connections {
        target: preview.browserUi
        function onProfile_bootstrap_pendingChanged() { if (preview.browserUi.profile_bootstrap_pending) preview.stop() }
        function onModeChanged() { if (preview.browserUi.mode !== "normal" && preview.browserUi.mode !== "command") preview.stop() }
    }
    Component.onDestruction: stop()
}

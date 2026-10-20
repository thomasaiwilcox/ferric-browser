import QtQuick
import QtWebEngine
import io.github.ferricbrowser 1.0

// Native find owns matching, highlighting, scrolling, and frame traversal.
// Nothing in the typing path waits for renderer JavaScript or frame discovery.
Item {
    id: search
    required property var browserUi
    required property var hostWindow
    required property var activeView
    required property var appearance
    anchors.fill: parent
    z: 25
    property var view: null
    property var captured: null
    property int epoch: 0
    property int requestSerial: 0
    property int activeSerial: 0
    property int nativeActiveMatch: 0
    property string query: ""
    property var result: ({})
    property var pending: []
    property bool busy: false
    property bool accepted: false
    property bool following: false
    property var nativeTabSource: null
    property var nativeTabTarget: null
    readonly property bool retained: !!view && query.length > 0
    readonly property string previewText: !retained || (browserUi.mode !== "normal" && browserUi.mode !== "search") ? ""
        : following ? "Activating match…" : busy ? "Searching…"
        : result.found ? (accepted ? "Match · " + followKeys() : "Match · Enter: keep result")
        : "No matches"

    // Search feedback remains visible when the normal status bar is hidden.
    Rectangle {
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.rightMargin: 12
        anchors.bottomMargin: Number(search.hostWindow.bottomChromeHeight || 0) + 8
        width: Math.min(Math.max(0, parent.width-24), preview.implicitWidth+20)
        height: preview.implicitHeight+12
        radius: 5
        color: search.appearance.panelColor
        border.color: search.appearance.mutedTextColor
        visible: search.retained && search.previewText.length > 0 && search.hostWindow.statusBarVisible !== true
        Text {
            id: preview
            anchors.fill: parent
            anchors.margins: 6
            text: search.previewText
            textFormat: Text.PlainText
            color: search.appearance.primaryTextColor
            elide: Text.ElideMiddle
            Accessible.name: search.previewText
            Accessible.role: Accessible.StaticText
        }
    }

    FerricScriptRunner { id: selectionBarrier }
    Connections {
        target: search.view
        ignoreUnknownSignals: true
        function onLoadingChanged() { if (search.view && search.view.loading) search.stop() }
        function onUrlChanged() { search.stop() }
        function onRenderProcessTerminated() { search.stop() }
        function onFindTextFinished(value) { search.nativeActiveMatch = value.activeMatch }
    }
    Connections {
        target: search.browserUi
        function onModeChanged() {
            if (search.browserUi.mode === "search") search.stop()
            else if (search.browserUi.mode !== "normal" && search.browserUi.mode !== "command") search.stop()
        }
        function onSearch_textChanged() {
            if (!search.browserUi.search_text.length && search.browserUi.mode !== "search") search.stop()
        }
    }
    onActiveViewChanged: {
        if (view && activeView !== view) stop()
        if (nativeTabSource && activeView !== nativeTabSource) clearNativeTabGesture()
    }
    Component.onDestruction: stop()

    function current() {
        return !!view && view === activeView && !!captured
            && browserUi.request_target_current(captured.tabId, captured.generation, captured.documentId)
    }
    function followKeys() {
        var labels = []
        function available(key) {
            return (!browserUi.key_has_binding || !browserUi.key_has_binding(key))
                && (!browserUi.mixed_key_route || browserUi.mixed_key_route(key) !== "page")
        }
        if (available("Enter")) labels.push("Enter: activate")
        if (available("Shift+Enter")) labels.push("Shift+Enter: new tab")
        return labels.length ? labels.join(" · ") : "Enter follows your configured bindings"
    }
    Timer {
        id: activationDeadline
        interval: 750
        onTriggered: {
            search.stop()
            search.browserUi.status_text = "The page did not respond to search activation"
        }
    }
    Timer { id: nativeTabDeadline; interval: 1000; onTriggered: search.clearNativeTabGesture() }
    function clearNativeTabGesture() {
        nativeTabDeadline.stop()
        nativeTabSource = null
        nativeTabTarget = null
    }
    function stop(keepNativeSelection) {
        ++epoch
        activationDeadline.stop()
        selectionBarrier.cancelFor(search)
        clearNativeTabGesture()
        var oldView = view
        view = null
        captured = null
        pending = []
        query = ""
        result = ({})
        busy = false
        accepted = false
        following = false
        // Invalidate our state before Qt's synchronous cancellation callback.
        if (oldView && keepNativeSelection !== true) oldView.findText("")
    }
    function clear(selectedView) {
        if (view === selectedView) stop()
        else if (selectedView) selectedView.findText("")
    }
    function accept() { accepted = true }
    function sensitive(text, mode) {
        return mode === "sensitive" || (mode !== "insensitive" && Array.from(text).some(function(letter) {
            return letter.toUpperCase() === letter && letter.toLowerCase() !== letter
        }))
    }
    function flags(request) {
        var value = request.backward ? WebEngineView.FindBackward : 0
        if (sensitive(request.query, request.caseMode)) value |= WebEngineView.FindCaseSensitively
        return value
    }
    function execute(request, selectedView) {
        if (following) stop()
        // Deferred background-tab finds retain their captured target but do not
        // acquire a foreground window's activation gesture.
        if (selectedView !== activeView) {
            selectedView.findText(request.query, flags(request))
            return
        }
        if (!view || view !== selectedView || !captured
                || captured.generation !== request.generation || captured.documentId !== request.documentId) {
            stop()
            view = selectedView
            captured = Object.assign({}, request)
            accepted = browserUi.mode !== "search"
        }
        var work = Object.assign({}, request, {serial:++requestSerial})
        query = work.query
        result = ({})
        if (work.newSearch) {
            // Qt cancels superseded native finds. Dispatch the new query now,
            // even while the previous callback is pending; never queue typing.
            pending = []
            start(work)
        } else {
            pending = pending.concat([work])
            pump()
        }
    }
    function pump() {
        if (busy || !pending.length || !current()) return
        var work = pending[0]
        pending = pending.slice(1)
        start(work)
    }
    function start(work) {
        if (!current()) { stop(); return }
        busy = true
        activeSerial = work.serial
        var version = epoch, owner = view
        owner.findText(work.query, flags(work), function(count) {
            if (version !== epoch || activeSerial !== work.serial || view !== owner) return
            if (!current()) { stop(); return }
            busy = false
            result = pending.length ? {} : {found:Number(count)>0, count:Number(count), activeMatch:nativeActiveMatch}
            pump()
        })
    }
    function follow(newTab) {
        if (!retained || !accepted || browserUi.mode !== "normal") return false
        if (following) return true
        if (busy || pending.length || !current()
                || !browserUi.search_activation_ready(captured.tabId, captured.generation, captured.documentId)) {
            browserUi.status_text = "Search is still running or its source has changed"
            return true
        }
        if (!result.found) {
            browserUi.status_text = "No search match to activate"
            return true
        }
        var owner = view, router = hostWindow.browserKeyRouterObject, version = epoch
        if (!router) return true
        following = true
        activationDeadline.restart()
        // Convert Chromium's own active find marker to selection/focus. The
        // renderer round trip orders that handoff before the native key; sending
        // Enter immediately races the asynchronous StopFinding IPC.
        owner.findText("")
        var scheduled = selectionBarrier.run(search, owner.mainFrame, "true", WebEngineScript.ApplicationWorld, function(ready) {
            if (version !== epoch || !following) return
            var root = hostWindow.rootWindow || hostWindow
            if (ready !== true || !current() || browserUi.mode !== "normal"
                    || (root.nativePromptActiveFor && root.nativePromptActiveFor(browserUi))
                    || !browserUi.search_activation_ready(captured.tabId, captured.generation, captured.documentId)) {
                stop()
                return
            }
            var target = captured
            stop(true)
            if (newTab) {
                nativeTabSource = owner
                nativeTabTarget = target
                nativeTabDeadline.restart()
            }
            if (!router.activateSearchResult(owner, newTab)) {
                clearNativeTabGesture()
                browserUi.status_text = "Search result's page is no longer active"
            }
        })
        if (!scheduled) stop()
        return true
    }
    function acceptNewTabRequest(source, request) {
        if (source !== nativeTabSource || !nativeTabTarget || !request.userInitiated
                || request.destination !== WebEngineView.NewViewInTab) return false
        var target = nativeTabTarget
        clearNativeTabGesture()
        // Chromium supplies the destination of its actual keyboard activation.
        // Route this one foreground-tab gesture through Ferric's typed tab action
        // instead of treating it as the application's ordinary popup window.
        if (!browserUi.search_activation_ready(target.tabId, target.generation, target.documentId)) return true
        if (browserUi.execute_ui_action("browser.tab.open", request.requestedUrl.toString())) {
            if (hostWindow.executePendingEngineAction) hostWindow.executePendingEngineAction()
            else hostWindow.applySwitcherEngineAction()
        }
        return true
    }
}

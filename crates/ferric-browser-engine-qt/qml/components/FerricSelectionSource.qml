import QtQuick
import io.github.ferricbrowser 1.0
import "../scripts/BrowserScripts.js" as Scripts

// Metadata is observed while the page owns focus. Text is read only on demand.
// This lifetime belongs to one view and never follows the hint overlay.
Item {
    id: source
    visible: false
    required property var view
    required property var browserUi
    property int epoch: 0
    property bool capturing: false
    property bool captureAgain: false
    property var captured: null
    property var captureCallbacks: []
    property var captureFinish: null
    readonly property int leaseCount: broker.leaseCount

    FerricSelectionFrameBroker { id: broker }
    Connections {
        target: source.view
        ignoreUnknownSignals: true
        function onLoadingChanged() { if (source.view.loading) source.invalidate() }
        function onUrlChanged() { source.invalidate() }
        function onRenderProcessTerminated() { source.invalidate() }
        function onViewUiChanged() { source.browserUi = source.view.viewUi; source.invalidate() }
        function onTabIndexChanged() { source.invalidate() }
    }
    Connections {
        target: source.browserUi
        ignoreUnknownSignals: true
        function onActive_tab_indexChanged() { source.invalidate() }
    }
    onBrowserUiChanged: invalidate()

    function invalidate() {
        epoch += 1
        captured = null
        capturing = false
        captureAgain = false
        captureFinish = null
        captureDeadline.stop()
        broker.clear()
        var callbacks = captureCallbacks
        captureCallbacks = []
        for (var i = 0; i < callbacks.length; ++i) callbacks[i](null)
    }
    function tabIdentity() {
        if (!view || !browserUi) return ""
        if (view.clipboardPopupToken) return "popup:" + view.clipboardPopupToken
        var index = view.tabIndex >= 0 ? view.tabIndex : browserUi.active_tab_index
        return String(browserUi.tab_id_for_index(index))
    }
    function snapshot() { return captured }
    function capture(callback) {
        if (!view || view.loading || !view.mainFrame || !view.mainFrame.isValid) { if (callback) callback(null); return }
        if (callback) {
            if (captureCallbacks.length >= 8) { callback(null); return }
            captureCallbacks = captureCallbacks.concat([callback])
        }
        if (capturing) { captureAgain = true; return }
        capturing = true
        var expected = epoch
        var tab = tabIdentity()
        var completed = false
        function finish(chain) {
            if (completed || expected !== epoch) return
            completed = true
            captureDeadline.stop()
            captureFinish = null
            if (chain && tab === tabIdentity()) captured = {epoch: expected, tab: tab, chain: chain}
            capturing = false
            if (captureAgain) { captureAgain = false; Qt.callLater(source.capture) }
            var callbacks = captureCallbacks
            captureCallbacks = []
            for (var i = 0; i < callbacks.length; ++i) callbacks[i](chain ? captured : null)
        }
        captureFinish = finish
        captureDeadline.start()
        function descend(owner, chain) {
            if (completed || expected !== epoch) return
            broker.run(owner, Scripts.selectionMetadata(), function(state) {
                if (completed || expected !== epoch) return
                if (!state || state.error || !state.focused) { finish(null); return }
                var next = chain.concat([{owner: owner, identity: state.identity, revision: state.revision}])
                if (!state.frame) { finish(next); return }
                if (next.length >= 8) { finish(null); return }
                broker.focusedChild(owner, function(child) {
                    if (!child || child.error) { finish(null); return }
                    descend(child.owner, next)
                })
            })
        }
        broker.attach(view.mainFrame, function(root) {
            if (completed || expected !== epoch) return
            if (!root || root.error) { finish(null); return }
            descend(root.owner, [])
        })
    }
    function validate(capture, callback) {
        if (!capture || capture.epoch !== epoch || capture.tab !== tabIdentity()
                || !capture.chain.length || capture.chain.length > 8) {
            callback({error: "selection-source-unavailable"}); return
        }
        var expected = epoch
        var completed = false
        var timer = deadline.createObject(source)
        function finish(result) {
            if (completed) return
            completed = true
            timer.destroy()
            callback(result)
        }
        timer.expired.connect(function() { finish({error: "selection-source-timeout"}) })
        timer.start()
        function check(index) {
            if (expected !== epoch || capture.tab !== tabIdentity()) {
                finish({error: "selection-source-stale"}); return
            }
            var entry = capture.chain[index]
            broker.run(entry.owner, Scripts.selectionMetadata(), function(state) {
                if (completed) return
                if (expected !== epoch || !state || state.error || state.identity !== entry.identity
                        || (index === capture.chain.length - 1 && state.revision !== entry.revision)) {
                    finish({error: "selection-source-changed"}); return
                }
                if (index + 1 < capture.chain.length) check(index + 1)
                else finish({owner: entry.owner, revision: entry.revision, epoch: expected})
            })
        }
        check(0)
    }
    function extract(callback, capture) {
        var retained = capture || snapshot()
        var completed = false
        var timer = deadline.createObject(source, {interval: 2000})
        function finish(result) {
            if (completed) return
            completed = true
            timer.destroy()
            callback(result)
        }
        timer.expired.connect(function() { finish({error: "selection-extraction-timeout"}) })
        timer.start()
        validate(retained, function(result) {
            if (completed) return
            if (result.error) { finish(result); return }
            broker.run(result.owner, Scripts.selection(result.revision), function(value) {
                finish(result.epoch === epoch ? (value || {error: "selection-unavailable"})
                                             : {error: "selection-source-stale"})
            })
        })
    }
    Timer {
        id: captureDeadline
        interval: 1500
        repeat: false
        onTriggered: if (source.captureFinish) source.captureFinish(null)
    }
    Component {
        id: deadline
        Timer {
            interval: 1500
            repeat: false
            signal expired()
            onTriggered: expired()
        }
    }
}

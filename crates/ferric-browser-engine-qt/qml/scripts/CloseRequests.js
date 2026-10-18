.pragma library

// nativeUrl belongs to the browser-owned content layer, not the webpage URL.
// These views have no web document to inspect. Keep an open native editor's
// decision separate from an ordinary page's asynchronous form-state check.
function nativePageState(view) {
    if (!view || !view.nativeUrl) return null
    if (typeof view.ownsTab === "function" && !view.ownsTab())
        return {state: "unknown", reason: "This page changed before it could be checked. Review it before closing."}
    if (view.nativeUrl === "rb://blank") return {state: "clean", reason: ""}
    if (view.nativeUrl === "rb://start") {
        if (view.startPage && view.startPage.closeNeedsConfirmation)
            return {state: "dirty", reason: "A start-page editor is still open. Save or cancel it before closing."}
        return {state: "clean", reason: ""}
    }
    return {state: "unknown", reason: "This browser page couldn't be checked before closing."}
}

function probePages(views, stillCurrent, requestScript, completed) {
    var finished = false
    function finish(state, reason) {
        if (finished || !stillCurrent()) return
        finished = true
        completed({state: state, reason: reason})
    }
    function advance(index) {
        if (finished || !stillCurrent()) return
        // Native tabs need no asynchronous page work. Iterate them so a
        // large blank/start-page session does not consume one stack frame per tab.
        while (index < views.length) {
            var native = nativePageState(views[index])
            if (!native) break
            if (native.state !== "clean") { finish(native.state, native.reason); return }
            index += 1
        }
        if (index >= views.length) { finish("clean", ""); return }
        var view = views[index]
        var replied = false
        var admitted = requestScript(view, function(value) {
            if (replied || finished || !stillCurrent()) return
            replied = true
            if (value === "clean") advance(index + 1)
            else if (value === "dirty") finish("dirty", "A page has unsaved form or editor state.")
            else finish("unknown", "Ferric Browser could not verify every page before closing.")
        })
        if (!admitted && !replied)
            finish("unknown", "A page couldn't be checked before closing. Review it before continuing.")
    }
    advance(0)
}

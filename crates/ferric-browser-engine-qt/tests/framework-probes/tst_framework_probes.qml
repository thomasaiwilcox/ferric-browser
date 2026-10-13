import QtQuick
import QtTest
import QtWebEngine

TestCase {
    id: tests
    name: "FrameworkProbes"
    when: windowShown
    width: 640
    height: 480

    WebEngineView {
        id: page
        anchors.fill: parent
        property bool loaded: false
        onLoadingChanged: function(request) {
            loaded = request.status === WebEngineView.LoadSucceededStatus
        }
    }
    property bool done: false
    property var result

    function evaluate(source, timeout) {
        done = false
        page.runJavaScript(source, WebEngineScript.MainWorld, function(value) {
            result = value
            done = true
        })
        tryVerify(function() { return done }, timeout || 5000)
        return result
    }

    function test_T050_framework_api_conflict_undo_permission_prototype() {
        page.loaded = false
        page.url = "http://127.0.0.1:8765/framework-probes.html"
        tryVerify(function() { return page.loaded }, 30000)
        tryVerify(function() {
            var state = evaluate("document.body.dataset.ready")
            return state === "ready" || state === "failed"
        }, 120000)
        var report = evaluate("window.__s22 || null")
        console.log("S22_T050_RESULT " + JSON.stringify(report))
        verify(report && report.status === "ready", "Framework fixture did not complete")
        compare(report.results.length, 6)
        for (var i = 0; i < report.results.length; i++) {
            var framework = report.results[i]
            verify(!framework.error,
                   framework.name + " fixture error: " + (framework.error || ""))
            verify(framework.focusApi, framework.name + " focus API did not accept the request")
            if (framework.name === "tiptap") {
                compare(framework.focusReached, false,
                        "offscreen fixture should retain the Tiptap DOM-focus limitation")
            } else {
                verify(framework.focusReached,
                       framework.name + " public focus API did not reach the editor")
            }
            verify(framework.extract, framework.name + " documented content API mismatch")
            verify(framework.focusNoExtract && framework.extractDenied,
                   framework.name + " focus/read consent boundary was not independent")
            verify(framework.permissionNoOp,
                   framework.name + " permission guard allowed a mutation")
            verify(framework.staleRejected,
                   framework.name + " stale revision changed content")
            verify(framework.ownerChanged && framework.documentChanged &&
                   framework.revisionStable && framework.ownerReplacementRejected,
                   framework.name + " exact owner replacement was not rejected")
            verify(framework.apply && framework.changed,
                   framework.name + " public apply API did not make the bounded edit")
            verify(framework.selectionPreserved,
                   framework.name + " did not preserve the end selection")
            if (framework.formatPreserved === "not_applicable") {
                verify(framework.name === "ace" || framework.name === "codemirror" ||
                       framework.name === "monaco",
                       framework.name + " unexpectedly omitted rich-format coverage")
            } else {
                verify(framework.formatPreserved,
                       framework.name + " did not preserve the unrelated mark/format state")
            }
            verify(framework.undoAvailable && framework.undoRestored,
                   framework.name + " apply was not restored through framework undo")
        }
    }
}

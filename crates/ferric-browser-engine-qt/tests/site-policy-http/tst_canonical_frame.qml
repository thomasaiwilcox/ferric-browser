import QtQuick
import QtTest
import QtWebEngine
import "../../qml/scripts/BrowserScripts.js" as BrowserScripts

TestCase {
    name: "CanonicalFrameSubject"
    when: windowShown
    width: 900
    height: 600
    property bool done: false
    property var result
    WebEngineView {
        id: page
        anchors.fill: parent
        property bool loaded: false
        onLoadingChanged: function(request) { loaded = request.status === WebEngineView.LoadSucceededStatus }
    }
    function evaluate(frame, source, world) {
        done = false
        frame.runJavaScript(source, world, function(value) { result = value; done = true })
        tryVerify(function() { return done }, 5000)
        return result
    }
    function test_focused_canonical_foreign_subject() {
        page.url = "http://127.0.0.1:8766/canonical-parent.html"
        tryVerify(function() { return page.loaded }, 15000)
        var child
        tryVerify(function() {
            var children = page.mainFrame.children
            if (children.length !== 1) return false
            child = children[0]
            return child.isValid && String(child.url) === "http://localhost:8766/canonical-child.html"
        }, 10000)
        compare(evaluate(child, "location.origin", WebEngineScript.MainWorld), "http://localhost:8766")
        compare(evaluate(page.mainFrame, "document.querySelector('iframe').contentDocument === null", WebEngineScript.MainWorld), true)
        compare(evaluate(child, "document.querySelector('#receiver').focus(); document.activeElement.id", WebEngineScript.ApplicationWorld), "receiver")
        compare(evaluate(page.mainFrame, "document.activeElement.id", WebEngineScript.MainWorld), "foreign")
        // Collect while the foreign receiver is focused. Top-document ordinary
        // discovery must not gain authority over that frame's DOM.
        var top = evaluate(page.mainFrame, BrowserScripts.hintCollector("links"), WebEngineScript.ApplicationWorld)
        compare(top.candidates.filter(function(c) { return c.text === "Foreign primary" }).length, 0)
        compare(top.candidates.filter(function(c) { return c.text === "Top primary" }).length, 1)
        // Public frame-scoped script execution establishes the test subject;
        // this is not a production broker authorization or routing claim.
        var local = evaluate(child, BrowserScripts.hintCollector("links"), WebEngineScript.ApplicationWorld)
        compare(local.candidates.filter(function(c) { return c.text === "Foreign primary" }).length, 1)
        compare(evaluate(child, "window.fixtureActivations", WebEngineScript.MainWorld), 0)
        evaluate(child, BrowserScripts.hintStopTracking(), WebEngineScript.ApplicationWorld)
        evaluate(page.mainFrame, BrowserScripts.hintStopTracking(), WebEngineScript.ApplicationWorld)
    }
}

import QtQuick
import QtTest
import QtWebEngine
import "../../qml/scripts/BrowserScripts.js" as BrowserScripts

TestCase {
    id: hintTest
    name: "HintGeometry"
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

    property bool probeDone: false
    property var probeResult

    function evaluate(source) {
        probeDone = false
        page.runJavaScript(source, WebEngineScript.ApplicationWorld, function(value) {
            probeResult = value
            probeDone = true
        })
        tryVerify(function() { return probeDone }, 5000)
        return probeResult
    }

    function loadFixture(rootStyle, bodyStyle, content) {
        page.loaded = false
        page.loadHtml("<!doctype html><html><head><style>"
                      + "html { margin: 0; " + rootStyle + " }"
                      + "body { margin: 0; " + bodyStyle + " }"
                      + "#app { position: absolute; top: 0; left: 0; width: 600px; height: 800px; }"
                      + "a, button { display: block; width: 160px; height: 30px; }"
                      + "</style></head><body><div id='app'>"
                      + content + "</div></body></html>", "https://fixture.test/")
        tryVerify(function() { return page.loaded }, 5000)
        // Wait for hit testing to see the new document's rendered contents.
        tryVerify(function() {
            return evaluate("document.elementFromPoint(10,10)?.id === 'target'") === true
        }, 5000)
    }

    function test_body_overflow_propagates_to_viewport() {
        loadFixture("", "overflow-x: hidden; overflow-y: scroll;",
                    "<a id='target' href='/watch?v=fixture'>Video</a>"
                    + "<button id='control' onclick='this.dataset.clicked=1'>Menu</button>"
                    + "<div style='overflow:hidden;width:180px;height:40px'>"
                    + "<button style='margin-top:30px'>Partial</button>"
                    + "<a href='/hidden' style='margin-top:60px'>Clipped</a></div>"
                    + "<a href='/offscreen' style='position:absolute;top:700px'>Offscreen</a>"
                    + "<div aria-label='Results' style='overflow:auto;width:180px;height:50px'>"
                    + "<div style='height:200px'>Scrollable content</div></div>")
        compare(evaluate("document.body.getBoundingClientRect().height"), 0)
        var links = evaluate(BrowserScripts.hintCollector("links")).candidates
        compare(links.length, 1)
        compare(links[0].text, "Video")
        verify(evaluate(BrowserScripts.hintFresh(links[0])).visible)

        var buttons = evaluate(BrowserScripts.hintCollector("buttons")).candidates
        compare(buttons.length, 2)
        var partial = buttons.filter(function(candidate) { return candidate.text === "Partial" })[0]
        verify(partial)
        compare(partial.geometry.height, 10)
        compare(evaluate(BrowserScripts.hintFresh(partial)).geometry.height, 10)
        var control = buttons.filter(function(candidate) { return candidate.text === "Menu" })[0]
        verify(evaluate(BrowserScripts.hintClick(control.element_id)))
        compare(evaluate("document.getElementById('control').dataset.clicked"), "1")

        var scrollables = evaluate(BrowserScripts.hintCollector("scrollables")).candidates
        var results = scrollables.filter(function(candidate) { return candidate.text === "Results" })[0]
        verify(results)
        verify(evaluate(BrowserScripts.hintFresh(results)).visible)
        verify(evaluate(BrowserScripts.hintSetScrollTarget(results.element_id, results.frame_path)).ok)

        var all = evaluate(BrowserScripts.hintCollector("all")).candidates
        verify(all.some(function(candidate) { return candidate.text === "Video" }))
        var target = all.filter(function(candidate) { return candidate.text === "Video" })[0]
        evaluate("document.getElementById('target').style.visibility='hidden';true")
        verify(!evaluate(BrowserScripts.hintFresh(target)).visible)
    }

    function test_root_overflow_propagates_to_viewport() {
        loadFixture("overflow:hidden;", "overflow:visible;",
                    "<a id='target' href='/root'>Root overflow</a>")
        compare(evaluate("document.documentElement.getBoundingClientRect().height"), 0)
        var links = evaluate(BrowserScripts.hintCollector("links")).candidates
        compare(links.length, 1)
        verify(evaluate(BrowserScripts.hintFresh(links[0])).visible)
    }

    function test_body_remains_clipping_box_when_root_owns_overflow() {
        loadFixture("overflow:auto;", "overflow:hidden;height:40px;",
                    "<a id='target' href='/visible'>Visible</a>"
                    + "<a href='/clipped' style='margin-top:40px'>Clipped by body</a>")
        var links = evaluate(BrowserScripts.hintCollector("links")).candidates
        compare(links.length, 1)
        compare(links[0].text, "Visible")
        verify(evaluate(BrowserScripts.hintFresh(links[0])).visible)
    }

    function test_contained_body_remains_a_clipping_box() {
        loadFixture("", "overflow:hidden;height:40px;contain:paint;",
                    "<a id='target' href='/visible'>Visible</a>"
                    + "<a href='/clipped' style='margin-top:40px'>Clipped by containment</a>")
        var links = evaluate(BrowserScripts.hintCollector("links")).candidates
        compare(links.length, 1)
        compare(links[0].text, "Visible")
        verify(evaluate(BrowserScripts.hintFresh(links[0])).visible)
    }

    function test_same_origin_frame_uses_its_own_viewport() {
        loadFixture("", "overflow-y:scroll;",
                    "<a id='target' href='/top'>Top</a>"
                    + "<iframe id='child' style='width:300px;height:100px;border:0'></iframe>")
        var childHtml = "<!doctype html><style>body{margin:0;overflow-y:scroll}"
                + "div{position:absolute;top:0;left:0}</style>"
                + "<div><a id='frame-target' href='/frame'>Frame video</a></div>"
        evaluate("document.getElementById('child').srcdoc=" + JSON.stringify(childHtml) + ";true")
        tryVerify(function() {
            return evaluate("document.getElementById('child').contentDocument"
                            + "?.elementFromPoint(5,5)?.id === 'frame-target'") === true
        }, 5000)
        var links = evaluate(BrowserScripts.hintCollector("links")).candidates
        compare(links.length, 2)
        var child = links.filter(function(candidate) { return candidate.text === "Frame video" })[0]
        verify(child)
        compare(child.frame_path, "0.0")
        verify(evaluate(BrowserScripts.hintFresh(child)).visible)
        evaluate("document.getElementById('child').style.visibility='hidden';true")
        verify(!evaluate(BrowserScripts.hintFresh(child)).visible)
    }
}

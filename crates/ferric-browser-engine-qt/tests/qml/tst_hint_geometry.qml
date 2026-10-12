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
                    + "<div id='results' aria-label='Results' style='overflow:auto;width:180px;height:50px'>"
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
        verify(evaluate(BrowserScripts.hintClick(control)))
        compare(evaluate("document.getElementById('control').dataset.clicked"), "1")

        evaluate("document.getElementById('results').style.pointerEvents='none';true")
        var scrollables = evaluate(BrowserScripts.hintCollector("scrollables")).candidates
        var results = scrollables.filter(function(candidate) { return candidate.text === "Results" })[0]
        verify(results)
        verify(evaluate(BrowserScripts.hintFresh(results)).visible)
        verify(evaluate(BrowserScripts.hintSetScrollTarget(results.element_id, results.frame_path)).ok)
        evaluate(BrowserScripts.scroll("scroll", "down", false, 1))
        tryVerify(function() { return evaluate("document.getElementById('results').scrollTop") > 0 })

        var all = evaluate(BrowserScripts.hintCollector("all")).candidates
        verify(all.some(function(candidate) { return candidate.text === "Video" }))
        var target = all.filter(function(candidate) { return candidate.text === "Video" })[0]
        evaluate("document.getElementById('target').style.visibility='hidden';true")
        verify(!evaluate(BrowserScripts.hintFresh(target)).visible)
    }

    function test_pointer_preflight_requires_the_retained_uncovered_target() {
        loadFixture("", "",
                    "<button id='target' aria-label='Pointer target' "
                    + "style='position:absolute;left:0;top:0;width:160px;height:40px'>"
                    + "Pointer</button>")
        var candidates = evaluate(BrowserScripts.hintCollector("buttons")).candidates
        compare(candidates.length, 1)
        var candidate = candidates[0]
        var fresh = evaluate(BrowserScripts.hintPointerPreflight(
                                 candidate, page.width, page.height))
        verify(fresh.ok, "uncovered retained control should be pointer eligible")
        verify(fresh.x >= 0 && fresh.x < page.width)
        verify(fresh.y >= 0 && fresh.y < page.height)
        compare(evaluate("document.elementFromPoint(" + fresh.css_x + "," + fresh.css_y + ").id"), "target")

        evaluate("var cover=document.createElement('div');cover.id='cover';" +
                 "cover.style='position:fixed;inset:0;z-index:9999;background:transparent';" +
                 "document.body.append(cover);true")
        verify(!evaluate(BrowserScripts.hintPointerPreflight(
                             candidate, page.width, page.height)).ok,
               "a covering page element must reject preflight")

        evaluate("document.getElementById('cover').remove();" +
                 "document.getElementById('target').replaceWith(" +
                 "document.getElementById('target').cloneNode(true));true")
        verify(!evaluate(BrowserScripts.hintPointerPreflight(
                             candidate, page.width, page.height)).ok,
               "a replacement with the same attributes cannot inherit retained authority")
    }

    function test_pointer_coordinates_map_css_viewport_to_qt_logical_view_at_zoom() {
        loadFixture("", "",
                    "<button id='target' aria-label='Pointer target' "
                    + "style='position:absolute;left:0;top:0;width:160px;height:40px'>"
                    + "Pointer</button>")
        var candidate = evaluate(BrowserScripts.hintCollector("buttons")).candidates[0]
        for (var zoom of [0.5, 1, 2]) {
            page.zoomFactor = zoom
            tryVerify(function() {
                return Math.abs(evaluate("window.innerWidth") - page.width / zoom) < 4
            }, 5000)
            var result = evaluate(BrowserScripts.hintPointerPreflight(
                                     candidate, page.width, page.height))
            verify(result.ok, "visible target should remain eligible at zoom " + zoom)
            verify(Math.abs(result.x - result.css_x * page.width / result.viewport_width) < 0.75)
            verify(Math.abs(result.y - result.css_y * page.height / result.viewport_height) < 0.75)
        }
        page.zoomFactor = 1
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
        var pointer = evaluate(BrowserScripts.hintPointerPreflight(
                                   child, page.width, page.height))
        verify(pointer.ok, "untransformed same-origin frame target should map to the view")
        evaluate("document.getElementById('child').style.visibility='hidden';true")
        verify(!evaluate(BrowserScripts.hintFresh(child)).visible)
    }
}

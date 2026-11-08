import QtQuick
import QtTest
import QtWebEngine
import "../../qml/scripts/BrowserScripts.js" as BrowserScripts
import "../qml-hints-v2/FixtureHarness.js" as Harness

// Real WebEngine differential tests. The immutable baseline is exported with
// git show by tools/performance/check_hint_cold_path.py, outside the checkout.
TestCase {
    id: tests
    name: "HintCollectorCache"
    when: windowShown
    width: 1000; height: 800
    property string baselinePageSource: ""
    property string baselineBrowserSource: ""
    property bool done: false
    property var result: null
    WebEngineView { id: baseline; width: 1000; height: 800; property bool loaded: false
        onLoadingChanged: function(request) { loaded = request.status === WebEngineView.LoadSucceededStatus } }
    WebEngineView { id: candidate; width: 1000; height: 800; property bool loaded: false
        onLoadingChanged: function(request) { loaded = request.status === WebEngineView.LoadSucceededStatus } }

    function read(url) {
        var finished = false, value = "", problem = ""
        Harness.readText(url, function(error, text) { problem = error || ""; value = text; finished = true })
        tryVerify(function() { return finished }, 5000)
        verify(!problem, problem)
        return value
    }
    function evaluate(page, source) {
        done = false; result = null
        page.runJavaScript(source, WebEngineScript.ApplicationWorld, function(value) { result = value; done = true })
        tryVerify(function() { return done }, 30000)
        return result
    }
    function loadPair(html) {
        baseline.loaded = false; candidate.loaded = false
        baseline.loadHtml(html, "https://hint-cache.invalid/")
        candidate.loadHtml(html, "https://hint-cache.invalid/")
        tryVerify(function() { return baseline.loaded && candidate.loaded }, 15000)
        for (var page of [baseline, candidate]) {
            tryVerify(function() {
                return evaluate(page, "Array.from(document.querySelectorAll('iframe')).every(f=>f.contentDocument&&f.contentDocument.readyState==='complete')") === true
            }, 10000)
        }
    }
    function source(original, family, policy, budget, facets) {
        if (!original) return BrowserScripts.hintCollector(family || "all", true, policy || null, "", !!facets, budget || null)
        var p = baselinePageSource.replace(/^\.pragma library\s*/, "")
        var b = baselineBrowserSource.replace(/^\.pragma library\s*/, "").replace(/^\.import[^\r\n]*\r?\n/gm, "")
        var invocation = "return hintCollector(" + JSON.stringify(family || "all")
                + ",true," + JSON.stringify(policy || null) + ",''," + !!facets + "," + JSON.stringify(budget || null) + ");"
        return new Function("PageInteractionScripts", b + invocation)(new Function(p + "\nreturn {VERSION:VERSION,hintCollector:hintCollector};")())
    }
    function compareCollectors(family, policy, budget, facets) {
        var left = evaluate(baseline, Harness.snapshotScript(source(true, family, policy, budget, facets)))
        var right = evaluate(candidate, Harness.snapshotScript(source(false, family, policy, budget, facets)))
        verify(left && !left.error, JSON.stringify(left))
        verify(right && !right.error, JSON.stringify(right))
        compare(JSON.stringify(Harness.canonical(right)), JSON.stringify(Harness.canonical(left)), family + " matches original")
        return right
    }
    function initTestCase() {
        failOnWarning(/ReferenceError|TypeError/)
        verify(testArtifactDirectory.length > 0, "run tools/performance/check_hint_cold_path.py")
        baselinePageSource = read(Harness.fileUrl(testArtifactDirectory + "/PageInteractionScripts.js"))
        baselineBrowserSource = read(Harness.fileUrl(testArtifactDirectory + "/BrowserScripts.js"))
        compare(Harness.gitBlobSha1(baselinePageSource), Harness.PAGE_INTERACTION_BLOB)
        compare(Harness.gitBlobSha1(baselineBrowserSource), Harness.BROWSER_SCRIPTS_BLOB)
    }
    function test_fixtures_data() {
        return [{tag:"mixed", file:"basic.html"}, {tag:"adversarial", file:"adversarial.html"},
                {tag:"shadow-frame", file:"shadows-frames.html"}, {tag:"caps", file:"caps.html"}]
    }
    function test_fixtures(data) {
        var fixture = read(Qt.resolvedUrl("../qml-hints-v2/fixtures/" + data.file))
        if (data.file === "basic.html") fixture = Harness.composeFixture(fixture, read(Qt.resolvedUrl("../qml-hints-v2/fixtures/basic-frame.html")))
        loadPair(fixture)
        for (var family of ["all", "actions", "links", "buttons", "inputs", "scrollables", "regions", "surfaces"])
            compareCollectors(family)
        compareCollectors("all", null, {inspections: 30, candidates: 5})
        compareCollectors("all", null, null, true)
    }
    function test_live_changes() {
        loadPair("<!doctype html><style>body{margin:0} .contents{display:contents}"
                 + ".clip{width:180px;height:60px;overflow:hidden;transform:translate(10px,5px)}"
                 + "button{width:100px;height:24px}</style><div class='clip' id='panel' data-hv2-id='panel'>"
                 + "<div class='contents'><button id='target' data-hv2-id='target'>Target</button></div>"
                 + "<div style='height:200px'></div></div><div style='visibility:hidden'>"
                 + "<button style='visibility:visible' data-hv2-id='override'>Visible child</button></div>"
                 + "<div style='pointer-events:none'><button style='pointer-events:auto' data-hv2-id='pointer'>Pointer child</button></div>")
        var first = compareCollectors("all")
        for (var id of ["target", "override", "pointer"])
            verify(first.candidates.some(function(row) { return row.fixture_id === id }), "fixture admits " + id)
        var policy = {selector_revision:"cache-policy", policies:{all:{policy:{mode:"replace", selectors:["#panel:has(button)","button"]}}}}
        compareCollectors("all", policy)
        var edits = ["document.getElementById('target').style.transform='translateX(30px)'",
                     "document.getElementById('panel').style.overflow='auto'",
                     "document.getElementById('panel').scrollTop=40",
                     "document.getElementById('target').disabled=true",
                     "document.getElementById('target').disabled=false;document.getElementById('panel').style.display='none'",
                     "document.getElementById('panel').style.display='block';document.getElementById('panel').inert=true"]
        var changed = false
        for (var edit of edits) {
            for (var page of [baseline, candidate]) evaluate(page, edit + ";true")
            var next = compareCollectors("all")
            changed = changed || JSON.stringify(Harness.canonical(first)) !== JSON.stringify(Harness.canonical(next))
            compareCollectors("scrollables")
            compareCollectors("regions")
        }
        verify(changed, "mutations exercise fresh geometry and eligibility")
        compareCollectors("all", policy)
    }
    function grid(count) {
        var html = "<!doctype html><style>html,body{margin:0;overflow:hidden}body{display:grid;grid-template-columns:repeat(100,9px)}"
                + "button{padding:0;border:0;width:8px;height:12px;font:8px sans-serif}</style>"
        for (var i = 0; i < count; i++) html += "<button data-hv2-id='b" + i + "'>x</button>"
        return html
    }
    function test_read_counts_data() { return [{tag:"flat",nested:false},{tag:"nested",nested:true}] }
    function test_read_counts(data) {
        var html = grid(500)
        if (data.nested) {
            html = "<!doctype html><style>body{margin:0;display:grid;grid-template-columns:repeat(50,18px)}button{padding:0;border:0;width:8px;height:12px}"
                    + "[role=button]{width:18px;height:14px}</style>"
            for (var i=0;i<500;i++) html += "<div role='button' data-hv2-id='outer"+i+"'><button data-hv2-id='inner"+i+"'>x</button></div>"
        }
        loadPair(html)
        function instrument(page) {
            evaluate(page, "window.__reads={rect:0,style:0};const rect=Element.prototype.getBoundingClientRect,style=window.getComputedStyle;"
                     + "Element.prototype.getBoundingClientRect=function(){window.__reads.rect++;return rect.call(this)};"
                     + "window.getComputedStyle=function(){window.__reads.style++;return style.apply(this,arguments)};true")
        }
        instrument(baseline); instrument(candidate)
        compareCollectors("buttons")
        var before = evaluate(baseline, "window.__reads"), after = evaluate(candidate, "window.__reads")
        console.info("HINT_COLD_READS " + JSON.stringify({before:before,after:after,count:data.nested ? 1000 : 500,nested:data.nested}))
        if (data.nested) verify(after.rect < before.rect, "reuse nested target rectangles")
        else verify(after.rect <= before.rect, "flat controls introduce no extra rectangle reads")
        verify(after.style < before.style, "avoid repeated scrollability style reads")
    }
    function test_cache_lifetime() {
        loadPair(grid(10))
        function observed(script) {
            var anchor = "try{const ftFrameIndexCache="
            compare(script.split(anchor).length, 2, "one cache-observation seam")
            return script.replace(anchor, "window.__lastHintCache=visibilityStyleCache;" + anchor)
        }
        function released() {
            verify(evaluate(candidate, "Object.values(window.__lastHintCache).every(v=>v===null)"),
                   "observer-retained contexts must not retain pass caches")
        }
        var script = observed(source(false, "buttons"))
        compare(evaluate(candidate, script).candidates.length, 10)
        released()
        var stale = observed(BrowserScripts.hintCollector("all", true, null, "missing-scope", false, null))
        compare(evaluate(candidate, stale).error, "stale-scope")
        released()
        var anchor = "visit(startRoot,startPath,startX,startY,0,startClip,false);"
        compare(script.split(anchor).length, 2)
        script = script.replace(anchor, "throw new Error('test collection failure');")
        compare(evaluate(candidate, "(function(){try{return " + script + ";}catch(e){return e.message;}})()"), "test collection failure")
        released()
    }
    function test_timings_data() { return [{tag:"100",count:100},{tag:"500",count:500},{tag:"1000",count:1000},{tag:"5000",count:5000}] }
    function test_timings(data) { measure(data) }
    function test_hidden_timings_data() { return [{tag:"hidden",count:1000,hidden:true},{tag:"offscreen",count:1000,offscreen:true}] }
    function test_hidden_timings(data) { measure(data) }
    function measure(data) {
        loadPair(grid(data.count))
        if (data.hidden || data.offscreen) {
            var edit = data.hidden ? "Array.from(document.querySelectorAll('button')).forEach(e=>e.style.display='none')"
                                   : "Array.from(document.querySelectorAll('button')).slice(500).forEach(e=>e.style.transform='translateY(2000px)')"
            for (var page of [baseline,candidate]) evaluate(page, edit + ";true")
        }
        compareCollectors("buttons")
        function timed(page, script) {
            return evaluate(page, "(function(){const start=performance.now();const r=" + script
                            + ";return {ms:performance.now()-start,count:r.candidates.length};})()")
        }
        var original = source(true, "buttons"), current = source(false, "buttons")
        for (var warm = 0; warm < 4; warm++) { timed(candidate, original); timed(candidate, current) }
        var before = [], after = []
        for (var sample = 0; sample < 20; sample++) {
            var a, b
            if (sample % 2) { b = timed(candidate, current); a = timed(candidate, original) }
            else { a = timed(candidate, original); b = timed(candidate, current) }
            var expected = data.hidden ? 0 : data.offscreen ? 500 : data.count
            compare(a.count, expected); compare(b.count, expected)
            before.push(a.ms); after.push(b.ms)
        }
        console.info("HINT_COLD_TIMING " + JSON.stringify({count:data.count,hidden:!!data.hidden,offscreen:!!data.offscreen,before_ms:before,after_ms:after,
            boundary:"uninstrumented renderer collection; offscreen/software; same document and renderer; alternating order; not key-to-paint latency"}))
    }
}

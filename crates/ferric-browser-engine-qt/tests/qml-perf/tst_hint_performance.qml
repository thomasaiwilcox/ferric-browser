import QtQuick
import QtQuick.Window
import QtTest
import QtWebEngine
import Ferric.Testing 1.0
import "../../qml/scripts/BrowserScripts.js" as BrowserScripts
import "../../qml/components"

// Explicit opt-in benchmark, outside default functional test discovery.
// The page calls the production ApplicationWorld script builder; a test-only
// source transform adds phase clocks without changing production resources.
TestCase {
    id: testCase
    name: "HintPerformance"
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

    Window {
        id: overlayWindow
        width: 640
        height: 480
        visible: true

        Item {
            id: paletteHost
            anchors.fill: parent
            property color warningColor: "#f0c54a"
            property color primaryTextColor: "#202020"
            property color backgroundColor: "#ffffff"
            function contrastText(color) { return "#202020" }

            FerricHintOverlay {
                id: hintOverlay
                browserWindow: paletteHost
                hintsVisible: false
                hintResults: []
                sourceViewport: ({width: 640, height: 480})
                hintState: ({mode: "label", prefix: "", matching_labels: [], active_label: ""})
                unmatchedPolicy: "show"
                markerScale: 1
                anchors.fill: parent
                z: 30
            }
        }
    }

    property bool done: false
    property var result
    property double elapsedNsecs: 0

    function runTimed(source) {
        done = false
        result = null
        var started = PerformanceClock.nowNsecs()
        page.runJavaScript(source, WebEngineScript.ApplicationWorld, function(value) {
            result = value
            elapsedNsecs = PerformanceClock.nowNsecs() - started
            done = true
        })
        tryVerify(function() { return done }, 30000)
        return {value: result, round_trip_ms: elapsedNsecs / 1000000.0}
    }

    function evaluate(source) {
        return runTimed(source).value
    }

    function replaceOnce(source, needle, replacement, stage) {
        var first = source.indexOf(needle)
        if (first < 0 || source.indexOf(needle, first + needle.length) >= 0)
            throw new Error("T043 instrumentation anchor mismatch: " + stage)
        return source.slice(0, first) + replacement + source.slice(first + needle.length)
    }

    function replaceAll(source, needle, replacement) {
        var parts = source.split(needle)
        return parts.join(replacement)
    }

    function profiledCollectorScript(source) {
        // Keep this transform test-only. Its timings are an instrumented view
        // of the exact generated collector, and the raw total is compared with
        // the uninstrumented preliminary run to show instrumentation overhead.
        source = replaceAll(source, "ftHintMatchesAny(el,selectors)",
                            "__profileMatch(el,selectors)")
        source = replaceAll(source, "ftHintMatchesAny(el,scrollSelectors)",
                            "__profileMatch(el,scrollSelectors)")
        source = replaceAll(source,
                            "ftHintDescription(el,candidateFamily,linkSelector)",
                            "__profileDescription(el,candidateFamily,linkSelector)")
        source = replaceAll(source,
                            "ftLocalVisible(el,ownerView,policy,visibilityStyleCache)",
                            "__profileLocalVisible(el,ownerView,policy,visibilityStyleCache)")
        source = replaceAll(source, "ftLocalVisible(el,ownerView,policy)",
                            "__profileLocalVisible(el,ownerView,policy)")
        source = replaceAll(source,
                            "ftProjectedRect(el,local,policy,visibilityStyleCache)",
                            "__profileProjectedRect(el,local,policy,visibilityStyleCache)")
        source = replaceAll(source, "ftProjectedRect(el,local,policy)",
                            "__profileProjectedRect(el,local,policy)")
        source = replaceAll(source, "||hasPreferredLink(el,r)",
                            "||__profileHasPreferredLink(el,r)")
        source = replaceAll(source, "root.querySelectorAll(selector).forEach(function(el){",
            "const __selectorStart=performance.now(),__matches=root.querySelectorAll(selector);"
            + "__profile.selector_match_ms+=performance.now()-__selectorStart;"
            + "__matches.forEach(function(el){")
        source = replaceAll(source, "root.querySelectorAll('*').forEach(function(el){",
            "const __walkStart=performance.now(),__walkNodes=root.querySelectorAll('*');"
            + "__profile.root_traversal_ms+=performance.now()-__walkStart;"
            + "__walkNodes.forEach(function(el){")
        source = replaceAll(source, "while((node=walker.nextNode())){",
            "while(true){const __walkStart=performance.now();node=walker.nextNode();"
            + "__profile.root_traversal_ms+=performance.now()-__walkStart;if(!node)break;")
        source = replaceOnce(source, "function hit(el,r){",
            "function __profiledHitInner(el,r){", "hit definition")
        source = replaceOnce(source, "function hasPreferredLink(el,r){",
            "function hit(el,r){const __hitStart=performance.now();try{return __profiledHitInner(el,r);}"
            + "finally{__profile.hit_test_ms+=performance.now()-__hitStart;}}"
            + "function hasPreferredLink(el,r){", "hit wrapper")

        var profilesStyleCache = source.indexOf(
                    "function ftComputedStyle(element,view,styleCache){") >= 0
        if (profilesStyleCache) {
            source = replaceOnce(source,
                "function ftComputedStyle(element,view,styleCache){",
                "function __profiledComputedStyleInner(element,view,styleCache){",
                "computed style cache")
        }
        var profilesLocalHit = source.indexOf("function ftHit(element,rect,linkAlias){") >= 0
        if (profilesLocalHit) {
            source = replaceOnce(source, "function ftHit(element,rect,linkAlias){",
                "function __profiledLocalHitInner(element,rect,linkAlias){", "local hit test")
        }
        var profilesAncestorChain = source.indexOf("function ftStyleChainVisible(element,view,policy,cache){") >= 0
        if (profilesAncestorChain) {
            source = replaceOnce(source,
                "if(state&&state[key]!==null){ftRememberStyleChain(cache,chain,key,state[key]);return state[key];}",
                "if(state&&state[key]!==null){__profile.ancestor_chain_cache_hits++;"
                    + "ftRememberStyleChain(cache,chain,key,state[key]);return state[key];}",
                "ancestor visibility cache")
        }
        var clippedRectSignature = source.indexOf(
                    "function ftClippedRect(element,raw,view,styleCache){") >= 0
                ? "function ftClippedRect(element,raw,view,styleCache){"
                : "function ftClippedRect(element,raw,view){"
        var profilesClippedRect = source.indexOf(clippedRectSignature) >= 0
        if (profilesClippedRect) {
            source = replaceOnce(source, clippedRectSignature,
                clippedRectSignature.replace("function ftClippedRect(",
                                             "function __profiledClippedRectInner("),
                "clipped rectangle")
        }

        var addStart = source.indexOf("function add(")
        var visitStart = source.indexOf("function visit(", addStart)
        if (addStart < 0 || visitStart < 0)
            throw new Error("T043 instrumentation anchor mismatch: candidate evaluation")
        var addDefinition = source.slice(addStart, visitStart)
                .replace("function add(", "function __profiledAddInner(")
        var addWrapper = "function add(){const __addStart=performance.now();"
                + "try{return __profiledAddInner.apply(null,arguments);}"
                + "finally{__profile.candidate_evaluation_ms+=performance.now()-__addStart;}}"
        source = source.slice(0, addStart) + addDefinition + addWrapper
                + source.slice(visitStart)

        var prelude = "const __profileStarted=performance.now();"
                + "const __profile={root_traversal_ms:0,selector_match_ms:0,"
                + "candidate_evaluation_ms:0,hit_test_ms:0,description_ms:0,"
                + "local_visibility_ms:0,projection_ms:0,preferred_link_ms:0,"
                + "style_cache_hits:0,style_cache_misses:0,"
                + "local_hit_test_ms:0,clip_rect_ms:0,ancestor_chain_cache_hits:0};"
                + "function __profileMatch(el,selected){const started=performance.now();"
                + "try{return ftHintMatchesAny(el,selected);}"
                + "finally{__profile.selector_match_ms+=performance.now()-started;}}"
                + "function __profileDescription(el,family,link){const started=performance.now();"
                + "try{return ftHintDescription(el,family,link);}"
                + "finally{__profile.description_ms+=performance.now()-started;}}"
                + "function __profileLocalVisible(el,view,policy){const started=performance.now();"
                + "try{return ftLocalVisible.apply(null,arguments);}"
                + "finally{__profile.local_visibility_ms+=performance.now()-started;}}"
                + "function __profileProjectedRect(el,local,policy){const started=performance.now();"
                + "try{return ftProjectedRect.apply(null,arguments);}"
                + "finally{__profile.projection_ms+=performance.now()-started;}}"
                + "function __profileHasPreferredLink(el,rect){const started=performance.now();"
                + "try{return hasPreferredLink(el,rect);}"
                + "finally{__profile.preferred_link_ms+=performance.now()-started;}}"
        if (profilesStyleCache) {
            prelude += "function ftComputedStyle(element,view,styleCache){"
                    + "if(styleCache&&styleCache.styles){if(styleCache.styles.has(element))__profile.style_cache_hits++;"
                    + "else __profile.style_cache_misses++;}"
                    + "return __profiledComputedStyleInner.apply(null,arguments);}"
        }
        if (profilesLocalHit) {
            prelude += "function ftHit(element,rect,linkAlias){const started=performance.now();"
                    + "try{return __profiledLocalHitInner.apply(null,arguments);}"
                    + "finally{__profile.local_hit_test_ms+=performance.now()-started;}}"
        }
        if (profilesClippedRect) {
            prelude += "function ftClippedRect(element,raw,view){const started=performance.now();"
                    + "try{return __profiledClippedRectInner.apply(null,arguments);}"
                    + "finally{__profile.clip_rect_ms+=performance.now()-started;}}"
        }
        source = replaceOnce(source,
            "const out=[];const seen=new Set();const activeIds=new Set();",
            prelude + "const out=[];const seen=new Set();const activeIds=new Set();",
            "collector prelude")
        if (source.indexOf("__profile.root_traversal_ms+=") < 0
                || source.indexOf("__profile.selector_match_ms+=") < 0)
            throw new Error("T043 instrumentation did not find traversal and selector stages")
        source = replaceOnce(source,
            "viewport:{width:innerWidth,height:innerHeight}}",
            "viewport:{width:innerWidth,height:innerHeight},profile:{"
            + "total_ms:performance.now()-__profileStarted,"
            + "root_traversal_ms:__profile.root_traversal_ms,"
            + "selector_match_ms:__profile.selector_match_ms,"
            + "candidate_evaluation_ms:__profile.candidate_evaluation_ms,"
            + "hit_test_ms:__profile.hit_test_ms,"
            + "description_ms:__profile.description_ms,"
            + "local_visibility_ms:__profile.local_visibility_ms,"
            + "projection_ms:__profile.projection_ms,"
            + "preferred_link_ms:__profile.preferred_link_ms,"
            + "style_cache_hits:__profile.style_cache_hits,"
            + "style_cache_misses:__profile.style_cache_misses,"
            + "local_hit_test_ms:__profile.local_hit_test_ms,"
            + "clip_rect_ms:__profile.clip_rect_ms,"
            + "ancestor_chain_cache_hits:__profile.ancestor_chain_cache_hits}}",
            "collector result")
        return source
    }

    function test_T043_samples_data() {
        return [{tag: "500-80", count: 500, zoom: 0.8},
                {tag: "500-100", count: 500, zoom: 1},
                {tag: "500-125", count: 500, zoom: 1.25},
                {tag: "1000-100", count: 1000, zoom: 1},
                {tag: "5000-100", count: 5000, zoom: 1},
                {tag: "1000-mutation", count: 1000, zoom: 1, mutation: true}]
    }

    function test_T043_samples(data) {
        page.loaded = false
        page.zoomFactor = data.zoom
        page.url = Qt.resolvedUrl("../fixtures/webapp-interaction/performance-targets.html?count="
                                  + data.count)
        tryVerify(function() { return page.loaded }, 10000)
        tryVerify(function() { return evaluate("document.body.dataset.ready") === "true" }, 10000)

        var collector = profiledCollectorScript(BrowserScripts.hintCollector("buttons"))
        var snapshot = evaluate(collector)
        compare(snapshot.candidates.length, data.count)
        evaluate("window.__ferricBenchmarkClicks=0;"
                 + "document.querySelector('button').addEventListener('click',"
                 + "function(){window.__ferricBenchmarkClicks++;});true")

        for (var warm = 0; warm < 10; ++warm) {
            var warmCollection = evaluate(collector)
            compare(warmCollection.candidates.length, data.count)
            var warmCandidate = warmCollection.candidates[0]
            verify(evaluate(BrowserScripts.hintFresh(warmCandidate)).visible)
        }

        var samples = []
        for (var sample = 0; sample < 100; ++sample) {
            if (data.mutation) {
                evaluate("document.querySelector('button').setAttribute('data-version','"
                         + sample + "');true")
                var dirty = evaluate(BrowserScripts.hintDirtyRevision())
                verify(dirty.dirty, "a fixture mutation marks the snapshot dirty")
            }

            var collected = runTimed(collector)
            var value = collected.value
            compare(value.candidates.length, data.count)
            var candidate = value.candidates[0]

            var fresh = runTimed(BrowserScripts.hintFresh(candidate))
            verify(fresh.value.visible, "the retained synthetic target stays fresh")

            var preflight = null
            if (typeof BrowserScripts.hintPointerPreflight === "function") {
                preflight = runTimed(BrowserScripts.hintPointerPreflight(
                    candidate, value.viewport.width, value.viewport.height))
                verify(preflight.value && preflight.value.ok,
                       "the synthetic target passes pointer preflight")
            }

            var clickScript = BrowserScripts.hintClick.length <= 1
                    ? BrowserScripts.hintClick(candidate.element_id)
                    : BrowserScripts.hintClick(candidate)
            var action = runTimed(clickScript)
            compare(action.value, true, "one explicit semantic click reaches the fixture target")

            var refreshRoundTrip = data.mutation ? collected.round_trip_ms : null

            samples.push({
                browser_round_trip_ms: collected.round_trip_ms,
                collector: value.profile,
                fresh_round_trip_ms: fresh.round_trip_ms,
                preflight_round_trip_ms: preflight ? preflight.round_trip_ms : null,
                action_round_trip_ms: action.round_trip_ms,
                refresh_round_trip_ms: refreshRoundTrip,
                payload_bytes: JSON.stringify(value).length,
                count: value.candidates.length
            })
        }
        compare(evaluate("window.__ferricBenchmarkClicks"), 100,
                "the measured action stage delivered exactly one click per sample")
        console.info("FERRIC_PERFORMANCE_JSON " + JSON.stringify({
            scenario: data.tag,
            count: data.count,
            zoom: data.zoom,
            warmups: 10,
            samples: samples,
            user_agent: evaluate("navigator.userAgent"),
            viewport: evaluate("({width:innerWidth,height:innerHeight,"
                                + "device_pixel_ratio:devicePixelRatio})")
        }))
        evaluate(BrowserScripts.hintStopTracking())
    }

    function test_T043_overlay_time_to_visible_data() {
        return [{tag: "overlay-500-80", zoom: 0.8},
                {tag: "overlay-500-100", zoom: 1},
                {tag: "overlay-500-125", zoom: 1.25}]
    }

    function test_T043_overlay_time_to_visible(data) {
        page.loaded = false
        page.zoomFactor = data.zoom
        page.url = Qt.resolvedUrl("../fixtures/webapp-interaction/performance-targets.html?count=500")
        tryVerify(function() { return page.loaded }, 10000)
        tryVerify(function() { return evaluate("document.body.dataset.ready") === "true" }, 10000)
        var collected = evaluate(BrowserScripts.hintCollector("buttons"))
        compare(collected.candidates.length, 500)
        hintOverlay.sourceViewport = collected.viewport

        var rows = collected.candidates.map(function(candidate, index) {
            return {label: ("000" + index).slice(-3),
                    x: candidate.geometry.x, y: candidate.geometry.y,
                    width: candidate.geometry.width, height: candidate.geometry.height,
                    text: candidate.text}
        })
        var layoutPreview = hintOverlay.layoutHints(rows, hintOverlay.width, hintOverlay.height,
                                                     collected.viewport.width,
                                                     collected.viewport.height, 1)
        compare(layoutPreview.length, rows.length)
        for (var placedIndex = 0; placedIndex < layoutPreview.length; ++placedIndex) {
            var placed = layoutPreview[placedIndex]
            compare(placed.label, rows[placedIndex].label)
            compare(placed.text, rows[placedIndex].text)
            verify(placed.marker_x >= 0 && placed.marker_y >= 0
                   && placed.marker_x + placed.marker_width <= hintOverlay.width
                   && placed.marker_y + placed.marker_height <= hintOverlay.height,
                   "every marker stays inside the overlay viewport")
            verify(placed.collision_cluster === 0 || placed.cluster.length > 1)
            if (placed.collision_cluster === 0) {
                for (var earlierIndex = 0; earlierIndex < placedIndex; ++earlierIndex) {
                    var earlier = layoutPreview[earlierIndex]
                    verify(!(placed.marker_x < earlier.marker_x + earlier.marker_width
                             && placed.marker_x + placed.marker_width > earlier.marker_x
                             && placed.marker_y < earlier.marker_y + earlier.marker_height
                             && placed.marker_y + placed.marker_height > earlier.marker_y),
                           "unclustered markers do not overlap")
                }
            }
        }
        var labels = rows.map(function(row) { return row.label })
        var hintState = {mode: "label", prefix: "", matching_labels: labels,
                         active_label: labels[0]}
        hintOverlay.hintState = {mode: "label", prefix: "", matching_labels: [labels[0]],
                                 active_label: labels[0]}
        compare(hintOverlay.matching(labels[0]), true)
        compare(hintOverlay.matching(labels[1]), false)
        hintOverlay.hintState = {mode: "label", prefix: "", matching_labels: [labels[1]],
                                 active_label: labels[1]}
        compare(hintOverlay.matching(labels[0]), false)
        compare(hintOverlay.matching(labels[1]), true)
        hintOverlay.hintState = hintState
        var samples = []
        var layoutSamples = []

        for (var warm = 0; warm < 10; ++warm) {
            hintOverlay.hintsVisible = false
            hintOverlay.hintResults = []
            tryVerify(function() { return !hintOverlay.visible })
            wait(1)
            hintOverlay.hintState = hintState
            hintOverlay.hintResults = rows
            hintOverlay.hintsVisible = true
            tryVerify(function() {
                return hintOverlay.visible && hintOverlay.laidOutHints.length === 500
            }, 10000)
            verify(waitForRendering(hintOverlay))
        }

        for (var sample = 0; sample < 100; ++sample) {
            hintOverlay.hintsVisible = false
            hintOverlay.hintResults = []
            tryVerify(function() { return !hintOverlay.visible })
            wait(1)
            var started = PerformanceClock.nowNsecs()
            hintOverlay.hintState = hintState
            var layoutStarted = PerformanceClock.nowNsecs()
            hintOverlay.hintResults = rows
            compare(hintOverlay.laidOutHints.length, 500)
            layoutSamples.push((PerformanceClock.nowNsecs() - layoutStarted) / 1000000.0)
            hintOverlay.hintsVisible = true
            tryVerify(function() {
                return hintOverlay.visible && hintOverlay.laidOutHints.length === 500
            }, 10000)
            verify(waitForRendering(hintOverlay))
            samples.push((PerformanceClock.nowNsecs() - started) / 1000000.0)
        }

        console.info("FERRIC_OVERLAY_PERFORMANCE_JSON " + JSON.stringify({
            scenario: data.tag,
            count: 500,
            zoom: data.zoom,
            warmups: 10,
            time_to_visible_ms: samples,
            layout_compute_ms: layoutSamples,
            renderer: "Qt Quick offscreen software",
            viewport: {width: width, height: height}
        }))
        hintOverlay.hintsVisible = false
        hintOverlay.hintResults = []
        evaluate(BrowserScripts.hintStopTracking())
    }
}

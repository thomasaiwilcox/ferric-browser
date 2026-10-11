import QtQuick
import QtTest
import QtWebEngine
import "../../qml/scripts/BrowserScripts.js" as BrowserScripts

// Explicit opt-in benchmark, outside the default functional test directory.
// It measures the production script in the renderer; no DOM substitute.
TestCase {
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
    property bool done: false
    property var result
    function evaluate(source) {
        done = false
        page.runJavaScript(source, WebEngineScript.ApplicationWorld, function(value) {
            result = value
            done = true
        })
        tryVerify(function() { return done }, 30000)
        return result
    }
    function test_T043_samples_data() {
        return [{tag:"500-80", count:500, zoom:0.8},
                {tag:"500-100", count:500, zoom:1},
                {tag:"500-125", count:500, zoom:1.25},
                {tag:"1000-100", count:1000, zoom:1},
                {tag:"5000-100", count:5000, zoom:1},
                {tag:"1000-mutation", count:1000, zoom:1, mutation:true}]
    }
    function test_T043_samples(data) {
        page.loaded = false
        page.zoomFactor = data.zoom
        page.url = Qt.resolvedUrl("../fixtures/webapp-interaction/performance-targets.html?count=" + data.count)
        tryVerify(function() { return page.loaded }, 5000)
        tryVerify(function() { return evaluate("document.body.dataset.ready") === "true" }, 5000)
        tryVerify(function() { return evaluate(BrowserScripts.hintCollector("buttons")).candidates.length === data.count }, 5000)
        var source = "(()=>{const start=performance.now();const snapshot="
                + BrowserScripts.hintCollector("buttons")
                + ";const collected=performance.now();const fresh="
                + "snapshot.candidates.length?" + BrowserScripts.hintFresh({element_id:1,frame_path:"0",kind:"button"})
                + ":null;return {collect_ms:collected-start,fresh_ms:performance.now()-collected,count:snapshot.candidates.length};})()"
        for (var warm = 0; warm < 10; ++warm) evaluate(source)
        var samples = []
        for (var sample = 0; sample < 100; ++sample) {
            if (data.mutation)
                evaluate("document.querySelector('button').setAttribute('data-version','" + sample + "');true")
            var measured = evaluate(source)
            compare(measured.count, data.count)
            samples.push(measured)
        }
        console.info("FERRIC_PERFORMANCE_JSON " + JSON.stringify({
                         scenario:data.tag, count:data.count, zoom:data.zoom,
                         warmups:10, samples:samples,
                         user_agent:evaluate("navigator.userAgent"),
                         viewport:evaluate("({width:innerWidth,height:innerHeight,device_pixel_ratio:devicePixelRatio})")
                     }))
        evaluate(BrowserScripts.hintStopTracking())
    }
}

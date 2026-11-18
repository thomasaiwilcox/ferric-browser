import QtQuick
import QtTest
import QtWebEngine
import "../../qml/scripts/PageInteractionScripts.js" as Scripts
import "../qml-hints-v2/FixtureHarness.js" as Harness

TestCase {
    id: test
    name: "HintFeed"
    when: windowShown
    width: 1000; height: 700
    WebEngineView { id: page; anchors.fill: parent; property bool loaded: false
        onLoadingChanged: function(r) { loaded = r.status === WebEngineView.LoadSucceededStatus } }
    property bool done: false
    property var result
    property string fixtureText: ""
    property string control: ""
    property string candidate: ""
    function read(path) {
        var ready=false, value="", problem=""
        Harness.readText(path,function(e,t){problem=e||"";value=t;ready=true})
        tryVerify(function(){return ready},5000);verify(!problem,problem);return value
    }
    function evaluate(source) {
        done=false;page.runJavaScript(source,WebEngineScript.ApplicationWorld,function(r){result=r;done=true})
        tryVerify(function(){return done},30000);return result
    }
    function builder(text) {
        return new Function(text.replace(/^\.(?:pragma|import)[^\r\n]*\r?\n/gm,"")+";return hintCollector;")()
    }
    function collect(source, countReads) {
        return evaluate("(()=>{let reads=0;const saved=[];"+(countReads ?
            "for(const key of ['scrollWidth','scrollHeight','clientWidth','clientHeight']){const d=Object.getOwnPropertyDescriptor(Element.prototype,key);if(!d||!d.get)continue;saved.push([key,d]);Object.defineProperty(Element.prototype,key,{...d,get(){reads++;return d.get.call(this)}});}" : "")+
            "try{const start=performance.now(),r="+source+";const ms=performance.now()-start;const s=window.__ferric_browserHintState;return {ms,reads,inspected:r.inspected_nodes,truncated:r.truncated,rows:r.candidates.map(c=>({id:s.elements.get(c.element_id).element.id,text:c.text,href:c.href,kind:c.kind,geometry:c.geometry})).sort((a,b)=>JSON.stringify(a).localeCompare(JSON.stringify(b)))};}finally{for(const [key,d]of saved)Object.defineProperty(Element.prototype,key,d);"+Scripts.hintStopTracking()+"}})()")
    }
    function initTestCase() {
        verify(testArtifactDirectory.length>0)
        control=read(Harness.fileUrl(testArtifactDirectory+"/control.js"))
        candidate=read(Harness.fileUrl(testArtifactDirectory+"/candidate.js"))
        fixtureText=read(Qt.resolvedUrl("../fixtures/hint-feed.html"))
        page.loadHtml(fixtureText,"https://hint-feed.invalid/")
        tryVerify(function(){return page.loaded},10000)
        tryVerify(function(){return evaluate("!!document.elementFromPoint(900,10)")},5000)
    }
    function test_fresh_pages() {
        const before=builder(control),after=builder(candidate),samples=[]
        for(let i=0;i<5;i++) {
            page.loaded=false;page.loadHtml(fixtureText,'https://hint-feed.invalid/');tryVerify(function(){return page.loaded},10000);evaluate('scrollTo(0,0);true');wait(150)
            const first=collect((i%2?after:before)('actions',true),false)
            // Reload again so the second source also observes a fresh document.
            page.loaded=false;page.loadHtml(fixtureText,'https://hint-feed.invalid/');tryVerify(function(){return page.loaded},10000);evaluate('scrollTo(0,0);true');wait(150)
            const second=collect((i%2?before:after)('actions',true),false)
            const a=i%2?second:first,b=i%2?first:second
            if(JSON.stringify(a.rows)!==JSON.stringify(b.rows))console.info('FRESH_DIFF '+JSON.stringify({a:a.rows.filter(x=>!b.rows.some(y=>JSON.stringify(x)===JSON.stringify(y))),b:b.rows.filter(x=>!a.rows.some(y=>JSON.stringify(x)===JSON.stringify(y))),scroll:evaluate('scrollY')}))
            compare(JSON.stringify(a.rows),JSON.stringify(b.rows),'fresh document coverage')
            samples.push({control_ms:a.ms,candidate_ms:b.ms})
        }
        console.info('FERRIC_FEED_COLD_JSON '+JSON.stringify(samples))
    }
    function test_feed() {
        const before=builder(control),after=builder(candidate);var measurements=[]
        for(const y of [0,1500,6000]) {
            evaluate("scrollTo(0,"+y+");true");wait(100)
            for(const family of ['actions','links','buttons','scrollables','regions','surfaces']) {
                const a=collect(before(family,true),true),b=collect(after(family,true),true)
                compare(JSON.stringify(b.rows),JSON.stringify(a.rows),family+" visible targets at "+y)
                if(y===0 && family==='buttons') for(const id of ['sticky','overflow']) verify(b.rows.some(r=>r.id===id),'visible '+id)
                measurements.push({y,family,control_reads:a.reads,candidate_reads:b.reads,control_inspected:a.inspected,candidate_inspected:b.inspected})
            }
            const a=before('actions',true),b=after('actions',true),samples={y,control:[],candidate:[]}
            for(let i=0;i<23;i++) {
                const order=i%2?[b,a]:[a,b];var pair=order.map(s=>collect(s,false));if(i%2)pair.reverse()
                compare(JSON.stringify(pair[0].rows),JSON.stringify(pair[1].rows),'timed target coverage')
                if(i>=3){samples.control.push(pair[0].ms);samples.candidate.push(pair[1].ms)}
            }
            measurements.push(samples)
        }
        // Replace and append cards, then recheck both collectors on the changed page.
        page.runJavaScript('changeFeed()',WebEngineScript.MainWorld);wait(100)
        compare(JSON.stringify(collect(before('actions',true),false).rows),JSON.stringify(collect(after('actions',true),false).rows))
        console.info('FERRIC_FEED_JSON '+JSON.stringify(measurements))
    }
}

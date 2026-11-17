import QtQuick
import QtTest
import QtWebEngine
import "../../qml/scripts/PageInteractionScripts.js" as Scripts

// Phase 0 diagnostic: real strict discovery and deferred policy extraction.
// Cheap proposals are observations only and never enter a native hint session.
TestCase {
    id: test
    name: "HintProgressiveFeasibility"
    when: windowShown
    width: 1000
    height: 700
    property bool done: false
    property var reply
    WebEngineView {
        id: page
        anchors.fill: parent
        property bool loaded: false
        onLoadingChanged: function(request) {
            loaded = request.status === WebEngineView.LoadSucceededStatus
        }
    }
    function evaluate(source, main) {
        done = false
        page.runJavaScript(source, main ? WebEngineScript.MainWorld : WebEngineScript.ApplicationWorld,
            function(value) { reply = value; done = true })
        tryVerify(function() { return done }, 10000)
        return reply
    }
    function plan() {
        return {version:1, observations:true,
            stamp:{profile:"profileid-1",window:"windowid-1",tab:"tabid-2",document:"documentid-3",
                profile_revision:"1",configuration_revision:"1",policy_revision:"1",target_generation:"1",
                selector_revision:"0",view_generation:"1",viewport_generation:"0",collection_epoch:"1",
                provider_epoch:"1",request_id:"1",scope_lease:"",family:"links"},
            context:{start:"top-document",base_depth:0,owner:""},
            rules:[{rule:{kind:"collection",id:"items"},root:{start:"top-document",steps:[]},
                container_selector:{steps:[[null,"#feed",1]]},item_selector:{steps:[[null,".item",1]]},
                primary_root_steps:[],primary_selector:{steps:[[null,"a",1]]},
                identity_attributes:["data-id"],slot_count:9,order:"dom"}]}
    }
    function timed(source) {
        return evaluate("(()=>{const t=performance.now(),value=" + source
            + ";return {ms:performance.now()-t,value:value}})()")
    }
    function test_boundaries_data() {
        return [{tag:"grid",grid:true,cover:false}, {tag:"feed",grid:false,cover:false},
            {tag:"covered-grid",grid:true,cover:true}, {tag:"covered-feed",grid:false,cover:true}]
    }
    function test_boundaries(data) {
        page.loaded = false
        page.loadHtml("<!doctype html><style>body{margin:0}#feed{display:"
            + (data.grid ? "grid;grid-template-columns:repeat(3,200px)" : "block")
            + "}.item{height:70px;width:200px}a{display:block;width:180px;height:30px}</style><main id=feed></main>",
            "https://progressive-fixture.invalid/")
        tryVerify(function() { return page.loaded }, 10000)
        compare(evaluate("(()=>{for(let i=0;i<300;i++){const item=document.createElement('article');"
            + "item.className='item';item.dataset.id=String(i);item.innerHTML='<a id=target-'+i+' href=/item/'+i+'>Item '+i+'</a>'+'<span>text</span>'.repeat(20);feed.append(item)}"
            + (data.cover ? "const r=document.getElementById('target-0').getBoundingClientRect(),cover=document.createElement('div');cover.style='position:fixed;background:white;z-index:10;left:'+r.left+'px;top:'+r.top+'px;width:'+r.width+'px;height:'+r.height+'px';document.body.append(cover);" : "")
            + "return true})()", true), true)
        wait(80)
        var request = plan(), rows = []
        for (var i = 0; i < 12; ++i) {
            var cheap = timed("(()=>{const ids=[];for(const el of document.querySelectorAll('a[href]')){const r=el.getBoundingClientRect();if(r.width>0&&r.height>0&&r.right>0&&r.bottom>0&&r.left<innerWidth&&r.top<innerHeight)ids.push(el.id)}return ids})()")
            var ordinary = timed(Scripts.hintCollector("links", false, undefined, "", false, undefined, request, true))
            verify(!ordinary.value.error && !ordinary.value.interaction_error, JSON.stringify(ordinary.value))
            var remaining = {inspections:50000-ordinary.value.interaction_work.inspections,
                predicate_steps:100000-ordinary.value.interaction_work.predicate_steps,annotation_bytes:2097152}
            var policy = timed(Scripts.hintFinishInteraction(request, ordinary.value.interaction_pending, remaining))
            verify(!policy.value.error, JSON.stringify(policy.value))
            var annotation = JSON.parse(policy.value.annotation_json)
            verify(!!annotation.outcomes[0].result["collection-observed"], JSON.stringify(annotation.outcomes[0]))
            var ids = evaluate("(()=>{const state=window.__ferric_browserHintState;return "
                + JSON.stringify(ordinary.value.candidates.map(function(c) { return c.element_id }))
                + ".map(id=>state.elements.get(id).element.id)})()")
            if (data.cover) {
                verify(cheap.value.indexOf("target-0") >= 0)
                compare(ids.indexOf("target-0"), -1)
            } else compare(JSON.stringify(cheap.value), JSON.stringify(ids))
            if (i >= 2) rows.push({cheap_ms:cheap.ms,ordinary_ms:ordinary.ms,policy_ms:policy.ms,
                cheap_ids:cheap.value,strict_ids:ids,inspections:ordinary.value.interaction_work.inspections,
                predicate_steps:policy.value.interaction_work.predicate_steps})
            evaluate(Scripts.hintStopTracking())
        }
        console.info("PROGRESSIVE_FEASIBILITY " + JSON.stringify({fixture:data.tag,rows:rows}))
    }
}

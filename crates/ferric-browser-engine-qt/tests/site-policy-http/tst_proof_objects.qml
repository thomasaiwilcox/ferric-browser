import QtQuick
import QtTest
import QtWebEngine
import io.github.ferricbrowser 1.0
import Ferric.Testing 1.0
import "../../qml/scripts/BrowserScripts.js" as Scripts
import "../../qml/components" as Components

TestCase {
    id: tests
    name: "PolicyProofObjects"
    when: windowShown
    width: 1000
    height: 800
    property bool done: false
    property var reply
    property string topOwner: ""
    property string foreignOwner: ""
    FerricHintFrameBroker { id: broker }
    Component { id: temporaryBroker; FerricHintFrameBroker {} }
    FrameReceiptProbe { id: probe }
    FrameReceiptProbe { id: foreignProbe }
    Components.FerricHintFrameProvider { id: projection; browserWindow: tests }
    WebEngineView {
        id: page
        anchors.fill: parent
        Component.onCompleted: {
            var script=WebEngine.script()
            script.name="ferric-native-ancestry-lifecycle-test"
            script.sourceCode=Scripts.hintNativeAncestryLifecycle()
            script.injectionPoint=WebEngineScript.DocumentCreation
            script.worldId=WebEngineScript.ApplicationWorld
            script.runsOnSubFrames=true
            userScripts.insert(script)
        }
        property bool loaded: false
        onLoadingChanged: function(request) { loaded=request.status===WebEngineView.LoadSucceededStatus }
    }
    function evaluate(script,world) {
        done=false
        page.runJavaScript(script,world===undefined?WebEngineScript.MainWorld:world,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000)
        return reply
    }
    function init() {
        foreignOwner=""
        page.loaded=false
        page.url="http://127.0.0.1:8766/roots.html"
        tryVerify(function(){return page.loaded},10000)
        page.forceActiveFocus()
        done=false
        broker.attach(page.mainFrame,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000)
        verify(!reply.error,JSON.stringify(reply))
        var owner=reply.owner
        topOwner=owner
        done=false
        probe.capture(broker,owner,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000)
        verify(reply.ok,JSON.stringify(reply))
        evaluate(Scripts.hintCollector("links"),WebEngineScript.ApplicationWorld)
        wait(50)
    }
    function cleanup() {
        if(foreignOwner)runForeign(Scripts.hintStopTracking())
        evaluate(Scripts.hintStopTracking(),WebEngineScript.ApplicationWorld)
        broker.clear();probe.reset();foreignProbe.reset()
    }
    function selector(css) { return {steps:[[null,css,1]]} }
    function stamp(family,lease) {
        return {profile:"profileid-1",window:"windowid-1",tab:"tabid-2",document:"documentid-3",
            profile_revision:"1",configuration_revision:"1",policy_revision:"1",target_generation:"1",
            selector_revision:"0",view_generation:"1",viewport_generation:"0",collection_epoch:"1",
            provider_epoch:"1",request_id:"1",scope_lease:lease||"",family:family||"links"}
    }
    function anchor(steps,target) {
        return {rule:{kind:"anchor",id:"fixture"},root:{start:"top-document",steps:steps||[]},target_selector:selector(target||".target")}
    }
    function collect(rule,lease,family,proxy,budget) {
        var request={version:1,observations:true,stamp:stamp(family,lease),context:{start:"top-document",base_depth:0,owner:""},rules:Array.isArray(rule)?rule:[rule||anchor()]}
        var value=JSON.parse(evaluate("JSON.stringify("+Scripts.hintCollector(family||"links",proxy===true,undefined,lease||"",false,budget,request)+")",WebEngineScript.ApplicationWorld))
        verify(!value.error,JSON.stringify(value))
        value.annotation=JSON.parse(value.annotation_json)
        value.producerPlan=JSON.stringify(request)
        return value
    }
    function stable(rule,lease,family,proxy,budget) {
        collect(rule,lease,family,proxy,budget);wait(50)
        return collect(rule,lease,family,proxy,budget)
    }
    function seal(value,annotation,scope) {
        done=false
        probe.validateObjects(broker,null,value.annotation_batch,JSON.stringify(value.annotation.stamp),value.revision,
            annotation===undefined?value.annotation_json:annotation,JSON.stringify(scope===undefined?null:scope),[],
            function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        return reply
    }
    function maskTracking(value) {
        wait(50)
        // Test-only: exercise exact object/containment checks independently of
        // the normal observer fence. Page MainWorld cannot change this state.
        compare(evaluate("(()=>{const s=window.__ferric_browserHintState;s.dirty=false;s.revision="+value.revision+";return true})()",WebEngineScript.ApplicationWorld),true)
    }
    function sealPlan(value,originalPlan) {
        done=false
        probe.validatePlannedObjects(broker,value.annotation_batch,JSON.stringify(value.annotation.stamp),value.revision,
            value.annotation_json,originalPlan,function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        return reply
    }
    function sealDebit(value,reported,granted) {
        done=false
        probe.validateDebitedObjects(broker,value.annotation_batch,JSON.stringify(value.annotation.stamp),value.revision,
            value.annotation_json,value.producerPlan,reported,granted,function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        return reply
    }
    function captureCatalogue(value,width,height,ordinaryGeometry,otherBroker) {
        var dimensions=evaluate("({width:innerWidth,height:innerHeight,debit:window.__ferric_browserHintState.interactionProof.annotation_work})",WebEngineScript.ApplicationWorld)
        var bounds=ordinaryGeometry===undefined?value.candidates.map(function(candidate,index){return {index:index,kind:candidate.kind,geometry:candidate.geometry}}):ordinaryGeometry
        done=false
        probe.captureFragments(otherBroker||broker,value.annotation_batch,JSON.stringify(value.annotation.stamp),value.revision,
            value.annotation_json,value.producerPlan,JSON.stringify(bounds),dimensions.debit,2097152,
            width===undefined?dimensions.width:width,height===undefined?dimensions.height:height,
            function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        return reply
    }
    function revalidateCatalogue(otherBroker) {
        done=false
        probe.revalidateFragments(otherBroker||broker,function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        return reply
    }
    function graphCompletions() {
        return probe.graphEvents().filter(function(event){return event.kind==="complete"})
    }
    function test_graph_and_document_reports_use_independent_counters() {
        var value=stable(anchor([],"#slotted-primary"))
        compare(seal(value),true)
        probe.configureGraphAccounting(-1)
        probe.configureDocumentAccounting(-1,-1)
        compare(captureCatalogue(value),true)
        compare(revalidateCatalogue(),true)
        compare(graphCompletions().length,4)
        var events=probe.documentEvents().filter(function(event){return event.kind==="complete"})
        compare(events.length,8)
        var stages=["top-capture-before","top-capture-after","top-reseal-before","top-reseal-after"]
        for(var index=0;index<events.length;index++) {
            compare(events[index].checkpoint,stages[Math.floor(index/2)])
            compare(events[index].document,0);compare(events[index].pass,index%2)
            compare(events[index].outcome,"known");compare(events[index].units,1)
        }
    }
    function test_graph_tickets_settle_before_capture_and_reseal_complete() {
        var value=stable(anchor([],"#slotted-primary"))
        compare(seal(value),true,"Use an actual native-valid original graph")
        probe.configureGraphAccounting(-1)
        compare(captureCatalogue(value),true)
        var events=graphCompletions()
        compare(events.length,2,"Both capture reservations settle before the caller resumes")
        compare(revalidateCatalogue(),true)
        events=graphCompletions()
        compare(events.length,4,"Reseal has two independent original-request slots")
        for(var index=0;index<events.length;index++) {
            compare(events[index].pass,index)
            compare(events[index].outcome,"known")
            verify(events[index].steps>0&&events[index].steps<=100000,JSON.stringify(events[index]))
        }
        compare(revalidateCatalogue(),false,"No further reseal may reuse its original slots")
        compare(graphCompletions().length,4,"A refused repeated slot issues no ticket")
    }
    function test_graph_admission_precedes_each_probe_data() {
        return [0,1,2,3].map(function(pass){return {tag:"refuse-pass-"+pass,pass:pass}})
    }
    function test_graph_admission_precedes_each_probe(data) {
        var value=stable(anchor([],"#slotted-primary"))
        compare(seal(value),true)
        compare(evaluate("(()=>{const b=window.__ferric_browserHintState.interactionProof;window.graphAdmissionSpy={batch:b,annotation:b.annotation,reads:0};Object.defineProperty(b,'annotation',{configurable:true,get(){window.graphAdmissionSpy.reads++;return window.graphAdmissionSpy.annotation;}});return true})()",WebEngineScript.ApplicationWorld),true)
        probe.configureGraphAccounting(data.pass)
        try {
            compare(captureCatalogue(value),data.pass>=2)
            if(data.pass>=2)compare(revalidateCatalogue(),false)
            var events=probe.graphEvents(),completed=graphCompletions()
            compare(events.filter(function(event){return event.kind==="reserve"}).length,data.pass+1)
            compare(completed.length,data.pass)
            compare(evaluate("window.graphAdmissionSpy.reads",WebEngineScript.ApplicationWorld),data.pass,
                "A refused pass must never read the retained carrier")
            for(var index=0;index<completed.length;index++)compare(completed[index].outcome,"known")
        } finally {
            evaluate("(()=>{const s=window.graphAdmissionSpy;Object.defineProperty(s.batch,'annotation',{configurable:true,writable:true,value:s.annotation});delete window.graphAdmissionSpy;return true})()",WebEngineScript.ApplicationWorld)
        }
    }
    function test_graph_rejection_reports_the_work_already_done_data() {
        return [{tag:"single",maximum:false},{tag:"maximum-inventory",maximum:true}]
    }
    function test_graph_rejection_reports_the_work_already_done(data) {
        var rules=[anchor([],"#slotted-primary")]
        if(data.maximum) {
            rules=[]
            for(var index=0;index<64;index++) {
                var query=anchor([],"#slotted-primary")
                query.rule.id="maximum-anchor-"+index
                rules.push(query)
            }
            for(var collection=0;collection<16;collection++)rules.push({
                rule:{kind:"collection",id:"maximum-collection-"+collection},
                root:{start:"top-document",steps:[]},container_selector:selector("#catalog"),
                item_selector:selector("#slotted-primary"),primary_root_steps:[],primary_self:true,
                order:"dom",slot_count:28})
        }
        var value=stable(rules)
        if(data.maximum) {
            compare(rules.length,80)
            compare(value.annotation.outcomes.length,80)
            for(var item=64;item<80;item++) {
                var observed=value.annotation.outcomes[item].result["collection-observed"]
                verify(!!observed,JSON.stringify(value.annotation.outcomes[item]))
                verify(observed.items.length>0,"Maximum collections must discover real item roots")
            }
        }
        compare(seal(value),true)
        compare(captureCatalogue(value),true,"The original maximum graph must be valid before corruption")
        var original=JSON.stringify(value.annotation.discovery_work)
        verify(!!value.annotation.discovery_work)
        verify(value.annotation.discovery_work.inspections>0)
        verify(value.annotation.discovery_work.predicate_steps>0)
        var candidates=JSON.stringify(value.candidates)
        var factsScript="(()=>{const b=window.__ferric_browserHintState.interactionProof;return JSON.stringify({batch:b.batch,annotation:b.annotation,plan:b.plan,debit:b.annotation_work})})()"
        var facts=evaluate(factsScript,WebEngineScript.ApplicationWorld)
        var debit=JSON.parse(facts).debit
        verify(debit>0&&debit<=2097152)
        compare(JSON.parse(facts).batch,value.annotation_batch)
        compare(JSON.parse(facts).annotation,value.annotation_json)
        probe.configureGraphAccounting(-1)
        compare(evaluate("(()=>{const b=window.__ferric_browserHintState.interactionProof;const c=JSON.parse(b.fragment_catalogue);c.targets[0].fragments.projected_primary_fragment_css.width-=7;b.fragment_catalogue=JSON.stringify(c);return true})()",WebEngineScript.ApplicationWorld),true)
        compare(captureCatalogue(value),false)
        var events=graphCompletions()
        compare(events.length,1)
        compare(events[0].outcome,"known")
        verify(events[0].steps>0,"A genuine graph rejection must charge its visited facts")
        verify(events[0].steps<=100000)
        compare(probe.hasFragmentCatalogue(),false)
        compare(evaluate(factsScript,WebEngineScript.ApplicationWorld),facts,
            "Rejected native sealing must retain original inventory and annotation debit")
        compare(JSON.stringify(value.annotation.discovery_work),original)
        compare(JSON.stringify(value.candidates),candidates)
        compare(captureCatalogue(value),false)
        compare(graphCompletions().length,1,"A rejected pass cannot renew its original grant")
    }
    function test_graph_expired_admission_never_dispatches() {
        var value=stable(anchor([],"#slotted-primary"))
        compare(seal(value),true)
        probe.configureGraphAccounting(-1,"expire")
        compare(captureCatalogue(value),false)
        var events=graphCompletions()
        compare(events.length,1)
        compare(events[0].outcome,"no-script")
        compare(events[0].steps,0)
    }
    function test_graph_result_requires_successful_ledger_settlement() {
        var value=stable(anchor([],"#slotted-primary"))
        compare(captureCatalogue(value),true,"First prove the original graph and catalogue are valid")
        probe.configureGraphAccounting(-1,"reject-settlement")
        compare(captureCatalogue(value),false,"A valid graph cannot bypass a refused ledger completion")
        var events=graphCompletions()
        compare(events.length,1)
        compare(events[0].outcome,"known")
        verify(events[0].steps>0)
        compare(probe.hasFragmentCatalogue(),false)
    }
    function test_graph_clear_settles_without_delivering_a_user_callback() {
        var value=stable(anchor([],"#slotted-primary"))
        compare(seal(value),true)
        var dimensions=evaluate("({width:innerWidth,height:innerHeight,debit:window.__ferric_browserHintState.interactionProof.annotation_work})",WebEngineScript.ApplicationWorld)
        var bounds=value.candidates.map(function(candidate,index){return {index:index,kind:candidate.kind,geometry:candidate.geometry}})
        probe.configureGraphAccounting(-1,"clear")
        var delivered=false
        probe.captureFragments(broker,value.annotation_batch,JSON.stringify(value.annotation.stamp),value.revision,
            value.annotation_json,value.producerPlan,JSON.stringify(bounds),dimensions.debit,2097152,
            dimensions.width,dimensions.height,function(result){delivered=true})
        tryVerify(function(){return graphCompletions().length>0},3000)
        var events=graphCompletions()
        compare(events.length,1)
        compare(events[0].outcome,"unknown")
        wait(1200)
        compare(graphCompletions().length,1,"Late native completion cannot refund a cancelled ticket")
        compare(delivered,false,"Clear still suppresses the original user callback")
    }
    function test_graph_timeout_burns_before_the_late_engine_reply() {
        var value=stable(anchor([],"#slotted-primary"))
        compare(seal(value),true)
        probe.configureGraphAccounting(-1)
        compare(evaluate("(()=>{const b=window.__ferric_browserHintState.interactionProof;window.graphTimeoutSpy={batch:b,annotation:b.annotation};Object.defineProperty(b,'annotation',{configurable:true,get(){const end=Date.now()+1400;while(Date.now()<end){}return window.graphTimeoutSpy.annotation;}});return true})()",WebEngineScript.ApplicationWorld),true)
        try {
            compare(captureCatalogue(value),false)
            var events=graphCompletions()
            compare(events.length,1)
            compare(events[0].outcome,"unknown")
            wait(1600)
            compare(graphCompletions().length,1,"A bounded report arriving after timeout cannot refund")
        } finally {
            evaluate("(()=>{const s=window.graphTimeoutSpy;Object.defineProperty(s.batch,'annotation',{configurable:true,writable:true,value:s.annotation});delete window.graphTimeoutSpy;return true})()",WebEngineScript.ApplicationWorld)
        }
    }
    function test_graph_broker_destruction_settles_without_a_user_callback() {
        var value=stable(anchor([],"#slotted-primary"))
        compare(seal(value),true)
        var other=temporaryBroker.createObject(tests)
        verify(other)
        probe.watchBroker(other)
        try {
            done=false
            other.attach(page.mainFrame,function(result){reply=result;done=true})
            tryVerify(function(){return done},10000)
            verify(!reply.error,JSON.stringify(reply))
            var owner=reply.owner
            done=false
            probe.capture(other,owner,function(result){reply=result;done=true})
            tryVerify(function(){return done},10000)
            verify(reply.ok,JSON.stringify(reply))
            compare(captureCatalogue(value,undefined,undefined,undefined,other),true,
                "The temporary broker must first acquire a genuinely valid catalogue")
            var dimensions=evaluate("({width:innerWidth,height:innerHeight,debit:window.__ferric_browserHintState.interactionProof.annotation_work})",WebEngineScript.ApplicationWorld)
            var bounds=value.candidates.map(function(candidate,index){return {index:index,kind:candidate.kind,geometry:candidate.geometry}})
            probe.configureGraphAccounting(-1,"destroy")
            var delivered=false
            probe.captureFragments(other,value.annotation_batch,JSON.stringify(value.annotation.stamp),value.revision,
                value.annotation_json,value.producerPlan,JSON.stringify(bounds),dimensions.debit,2097152,
                dimensions.width,dimensions.height,function(result){delivered=true})
            tryVerify(function(){return graphCompletions().length>0},3000)
            compare(probe.watchedBrokerAlive(),false)
            var events=graphCompletions()
            compare(events.length,1)
            compare(events[0].outcome,"unknown")
            wait(1200)
            compare(graphCompletions().length,1)
            compare(delivered,false)
        } finally {
            if(probe.watchedBrokerAlive())other.destroy()
        }
    }
    function ordinaryInput() {
        return {version:1,family:"links",selectors:{family:"links",configuration_revision:1,
            selector_revision:"0000000000000000",policies:{links:{policy:{mode:"replace",selectors:["#ordinary-original"]}}}},
            proxy:false,scope_lease:"",include_facets:false,deferred:true,
            budget:{depth:8,inspections:50000,candidates:5000,predicate_steps:100000,annotation_bytes:2097152}}
    }
    function test_frame_projection_preserves_the_visible_presentation_anchor() {
        var rect={x:10,y:20,width:40,height:20}
        var candidate={geometry:rect,presentation:{anchor:{x:30,y:30},fragments:[rect]}}
        var maps=[{left:100,top:50,scaleX:2,scaleY:3,content:{left:100,top:50,right:500,bottom:500}}]
        var result=projection.project(candidate,"frame-owner",maps)
        verify(result)
        compare(result.presentation.anchor.x,160)
        compare(result.presentation.anchor.y,140)
        compare(candidate.presentation.anchor.x,30,"Projection leaves the original local snapshot unchanged")
    }
    function test_frame_projection_repositions_an_anchor_clipped_by_its_parent() {
        var rect={x:0,y:0,width:100,height:20}
        var candidate={geometry:rect,presentation:{anchor:{x:10,y:10},fragments:[rect]}}
        var result=projection.project(candidate,"frame-owner",[{left:0,top:0,scaleX:1,scaleY:1,
            content:{left:50,top:0,right:100,bottom:20}}])
        verify(result)
        compare(result.presentation.anchor.x,75)
        compare(result.presentation.anchor.y,10)
        compare(result.presentation.fragments[0].x,50)
    }
    function test_native_top_fragment_geometry_keeps_the_original_viewport() {
        var value=stable(anchor([],"#slotted-primary"))
        compare(captureCatalogue(value),true)
        done=false
        probe.fragmentGeometry(broker,function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        verify(reply,JSON.stringify(reply))
        compare(reply.mappings.length,0)
        compare(reply.viewport.width,value.viewport.width)
        compare(reply.viewport.height,value.viewport.height)
        evaluate("window.__ferric_browserHintState.revision++;true",WebEngineScript.ApplicationWorld)
        done=false
        probe.fragmentGeometry(broker,function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        verify(!reply,"A later collector revision cannot reuse the original geometry")
    }
    function ordinaryInputEvidence(actual,expected,resumeBudget,scope) {
        var request={version:1,observations:true,stamp:stamp(expected.family,scope&&scope.lease_id),
            context:{start:"top-document",base_depth:0,owner:""},rules:[anchor([],".target")],
            __test_ordinary_input:expected,__test_original_scope:scope||null}
        function runPass() {
            var value=JSON.parse(evaluate("JSON.stringify("+Scripts.hintCollector(actual.family,actual.proxy,
                actual.selectors,actual.scope_lease,actual.include_facets,actual.budget,request,actual.deferred)+")",WebEngineScript.ApplicationWorld))
            verify(!value.error,JSON.stringify(value))
            if(actual.deferred) {
                verify(!!value.interaction_pending,JSON.stringify(value))
                compare(evaluate("Object.getOwnPropertyDescriptor(window.__ferric_browserHintState.pendingInteraction,'ordinaryInput').writable",WebEngineScript.ApplicationWorld),false)
                value=JSON.parse(evaluate("JSON.stringify("+Scripts.hintFinishInteraction(request,value.interaction_pending,resumeBudget)+")",WebEngineScript.ApplicationWorld))
                verify(!value.error,JSON.stringify(value))
            }
            verify(typeof value.annotation_json==="string",JSON.stringify(value))
            value.annotation=JSON.parse(value.annotation_json)
            value.producerPlan=JSON.stringify(request)
            return value
        }
        runPass();wait(50)
        var value=runPass()
        compare(evaluate("Object.getOwnPropertyDescriptor(window.__ferric_browserHintState.interactionProof,'ordinary_input').writable",WebEngineScript.ApplicationWorld),false)
        verify(value.candidates.length>0,JSON.stringify(value))
        compare(seal(value,undefined,scope),true,"The actual raw graph, scope and native identities are valid before testing original-input binding")
        return value
    }
    function ordinaryInputFixture() {
        compare(evaluate("document.body.innerHTML='<a id=ordinary-original class=target href=#original>Original</a><a id=ordinary-alternate class=target href=#alternate>Alternate</a>';true"),true)
    }
    function test_native_catalogue_binds_original_ordinary_input_data() {
        return [{tag:"same-revision-selectors",change:"selectors"},{tag:"family",change:"family"},
            {tag:"proxy",change:"proxy"},{tag:"facet-flag",change:"include_facets"},
            {tag:"deferral",change:"deferred"},{tag:"depth-grant",change:"depth"},
            {tag:"inspection-grant",change:"inspections"},{tag:"candidate-grant",change:"candidates"},
            {tag:"predicate-grant",change:"predicate_steps"},{tag:"byte-grant",change:"annotation_bytes"}]
    }
    function test_native_catalogue_binds_original_ordinary_input(data) {
        ordinaryInputFixture()
        var expected=ordinaryInput(),actual=JSON.parse(JSON.stringify(expected))
        if(data.change==="selectors")actual.selectors.policies.links.policy.selectors=["#ordinary-alternate"]
        else if(data.change==="family")actual.family="all"
        else if(data.change==="proxy"||data.change==="include_facets")actual[data.change]=true
        else if(data.change==="deferred")actual.deferred=false
        else actual.budget[data.change]--
        var value=ordinaryInputEvidence(actual,expected)
        if(data.change==="selectors"){
            compare(value.candidates.length,1)
            compare(value.candidates[0].text,"Alternate")
            compare(actual.selectors.selector_revision,expected.selectors.selector_revision)
        }
        compare(captureCatalogue(value),false,"A different ordinary query or grant cannot inherit the original request")
        compare(probe.hasFragmentCatalogue(),false)
    }
    function test_native_catalogue_keeps_original_ordinary_grant_through_resume_data() {
        return [{tag:"unchanged"},{tag:"smaller-policy-grant",resume:true},{tag:"large-selector-input",large:true}]
    }
    function test_native_catalogue_keeps_original_ordinary_grant_through_resume(data) {
        ordinaryInputFixture()
        var expected=ordinaryInput()
        if(data.large){
            expected.selectors.policies.links.policy.selectors=Array(12).fill("#ordinary-original:not([data-"+"x".repeat(500)+"])")
            verify(JSON.stringify(expected).length>4096)
        }
        var value=ordinaryInputEvidence(expected,expected,data.resume?{inspections:1000,predicate_steps:1000,annotation_bytes:2096000}:undefined)
        compare(captureCatalogue(value),true)
        compare(revalidateCatalogue(),true,"Resealing retains the same original input")
    }
    function test_original_ordinary_input_preserves_an_acquired_element_scope() {
        var scope=scopedFixture(false),expected=ordinaryInput()
        expected.family="all";expected.selectors.family="all"
        expected.selectors.policies={all:{policy:{mode:"replace",selectors:["#scoped-target"]}}}
        expected.scope_lease=scope.lease_id
        var value=ordinaryInputEvidence(expected,expected,undefined,scope)
        compare(value.candidates.length,1)
        compare(value.candidates[0].text,"Scoped target")
        compare(captureCatalogue(value),true)
        compare(revalidateCatalogue(),true,"The acquired native Element permit survives input binding and reseal")
        for(var lease of ["","wrong-original-lease"]){
            var request=JSON.parse(value.producerPlan)
            var result=evaluate(Scripts.hintCollector(expected.family,expected.proxy,expected.selectors,lease,
                expected.include_facets,expected.budget,request,true),WebEngineScript.ApplicationWorld)
            verify(!!result.error,JSON.stringify(result))
            compare(result.candidates.length,0,"An omitted or substituted Element permit cannot become an unscoped scan")
        }
    }
    function test_native_catalogue_receipt_retains_original_batch_and_viewport() {
        var value=stable(anchor([],"#slotted-primary"))
        compare(captureCatalogue(value,-1,800),false)
        compare(captureCatalogue(value,1,1),false)
        compare(captureCatalogue(value),true)
        compare(revalidateCatalogue(),true)
        var replacement=collect(anchor([],"#slotted-primary"))
        verify(replacement.annotation_batch!==value.annotation_batch)
        compare(seal(replacement),true)
        compare(revalidateCatalogue(),false,"A later collection cannot replace the original receipt")
        compare(captureCatalogue(replacement),true)
        compare(revalidateCatalogue(),true)
        broker.clear()
        compare(revalidateCatalogue(),false)
    }
    function test_native_catalogue_receipt_cannot_cross_broker_namespaces() {
        var value=stable(anchor([],"#slotted-primary"))
        compare(captureCatalogue(value),true)
        var other=Qt.createQmlObject('import io.github.ferricbrowser 1.0; FerricHintFrameBroker {}',tests)
        compare(revalidateCatalogue(other),false)
        other.destroy()
        compare(revalidateCatalogue(),true)
    }
    function test_native_catalogue_receipt_uses_original_ordinary_bounds() {
        var value=stable(anchor([],"#slotted-primary"))
        var bounds=value.candidates.map(function(candidate,index){return {index:index,kind:candidate.kind,geometry:candidate.geometry}})
        var target=JSON.parse(evaluate("window.__ferric_browserHintState.interactionProof.fragment_catalogue",WebEngineScript.ApplicationWorld)).targets[0]
        bounds[target.candidate.index].geometry={x:0,y:0,width:0.001,height:0.001}
        compare(captureCatalogue(value,undefined,undefined,bounds),false)
        compare(captureCatalogue(value),true)
        compare(revalidateCatalogue(),true)
    }
    function test_clear_during_catalogue_capture_discards_old_completion() {
        var value=stable(anchor([],"#slotted-primary"))
        compare(captureCatalogue(value),true)
        var dimensions=evaluate("({width:innerWidth,height:innerHeight,debit:window.__ferric_browserHintState.interactionProof.annotation_work})",WebEngineScript.ApplicationWorld)
        var bounds=value.candidates.map(function(candidate,index){return {index:index,kind:candidate.kind,geometry:candidate.geometry}})
        var calls=0
        probe.captureFragments(broker,value.annotation_batch,JSON.stringify(value.annotation.stamp),value.revision,
            value.annotation_json,value.producerPlan,JSON.stringify(bounds),dimensions.debit,2097152,
            dimensions.width,dimensions.height,function(result){calls++})
        verify(probe.pendingScripts(broker)>0,"Clear must interrupt an admitted native operation")
        broker.clear()
        wait(100)
        compare(calls,0)
        compare(probe.hasFragmentCatalogue(),false)
        done=false
        broker.attach(page.mainFrame,function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        verify(!reply.error,JSON.stringify(reply))
        var owner=reply.owner
        done=false
        probe.capture(broker,owner,function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        verify(reply.ok,JSON.stringify(reply))
        compare(captureCatalogue(stable(anchor([],"#slotted-primary"))),true)
    }
    function test_native_catalogue_receipt_seals_exact_snapshot_text() {
        var value=stable(anchor([],"#slotted-primary"))
        // Leave byte headroom before capturing the immutable receipt, so the
        // added whitespace isolates its exact-string check from the byte floor.
        compare(evaluate("window.__ferric_browserHintState.interactionProof.annotation_work=2097152;true",WebEngineScript.ApplicationWorld),true)
        compare(captureCatalogue(value),true)
        var saved=evaluate("window.__ferric_browserHintState.interactionProof.fragment_catalogue",WebEngineScript.ApplicationWorld)
        compare(evaluate("window.__ferric_browserHintState.interactionProof.fragment_catalogue=' '+window.__ferric_browserHintState.interactionProof.fragment_catalogue;true",WebEngineScript.ApplicationWorld),true)
        var debit=evaluate("window.__ferric_browserHintState.interactionProof.annotation_work",WebEngineScript.ApplicationWorld)
        compare(sealDebit(value,debit,2097152),true,"Unchanged catalogue meaning and raw graph still pass")
        compare(revalidateCatalogue(),false,"A receipt retains the exact original snapshot")
        compare(evaluate("window.__ferric_browserHintState.interactionProof.fragment_catalogue="+JSON.stringify(saved)+";true",WebEngineScript.ApplicationWorld),true)
        compare(revalidateCatalogue(),true)
    }
    function test_retained_annotation_is_admitted_before_parsing_data() {
        return [{tag:"code-unit-overflow",kind:"units",accept:false},
            {tag:"utf8-overflow-below-unit-cap",kind:"utf8",accept:false},
            {tag:"declared-live-byte-overflow",kind:"debit",accept:false},
            {tag:"exact-utf8-cap",kind:"exact",accept:true},
            {tag:"equivalent-small-spelling",kind:"small",accept:true}]
    }
    function test_retained_annotation_is_admitted_before_parsing(data) {
        compare(evaluate("document.body.innerHTML='<a id=carrier-target class=target href=#carrier>Ā漢😀</a>';true"),true)
        var rule=anchor([],"#carrier-target")
        rule.predicates={hint_name:{equals:"Ā漢😀"}}
        var value=stable(rule)
        compare(seal(value),true,"Real original raw/native evidence passes before carrier mutation")
        var setup=evaluate("(()=>{const batch=window.__ferric_browserHintState.interactionProof,annotation=batch.annotation,originalBytes=new TextEncoder().encode(annotation).length,cap=2*1024*1024,kind="+JSON.stringify(data.kind)+";let padding=2;if(kind==='units')padding=cap;else if(kind==='utf8')padding=cap-annotation.length;else if(kind==='exact')padding=cap-originalBytes;else if(kind==='debit')padding=batch.annotation_work-batch.fragment_catalogue.length-originalBytes+1;if(padding<0)throw Error('invalid-carrier-fixture');const carrier=' '.repeat(padding)+annotation,spy={parse:JSON.parse,batch:batch,annotation:annotation,carrier:carrier,calls:0};window.annotationParseSpy=spy;JSON.parse=function(text){if(text===spy.carrier)spy.calls++;return spy.parse.apply(this,arguments);};batch.annotation=carrier;return {units:carrier.length,bytes:padding+originalBytes,debit:batch.annotation_work,liveLimit:batch.annotation_work-batch.fragment_catalogue.length}})()",WebEngineScript.ApplicationWorld)
        try {
            if(data.kind==="utf8"){
                compare(setup.units,2097152)
                verify(setup.bytes>2097152,JSON.stringify(setup))
            }
            if(data.kind==="exact")compare(setup.bytes,2097152)
            if(data.kind==="debit")compare(setup.bytes,setup.liveLimit+1)
            var accepted=data.kind==="debit"?sealDebit(value,setup.debit,2097152):seal(value)
            compare(accepted,data.accept,"The retained carrier must fit its own byte admission before semantic comparison")
            var calls=evaluate("window.annotationParseSpy.calls",WebEngineScript.ApplicationWorld)
            if(data.accept)verify(calls>0,"The compatible representation reaches JSON parsing")
            else compare(calls,0,"An inadmissible renderer carrier never reaches JSON.parse")
        } finally {
            evaluate("(()=>{const spy=window.annotationParseSpy;if(spy){JSON.parse=spy.parse;spy.batch.annotation=spy.annotation;delete window.annotationParseSpy;}return true})()",WebEngineScript.ApplicationWorld)
        }
    }
    function test_catalogue_returns_the_same_string_that_was_validated() {
        var value=stable(anchor([],"#slotted-primary"))
        compare(seal(value),true)
        compare(captureCatalogue(value),true)
        compare(evaluate("(()=>{const batch=window.__ferric_browserHintState.interactionProof,catalogue=batch.fragment_catalogue,forged=JSON.parse(catalogue);forged.targets[0].fragments.projected_primary_fragment_css.width=-7;const spy={batch:batch,annotation:batch.annotation,catalogue:catalogue,forged:JSON.stringify(forged),reads:0,forgedReads:0,debit:batch.annotation_work};window.catalogueReadSpy=spy;batch.annotation_work=2097152;Object.defineProperty(batch,'annotation',{configurable:true,enumerable:true,get:function(){spy.reads=0;return spy.annotation;}});Object.defineProperty(batch,'fragment_catalogue',{configurable:true,enumerable:true,get:function(){if(++spy.reads===2){spy.forgedReads++;return spy.forged;}return spy.catalogue;}});return true})()",WebEngineScript.ApplicationWorld),true)
        try {
            compare(captureCatalogue(value),true)
            var fetched=evaluate("window.catalogueReadSpy.forgedReads",WebEngineScript.ApplicationWorld)
            evaluate("(()=>{const spy=window.catalogueReadSpy;Object.defineProperty(spy.batch,'annotation',{value:spy.annotation,writable:true,configurable:true,enumerable:true});Object.defineProperty(spy.batch,'fragment_catalogue',{value:spy.catalogue,writable:true,configurable:true,enumerable:true});return true})()",WebEngineScript.ApplicationWorld)
            compare(fetched,0,"A validated native probe cannot refetch a different catalogue on return")
            compare(revalidateCatalogue(),true,"The retained opaque receipt contains the verified original catalogue")
        } finally {
            evaluate("(()=>{const spy=window.catalogueReadSpy;if(spy){Object.defineProperty(spy.batch,'annotation',{value:spy.annotation,writable:true,configurable:true,enumerable:true});Object.defineProperty(spy.batch,'fragment_catalogue',{value:spy.catalogue,writable:true,configurable:true,enumerable:true});spy.batch.annotation_work=spy.debit;delete window.catalogueReadSpy;}return true})()",WebEngineScript.ApplicationWorld)
        }
    }
    function test_native_catalogue_receipt_is_captured_geometry_not_a_fresh_action_permit() {
        var value=stable(anchor([],"#slotted-primary"))
        compare(captureCatalogue(value),true)
        var before=evaluate("document.getElementById('slotted-primary').getBoundingClientRect().x")
        compare(evaluate("document.getElementById('slotted-primary').style.cssText='position:relative;left:30px';true"),true)
        verify(evaluate("document.getElementById('slotted-primary').getBoundingClientRect().x")!==before)
        // Deliberately mask only the observer fence to isolate the receipt's
        // historical-fact contract. Shipping freshness still requires it and
        // subsequent bounded geometry/hit checks before any action.
        maskTracking(value)
        compare(revalidateCatalogue(),true)
    }
    function test_native_catalogue_preserves_collector_and_owner_coordinate_spaces_data() {
        return [{tag:"top",kind:"top"},{tag:"shadow",kind:"shadow"},{tag:"same-origin-frame",kind:"frame"},
            {tag:"wrapped",kind:"wrapped"},{tag:"predicate-excluded",kind:"excluded"},{tag:"unavailable",kind:"unavailable"}]
    }
    function test_native_catalogue_preserves_collector_and_owner_coordinate_spaces(data) {
        var rule=anchor([],"#slotted-primary")
        if(data.kind==="shadow")rule=shadowFixture()
        if(data.kind==="frame")rule=anchor([{kind:"same-origin-frame",host_selector:selector("#same-origin")}],"#frame-primary")
        if(data.kind==="wrapped") {
            compare(evaluate("document.getElementById('delayed').innerHTML='<div style=width:100px><a id=wrapped href=#wrapped>Several wrapped words on multiple lines</a></div>';true"),true)
            rule=anchor([],"#wrapped")
        }
        if(data.kind==="excluded")rule.predicates={hint_name:{equals:"different"}}
        if(data.kind==="unavailable")rule=anchor([{kind:"open-shadow",host_selector:selector("#absent")}])
        var value=stable(rule)
        var count=evaluate("JSON.parse(window.__ferric_browserHintState.interactionProof.fragment_catalogue).targets.length",WebEngineScript.ApplicationWorld)
        if(data.kind==="unavailable")compare(count,0)
        else verify(count>0,"The positive control must retain a raw target")
        if(data.kind==="wrapped")verify(evaluate("JSON.parse(window.__ferric_browserHintState.interactionProof.fragment_catalogue).targets[0].fragments.eligible_fragments.length",WebEngineScript.ApplicationWorld)>1)
        compare(captureCatalogue(value),true)
        compare(revalidateCatalogue(),true)
    }
    function test_native_debit_is_required_from_the_original_batch_data() {
        return [{tag:"missing",change:"missing"},{tag:"non-ascii",change:"non-ascii"},{tag:"over-budget",change:"over-budget"}]
    }
    function test_native_debit_is_required_from_the_original_batch(data) {
        var value=stable(anchor())
        var saved=evaluate("(()=>{const batch=window.__ferric_browserHintState.interactionProof;return {debit:batch.annotation_work,catalogue:batch.fragment_catalogue}})()",WebEngineScript.ApplicationWorld)
        verify(Number.isSafeInteger(saved.debit)&&saved.debit>0)
        compare(seal(value),true)
        compare(sealDebit(value,saved.debit,2097152),true)
        if(data.change==="missing")compare(evaluate("delete window.__ferric_browserHintState.interactionProof.annotation_work;true",WebEngineScript.ApplicationWorld),true)
        if(data.change==="non-ascii")compare(evaluate("window.__ferric_browserHintState.interactionProof.fragment_catalogue='é';true",WebEngineScript.ApplicationWorld),true)
        if(data.change==="over-budget")compare(evaluate("window.__ferric_browserHintState.interactionProof.fragment_catalogue=' '.repeat("+(saved.debit+1)+");true",WebEngineScript.ApplicationWorld),true)
        compare(seal(value),true,"The graph remains current; this rejection must come from its debit facts")
        compare(sealDebit(value,saved.debit,2097152),false)
        compare(evaluate("(()=>{const batch=window.__ferric_browserHintState.interactionProof;batch.annotation_work="+saved.debit+";batch.fragment_catalogue="+JSON.stringify(saved.catalogue)+";return true})()",WebEngineScript.ApplicationWorld),true)
        compare(sealDebit(value,saved.debit,2097152),true)
    }
    function test_native_debit_cannot_refund_discarded_prefix_reservations() {
        compare(evaluate("(()=>{document.body.innerHTML='<a class=debit href=#one>One</a><a class=debit href=#two>Two</a>';document.querySelectorAll('a')[1].setAttribute('data-key','x'.repeat(10000));return true})()"),true)
        var rule=anchor([],".debit");rule.predicates={attributes:[{name:"data-key",present:true}]}
        var value=stable(rule,"","links",false,{annotation_bytes:4096})
        compare(value.annotation.outcomes[0].result.unavailable.reason,"annotation-limit")
        var saved=evaluate("(()=>{const batch=window.__ferric_browserHintState.interactionProof;return {debit:batch.annotation_work,live:new TextEncoder().encode(batch.annotation).length+new TextEncoder().encode(batch.fragment_catalogue).length}})()",WebEngineScript.ApplicationWorld)
        compare(saved.debit,4096)
        verify(saved.live<saved.debit)
        compare(seal(value),true)
        compare(sealDebit(value,saved.live,4096),false,"Live bytes alone cannot refund a discarded prefix")
        compare(sealDebit(value,4096,4096),true)
    }
    function test_native_catalogue_matches_the_final_raw_targets_data() {
        return ["invalid-json","unknown-field","missing-target","extra-target","duplicate-target",
            "candidate-reference","document","kind","policy","border","viewport","primary-index",
            "primary-rect","empty-fragments","duplicate-fragment","fragment-index","fragment-rect",
            "scope-clip","owner-clip"].map(function(change){return {tag:change,change:change}})
    }
    function test_native_catalogue_matches_the_final_raw_targets(data) {
        var value=stable(anchor([],"#slotted-primary"))
        var original=evaluate("window.__ferric_browserHintState.interactionProof.fragment_catalogue",WebEngineScript.ApplicationWorld)
        var parsed=JSON.parse(original)
        verify(parsed.targets.length>0)
        // A test-only full debit isolates catalogue integrity from the byte
        // floor. The unchanged original must pass before any adversarial edit.
        compare(evaluate("window.__ferric_browserHintState.interactionProof.annotation_work=2097152;true",WebEngineScript.ApplicationWorld),true)
        compare(sealDebit(value,2097152,2097152),true)
        var script="(()=>{const batch=window.__ferric_browserHintState.interactionProof;const catalogue=JSON.parse(batch.fragment_catalogue),entry=catalogue.targets[0],facts=entry.fragments;const change="+JSON.stringify(data.change)+";"
            +"if(change==='invalid-json'){batch.fragment_catalogue='!';return true;}"
            +"if(change==='unknown-field')catalogue.extra=true;"
            +"if(change==='missing-target')catalogue.targets.shift();"
            +"if(change==='extra-target'){const extra=JSON.parse(JSON.stringify(entry));extra.candidate.index=5000;catalogue.targets.push(extra);}"
            +"if(change==='duplicate-target')catalogue.targets.push(entry);"
            +"if(change==='candidate-reference')entry.candidate.element_id++;"
            +"if(change==='document')entry.document++;"
            +"if(change==='kind')facts.kind='unrecognized';"
            +"if(change==='policy')facts.policy='unrecognized';"
            +"if(change==='border')facts.target_border_box_css.width++;"
            +"if(change==='viewport')facts.top_viewport_css.width++;"
            +"if(change==='primary-index')facts.fragment_index=8;"
            +"if(change==='primary-rect')facts.projected_primary_fragment_css.x++;"
            +"if(change==='empty-fragments')facts.eligible_fragments=[];"
            +"if(change==='duplicate-fragment')facts.eligible_fragments.push(facts.eligible_fragments[0]);"
            +"if(change==='fragment-index')facts.eligible_fragments[0].index=8;"
            +"if(change==='fragment-rect')facts.eligible_fragments[0].rect.width=-1;"
            +"if(change==='scope-clip')facts.scope_clip_css.width=-1;"
            +"if(change==='owner-clip')facts.owner_document_clip_css.height=-1;"
            // Synchronize the private cache as well: malformed geometry must
            // fail its raw witness/shape checks, not just a string comparison.
            +"for(const target of catalogue.targets){const captured=batch.targets.get(target.candidate.index);if(captured)batch.targets.set(target.candidate.index,Object.assign({},captured,{fragment:target,fragment_json:JSON.stringify(target),fragment_bytes:JSON.stringify(target).length}));}"
            +"batch.fragment_catalogue=JSON.stringify(catalogue);return true})()"
        compare(evaluate(script,WebEngineScript.ApplicationWorld),true)
        compare(seal(value),true,"Original live graph remains valid")
        compare(sealDebit(value,2097152,2097152),false,"Catalogue integrity must reject "+data.change)
    }
    function test_native_catalogue_preserves_original_index_when_rectangles_tie() {
        var value=stable(anchor([],"#slotted-primary"))
        var original=JSON.parse(evaluate("window.__ferric_browserHintState.interactionProof.fragment_catalogue",WebEngineScript.ApplicationWorld))
        compare(original.targets.length,1)
        compare(original.targets[0].fragments.eligible_fragments.length,1)
        compare(original.targets[0].fragments.fragment_index,0)
        compare(seal(value),true)
        compare(evaluate("window.__ferric_browserHintState.interactionProof.annotation_work=2097152;true",WebEngineScript.ApplicationWorld),true)
        compare(sealDebit(value,2097152,2097152),true)
        value.annotation_json=evaluate("(()=>{const batch=window.__ferric_browserHintState.interactionProof,catalogue=JSON.parse(batch.fragment_catalogue),entry=catalogue.targets[0],facts=entry.fragments;facts.eligible_fragments.push({index:1,rect:facts.eligible_fragments[0].rect});facts.fragment_index=1;const annotation=JSON.parse(batch.annotation);annotation.outcomes[0].result['anchor-observed'].targets[0].fragment_index=1;batch.annotation=JSON.stringify(annotation);batch.fragment_catalogue=JSON.stringify(catalogue);const captured=batch.targets.get(entry.candidate.index);batch.targets.set(entry.candidate.index,Object.assign({},captured,{fragment:entry,fragment_json:JSON.stringify(entry),fragment_bytes:JSON.stringify(entry).length}));return batch.annotation})()",WebEngineScript.ApplicationWorld)
        value.annotation=JSON.parse(value.annotation_json)
        compare(seal(value),true)
        compare(sealDebit(value,2097152,2097152),false,"Equal Y/X rectangles retain the first original index")
    }
    function test_original_query_plan_is_required_despite_valid_graph_identity() {
        var rule=anchor([],".target")
        rule.predicates={width_px:{min:0.01}}
        var value=stable(rule)
        compare(seal(value),true)
        compare(sealPlan(value,value.producerPlan),true)
        var numericSpelling=value.producerPlan.replace('"min":0.01','"min":1e-2')
        verify(numericSpelling!==value.producerPlan)
        compare(sealPlan(value,numericSpelling),true)
        var different=JSON.parse(value.producerPlan)
        different.rules[0].target_selector.steps[0][1]="#absent"
        compare(sealPlan(value,JSON.stringify(different)),false)
        // The unmodified graph still seals, proving the negative did not
        // depend on retiring its original nodes or changing its stamp.
        compare(seal(value),true)
        compare(sealPlan(value,value.producerPlan),true)
        compare(evaluate("delete window.__ferric_browserHintState.interactionProof.plan;true",WebEngineScript.ApplicationWorld),true)
        compare(seal(value),true)
        compare(sealPlan(value,value.producerPlan),false)
    }
    function test_near_limit_query_plan_seals_without_a_smaller_native_limit_data() {
        return [{tag:"short-compounds",predicates:false},{tag:"escaped-predicates",predicates:true}]
    }
    function test_near_limit_query_plan_seals_without_a_smaller_native_limit(data) {
        var steps=[]
        for(var i=0;i<512;i++)steps.push([i===0?null:"descendant","a",1])
        var rules=[]
        for(var n=0;n<32;n++) {
            var rule=anchor([])
            rule.rule.id="max"+n
            if(n<16)rule.target_selector={steps:steps}
            else {
                rule.rule.kind="collection"
                delete rule.target_selector
                rule.slot_count=1
                rule.container_self=true
                rule.item_selector={steps:steps}
                rule.primary_root_steps=[]
                rule.primary_self=true
                rule.order="dom"
            }
            if(data.predicates) {
                var match='\\"'.repeat(128)
                var predicates={hint_name:{equals:match},attributes:[]}
                for(var a=0;a<4;a++)predicates.attributes.push({name:"data-"+a,equals:match})
                rule[n<16?"predicates":"primary_predicates"]=predicates
            }
            rules.push(rule)
        }
        var value=stable(rules)
        compare(value.annotation.outcomes.length,32)
        verify(value.producerPlan.length>200000)
        compare(seal(value),true)
        compare(sealPlan(value,value.producerPlan),true)
    }
    function shadowFixture() {
        compare(evaluate("(()=>{const host=document.getElementById('catalog');host.innerHTML='<a id=one class=target slot=primary href=#one>One</a>';host.shadowRoot.innerHTML='<slot name=primary></slot>';return true})()"),true)
        return anchor([{kind:"open-shadow",host_selector:selector("#catalog")},
            {kind:"assigned-slot",host_selector:selector("slot"),assigned_selector:selector("#one")}],"#one")
    }
    function test_exact_original_graph_and_harmless_json_reencoding() {
        var value=stable(shadowFixture())
        compare(seal(value),true)
        function reorder(value) {
            if(value===null||typeof value!=="object")return value
            if(Array.isArray(value))return value.map(reorder)
            var result={}
            Object.keys(value).reverse().forEach(function(key){result[key]=reorder(value[key])})
            return result
        }
        var encoded=" \n"+JSON.stringify(reorder(value.annotation))+"\n"
        verify(encoded!==value.annotation_json)
        compare(seal(value,encoded),true)
    }
    function test_live_valid_ids_cannot_substitute_for_original_references() {
        var value=stable(shadowFixture())
        var wire=JSON.parse(value.annotation_json),root=wire.roots[0]
        root.hops[0]["open-shadow"].host=root.resolved
        compare(seal(value,JSON.stringify(wire)),false)
        compare(seal(value),true)
    }
    function test_complete_live_graph_cannot_be_reassigned_to_a_different_rule() {
        compare(evaluate("(()=>{document.getElementById('catalog').shadowRoot.innerHTML='<a id=one href=#one>One</a>';const other=document.createElement('div');other.id='another';document.body.append(other);other.attachShadow({mode:'open'}).innerHTML='<a id=two href=#two>Two</a>';return true})()"),true)
        var first=anchor([{kind:"open-shadow",host_selector:selector("#catalog")}],"#one")
        var second=anchor([{kind:"open-shadow",host_selector:selector("#another")}],"#two")
        second.rule.id="second"
        var value=stable([first,second])
        compare(seal(value),true)
        var wire=JSON.parse(value.annotation_json)
        wire.outcomes[0].result=wire.outcomes[1].result
        var selected=wire.outcomes[1].result["anchor-observed"].root
        wire.roots=wire.roots.filter(function(root){return root.id===selected})
        compare(seal(value,JSON.stringify(wire)),false)
    }
    function test_root_replacement_is_detected_with_observer_flags_masked() {
        var value=stable(shadowFixture())
        compare(seal(value),true)
        compare(evaluate("(()=>{const host=document.getElementById('catalog'),next=host.cloneNode(true);next.attachShadow({mode:'open'}).innerHTML='<slot name=primary></slot>';host.replaceWith(next);return true})()"),true)
        maskTracking(value)
        compare(seal(value),false)
    }
    function test_direct_slot_relationship_is_checked_with_flags_masked() {
        var value=stable(shadowFixture())
        compare(evaluate("document.getElementById('one').slot='other';true"),true)
        maskTracking(value)
        compare(seal(value),false)
    }
    function test_raw_excluded_items_are_checked_before_normalization() {
        compare(evaluate("(()=>{const list=document.createElement('div');list.id='items';for(let i=0;i<2;i++){const item=document.createElement('article');item.className='item';item.id='item'+i;item.dataset.include=i?'no':'yes';item.attachShadow({mode:'open'}).innerHTML='<a href=#'+i+'>Primary '+i+'</a>';list.append(item)}document.body.append(list);return true})()"),true)
        var rule={rule:{kind:"collection",id:"fixture"},root:{start:"top-document",steps:[]},container_selector:selector("#items"),
            item_selector:selector(".item"),primary_root_steps:[{kind:"open-shadow",self:true}],primary_selector:selector("a"),
            item_attributes:[{name:"data-include",equals:"yes"}],slot_count:1,order:"dom"}
        var value=stable(rule)
        compare(value.annotation.outcomes[0].result["collection-observed"].items.length,2)
        compare(seal(value),true)
        compare(evaluate("document.getElementById('item1').remove();true"),true)
        maskTracking(value)
        compare(seal(value),false)
    }
    function test_related_proxy_replacement_is_checked_with_flags_masked() {
        compare(evaluate("document.body.insertAdjacentHTML('beforeend','<input id=check type=checkbox style=\"opacity:0;position:absolute\"><label class=target for=check style=\"display:block;width:200px;height:40px\">Proxy</label>');true"),true)
        var value=stable(anchor(),"","actions",true)
        compare(value.annotation.outcomes[0].result["anchor-observed"].targets.length,1)
        compare(seal(value),true)
        compare(evaluate("(()=>{const old=document.getElementById('check');old.replaceWith(old.cloneNode());return true})()"),true)
        maskTracking(value)
        compare(seal(value),false)
    }
    function scopedFixture(child, documentExpression) {
        compare(evaluate("(()=>{const doc="+(documentExpression||(child?"document.getElementById('same-origin').contentDocument":"document"))+";doc.body.insertAdjacentHTML('beforeend','<section id=permit role=region aria-label=Proof-permit style=\"display:block;width:300px;height:80px\"><button class=target id=scoped-target>Scoped target</button></section><section id=outside>Outside</section>');return true})()"),true)
        var snapshot={family:"regions",configuration_revision:0,selector_revision:"1111111111111111",policies:{}}
        var regions=evaluate(Scripts.hintCollector("regions",false,snapshot),WebEngineScript.ApplicationWorld)
        var row=regions.candidates.filter(function(row){return row.text==="Proof-permit"})[0]
        verify(!!row,JSON.stringify(regions))
        var scope={lease_id:"proof-permit",element_id:row.element_id,frame_path:row.frame_path,kind:row.kind,
            selector_revision:snapshot.selector_revision,parent_scope_lease_id:null}
        var request=Object.assign({configuration_revision:0},scope)
        compare(evaluate(Scripts.hintAcquireScopeLease(request,row,snapshot),WebEngineScript.ApplicationWorld).ok,true)
        evaluate(Scripts.hintStopTracking(scope.lease_id),WebEngineScript.ApplicationWorld)
        return scope
    }
    function test_original_scope_and_current_containment_are_required() {
        var scope=scopedFixture(false),value=stable(anchor(),scope.lease_id,"all")
        compare(seal(value,undefined,scope),true)
        compare(seal(value),false)
        var wrong=Object.assign({},scope,{element_id:scope.element_id+1})
        compare(seal(value,undefined,wrong),false)
        compare(evaluate("document.getElementById('outside').append(document.getElementById('scoped-target'));true"),true)
        maskTracking(value)
        compare(seal(value,undefined,scope),false)
    }
    function test_child_scope_is_bound_even_when_raw_negative_uses_only_top() {
        var scope=scopedFixture(true),rule=anchor([],".missing")
        rule.scope_selector=selector(".missing")
        var value=stable(rule,scope.lease_id,"all")
        compare(value.annotation.outcomes[0].result["search-complete"].cardinality,"absent")
        compare(value.annotation.documents.length,1)
        compare(value.annotation.documents[0].frame_path,"0")
        compare(seal(value,undefined,scope),true)
    }
    function test_scope_ancestor_is_excluded_before_target_evidence_lookup() {
        var scope=scopedFixture(false)
        compare(evaluate("(()=>{const permit=document.getElementById('permit'),ancestor=document.createElement('a');ancestor.id='outside-ancestor';ancestor.href='#outside';permit.before(ancestor);ancestor.append(permit);return true})()"),true)
        var value=stable(anchor([],"#outside-ancestor"),scope.lease_id,"all")
        var observed=value.annotation.outcomes[0].result["anchor-observed"]
        verify(!!observed,JSON.stringify(value.annotation.outcomes[0]))
        compare(observed.targets.length,0)
        compare(seal(value,undefined,scope),true)
    }
    function test_scoped_label_cannot_admit_an_outside_control_data() {
        return [{tag:"checkbox",type:"checkbox"},{tag:"radio",type:"radio"}]
    }
    function test_scoped_label_cannot_admit_an_outside_control(data) {
        var scope=scopedFixture(false)
        compare(evaluate("document.getElementById('outside').innerHTML='<input id=scope-control type="+data.type+" style=\"opacity:0;position:absolute\">';document.getElementById('permit').insertAdjacentHTML('beforeend','<label for=scope-control style=\"display:block;width:200px;height:30px\">Scoped proxy</label>');true"),true)
        var value=evaluate(Scripts.hintCollector("actions",true,undefined,scope.lease_id),WebEngineScript.ApplicationWorld)
        verify(!value.error,JSON.stringify(value))
        compare(value.candidates.filter(function(row){return row.kind==="label-control"}).length,0)
        compare(evaluate("document.getElementById('scope-control').checked"),false)
    }
    function test_scoped_proxy_rechecks_related_containment_before_action_data() {
        return [{tag:"checkbox",type:"checkbox"},{tag:"radio",type:"radio"}]
    }
    function test_scoped_proxy_rechecks_related_containment_before_action(data) {
        var scope=scopedFixture(false)
        compare(evaluate("document.getElementById('permit').insertAdjacentHTML('beforeend','<input id=scope-control type="+data.type+" style=\"opacity:0;position:absolute\"><label for=scope-control style=\"display:block;width:200px;height:30px\">Scoped proxy</label>');true"),true)
        var value=evaluate(Scripts.hintCollector("actions",true,undefined,scope.lease_id),WebEngineScript.ApplicationWorld)
        verify(!value.error,JSON.stringify(value))
        var rows=value.candidates.filter(function(row){return row.kind==="label-control"})
        compare(rows.length,1)
        var row=rows[0]
        compare(evaluate(Scripts.hintFresh(row),WebEngineScript.ApplicationWorld).visible,true)
        compare(evaluate(Scripts.hintClick(row),WebEngineScript.ApplicationWorld),true)
        compare(evaluate("document.getElementById('scope-control').checked"),true)
        compare(evaluate("document.getElementById('scope-control').checked=false;document.getElementById('outside').append(document.getElementById('scope-control'));true"),true)
        compare(evaluate(Scripts.hintFresh(row),WebEngineScript.ApplicationWorld).visible,false)
        compare(evaluate(Scripts.hintClick(row),WebEngineScript.ApplicationWorld),false)
        compare(evaluate("document.getElementById('scope-control').checked"),false)
    }
    function test_nested_frame_scope_has_no_additional_composed_depth_limit_data() {
        return [{tag:"deep-permit-negative",scopeAtTop:false},{tag:"top-permit-deep-target",scopeAtTop:true}]
    }
    function test_nested_frame_scope_has_no_additional_composed_depth_limit(data) {
        var scope=data.scopeAtTop?scopedFixture(false):null
        compare(evaluate("(()=>{window.deepScopeReady=false;"+(data.scopeAtTop?"document.getElementById('permit').replaceChildren();":"document.body.replaceChildren();")+"function build(doc,depth){let parent="+(data.scopeAtTop?"depth===1?doc.getElementById('permit'):doc.body":"doc.body")+";for(let i=0;i<180;i++){const wrapper=doc.createElement('div');wrapper.style.display='contents';parent.append(wrapper);parent=wrapper}const frame=doc.createElement('iframe');frame.className='deep-scope-frame';frame.style.cssText='width:500px;height:200px;border:0';frame.onload=()=>{if(depth<3)build(frame.contentDocument,depth+1);else{window.deepScopeDocument=frame.contentDocument;window.deepScopeReady=true}};frame.srcdoc='<!doctype html><body style=margin:0></body>';parent.append(frame)}build(document,1);return true})()"),true)
        tryVerify(function(){return evaluate("window.deepScopeReady")===true},10000)
        // Each frame has 180 ordinary ancestors, within the collector's local
        // traversal bounds; the complete composed path exceeds 512 parents.
        if(!data.scopeAtTop)scope=scopedFixture(false,"window.deepScopeDocument")
        var rule=anchor([],".missing")
        if(data.scopeAtTop) {
            compare(evaluate("window.deepScopeDocument.body.innerHTML='<button class=target>Deep target</button>';true"),true)
            rule=anchor([1,2,3].map(function(){return {kind:"same-origin-frame",host_selector:selector(".deep-scope-frame")}}),".target")
        } else rule.scope_selector=selector(".missing")
        var value=stable(rule,scope.lease_id,"all")
        if(data.scopeAtTop) {
            verify(!!value.annotation.outcomes[0].result["anchor-observed"],JSON.stringify(value.annotation.outcomes[0]))
            compare(value.annotation.outcomes[0].result["anchor-observed"].targets.length,1)
            compare(value.annotation.documents.length,4)
        } else {
            compare(value.annotation.outcomes[0].result["search-complete"].cardinality,"absent")
            compare(value.annotation.documents.length,1)
        }
        compare(seal(value,undefined,scope),true)
    }
    function test_unknown_facts_and_retired_batches_do_not_bind() {
        var rule=shadowFixture(),value=stable(rule)
        compare(seal(value),true)
        var changed=JSON.parse(value.annotation_json)
        changed.outcomes[0].result["anchor-observed"].targets[0].name={missing:{}}
        compare(seal(value,JSON.stringify(changed)),false)
        var fresh=collect(rule)
        compare(seal(fresh),true)
        compare(seal(value),false)
        evaluate(Scripts.hintStopTracking(),WebEngineScript.ApplicationWorld)
        compare(seal(fresh),false)
    }
    function test_independent_byte_bounds_reject_before_native_work() {
        var value=stable(shadowFixture())
        compare(seal(value," ".repeat(2*1024*1024+1)),false)
        done=false
        probe.validateObjects(broker,null,value.annotation_batch,JSON.stringify(value.annotation.stamp),value.revision,
            value.annotation_json," ".repeat(2049),[],function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        compare(reply,false)
        compare(seal(value),true)
    }
    function runForeign(script) {
        done=false
        broker.run(foreignOwner,script,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000)
        return reply
    }
    function prepareForeign() {
        compare(evaluate("(()=>{const frame=document.getElementById('same-origin');window.foreignObjectReady=false;frame.addEventListener('load',()=>window.foreignObjectReady=true,{once:true});frame.removeAttribute('srcdoc');frame.src='http://localhost:8766/native-receipt-child.html';return true})()"),true)
        tryVerify(function(){return evaluate("window.foreignObjectReady")===true},10000)
        for(var attempt=0;attempt<30&&!foreignOwner;attempt++) {
            evaluate("document.getElementById('same-origin').focus();document.getElementById('same-origin').contentWindow.postMessage('receipt-focus','http://localhost:8766');true")
            wait(20)
            done=false
            broker.focusedChild(topOwner,function(value){reply=value;done=true})
            tryVerify(function(){return done},10000)
            if(reply&&!reply.error)foreignOwner=reply.owner
        }
        verify(!!foreignOwner,JSON.stringify(reply))
        done=false
        foreignProbe.capture(broker,foreignOwner,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000)
        verify(reply.ok,JSON.stringify(reply))
        compare(reply.documents.length,2)
        compare(reply.documents[1].origin,"http://localhost:8766")
        compare(reply.documents[1].focused,true)
        compare(runForeign("(()=>{const host=document.createElement('div');host.id='foreign-root';document.body.append(host);host.attachShadow({mode:'open'}).innerHTML='<a id=foreign-link href=#foreign>Foreign link</a>';return true})()"),true)
    }
    function collectForeign() {
        var request={version:1,observations:true,stamp:stamp(),
            context:{start:"focused-frame",base_depth:1,owner:foreignOwner,receiver_origin:"http://localhost:8766"},
            rules:[{rule:{kind:"anchor",id:"fixture"},
                root:{start:"focused-frame",receiver_origin:"http://localhost:8766",steps:[{kind:"open-shadow",host_selector:selector("#foreign-root")}]},
                target_selector:selector("#foreign-link")}]}
        var script="JSON.stringify("+Scripts.hintCollector("links",false,undefined,"",false,undefined,request)+")"
        runForeign(script);wait(50)
        var value=JSON.parse(runForeign(script))
        verify(!value.error,JSON.stringify(value))
        value.producerPlan=JSON.stringify(request)
        value.annotation=JSON.parse(value.annotation_json)
        compare(value.annotation.documents.length,1)
        compare(value.annotation.documents[0].frame_owner,foreignOwner)
        compare(value.annotation.outcomes[0].result["anchor-observed"].targets.length,1)
        return value
    }
    function test_native_foreign_fragment_geometry_uses_the_retained_parent() {
        prepareForeign()
        done=false
        foreignProbe.captureComposed(broker,function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        verify(reply.ok,JSON.stringify(reply))
        var value=collectForeign()
        var dimensions=runForeign("({width:innerWidth,height:innerHeight,debit:window.__ferric_browserHintState.interactionProof.annotation_work})")
        var bounds=value.candidates.map(function(candidate,index){return {index:index,kind:candidate.kind,geometry:candidate.geometry}})
        done=false
        foreignProbe.captureFragments(broker,value.annotation_batch,JSON.stringify(value.annotation.stamp),value.revision,
            value.annotation_json,value.producerPlan,JSON.stringify(bounds),dimensions.debit,2097152,
            dimensions.width,dimensions.height,function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        compare(reply,true)
        done=false
        foreignProbe.fragmentGeometry(broker,function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        verify(reply,JSON.stringify(reply))
        compare(reply.mappings.length,1)
        var geometry=reply
        var expected=evaluate("(()=>{const frame=document.getElementById('same-origin'),r=frame.getBoundingClientRect();return {left:r.left+frame.clientLeft,top:r.top+frame.clientTop}})()")
        compare(geometry.mappings[0].left,expected.left)
        compare(geometry.mappings[0].top,expected.top)
        compare(geometry.mappings[0].scaleX,1)
        compare(geometry.mappings[0].scaleY,1)
        evaluate("document.getElementById('same-origin').style.marginLeft='10px';true")
        done=false
        foreignProbe.fragmentGeometry(broker,function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        verify(!reply,"Changed parent ancestry retires geometry from the original catalogue")
    }
    function test_native_foreign_projection_requires_one_common_point_data() {
        return [{tag:"disjoint",opposite:true,count:0},{tag:"common",opposite:false,count:1}]
    }
    function test_native_foreign_projection_requires_one_common_point(data) {
        prepareForeign()
        runForeign("(()=>{const doc=document,link=doc.getElementById('foreign-root').shadowRoot.getElementById('foreign-link'),cover=doc.createElement('div');link.style='position:fixed;left:0;top:0;display:block;width:200px;height:80px';cover.style='position:fixed;z-index:5;left:"+(data.opposite?0:100)+"px;top:0;width:100px;height:80px;background:white';doc.body.append(cover);return true})()")
        evaluate("(()=>{const frame=document.getElementById('same-origin'),r=frame.getBoundingClientRect(),cover=document.createElement('div');cover.style='position:fixed;z-index:5;left:'+(r.left+frame.clientLeft+100)+'px;top:'+(r.top+frame.clientTop)+'px;width:100px;height:80px;background:white';document.body.append(cover);return true})()")
        done=false
        foreignProbe.captureComposed(broker,function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        verify(reply.ok,JSON.stringify(reply))
        var value=collectForeign()
        var dimensions=runForeign("({width:innerWidth,height:innerHeight,debit:window.__ferric_browserHintState.interactionProof.annotation_work})")
        var bounds=value.candidates.map(function(candidate,index){return {index:index,kind:candidate.kind,geometry:candidate.geometry}})
        done=false
        foreignProbe.captureFragments(broker,value.annotation_batch,JSON.stringify(value.annotation.stamp),value.revision,
            value.annotation_json,value.producerPlan,JSON.stringify(bounds),dimensions.debit,2097152,
            dimensions.width,dimensions.height,function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        compare(reply,true)
        done=false
        foreignProbe.projectFragments(broker,function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        verify(reply,JSON.stringify(reply))
        compare(reply.targets.length,data.count,"A child hit and a different parent hit do not prove exposure")
        compare(reply.limited,false)
    }
    function sealForeign(value,worldProbe) {
        done=false
        var world=worldProbe||foreignProbe
        world.validateObjects(broker,foreignProbe,value.annotation_batch,JSON.stringify(value.annotation.stamp),value.revision,
            value.annotation_json,"null",[value.annotation.documents[0].id],function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        return reply
    }
    function test_foreign_objects_bind_the_exact_native_world_and_original_root() {
        prepareForeign()
        var value=collectForeign()
        compare(sealForeign(value),true)
        compare(sealForeign(value,probe),false)
        compare(sealForeign(value),true)
        compare(runForeign("(()=>{const old=document.getElementById('foreign-root'),next=old.cloneNode(true);next.attachShadow({mode:'open'}).innerHTML=old.shadowRoot.innerHTML;old.replaceWith(next);return true})()"),true)
        wait(50)
        compare(runForeign("(()=>{const state=window.__ferric_browserHintState;state.dirty=false;state.revision="+value.revision+";return true})()"),true)
        compare(sealForeign(value),false)
    }
    function test_foreign_focus_loss_does_not_reject_an_independent_top_graph() {
        prepareForeign()
        var foreign=collectForeign()
        compare(sealForeign(foreign),true)
        compare(evaluate("document.getElementById('slotted-primary').focus();document.activeElement===document.getElementById('slotted-primary')"),true)
        compare(sealForeign(foreign),false)
        var top=stable(anchor([],"#slotted-primary"))
        compare(top.annotation.outcomes[0].result["anchor-observed"].targets.length,1)
        compare(seal(top),true)
    }
}

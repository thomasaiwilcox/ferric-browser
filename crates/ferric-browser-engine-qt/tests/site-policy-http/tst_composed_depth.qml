import QtQuick
import QtTest
import QtWebEngine
import io.github.ferricbrowser 1.0
import Ferric.Testing 1.0
import "../../qml/scripts/BrowserScripts.js" as Scripts

// Native ancestry/depth primitive. No shipping chooser, merged policy result,
// shared verification-budget qualification or action authority is inferred.
TestCase {
    id: tests
    name: "PolicyComposedDepth"
    when: windowShown
    width: 1000
    height: 800
    property bool done: false
    property var reply
    property string topOwner: ""
    property string selectedOwner: ""
    property var ancestry
    FerricHintFrameBroker { id: broker }
    FrameReceiptProbe { id: selectedProbe }
    FrameReceiptProbe { id: topProbe }
    FrameReceiptProbe { id: freshProbe }
    Component { id: temporaryBroker; FerricHintFrameBroker {} }
    WebEngineView {
        id: page
        anchors.fill: parent
        property bool loaded: false
        Component.onCompleted: {
            var script=WebEngine.script()
            script.name="ferric-native-ancestry-lifecycle-test"
            script.sourceCode=Scripts.hintNativeAncestryLifecycle()
            script.injectionPoint=WebEngineScript.DocumentCreation
            script.worldId=WebEngineScript.ApplicationWorld
            script.runsOnSubFrames=true
            userScripts.insert(script)
        }
        onLoadingChanged: function(request) { loaded=request.status===WebEngineView.LoadSucceededStatus }
    }
    function evaluate(script,world) {
        done=false
        page.runJavaScript(script,world===undefined?WebEngineScript.MainWorld:world,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000)
        return reply
    }
    function init() {
        page.loaded=false
        page.url="http://127.0.0.1:8766/roots.html"
        tryVerify(function(){return page.loaded},10000)
        page.forceActiveFocus()
        selectedOwner="";ancestry=null
    }
    function cleanup() {
        if(selectedOwner)runChild(Scripts.hintStopTracking())
        evaluate(Scripts.hintStopTracking(),WebEngineScript.ApplicationWorld)
        broker.clear();selectedProbe.reset();topProbe.reset();freshProbe.reset()
    }
    function setupFrame(mode,localDepth,sameOrigin) {
        var origin=sameOrigin?"http://127.0.0.1:8766":"http://localhost:8766"
        var source=origin+"/native-ancestry-child.html?depth="+localDepth
        compare(evaluate("(()=>{document.body.innerHTML='<div id=spare></div>';let parent=document.body;const mode="+JSON.stringify(mode)+";if(mode==='shadow'||mode==='slot'||mode==='forwarded'){const host=document.createElement('ancestry-parent');host.id='parent';host.style.display='block';document.body.append(host);const root=host.attachShadow({mode:'open'});if(mode==='shadow')parent=root;else{root.innerHTML=mode==='forwarded'?'<ancestry-inner id=inner><slot name=receiver></slot></ancestry-inner>':'<slot name=receiver></slot>';if(mode==='forwarded'){const inner=root.querySelector('#inner'),forwarded=root.querySelector('slot'),slots=inner.attachShadow({mode:'open',slotAssignment:'manual'});slots.innerHTML='<slot id=first></slot><slot id=second></slot>';const first=slots.querySelector('#first'),second=slots.querySelector('#second');first.assign(forwarded);window.ancestryForward={forwarded:forwarded,first:first,second:second};}parent=host;}}const frame=document.createElement('iframe');frame.id='receiver';frame.style.cssText='width:600px;height:300px';if(mode==='slot'||mode==='forwarded')frame.slot='receiver';window.ancestryReady=false;frame.addEventListener('load',()=>window.ancestryReady=true,{once:true});frame.src="+JSON.stringify(source)+";parent.append(frame);window.ancestryFrame=frame;return true})()"),true)
        tryVerify(function(){return evaluate("window.ancestryReady")===true},10000)
        done=false
        broker.attach(page.mainFrame,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000)
        verify(!reply.error,JSON.stringify(reply));topOwner=reply.owner
        for(var attempt=0;attempt<30&&!selectedOwner;attempt++) {
            evaluate("window.ancestryFrame.contentWindow.postMessage('ancestry-focus',"+JSON.stringify(origin)+");true")
            wait(20)
            done=false
            broker.focusedChild(topOwner,function(value){reply=value;done=true})
            tryVerify(function(){return done},10000)
            if(reply&&!reply.error&&reply.owner)selectedOwner=reply.owner
        }
        verify(!!selectedOwner,JSON.stringify(reply))
        done=false
        selectedProbe.capture(broker,selectedOwner,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000);verify(reply.ok,JSON.stringify(reply))
        done=false
        topProbe.capture(broker,topOwner,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000);verify(reply.ok,JSON.stringify(reply))
    }
    function captureComposed(probe) {
        done=false
        probe.captureComposed(broker,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000)
        return reply
    }
    function validate(probe) {
        done=false
        probe.validate(broker,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000)
        return reply
    }
    function runChild(script) {
        done=false
        broker.run(selectedOwner,script,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000)
        return reply
    }
    function sealWorld(value,top) {
        done=false
        var probe=top?topProbe:selectedProbe
        probe.validateObjects(broker,null,value.annotation_batch,JSON.stringify(value.annotation.stamp),value.revision,
            value.annotation_json,"null",[],function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        return reply
    }
    function selector(css) { return {steps:[[null,css,1]]} }
    function collect(localDepth,top) {
        var stamp={profile:"profileid-1",window:"windowid-1",tab:"tabid-2",document:"documentid-3",
            profile_revision:"1",configuration_revision:"1",policy_revision:"1",target_generation:"1",
            selector_revision:"0",view_generation:"1",viewport_generation:"0",collection_epoch:"1",
            provider_epoch:"1",request_id:"1",scope_lease:"",family:"links"}
        var context=top?{start:"top-document",owner:"",base_depth:0,
            focused_document:{epoch:ancestry.epoch,origin:ancestry.origin,depth:ancestry.native_depth}}:
            {start:"focused-frame",owner:selectedOwner,base_depth:ancestry.composed_depth,receiver_origin:ancestry.origin}
        var steps=[]
        for(var level=0;level<localDepth;level++)steps.push({kind:"open-shadow",host_selector:selector("#root"+level)})
        var request={version:1,observations:true,stamp:stamp,context:context,rules:[{rule:{kind:"anchor",id:"depth"},
            root:{start:"focused-frame",receiver_origin:ancestry.origin,steps:steps},target_selector:selector("#target")}]}
        var budget={depth:top?8:8-ancestry.composed_depth,inspections:50000,candidates:5000,predicate_steps:100000,annotation_bytes:2097152}
        var script="JSON.stringify("+Scripts.hintCollector("links",false,undefined,"",false,budget,request)+")"
        var value=JSON.parse(top?evaluate(script,WebEngineScript.ApplicationWorld):runChild(script))
        verify(!value.error,JSON.stringify(value))
        value.annotation=JSON.parse(value.annotation_json)
        return value
    }
    function test_native_composed_depth_controls_real_discovery_data() {
        return [{tag:"direct-foreign-seven",mode:"direct",local:7,top:false,composed:1,valid:true},
            {tag:"shadow-foreign-six",mode:"shadow",local:6,top:false,composed:2,valid:true},
            {tag:"shadow-foreign-seven",mode:"shadow",local:7,top:false,composed:2,valid:false},
            {tag:"slotted-light-foreign-seven",mode:"slot",local:7,top:false,composed:1,valid:true},
            {tag:"forwarded-slot-light-foreign-seven",mode:"forwarded",local:7,top:false,composed:1,valid:true},
            {tag:"shadow-same-origin-six",mode:"shadow",local:6,top:true,composed:2,valid:true},
            {tag:"shadow-same-origin-seven",mode:"shadow",local:7,top:true,composed:2,valid:false}]
    }
    function test_native_composed_depth_controls_real_discovery(data) {
        setupFrame(data.mode,data.local,data.top)
        ancestry=captureComposed(selectedProbe)
        verify(ancestry.ok,JSON.stringify(ancestry))
        compare(ancestry.native_depth,1);compare(ancestry.composed_depth,data.composed)
        compare(validate(selectedProbe),true)
        collect(data.local,data.top);wait(50)
        var value=collect(data.local,data.top),result=value.annotation.outcomes[0].result
        if(data.valid) {
            verify(!!result['anchor-observed'],JSON.stringify(result))
            compare(result['anchor-observed'].targets.length,1)
            verify(value.candidates.length>0)
            compare(sealWorld(value,data.top),true,"Exact native graph accepts the global depth-eight control")
        } else {
            compare(result.unavailable.reason,"unsupported-root")
            compare(value.candidates.length,0)
        }
        compare(validate(selectedProbe),true,"Producer reads must preserve parent ancestry")
    }
    function test_native_graph_uses_captured_depth_despite_a_wrong_builder_context() {
        setupFrame("shadow",7,false)
        ancestry=captureComposed(selectedProbe);verify(ancestry.ok,JSON.stringify(ancestry))
        compare(ancestry.native_depth,1);compare(ancestry.composed_depth,2)
        // Only the QML proposal changes. The opaque native receipt retains 2.
        ancestry.composed_depth=1
        collect(7,false);wait(50)
        var value=collect(7,false),result=value.annotation.outcomes[0].result
        verify(!!result['anchor-observed'],JSON.stringify(result))
        compare(result['anchor-observed'].targets.length,1)
        compare(value.candidates.length,1)
        compare(validate(selectedProbe),true)
        compare(sealWorld(value,false),false,"Native physical-depth checking uses captured ancestry")
    }
    function test_composed_receipt_rejects_focus_moving_into_existing_child() {
        setupFrame("direct",0,false)
        compare(runChild("(()=>{const frame=document.createElement('iframe');frame.id='nested';frame.srcdoc='<button id=button>Nested target</button>';document.body.append(frame);return true})()"),true)
        tryVerify(function(){return runChild("!!document.querySelector('#nested').contentDocument.getElementById('button')")===true},10000)
        compare(runChild("(()=>{document.getElementById('target').focus();return document.hasFocus()})()"),true)
        ancestry=captureComposed(selectedProbe);verify(ancestry.ok,JSON.stringify(ancestry))
        compare(validate(selectedProbe),true)
        compare(runChild("(()=>{document.querySelector('#nested').contentDocument.getElementById('button').focus();return document.hasFocus()})()"),true)
        compare(validate(selectedProbe),false,"An ancestor retaining hasFocus is no longer the selected leaf")
    }
    function test_slot_aba_permanently_retires_original_ancestry() {
        setupFrame("slot",0,false)
        ancestry=captureComposed(selectedProbe);verify(ancestry.ok,JSON.stringify(ancestry))
        compare(validate(selectedProbe),true)
        compare(evaluate("window.ancestryFrame.slot='other';window.ancestryFrame.slot='receiver';window.ancestryFrame.contentWindow.postMessage('ancestry-focus','http://localhost:8766');true"),true)
        wait(50)
        done=false
        freshProbe.capture(broker,selectedOwner,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000);verify(reply.ok,JSON.stringify(reply))
        compare(validate(freshProbe),true,"Native frame/epoch/focus are unchanged")
        compare(validate(selectedProbe),false)
        compare(validate(selectedProbe),false,"Draining mutation records cannot revive the old receipt")
        verify(captureComposed(freshProbe).ok)
    }
    function test_ancestry_ignores_unrelated_updates_with_bounded_observation_data() {
        return [{tag:"direct-style",mode:"direct",change:"style",valid:true},
            {tag:"direct-clock",mode:"direct",change:"text",valid:true},
            {tag:"host-color",mode:"direct",change:"color",valid:true},
            {tag:"host-moved",mode:"direct",change:"move",valid:false},
            {tag:"shadow-style",mode:"shadow",change:"style",valid:true},
            {tag:"slot-style",mode:"slot",change:"style",valid:true},
            {tag:"overflow",mode:"direct",change:"overflow",valid:false}]
    }
    function test_ancestry_ignores_unrelated_updates_with_bounded_observation(data) {
        setupFrame(data.mode,0,false)
        // Keep the clock outside normal flow: adding text to an empty block
        // before the iframe would genuinely move the receiver below it.
        compare(evaluate("document.getElementById('spare').style.cssText='position:fixed;left:900px;top:0';true"),true)
        ancestry=captureComposed(selectedProbe);verify(ancestry.ok,JSON.stringify(ancestry))
        compare(validate(selectedProbe),true)
        var mutation=data.change==="style"?"document.getElementById('spare').style.opacity='0.9'":
            data.change==="text"?"document.getElementById('spare').textContent='42'":
            data.change==="color"?"window.ancestryFrame.style.backgroundColor='rgb(1,2,3)'":
            data.change==="move"?"window.ancestryFrame.style.marginLeft='10px'":
            "for(let n=0;n<1025;n++)document.getElementById('spare').setAttribute('data-clock',String(n))"
        compare(evaluate("(()=>{"+mutation+";return true})()"),true)
        wait(30)
        compare(validate(selectedProbe),data.valid)
    }
    function test_stop_releases_ancestry_before_ordinary_tracking_exists() {
        setupFrame("shadow",0,false)
        compare(evaluate("typeof window.__ferric_browserHintState",WebEngineScript.ApplicationWorld),"undefined")
        ancestry=captureComposed(selectedProbe);verify(ancestry.ok,JSON.stringify(ancestry))
        compare(evaluate("window.__ferric_hintNativeAncestry.size",WebEngineScript.ApplicationWorld),1)
        compare(evaluate("(()=>{window.ancestryDisconnects=0;for(const r of window.__ferric_hintNativeAncestry.values()){const stop=r.observer.disconnect.bind(r.observer);r.observer.disconnect=()=>{window.ancestryDisconnects++;stop();};}return true})()",WebEngineScript.ApplicationWorld),true)
        compare(evaluate(Scripts.hintStopTracking(),WebEngineScript.ApplicationWorld),true)
        compare(evaluate("window.__ferric_hintNativeAncestry.size",WebEngineScript.ApplicationWorld),0)
        compare(evaluate("window.ancestryDisconnects",WebEngineScript.ApplicationWorld),1)
        compare(validate(selectedProbe),false)
    }
    function test_missing_creation_lifecycle_cannot_mint_ancestry() {
        setupFrame("shadow",0,false)
        compare(validate(selectedProbe),true,"The original native frame remains associated")
        compare(evaluate("window.__ferric_hintNativeAncestryLifecycle",WebEngineScript.ApplicationWorld),1)
        compare(evaluate("delete window.__ferric_hintNativeAncestryLifecycle;true",WebEngineScript.ApplicationWorld),true)
        var result=captureComposed(selectedProbe)
        compare(result.ok,false)
        compare(selectedProbe.receiptFrameValid(),false)
        compare(evaluate("window.__ferric_hintNativeAncestry instanceof Map?window.__ferric_hintNativeAncestry.size:0",WebEngineScript.ApplicationWorld),0)
    }
    function test_forwarded_slot_aba_retires_unchanged_direct_assignment() {
        setupFrame("forwarded",0,false)
        ancestry=captureComposed(selectedProbe);verify(ancestry.ok,JSON.stringify(ancestry))
        compare(ancestry.composed_depth,1)
        compare(validate(selectedProbe),true)
        compare(evaluate("(()=>{const frame=window.ancestryFrame,f=window.ancestryForward,original=frame.assignedSlot;f.second.assign(f.forwarded);f.first.assign(f.forwarded);frame.contentWindow.postMessage('ancestry-focus','http://localhost:8766');return frame.assignedSlot===original})()"),true)
        wait(50)
        done=false
        freshProbe.capture(broker,selectedOwner,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000);verify(reply.ok,JSON.stringify(reply))
        compare(validate(freshProbe),true)
        compare(validate(selectedProbe),false)
        compare(validate(selectedProbe),false)
        verify(captureComposed(freshProbe).ok)
    }
    function test_host_path_aba_retires_restored_native_document_and_geometry() {
        setupFrame("direct",0,false)
        ancestry=captureComposed(selectedProbe);verify(ancestry.ok,JSON.stringify(ancestry))
        compare(validate(selectedProbe),true)
        var unchanged=evaluate("(()=>{const frame=window.ancestryFrame,parent=frame.parentNode,next=frame.nextSibling,spare=document.getElementById('spare'),before=frame.getBoundingClientRect(),view=frame.contentWindow;if(typeof spare.moveBefore!=='function')return false;spare.moveBefore(frame,null);parent.moveBefore(frame,next);const after=frame.getBoundingClientRect();frame.contentWindow.postMessage('ancestry-focus','http://localhost:8766');return frame.contentWindow===view&&before.x===after.x&&before.y===after.y&&before.width===after.width&&before.height===after.height})()")
        compare(unchanged,true,"The host path must change and return without changing geometry or native document")
        wait(50)
        done=false
        freshProbe.capture(broker,selectedOwner,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000);verify(reply.ok,JSON.stringify(reply))
        compare(validate(freshProbe),true)
        compare(validate(selectedProbe),false)
        compare(validate(selectedProbe),false)
        verify(captureComposed(freshProbe).ok)
    }
    function test_receipt_retirement_and_clear_release_observed_roots() {
        setupFrame("shadow",0,false)
        ancestry=captureComposed(selectedProbe);verify(ancestry.ok,JSON.stringify(ancestry))
        compare(evaluate("window.__ferric_hintNativeAncestry.size",WebEngineScript.ApplicationWorld),1)
        selectedProbe.reset()
        tryVerify(function(){return evaluate("window.__ferric_hintNativeAncestry.size",WebEngineScript.ApplicationWorld)===0},10000)
        done=false
        freshProbe.capture(broker,selectedOwner,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000);verify(reply.ok,JSON.stringify(reply))
        verify(captureComposed(freshProbe).ok)
        broker.clear()
        compare(evaluate("window.__ferric_hintNativeAncestry.size",WebEngineScript.ApplicationWorld),0)
        compare(validate(freshProbe),false)
    }
    function test_cleanup_reserves_capacity_with_live_observers_data() {
        return [{tag:"receipt-one",mode:"receipt",count:1},{tag:"clear-one",mode:"clear",count:1},
            {tag:"receipt-shared-parent",mode:"receipt",count:2},{tag:"clear-shared-parent",mode:"clear",count:2}]
    }
    function test_cleanup_reserves_capacity_with_live_observers(data) {
        setupFrame("shadow",0,false)
        verify(captureComposed(selectedProbe).ok)
        if(data.count===2) {
            done=false
            freshProbe.capture(broker,selectedOwner,function(value){reply=value;done=true})
            tryVerify(function(){return done},10000);verify(reply.ok,JSON.stringify(reply))
            verify(captureComposed(freshProbe).ok)
        }
        compare(evaluate("window.__ferric_hintNativeAncestry.size",WebEngineScript.ApplicationWorld),data.count)
        compare(evaluate("(()=>{window.ancestryDisconnects=0;for(const record of window.__ferric_hintNativeAncestry.values()){const stop=record.observer.disconnect.bind(record.observer);record.observer.disconnect=()=>{window.ancestryDisconnects++;stop();};}return true})()",WebEngineScript.ApplicationWorld),true)
        compare(selectedProbe.pendingScripts(broker),1,"Receipts for one native parent document share one reserved cleanup slot")
        var refused=0
        for(var index=0;index<128;index++)broker.run(topOwner,"true",function(value){
            if(value&&value.error==="frame-operation-limit")refused++
        })
        compare(refused,1,"Ordinary dispatch cannot borrow the cleanup reservation")
        compare(selectedProbe.pendingScripts(broker),128)
        if(data.mode==="clear")broker.clear()
        else {selectedProbe.reset();freshProbe.reset()}
        tryVerify(function(){return evaluate("window.__ferric_hintNativeAncestry.size",WebEngineScript.ApplicationWorld)===0},10000)
        compare(evaluate("window.ancestryDisconnects",WebEngineScript.ApplicationWorld),data.count)
        tryVerify(function(){return selectedProbe.pendingScripts(broker)===0},10000)
    }
    function test_full_dispatch_refuses_capture_before_installing_observers() {
        setupFrame("shadow",0,false)
        compare(validate(selectedProbe),true)
        for(var index=0;index<128;index++)broker.run(topOwner,"true",function(){})
        compare(selectedProbe.pendingScripts(broker),128)
        done=false
        selectedProbe.captureComposed(broker,function(value){reply=value;done=true})
        verify(done,"A saturated native dispatcher refuses before starting the probe")
        compare(reply.ok,false)
        compare(evaluate("window.__ferric_hintNativeAncestry instanceof Map?window.__ferric_hintNativeAncestry.size:0",WebEngineScript.ApplicationWorld),0)
        tryVerify(function(){return selectedProbe.pendingScripts(broker)===0},10000)
    }
    function test_broker_destruction_releases_at_capacity_while_receipt_survives() {
        setupFrame("shadow",0,false)
        var retiring=temporaryBroker.createObject(tests)
        verify(!!retiring)
        try {
            var parent=captureIndependentBroker(retiring)
            compare(evaluate("window.__ferric_hintNativeAncestry.size",WebEngineScript.ApplicationWorld),1)
            var refused=0
            for(var index=0;index<128;index++)retiring.run(parent,"true",function(value){
                if(value&&value.error==="frame-operation-limit")refused++
            })
            compare(refused,1)
            compare(freshProbe.pendingScripts(retiring),128)
            retiring.destroy();retiring=null
            tryVerify(function(){return evaluate("window.__ferric_hintNativeAncestry.size",WebEngineScript.ApplicationWorld)===0},10000)
            verify(freshProbe.receiptFrameValid(),"The retained receipt's native frame still exists after broker destruction")
            freshProbe.reset()
            compare(evaluate("window.__ferric_hintNativeAncestry.size",WebEngineScript.ApplicationWorld),0)
        } finally {
            if(retiring)retiring.destroy()
        }
    }
    function captureIndependentBroker(value) {
        done=false
        value.attach(page.mainFrame,function(result){reply=result;done=true})
        tryVerify(function(){return done},10000);verify(!reply.error,JSON.stringify(reply))
        var parent=reply.owner
        done=false
        value.focusedChild(parent,function(result){reply=result;done=true})
        tryVerify(function(){return done},10000);verify(!!reply.owner&&!reply.error,JSON.stringify(reply))
        var child=reply.owner
        done=false
        freshProbe.capture(value,child,function(result){reply=result;done=true})
        tryVerify(function(){return done},10000);verify(reply.ok,JSON.stringify(reply))
        done=false
        freshProbe.captureComposed(value,function(result){reply=result;done=true})
        tryVerify(function(){return done},10000);verify(reply.ok,JSON.stringify(reply))
        return parent
    }
    function test_failed_cleanup_burns_its_slot_without_a_retry_loop() {
        setupFrame("shadow",0,false)
        var retiring=temporaryBroker.createObject(tests)
        verify(!!retiring)
        try {
            var parent=captureIndependentBroker(retiring)
            compare(evaluate("window.__ferric_hintNativeAncestry.size",WebEngineScript.ApplicationWorld),1)
            compare(evaluate("(()=>{const pair=window.__ferric_hintNativeAncestry.entries().next().value;window.brokenAncestryRelease={token:pair[0],record:pair[1],release:pair[1].release,attempts:0};pair[1].release=()=>{window.brokenAncestryRelease.attempts++;throw Error('injected-cleanup-failure');};return true})()",WebEngineScript.ApplicationWorld),true)
            freshProbe.reset()
            tryVerify(function(){return evaluate("window.brokenAncestryRelease.attempts",WebEngineScript.ApplicationWorld)===1},10000)
            wait(50)
            compare(freshProbe.pendingScripts(retiring),1,"An invalid native reply cannot prove disconnect or return capacity")
            var refused=0
            for(var index=0;index<128;index++)retiring.run(parent,"true",function(value){
                if(value&&value.error==="frame-operation-limit")refused++
            })
            compare(refused,1)
            compare(freshProbe.pendingScripts(retiring),128)
            retiring.clear()
            tryVerify(function(){return freshProbe.pendingScripts(retiring)===1},10000)
            compare(evaluate("window.brokenAncestryRelease.attempts",WebEngineScript.ApplicationWorld),1,"Cancellation cannot retry unknown cleanup")
            compare(evaluate("window.__ferric_hintNativeAncestry.size",WebEngineScript.ApplicationWorld),1)
        } finally {
            // Restore the deliberately corrupted fixture before tearing down
            // its independent broker; this is not a production cleanup retry.
            evaluate("(()=>{const broken=window.brokenAncestryRelease;if(broken){broken.record.release=broken.release;broken.record.release();window.__ferric_hintNativeAncestry.delete(broken.token);delete window.brokenAncestryRelease;}return true})()",WebEngineScript.ApplicationWorld)
            freshProbe.reset()
            if(retiring)retiring.destroy()
        }
    }
    function test_native_parent_navigation_disconnects_retained_old_document_data() {
        return [{tag:"navigation",mode:"navigation"},{tag:"detachment",mode:"detachment"},{tag:"pagehide-handler-interference",mode:"interference"}]
    }
    function test_native_parent_navigation_disconnects_retained_old_document(data) {
        // Keep the old parent Document and native observer reachable from Top
        // after real navigation; frame invalidity alone cannot prove cleanup.
        compare(evaluate("(()=>{document.body.innerHTML='';window.ancestryReady=false;const parent=document.createElement('iframe');parent.style.cssText='width:700px;height:400px';parent.addEventListener('load',()=>window.ancestryReady=true,{once:true});parent.src='/native-ancestry-parent.html';document.body.append(parent);window.ancestryParent=parent;return true})()"),true)
        tryVerify(function(){return evaluate("window.ancestryReady")===true},10000)
        done=false
        broker.attach(page.mainFrame,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000);verify(!reply.error,JSON.stringify(reply));topOwner=reply.owner
        var parentOwner=""
        for(var attempt=0;attempt<30&&!selectedOwner;attempt++) {
            evaluate("window.ancestryParent.contentDocument.getElementById('receiver').contentWindow.postMessage('ancestry-focus','http://localhost:8766');true")
            wait(20)
            done=false
            broker.focusedChild(topOwner,function(value){reply=value;done=true})
            tryVerify(function(){return done},10000)
            if(!reply||reply.error||!reply.owner)continue
            parentOwner=reply.owner
            done=false
            broker.focusedChild(parentOwner,function(value){reply=value;done=true})
            tryVerify(function(){return done},10000)
            if(reply&&!reply.error&&reply.owner)selectedOwner=reply.owner
        }
        verify(!!selectedOwner,JSON.stringify(reply))
        if(data.mode==="interference")compare(evaluate("window.ancestryParent.contentWindow.addEventListener('pagehide',event=>event.stopImmediatePropagation(),true);true"),true)
        done=false
        selectedProbe.capture(broker,selectedOwner,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000);verify(reply.ok,JSON.stringify(reply))
        ancestry=captureComposed(selectedProbe);verify(ancestry.ok,JSON.stringify(ancestry))
        compare(ancestry.native_depth,2);compare(ancestry.composed_depth,2)
        compare(validate(selectedProbe),true)
        compare(evaluate("(()=>{const view=document.querySelector('iframe').contentWindow;window.retainedAncestryDocument=view.document;window.retainedAncestryRecord=view.__ferric_hintNativeAncestry.values().next().value;const record=window.retainedAncestryRecord;record.disconnects=0;const stop=record.observer.disconnect.bind(record.observer);record.observer.disconnect=()=>{record.disconnects++;stop();};return record.depth===0})()",WebEngineScript.ApplicationWorld),true)
        if(data.mode!=="detachment") {
            compare(evaluate("window.ancestryReady=false;window.ancestryParent.addEventListener('load',()=>window.ancestryReady=true,{once:true});window.ancestryParent.src='/roots-empty-child.html?retired=1';true"),true)
            tryVerify(function(){return evaluate("window.ancestryReady")===true},10000)
        } else {
            compare(evaluate("window.ancestryParent.remove();true"),true)
            wait(50)
        }
        var retired=evaluate("(()=>{window.retainedAncestryDocument.body.setAttribute('data-retained-probe','changed');return {disconnects:window.retainedAncestryRecord.disconnects,records:window.retainedAncestryRecord.observer.takeRecords().length}})()",WebEngineScript.ApplicationWorld)
        verify(retired.disconnects===1&&retired.records===0,"Retained old realm cleanup: "+JSON.stringify(retired))
        compare(validate(selectedProbe),false)
        compare(evaluate("window.retainedAncestryRecord=null;window.retainedAncestryDocument=null;true",WebEngineScript.ApplicationWorld),true)
    }
}

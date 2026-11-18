import QtQuick
import QtTest
import QtWebEngine
import io.github.ferricbrowser 1.0
import Ferric.Testing 1.0
import "../../qml/scripts/BrowserScripts.js" as Scripts

// Native receipt/producer boundary only. This fixture does not mint the
// application's entered-frame issuer or qualify its capability-menu journey.
TestCase {
    id: tests
    name: "PolicyEnteredDocument"
    when: windowShown
    width: 1000
    height: 800
    property bool done: false
    property var reply
    property var entered
    property string topOwner: ""
    FerricHintFrameBroker { id: broker }
    FrameReceiptProbe { id: selectedProbe }
    FrameReceiptProbe { id: topProbe }
    FrameReceiptProbe { id: siblingProbe }
    WebEngineView {
        id: page
        anchors.fill: parent
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
        page.loaded=false
        page.url="http://127.0.0.1:8766/roots.html"
        tryVerify(function(){return page.loaded},10000)
        captureNativeScope()
    }
    function captureNativeScope() {
        page.forceActiveFocus()
        done=false
        broker.attach(page.mainFrame,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000)
        verify(!reply.error,JSON.stringify(reply));topOwner=reply.owner
        var selected=""
        for(var attempt=0;attempt<30&&!selected;attempt++) {
            evaluate("document.getElementById('same-origin').contentDocument.querySelector('a').focus();true")
            wait(20)
            done=false
            broker.focusedChild(topOwner,function(value){reply=value;done=true})
            tryVerify(function(){return done},10000)
            if(reply&&!reply.error)selected=reply.owner
        }
        verify(!!selected,JSON.stringify(reply))
        done=false
        selectedProbe.capture(broker,selected,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000)
        verify(reply.ok,JSON.stringify(reply))
        var docs=reply.documents,leaf=docs[docs.length-1]
        entered={epoch:leaf.epoch,origin:leaf.origin,depth:docs.length-1}
        done=false
        topProbe.capture(broker,topOwner,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000)
        verify(reply.ok,JSON.stringify(reply))
    }
    function cleanup() {
        evaluate(Scripts.hintStopTracking(),WebEngineScript.ApplicationWorld)
        broker.clear();selectedProbe.reset();topProbe.reset();siblingProbe.reset()
    }
    function selector(css) { return {steps:[[null,css,1]]} }
    function anchor(name,css,child) {
        return {rule:{kind:"anchor",id:name},root:{start:"top-document",steps:child?
            [{kind:"same-origin-frame",host_selector:selector("#same-origin")}]:[]},target_selector:selector(css)}
    }
    function collect(rules,metadata,budget,lease,snapshot,contextOverrides) {
        var stamp={profile:"profileid-1",window:"windowid-1",tab:"tabid-2",document:"documentid-3",
            profile_revision:"1",configuration_revision:"1",policy_revision:"1",target_generation:"1",
            selector_revision:"0",view_generation:"1",viewport_generation:"0",collection_epoch:"1",
            provider_epoch:"1",request_id:"1",scope_lease:lease||"",family:"links"}
        var request={version:1,observations:true,stamp:stamp,
            context:{start:"top-document",owner:"",base_depth:0,entered_document:metadata===undefined?entered:metadata},
            rules:rules||[anchor("inside","#frame-primary",true)]}
        Object.assign(request.context,contextOverrides||{})
        var value=JSON.parse(evaluate("JSON.stringify("+Scripts.hintCollector("links",false,snapshot,lease||"",false,budget,request)+")",WebEngineScript.ApplicationWorld))
        if(value.annotation_json)value.annotation=JSON.parse(value.annotation_json)
        return value
    }
    function stable(rules,metadata,budget,lease,snapshot,contextOverrides) {
        collect(rules,metadata,budget,lease,snapshot,contextOverrides);wait(50)
        return collect(rules,metadata,budget,lease,snapshot,contextOverrides)
    }
    function seal(value,scope) {
        done=false
        topProbe.validateEnteredObjects(broker,selectedProbe,value.annotation_batch,JSON.stringify(value.annotation.stamp),
            value.revision,value.annotation_json,JSON.stringify(scope||null),function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        return reply
    }
    function maskTracking(value) {
        compare(evaluate("(()=>{const s=window.__ferric_browserHintState;s.dirty=false;s.revision="+value.revision+";return true})()",WebEngineScript.ApplicationWorld),true)
    }
    function test_entered_document_restricts_ordinary_and_policy_targets() {
        var value=stable([anchor("outside","#slotted-primary",false),anchor("inside","#frame-primary",true)])
        verify(!value.error,JSON.stringify(value))
        verify(value.candidates.length>0)
        verify(value.candidates.every(function(row){return row.frame_owner===undefined&&row.frame_path.indexOf("0.0")===0}),JSON.stringify(value.candidates))
        var outside=value.annotation.outcomes[0].result["anchor-observed"]
        verify(!!outside,JSON.stringify(value.annotation.outcomes[0]));compare(outside.targets.length,0)
        var inside=value.annotation.outcomes[1].result["anchor-observed"]
        verify(!!inside,JSON.stringify(value.annotation.outcomes[1]));compare(inside.targets.length,1)
        var ref=inside.targets[0].candidate
        compare(ref.frame_owner,"");compare(value.candidates[ref.index].element_id,ref.element_id)
        compare(value.candidates[ref.index].frame_path,"0.0")
        compare(seal(value),true)
        done=false
        topProbe.validateObjects(broker,null,value.annotation_batch,JSON.stringify(value.annotation.stamp),
            value.revision,value.annotation_json,"null",[],function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        compare(reply,false)
    }
    function test_top_only_negative_retains_private_entered_document() {
        var value=stable([anchor("absent",".missing",false)])
        verify(!value.error,JSON.stringify(value))
        compare(value.annotation.documents.length,1);compare(value.annotation.documents[0].frame_path,"0")
        compare(value.annotation.outcomes[0].result["anchor-observed"].targets.length,0)
        compare(evaluate("(()=>{const b=window.__ferric_browserHintState.interactionProof,p=b.entered;return !!p&&WeakRef.prototype.deref.call(b.documents.get(p.document).document)===document.getElementById('same-origin').contentDocument})()",WebEngineScript.ApplicationWorld),true)
        compare(seal(value),true)
        compare(evaluate("document.getElementById('same-origin').contentWindow.__ferric_hintFrameDocument='replaced';true",WebEngineScript.ApplicationWorld),true)
        maskTracking(value)
        compare(seal(value),false)
    }
    function test_nested_descendant_keeps_entered_document_and_charges_depth() {
        compare(evaluate("(()=>{const doc=document.getElementById('same-origin').contentDocument;doc.body.innerHTML='<iframe id=nested style=\"width:300px;height:80px\" srcdoc=\"<a id=deep href=#deep>Deep control</a>\"></iframe>';return true})()"),true)
        tryVerify(function(){return evaluate("!!document.getElementById('same-origin').contentDocument.getElementById('nested').contentDocument.getElementById('deep')")===true},10000)
        compare(evaluate("document.getElementById('same-origin').contentDocument.getElementById('nested').contentDocument.getElementById('deep').focus();true"),true)
        var rule=anchor("nested","#deep",true)
        rule.root.steps.push({kind:"same-origin-frame",host_selector:selector("#nested")})
        var value=stable([rule])
        verify(!value.error,JSON.stringify(value));compare(value.candidates.length,1)
        compare(value.candidates[0].frame_path,"0.0.0")
        compare(value.annotation.outcomes[0].result["anchor-observed"].targets.length,1)
        compare(seal(value),true)
        var limited=collect([rule],undefined,{depth:1})
        verify(!limited.error,JSON.stringify(limited));compare(limited.candidates.length,0)
        verify(!!limited.annotation.outcomes[0].result.unavailable)
    }
    function test_recollection_and_stop_retire_the_original_permit_batch() {
        var value=stable()
        compare(seal(value),true)
        evaluate(Scripts.hintCollector("links"),WebEngineScript.ApplicationWorld)
        compare(evaluate("window.__ferric_browserHintState.enteredScope===null",WebEngineScript.ApplicationWorld),true)
        compare(seal(value),false)
        value=stable()
        compare(seal(value),true)
        evaluate(Scripts.hintStopTracking(),WebEngineScript.ApplicationWorld)
        compare(evaluate("window.__ferric_browserHintState.enteredScope===null",WebEngineScript.ApplicationWorld),true)
        compare(seal(value),false)
    }
    function test_saved_record_rejects_changed_entered_identity() {
        var value=stable(),row=value.candidates.filter(function(candidate){return candidate.text==="Frame primary"})[0]
        verify(!!row)
        compare(evaluate(Scripts.hintFresh(row),WebEngineScript.ApplicationWorld).visible,true)
        compare(evaluate("(()=>{const s=window.__ferric_browserHintState;s.enteredScope=Object.assign({},s.enteredScope);return true})()",WebEngineScript.ApplicationWorld),true)
        compare(evaluate(Scripts.hintFresh(row),WebEngineScript.ApplicationWorld).visible,false)
    }
    function test_replaced_same_url_document_cannot_supply_original_identity() {
        var value=stable()
        compare(seal(value),true)
        compare(evaluate("(()=>{const frame=document.getElementById('same-origin');window.oldEnteredDocument=frame.contentDocument;window.enteredReloaded=false;frame.addEventListener('load',()=>window.enteredReloaded=true,{once:true});frame.srcdoc=frame.srcdoc;return true})()"),true)
        tryVerify(function(){return evaluate("window.enteredReloaded===true")===true},10000)
        maskTracking(value)
        compare(seal(value),false)
        var next=collect()
        verify(!!next.error);compare(next.candidates.length,0)
    }
    function acquireElementScope(location) {
        compare(evaluate(`(()=>{
            const doc=${location==="inside"?"document.getElementById('same-origin').contentDocument":"document"};
            let region;
            if(${JSON.stringify(location)}==='ancestor')region=doc.body;
            else {region=doc.createElement('section');region.style='display:block;width:280px;height:70px';doc.body.append(region);}
            region.setAttribute('role','region');region.setAttribute('aria-label','Entered permit');
            if(${JSON.stringify(location)}==='inside')region.append(doc.getElementById('frame-primary'));
            document.getElementById('same-origin').contentDocument.getElementById('frame-primary').focus();
            return true;
        })()`),true)
        var snapshot={family:"regions",configuration_revision:0,selector_revision:"1111111111111111",policies:{}}
        var regions=evaluate(Scripts.hintCollector("regions",false,snapshot),WebEngineScript.ApplicationWorld)
        var row=regions.candidates.filter(function(candidate){return candidate.text==="Entered permit"})[0]
        verify(!!row,JSON.stringify(regions))
        var scope={lease_id:"entered-element",element_id:row.element_id,frame_path:row.frame_path,kind:row.kind,
            selector_revision:snapshot.selector_revision,parent_scope_lease_id:null}
        var request=Object.assign({configuration_revision:0},scope)
        compare(evaluate(Scripts.hintAcquireScopeLease(request,row,snapshot),WebEngineScript.ApplicationWorld).ok,true)
        evaluate(Scripts.hintStopTracking(scope.lease_id),WebEngineScript.ApplicationWorld)
        return scope
    }
    function test_element_lease_and_entered_document_both_apply_data() {
        return [{tag:"inside",location:"inside"},{tag:"ancestor",location:"ancestor"},{tag:"disjoint",location:"outside"}]
    }
    function test_element_lease_and_entered_document_both_apply(data) {
        var scope=acquireElementScope(data.location),value=stable(undefined,undefined,undefined,scope.lease_id)
        if(data.location==="outside") {
            verify(!!value.error);compare(value.candidates.length,0)
            compare(evaluate("window.__ferric_browserHintState.interactionProof===null",WebEngineScript.ApplicationWorld),true)
            return
        }
        verify(!value.error,JSON.stringify(value))
        verify(value.candidates.length>0)
        verify(value.candidates.every(function(row){return row.frame_path.indexOf("0.0")===0}))
        if(data.location==="inside")compare(value.candidates.length,1)
        compare(seal(value,scope),true)
        compare(seal(value),false)
    }
    function test_same_origin_sibling_receipt_cannot_replace_original_scope() {
        compare(evaluate("(()=>{window.siblingLoaded=false;const frame=document.createElement('iframe');frame.id='scope-sibling';frame.style='width:300px;height:80px';frame.addEventListener('load',()=>window.siblingLoaded=true,{once:true});frame.srcdoc='<a id=sibling href=#same>Sibling</a>';document.body.append(frame);return true})()"),true)
        tryVerify(function(){return evaluate("window.siblingLoaded===true")===true},10000)
        evaluate("document.getElementById('scope-sibling').contentDocument.getElementById('sibling').focus();true")
        done=false
        broker.focusedChild(topOwner,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000);verify(!reply.error,JSON.stringify(reply))
        var owner=reply.owner
        done=false
        siblingProbe.capture(broker,owner,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000);verify(reply.ok,JSON.stringify(reply))
        compare(reply.documents[reply.documents.length-1].origin,entered.origin)
        verify(reply.documents[reply.documents.length-1].epoch!==entered.epoch)
        evaluate("document.getElementById('same-origin').contentDocument.getElementById('frame-primary').focus();true")
        var value=stable()
        compare(seal(value),true)
        done=false
        topProbe.validateEnteredObjects(broker,siblingProbe,value.annotation_batch,JSON.stringify(value.annotation.stamp),value.revision,
            value.annotation_json,"null",function(result){reply=result;done=true})
        tryVerify(function(){return done},10000);compare(reply,false)
        compare(seal(value),true)
    }
    function test_entered_resolution_uses_shared_inspection_and_predicate_work_data() {
        return [{tag:"inspection",budget:{inspections:0}},{tag:"predicate",budget:{predicate_steps:0}},{tag:"depth",budget:{depth:0}}]
    }
    function test_entered_resolution_uses_shared_inspection_and_predicate_work(data) {
        var value=collect(undefined,undefined,data.budget)
        verify(!!value.error);compare(value.candidates.length,0)
        compare(evaluate("window.__ferric_browserHintState.interactionProof===null",WebEngineScript.ApplicationWorld),true)
    }
    function test_transformed_frame_keeps_top_viewport_projection() {
        compare(evaluate("document.getElementById('same-origin').style.cssText='width:180px;height:100px;transform:translate(40px,30px) scale(.8);transform-origin:top left';true"),true)
        var value=stable()
        verify(!value.error,JSON.stringify(value));verify(value.candidates.length>0)
        var box=evaluate("(()=>{const f=document.getElementById('same-origin'),r=f.getBoundingClientRect();return {left:r.left,top:r.top,right:r.right,bottom:r.bottom}})()")
        for(var row of value.candidates) {
            verify(row.geometry.x>=box.left&&row.geometry.y>=box.top)
            verify(row.geometry.x+row.geometry.width<=box.right+.001)
            verify(row.geometry.y+row.geometry.height<=box.bottom+.001)
        }
        compare(value.annotation.outcomes[0].result["anchor-observed"].targets.length,1)
        compare(seal(value),true)
    }
    function captureShadowFrameScope() {
        compare(evaluate("(()=>{const host=document.createElement('div');host.id='entered-shadow';host.style='position:fixed;left:500px;top:400px;width:300px;height:100px';document.body.append(host);host.attachShadow({mode:'open'}).innerHTML='<iframe id=shadow-frame style=\"width:280px;height:80px\" srcdoc=\"<a id=shadow-link href=#shadow>Shadow entry</a>\"></iframe>';return true})()"),true)
        tryVerify(function(){return evaluate("!!document.getElementById('entered-shadow').shadowRoot.getElementById('shadow-frame').contentDocument.getElementById('shadow-link')")===true},10000)
        evaluate("document.getElementById('entered-shadow').shadowRoot.getElementById('shadow-frame').contentDocument.getElementById('shadow-link').focus();true")
        done=false
        broker.focusedChild(topOwner,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000);verify(!reply.error,JSON.stringify(reply))
        var owner=reply.owner
        done=false
        selectedProbe.capture(broker,owner,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000);verify(reply.ok,JSON.stringify(reply))
        var docs=reply.documents,leaf=docs[docs.length-1]
        entered={epoch:leaf.epoch,origin:leaf.origin,depth:docs.length-1}
        compare(entered.depth,1)
    }
    function test_shadow_frame_route_spends_its_global_depth_before_walk() {
        captureShadowFrameScope()
        var rule=anchor("shadow-entry","#shadow-link",false)
        rule.root.steps=[{kind:"open-shadow",host_selector:selector("#entered-shadow")},
            {kind:"same-origin-frame",host_selector:selector("#shadow-frame")}]
        var value=stable([rule],undefined,{depth:2})
        verify(!value.error,JSON.stringify(value));compare(value.candidates.length,1)
        compare(value.candidates[0].frame_path,"0.1")
        compare(value.annotation.outcomes[0].result["anchor-observed"].targets.length,1)
        compare(seal(value),true)
        var limited=collect([rule],undefined,{depth:1})
        verify(!!limited.error);compare(limited.candidates.length,0)
    }
    function innerShadowSteps(count) {
        compare(evaluate(`(()=>{
            const doc=document.getElementById('entered-shadow').shadowRoot.getElementById('shadow-frame').contentDocument;
            let root=doc.body;
            for(let i=0;i<${count};i++){
                const host=doc.createElement('div');host.id='inner-depth-'+i;root.append(host);
                root=host.attachShadow({mode:'open'});
            }
            return true;
        })()`),true)
        var steps=[]
        for(var i=0;i<count;i++)steps.push({kind:"open-shadow",host_selector:selector("#inner-depth-"+i)})
        return steps
    }
    function test_focused_root_uses_composed_depth_in_entered_shadow_document_data() {
        return [{tag:"depth-eight",hops:6,allowed:true},{tag:"depth-nine",hops:7,allowed:false}]
    }
    function test_focused_root_uses_composed_depth_in_entered_shadow_document(data) {
        captureShadowFrameScope()
        var steps=innerShadowSteps(data.hops)
        var rule={rule:{kind:"anchor",id:"focused-depth"},root:{start:"focused-frame",receiver_origin:entered.origin,steps:steps},
            target_selector:selector(".missing")}
        var value=stable([rule],undefined,undefined,"",undefined,{focused_document:entered})
        verify(!value.error,JSON.stringify(value))
        var result=value.annotation.outcomes[0].result
        if(data.allowed) {
            verify(!!result["anchor-observed"],JSON.stringify(result));compare(result["anchor-observed"].targets.length,0)
        } else {
            verify(!!result.unavailable,JSON.stringify(result));compare(result.unavailable.reason,"unsupported-root")
        }
        compare(seal(value),true)
    }
    function test_native_seal_independently_rejects_depth_nine_private_graph() {
        captureShadowFrameScope()
        var steps=innerShadowSteps(7)
        var rule={rule:{kind:"anchor",id:"native-depth"},root:{start:"focused-frame",receiver_origin:entered.origin,steps:steps.slice(0,6)},
            target_selector:selector(".missing")}
        var value=stable([rule],undefined,undefined,"",undefined,{focused_document:entered})
        compare(seal(value),true)
        // Test-only producer corruption: all nodes and native receipts remain
        // genuine. Make the private graph internally consistent at depth nine
        // so the native bound is exercised independently of the producer bound.
        value.annotation_json=evaluate(`(()=>{
            const state=window.__ferric_browserHintState,batch=state.interactionProof;
            const wire=JSON.parse(batch.annotation),root=wire.roots[0];
            const doc=document.getElementById('entered-shadow').shadowRoot.getElementById('shadow-frame').contentDocument;
            let domain=doc.body;
            for(let i=0;i<6;i++)domain=domain.querySelector('#inner-depth-'+i).shadowRoot;
            const host=domain.getElementById('inner-depth-6'),shadow=host.shadowRoot;
            function reference(node){
                let id=state.weak.get(node);
                if(!id){id=state.next++;state.weak.set(node,id);}
                batch.nodes.set(id,{object:new WeakRef(node),document:root.start.document,
                    root:new WeakRef(node.getRootNode()),assigned:null});
                return {document:root.start.document,node:id};
            }
            const ref=reference(shadow);
            root.hops.push({'open-shadow':{host:reference(host),root:ref}});
            root.resolved=ref;
            wire.outcomes[0].result['anchor-observed'].scope=ref;
            batch.annotation=JSON.stringify(wire);
            return batch.annotation;
        })()`,WebEngineScript.ApplicationWorld)
        value.annotation=JSON.parse(value.annotation_json)
        maskTracking(value)
        compare(seal(value),false)
    }
    function collectionDepthRule(primaryHops) {
        captureShadowFrameScope()
        compare(evaluate(`(()=>{
            const doc=document.getElementById('entered-shadow').shadowRoot.getElementById('shadow-frame').contentDocument;
            const host=doc.createElement('div');host.id='collection-depth-host';
            host.innerHTML='<div id=assigned-items slot=items><article id=depth-item></article></div>';
            doc.body.append(host);
            host.attachShadow({mode:'open'}).innerHTML='<slot id=depth-slot name=items></slot>';
            let current=host.querySelector('article');
            for(let i=0;i<5;i++){
                const shadow=current.attachShadow({mode:'open'});
                if(i<4){const next=doc.createElement('div');next.id='primary-depth-'+i;shadow.append(next);current=next;}
            }
            return true;
        })()`),true)
        var primary=[{kind:"open-shadow",self:true}]
        for(var i=1;i<primaryHops;i++)primary.push({kind:"open-shadow",host_selector:selector("#primary-depth-"+(i-1))})
        return {rule:{kind:"collection",id:"combined-depth"},root:{start:"focused-frame",receiver_origin:entered.origin,
            steps:[{kind:"open-shadow",host_selector:selector("#collection-depth-host")},
                {kind:"assigned-slot",host_selector:selector("#depth-slot"),assigned_selector:selector("#assigned-items")}]},
            container_self:true,item_selector:selector("article"),primary_root_steps:primary,primary_selector:selector(".missing"),
            slot_count:1,order:"dom"}
    }
    function test_collection_outer_and_primary_share_authored_depth_data() {
        return [{tag:"depth-eight",hops:4,allowed:true},{tag:"depth-nine",hops:5,allowed:false}]
    }
    function test_collection_outer_and_primary_share_authored_depth(data) {
        var rule=collectionDepthRule(data.hops)
        var value=stable([rule],undefined,undefined,"",undefined,{focused_document:entered})
        verify(!value.error,JSON.stringify(value))
        var collection=value.annotation.outcomes[0].result["collection-observed"]
        verify(!!collection,JSON.stringify(value.annotation.outcomes[0]));compare(collection.items.length,1)
        var primary=collection.items[0].primary
        if(data.allowed) {
            verify(!!primary["targets-complete"],JSON.stringify(primary));compare(primary["targets-complete"].targets.length,0)
        } else {
            verify(!!primary.unavailable,JSON.stringify(primary));compare(primary.unavailable.reason,"unsupported-root")
        }
        compare(seal(value),true)
    }
    function test_native_seal_independently_bounds_outer_plus_primary_slot_hops() {
        var rule=collectionDepthRule(4)
        var value=stable([rule],undefined,undefined,"",undefined,{focused_document:entered})
        compare(seal(value),true)
        // The extra primary hop is physically only depth seven. The two
        // outer declared slot/root hops still count, making the sum nine.
        value.annotation_json=evaluate(`(()=>{
            const state=window.__ferric_browserHintState,batch=state.interactionProof,wire=JSON.parse(batch.annotation);
            const item=wire.outcomes[0].result['collection-observed'].items[0];
            const primary=wire.roots.find(root=>root.id===item.primary['targets-complete'].root);
            const doc=document.getElementById('entered-shadow').shadowRoot.getElementById('shadow-frame').contentDocument;
            let domain=doc.getElementById('depth-item').shadowRoot;
            for(let i=0;i<3;i++)domain=domain.getElementById('primary-depth-'+i).shadowRoot;
            const host=domain.getElementById('primary-depth-3'),shadow=host.shadowRoot;
            function reference(node){
                let id=state.weak.get(node);
                if(!id){id=state.next++;state.weak.set(node,id);}
                batch.nodes.set(id,{object:new WeakRef(node),document:primary.start.document,
                    root:new WeakRef(node.getRootNode()),assigned:null});
                return {document:primary.start.document,node:id};
            }
            const ref=reference(shadow);
            primary.hops.push({'open-shadow':{host:reference(host),root:ref}});primary.resolved=ref;
            batch.annotation=JSON.stringify(wire);
            return batch.annotation;
        })()`,WebEngineScript.ApplicationWorld)
        value.annotation=JSON.parse(value.annotation_json)
        maskTracking(value)
        compare(seal(value),false)
    }
    function test_file_origin_uses_native_nonopaque_identity() {
        evaluate(Scripts.hintStopTracking(),WebEngineScript.ApplicationWorld)
        broker.clear();selectedProbe.reset();topProbe.reset()
        page.loaded=false
        page.url=Qt.resolvedUrl("../fixtures/site-policy/roots.html")
        tryVerify(function(){return page.loaded},10000)
        captureNativeScope()
        console.info("Entered file native origin",entered.origin)
        var value=stable()
        if(entered.origin==="null") {
            verify(!!value.error);compare(value.candidates.length,0)
        } else {
            verify(!value.error,JSON.stringify(value))
            compare(value.annotation.outcomes[0].result["anchor-observed"].targets.length,1)
            compare(seal(value),true)
        }
    }
    function test_invalid_native_metadata_never_falls_back_to_top_data() {
        return [{tag:"wrong-epoch",metadata:{epoch:"11111111-1111-4111-8111-111111111111",origin:"",depth:1}},
                {tag:"invalid-depth",metadata:{epoch:"bad",origin:"",depth:0}}]
    }
    function test_invalid_native_metadata_never_falls_back_to_top(data) {
        var metadata=Object.assign({},data.metadata)
        metadata.origin=entered.origin
        var value=collect(undefined,metadata)
        verify(!!value.error,JSON.stringify(value));compare(value.candidates.length,0)
        compare(evaluate("window.__ferric_browserHintState.interactionProof===null",WebEngineScript.ApplicationWorld),true)
    }
    function test_failed_entered_preparation_retires_old_unentered_rows_data() {
        return [{tag:"epoch",wrongEpoch:true},{tag:"inspection",budget:{inspections:0}},
                {tag:"invalid-budget",budget:{inspections:-1}},{tag:"late-predicate",budget:{predicate_steps:4}},
                {tag:"invalid-root",context:{base_depth:-1}},{tag:"selector-pending",snapshot:{error:"selector-policy-pending"}}]
    }
    function test_failed_entered_preparation_retires_old_unentered_rows(data) {
        var ordinary=evaluate(Scripts.hintCollector("links"),WebEngineScript.ApplicationWorld)
        var row=ordinary.candidates.filter(function(candidate){return candidate.text==="Slotted primary"})[0]
        verify(!!row)
        compare(evaluate(Scripts.hintFresh(row),WebEngineScript.ApplicationWorld).visible,true)
        var metadata=Object.assign({},entered)
        if(data.wrongEpoch)metadata.epoch="11111111-1111-4111-8111-111111111111"
        var failed=collect(undefined,metadata,data.budget,"",data.snapshot,data.context)
        verify(!!failed.error);compare(failed.candidates.length,0)
        compare(evaluate(Scripts.hintFresh(row),WebEngineScript.ApplicationWorld).visible,false)
        compare(evaluate(Scripts.hintClick(row),WebEngineScript.ApplicationWorld),false)
    }
}

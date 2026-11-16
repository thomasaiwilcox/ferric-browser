import QtQuick
import QtTest
import QtWebEngine
import io.github.ferricbrowser 1.0
import Ferric.Testing 1.0
import "../../qml/scripts/BrowserScripts.js" as Scripts

// Exercises the real isolated collector. A pending collection is data only;
// native request ownership and the multi-world shipping consumer are separate.
TestCase {
    id: tests
    name: "PolicyDeferredCollection"
    when: windowShown
    width: 1000
    height: 800
    property bool done: false
    property var reply
    property string topOwner: ""
    property string childOwner: ""
    property var topReceipt
    FerricHintFrameBroker { id: broker }
    FrameReceiptProbe { id: probe }
    FrameReceiptProbe { id: childProbe }
    WebEngineView {
        id: page
        anchors.fill: parent
        property bool loaded: false
        onLoadingChanged: function(request) { loaded=request.status===WebEngineView.LoadSucceededStatus }
    }
    function evaluate(script, world) {
        done=false
        page.runJavaScript(script, world===undefined?WebEngineScript.ApplicationWorld:world,
            function(value) { reply=value;done=true })
        tryVerify(function() { return done },10000)
        return reply
    }
    function init() {
        childOwner=""
        page.loaded=false
        page.url="http://127.0.0.1:8766/roots.html"
        tryVerify(function() { return page.loaded },10000)
        compare(evaluate("(()=>{document.body.innerHTML='<a id=target href=#target style=\"display:block;width:200px;height:40px\">Deferred target</a>';return true})()",WebEngineScript.MainWorld),true)
        page.forceActiveFocus()
        compare(evaluate("document.getElementById('target').focus();true",WebEngineScript.MainWorld),true)
        tryVerify(function() { return evaluate("document.hasFocus()")===true },5000)
        done=false
        broker.attach(page.mainFrame,function(value) { reply=value;done=true })
        tryVerify(function() { return done },5000)
        verify(!reply.error,JSON.stringify(reply))
        var owner=reply.owner
        topOwner=owner
        done=false
        probe.capture(broker,owner,function(value) { reply=value;done=true })
        tryVerify(function() { return done },5000)
        verify(reply.ok,JSON.stringify(reply))
        topReceipt=reply.documents
        compare(topReceipt[0].focused,true)
        // Settle the existing observer's initial notification, as the normal
        // Hint workspace does before its retained-request proof checks.
        evaluate(Scripts.hintCollector("links"))
        wait(60)
    }
    function cleanup() {
        if(childOwner)world(Scripts.hintStopTracking(),childOwner)
        evaluate(Scripts.hintStopTracking());broker.clear();probe.reset();childProbe.reset()
    }
    function world(script,owner) {
        if(!owner)return evaluate(script)
        done=false
        broker.run(owner,script,function(value) { reply=value;done=true })
        tryVerify(function() { return done },10000)
        return reply
    }
    function request(family) {
        return {version:1,observations:true,
            stamp:{profile:"profileid-1",window:"windowid-1",tab:"tabid-2",document:"documentid-3",
                profile_revision:"1",configuration_revision:"1",policy_revision:"1",target_generation:"1",
                selector_revision:"0",view_generation:"1",viewport_generation:"0",collection_epoch:"1",
                provider_epoch:"1",request_id:"1",scope_lease:"",family:family||"links"},
            context:{start:"top-document",base_depth:0,owner:""},
            rules:[{rule:{kind:"anchor",id:"target"},root:{start:"top-document",steps:[]},
                target_selector:{steps:[[null,"#target",1]]}}]}
    }
    function prepare(plan, budget, owner) {
        var script=Scripts.hintCollector(plan.stamp.family,false,undefined,plan.stamp.scope_lease,false,budget,plan,true)
        var value=JSON.parse(world("JSON.stringify("+script+")",owner))
        verify(!value.error,JSON.stringify(value))
        return value
    }
    function test_ordinary_stage_defers_policy_extraction_data() {
        return [{tag:"links",family:"links"},{tag:"actions",family:"actions"}]
    }
    function test_ordinary_stage_defers_policy_extraction(data) {
        var value=prepare(request(data.family))
        compare(value.candidates.length,1)
        compare(value.candidates[0].text,"Deferred target")
        compare(value.annotation_json,undefined,"Ordinary discovery must finish before policy extraction")
        verify(typeof value.interaction_pending==="string" && value.interaction_pending.length===36)
        compare(evaluate("window.__ferric_browserHintState.interactionProof===null"),true)
    }
    function finish(plan,value,budget,owner,omitCandidates,refresh) {
        return JSON.parse(world("JSON.stringify("+Scripts.hintFinishInteraction(plan,value.interaction_pending,budget,omitCandidates,refresh)+")",owner))
    }
    function refreshedFinish(plan,ordinary,budget) {
        budget=budget||{inspections:50000-ordinary.interaction_work.inspections,
            predicate_steps:100000-ordinary.interaction_work.predicate_steps,annotation_bytes:2097152}
        var refreshBudget=Object.assign({depth:8,candidates:5000},budget)
        return finish(plan,ordinary,budget,undefined,false,
            Scripts.hintCollector(plan.stamp.family,false,undefined,"",false,refreshBudget,plan,true))
    }
    function test_busy_page_refreshes_an_identical_inventory_data() {
        return [{tag:"style",mutation:"document.body.style.backgroundColor='rgb(1,2,3)'"},
            {tag:"unrelated-text",mutation:"document.title='Clock tick'"},
            {tag:"target-attribute",mutation:"document.getElementById('target').setAttribute('data-change','changed')"}]
    }
    function test_busy_page_refreshes_an_identical_inventory(data) {
        var plan=request(),ordinary=prepare(plan)
        evaluate(data.mutation+";true",WebEngineScript.MainWorld)
        wait(30)
        verify(evaluate("window.__ferric_browserHintState.dirty"))
        var completed=refreshedFinish(plan,ordinary)
        verify(!completed.error,JSON.stringify(completed))
        compare(JSON.stringify(completed.candidates),JSON.stringify(ordinary.candidates))
        compare(completed.annotation_batch,ordinary.interaction_pending)
        verify(completed.revision>ordinary.revision)
        verify(completed.interaction_work.inspections>=ordinary.interaction_work.inspections,
            "The refresh must spend the remaining request allowance, not reset it")
        var wire=JSON.parse(completed.annotation_json)
        compare(wire.outcomes[0].result["anchor-observed"].targets.length,1)
        compare(wire.discovery_work.inspections,ordinary.interaction_work.inspections+completed.interaction_work.inspections)
    }
    function test_busy_page_refuses_a_changed_inventory_data() {
        return [{tag:"text",mutation:"document.getElementById('target').textContent='Changed target'"},
            {tag:"geometry",mutation:"document.getElementById('target').style.width='250px'"},
            {tag:"replacement",mutation:"const e=document.getElementById('target');e.replaceWith(e.cloneNode(true))"},
            {tag:"new-target",mutation:"document.body.append(document.getElementById('target').cloneNode(true))"}]
    }
    function test_busy_page_refuses_a_changed_inventory(data) {
        var plan=request(),ordinary=prepare(plan)
        evaluate("(()=>{"+data.mutation+";return true})()",WebEngineScript.MainWorld)
        wait(30)
        compare(refreshedFinish(plan,ordinary).error,"stale-interaction-collection")
        compare(evaluate("window.__ferric_browserHintState.interactionProof===null"),true)
    }
    function test_busy_page_refresh_cannot_renew_the_inspection_budget() {
        var plan=request(),ordinary=prepare(plan)
        evaluate("document.title='Clock tick';true",WebEngineScript.MainWorld);wait(30)
        var completed=refreshedFinish(plan,ordinary,{inspections:0,predicate_steps:100000,annotation_bytes:2097152})
        verify(!!completed.error,JSON.stringify(completed))
        compare(evaluate("window.__ferric_browserHintState.interactionProof===null"),true)
    }
    function test_refresh_recomputes_policy_matching_even_when_candidates_are_identical() {
        var plan=request(),ordinary=prepare(plan)
        evaluate("document.getElementById('target').id='no-longer-matches';true",WebEngineScript.MainWorld)
        wait(30)
        var completed=refreshedFinish(plan,ordinary)
        verify(!completed.error,JSON.stringify(completed))
        compare(JSON.stringify(completed.candidates),JSON.stringify(ordinary.candidates))
        compare(JSON.parse(completed.annotation_json).outcomes[0].result["anchor-observed"].targets.length,0)
    }
    function test_refresh_rejects_an_inflated_grant_before_reading_the_dom() {
        var plan=request(),ordinary=prepare(plan)
        evaluate("document.title='Clock tick';true",WebEngineScript.MainWorld);wait(30)
        evaluate("(()=>{const original=document.createTreeWalker;window.refreshWalks=0;document.createTreeWalker=function(){window.refreshWalks++;return original.apply(this,arguments)};return true})()")
        var completed=refreshedFinish(plan,ordinary,{inspections:50000,predicate_steps:100000,annotation_bytes:2097152})
        compare(completed.error,"invalid-collection-budget")
        compare(evaluate("window.refreshWalks"),0)
    }
    function test_snapshot_sealing_survives_refresh_notifications_data() {
        return [{tag:"unrelated-update",replace:false},{tag:"detached-target",replace:true}]
    }
    function test_snapshot_sealing_survives_refresh_notifications(data) {
        var plan=request(),ordinary=prepare(plan),completed=finish(plan,ordinary)
        verify(!completed.error,JSON.stringify(completed))
        evaluate(data.replace?"document.getElementById('target').remove();true":"document.title='Clock tick';true",WebEngineScript.MainWorld)
        wait(30)
        verify(evaluate("window.__ferric_browserHintState.dirty"))
        compare(seal(completed),!data.replace,
            "Snapshot identity survives unrelated updates; detached target objects never seal")
    }
    function test_resume_uses_original_weak_inventory_and_candidate_indices_once_data() {
        return [{tag:"full-projection",omitCandidates:false},{tag:"selection-projection",omitCandidates:true}]
    }
    function test_resume_uses_original_weak_inventory_and_candidate_indices_once(data) {
        var plan=request(),ordinary=prepare(plan)
        verify(typeof ordinary.interaction_pending==="string",JSON.stringify(ordinary))
        compare(evaluate("(()=>{const p=window.__ferric_browserHintState.pendingInteraction;return p.roots.entries.every(entry=>entry.domain instanceof WeakRef&&entry.tree instanceof WeakRef&&entry.nodes.every(node=>node instanceof WeakRef))})()"),true)
        compare(evaluate("(()=>{document.createTreeWalker=function(){throw Error('unexpected-second-walk')};return true})()"),true)
        var completed=finish(plan,ordinary,undefined,undefined,data.omitCandidates)
        verify(!completed.error,JSON.stringify(completed))
        compare(completed.annotation_batch,ordinary.interaction_pending)
        if(data.omitCandidates)compare(completed.candidates.length,0)
        else compare(JSON.stringify(completed.candidates),JSON.stringify(ordinary.candidates))
        compare(completed.selector_revision,ordinary.selector_revision)
        compare(evaluate("window.__ferric_browserHintState.interactionProof.plan"),JSON.stringify(plan))
        var annotation=JSON.parse(completed.annotation_json)
        compare(annotation.outcomes[0].result["anchor-observed"].targets.length,1)
        compare(annotation.outcomes[0].result["anchor-observed"].targets[0].candidate.index,0)
        compare(annotation.discovery_work.inspections,completed.inspected_nodes)
        compare(seal(completed),true)
        compare(evaluate("window.__ferric_browserHintState.pendingInteraction===null"),true)
        compare(finish(plan,ordinary).error,"stale-interaction-collection")
        compare(seal(completed),true,"A duplicate completion cannot clear the original completed graph")
    }
    function seal(value,scope) {
        var annotation=JSON.parse(value.annotation_json)
        done=false
        probe.validateObjects(broker,null,value.annotation_batch,JSON.stringify(annotation.stamp),value.revision,
            value.annotation_json,JSON.stringify(scope||null),[],function(result) { reply=result;done=true })
        tryVerify(function() { return done },10000)
        return reply
    }
    function test_original_request_and_pending_identity_cannot_be_replaced_data() {
        return [{tag:"stamp",change:"stamp"},{tag:"rule",change:"rule"},{tag:"world",change:"world"},
            {tag:"nonce",change:"nonce"},{tag:"epoch",change:"epoch"},
            {tag:"scope",change:"scope"},{tag:"entered",change:"entered"},{tag:"record",change:"record"}]
    }
    function test_original_request_and_pending_identity_cannot_be_replaced(data) {
        var plan=request(),ordinary=prepare(plan),originalNonce=ordinary.interaction_pending
        if(data.change==="stamp")plan.stamp.provider_epoch="2"
        if(data.change==="rule")plan.rules[0].target_selector.steps[0][1]="a"
        if(data.change==="world")plan.context.owner="frame-11111111-1111-4111-8111-111111111111"
        if(data.change==="nonce")ordinary.interaction_pending="11111111-1111-4111-8111-111111111111"
        if(data.change==="epoch")evaluate("window.__ferric_hintFrameDocument='replaced';true")
        if(data.change==="scope")evaluate("window.__ferric_browserHintState.scopeLease={leaseId:'substituted'};true")
        if(data.change==="entered")evaluate("window.__ferric_browserHintState.enteredScope={closed:true};true")
        if(data.change==="record") {
            compare(evaluate("(()=>{const s=window.__ferric_browserHintState,id=s.weak.get(document.getElementById('target'));s.elements.set(id,Object.assign({},s.elements.get(id)));return true})()"),true)
            var completed=finish(plan,ordinary)
            verify(!completed.error,JSON.stringify(completed))
            compare(JSON.parse(completed.annotation_json).outcomes[0].result.unavailable.reason,"changed-document")
        } else compare(finish(plan,ordinary).error,"stale-interaction-collection")
        if(data.change==="nonce")compare(evaluate("window.__ferric_browserHintState.pendingInteraction.nonce"),originalNonce)
        else compare(evaluate("window.__ferric_browserHintState.pendingInteraction===null"),true)
    }
    function test_stop_recollection_and_mutation_retire_the_pending_inventory_data() {
        return [{tag:"stop",change:"stop"},{tag:"ordinary",change:"ordinary"},{tag:"new-preparation",change:"new"},
            {tag:"mutation",change:"mutation"},{tag:"navigation",change:"navigation"}]
    }
    function test_stop_recollection_and_mutation_retire_the_pending_inventory(data) {
        var plan=request(),ordinary=prepare(plan)
        if(data.change==="stop")evaluate(Scripts.hintStopTracking())
        if(data.change==="ordinary")evaluate(Scripts.hintCollector("links"))
        var replacement
        if(data.change==="new")replacement=prepare(plan)
        if(data.change==="mutation") {
            evaluate("document.getElementById('target').setAttribute('data-change','changed');true",WebEngineScript.MainWorld)
            wait(30)
        }
        if(data.change==="navigation") {
            page.loaded=false
            page.reload()
            tryVerify(function() { return page.loaded },10000)
        }
        compare(finish(plan,ordinary).error,"stale-interaction-collection")
        if(replacement)compare(evaluate("window.__ferric_browserHintState.pendingInteraction.nonce"),replacement.interaction_pending)
        else compare(evaluate("!window.__ferric_browserHintState||window.__ferric_browserHintState.pendingInteraction===null"),true)
    }
    function test_remaining_work_is_not_reset_data() {
        return [{tag:"predicate-zero",budget:{predicate_steps:0},reason:"predicate-limit"},
            {tag:"inspection-zero",budget:{inspections:0},reason:"inspection-limit"},
            {tag:"annotation-zero",budget:{annotation_bytes:0},error:"annotation-limit"},
            {tag:"oversized-inspections",budget:{inspections:50000},error:"invalid-collection-budget"},
            {tag:"fraction",budget:{predicate_steps:0.5},error:"invalid-collection-budget"},
            {tag:"string",budget:{annotation_bytes:"2097152"},error:"invalid-collection-budget"},
            {tag:"new-admissions",budget:{candidates:5000},error:"invalid-collection-budget"}]
    }
    function test_remaining_work_is_not_reset(data) {
        var plan=request(),ordinary=prepare(plan),value=finish(plan,ordinary,data.budget)
        if(data.error)compare(value.error,data.error)
        else {
            verify(!value.error,JSON.stringify(value))
            var wire=JSON.parse(value.annotation_json)
            compare(wire.outcomes[0].result.unavailable.reason,data.reason)
            compare(value.candidates.length,1)
            if(data.budget.inspections===0)compare(wire.discovery_work.inspections,ordinary.inspected_nodes)
            if(data.budget.predicate_steps===0)compare(wire.discovery_work.predicate_steps,ordinary.interaction_work.predicate_steps)
        }
        if(data.error&&data.budget.annotation_bytes===0)compare(value.interaction_work.annotation_bytes,0)
        compare(evaluate("window.__ferric_browserHintState.pendingInteraction===null"),true)
    }
    function test_late_finish_preserves_replacement_pending_collection() {
        var plan=request(),first=prepare(plan)
        plan.stamp.request_id="2"
        var second=prepare(plan)
        verify(first.interaction_pending!==second.interaction_pending)
        compare(finish(request(),first).error,"stale-interaction-collection")
        compare(evaluate("window.__ferric_browserHintState.pendingInteraction&&window.__ferric_browserHintState.pendingInteraction.nonce"),second.interaction_pending)
        var completed=finish(plan,second)
        verify(!completed.error,JSON.stringify(completed))
        compare(seal(completed),true)
    }
    function test_late_finish_preserves_replacement_completed_graph() {
        var first=prepare(request()),plan=request()
        plan.stamp.request_id="2"
        var second=prepare(plan),completed=finish(plan,second)
        verify(!completed.error,JSON.stringify(completed))
        compare(seal(completed),true)
        compare(finish(request(),first).error,"stale-interaction-collection")
        compare(seal(completed),true)
    }
    function test_deep_inventory_does_not_repeat_uncharged_ancestry_walks() {
        compare(evaluate("(()=>{let root=document.createElement('div');document.body.append(root);for(let i=0;i<180;i++){const next=document.createElement('div');root.append(next);root=next;}return true})()",WebEngineScript.MainWorld),true)
        evaluate(Scripts.hintCollector("links"));wait(60)
        var plan=request()
        plan.rules[0].target_selector.steps[0][1]="#absent"
        var ordinary=prepare(plan)
        compare(evaluate("(()=>{const original=Node.prototype.getRootNode;window.__rootReads=0;Node.prototype.getRootNode=function(){window.__rootReads++;return original.apply(this,arguments)};return true})()"),true)
        var completed=finish(plan,ordinary,{predicate_steps:100})
        verify(!completed.error,JSON.stringify(completed))
        compare(JSON.parse(completed.annotation_json).outcomes[0].result.unavailable.reason,"predicate-limit")
        verify(evaluate("window.__rootReads")<=8,"A charged preorder scan must not invoke native root ancestry walks per node")
    }
    function test_deferred_geometry_rejects_animation_without_dom_mutation() {
        compare(evaluate("(()=>{document.head.insertAdjacentHTML('beforeend','<style>@keyframes deferredMove {from {transform:translateX(0)} to {transform:translateX(200px)}}</style>');document.getElementById('target').style.animation='deferredMove 10s linear paused';document.getAnimations()[0].currentTime=0;return true})()",WebEngineScript.MainWorld),true)
        evaluate(Scripts.hintCollector("links"));wait(60)
        var plan=request(),ordinary=prepare(plan)
        compare(evaluate("document.getAnimations()[0].currentTime=5000;true",WebEngineScript.MainWorld),true)
        wait(30)
        compare(evaluate("!!window.__ferric_browserHintState.pendingInteraction"),true,"Exercise the geometry check independently of observer invalidation")
        var completed=finish(plan,ordinary)
        verify(!completed.error,JSON.stringify(completed))
        compare(JSON.parse(completed.annotation_json).outcomes[0].result.unavailable.reason,"changed-document")
    }
    function test_annotation_exhaustion_retains_spent_work_and_ordinary_rows() {
        compare(evaluate("document.getElementById('target').setAttribute('data-big','x'.repeat(1800));true",WebEngineScript.MainWorld),true)
        evaluate(Scripts.hintCollector("links"));wait(60)
        var plan=request()
        plan.rules[0].predicates={attributes:[{name:"data-big",exists:true}]}
        var ordinary=prepare(plan),completed=finish(plan,ordinary,{annotation_bytes:700})
        compare(ordinary.candidates.length,1)
        verify(completed.interaction_work.inspections>0,JSON.stringify(completed))
        verify(completed.interaction_work.predicate_steps>0,JSON.stringify(completed))
        verify(Number.isSafeInteger(completed.interaction_work.annotation_bytes)&&completed.interaction_work.annotation_bytes>0,
            "Annotation exhaustion must retain the charged reservation debit")
        if(completed.error)compare(completed.error,"annotation-limit")
        else compare(JSON.parse(completed.annotation_json).outcomes[0].result.unavailable.reason,"annotation-limit")
        compare(finish(plan,ordinary).error,"stale-interaction-collection")
    }
    function test_malformed_later_rule_does_not_refund_prior_extraction_work() {
        var plan=request()
        plan.rules.push({rule:{kind:"anchor",id:""}})
        var ordinary=prepare(plan),completed=finish(plan,ordinary)
        compare(completed.error,"invalid-request")
        verify(completed.interaction_work.inspections>0,JSON.stringify(completed))
        verify(completed.interaction_work.predicate_steps>0,JSON.stringify(completed))
        verify(Number.isSafeInteger(completed.interaction_work.annotation_bytes)&&completed.interaction_work.annotation_bytes>0,
            "A later invalid rule cannot erase the earlier charged annotation reservations")
        compare(evaluate("window.__ferric_browserHintState.interactionProof===null"),true)
    }
    function test_shadow_slot_inventory_and_assignment_aba_data() {
        return [{tag:"stable",aba:false},{tag:"assignment-aba",aba:true}]
    }
    function test_shadow_slot_inventory_and_assignment_aba(data) {
        compare(evaluate("(()=>{document.body.innerHTML='<div id=host><a id=target href=#one>Assigned target</a><a id=other href=#two>Other target</a></div>';const host=document.getElementById('host'),root=host.attachShadow({mode:'open',slotAssignment:'manual'});root.innerHTML='<slot></slot>';root.querySelector('slot').assign(document.getElementById('target'));return root.slotAssignment==='manual'})()",WebEngineScript.MainWorld),true)
        evaluate(Scripts.hintCollector("links"));wait(60)
        var plan=request()
        plan.rules[0].root.steps=[{kind:"open-shadow",host_selector:{steps:[[null,"#host",1]]}},
            {kind:"assigned-slot",host_selector:{steps:[[null,"slot",1]]},assigned_selector:{steps:[[null,"#target",1]]}}]
        var ordinary=prepare(plan)
        if(data.aba) {
            evaluate("(()=>{const slot=document.getElementById('host').shadowRoot.querySelector('slot');slot.assign(document.getElementById('other'));slot.assign(document.getElementById('target'));return true})()",WebEngineScript.MainWorld)
            wait(30)
            compare(finish(plan,ordinary).error,"stale-interaction-collection")
        } else {
            var completed=finish(plan,ordinary)
            verify(!completed.error,JSON.stringify(completed))
            compare(JSON.parse(completed.annotation_json).outcomes[0].result["anchor-observed"].targets.length,1)
            compare(seal(completed),true)
        }
    }
    function test_element_scope_intersects_deferred_roots_and_native_seal() {
        compare(evaluate("(()=>{const region=document.createElement('section');region.setAttribute('role','region');region.setAttribute('aria-label','Deferred permit');region.style='display:block;width:300px;height:100px';document.body.append(region);region.append(document.getElementById('target'));document.body.insertAdjacentHTML('beforeend','<a href=#outside>Outside</a>');return true})()",WebEngineScript.MainWorld),true)
        var snapshot={family:"regions",configuration_revision:0,selector_revision:"1111111111111111",policies:{}}
        var regions=evaluate(Scripts.hintCollector("regions",false,snapshot))
        var row=regions.candidates.filter(function(candidate) { return candidate.text==="Deferred permit" })[0]
        verify(!!row,JSON.stringify(regions))
        var scope={lease_id:"deferred-element",element_id:row.element_id,frame_path:row.frame_path,kind:row.kind,
            selector_revision:snapshot.selector_revision,parent_scope_lease_id:null}
        compare(evaluate(Scripts.hintAcquireScopeLease(Object.assign({configuration_revision:0},scope),row,snapshot)).ok,true)
        evaluate(Scripts.hintStopTracking(scope.lease_id))
        evaluate(Scripts.hintCollector("actions",false,undefined,scope.lease_id));wait(60)
        var plan=request("actions")
        plan.stamp.scope_lease=scope.lease_id
        plan.rules[0].target_selector.steps[0][1]="a"
        var ordinary=prepare(plan),completed=finish(plan,ordinary)
        verify(!completed.error,JSON.stringify(completed))
        compare(completed.candidates.length,1)
        compare(JSON.parse(completed.annotation_json).outcomes[0].result["anchor-observed"].targets.length,1)
        compare(seal(completed,scope),true)
    }
    function test_native_selected_worlds_resume_original_inventories_data() {
        return [{tag:"same-origin-focused",kind:"focused"},{tag:"same-origin-entered",kind:"entered"},{tag:"foreign",kind:"foreign"}]
    }
    function test_native_selected_worlds_resume_original_inventories(data) {
        var foreign=data.kind==="foreign"
        var source=foreign?"frame.src='http://localhost:8766/provider-race-child.html';":"frame.srcdoc='<a id=target href=#child style=\"display:block;width:180px;height:40px\">Child target</a>';"
        compare(evaluate("(()=>{window.childLoaded=false;const frame=document.createElement('iframe');frame.id='child';frame.style='display:block;width:400px;height:180px';frame.addEventListener('load',()=>window.childLoaded=true);"+source+"document.body.append(frame);return true})()",WebEngineScript.MainWorld),true)
        tryVerify(function() { return evaluate("window.childLoaded",WebEngineScript.MainWorld)===true },10000)
        if(foreign) {
            evaluate("document.getElementById('child').contentWindow.postMessage('fixture-focus','*');true",WebEngineScript.MainWorld)
            wait(40)
        } else evaluate("document.getElementById('child').contentDocument.getElementById('target').focus();true",WebEngineScript.MainWorld)
        done=false
        broker.focusedChild(topOwner,function(value) { reply=value;done=true })
        tryVerify(function() { return done },10000)
        verify(!reply.error,JSON.stringify(reply));childOwner=reply.owner
        done=false
        childProbe.capture(broker,childOwner,function(value) { reply=value;done=true })
        tryVerify(function() { return done },10000)
        verify(reply.ok,JSON.stringify(reply))
        var selectedReceipt=reply.documents
        var leaf=reply.documents[reply.documents.length-1],metadata={epoch:leaf.epoch,origin:leaf.origin,depth:1}
        var owner=foreign?childOwner:"",plan=request()
        if(data.kind==="entered") {
            plan.context.entered_document=metadata
            plan.rules[0].root.steps=[{kind:"same-origin-frame",host_selector:{steps:[[null,"#child",1]]}}]
        } else {
            plan.rules[0].root={start:"focused-frame",receiver_origin:leaf.origin,steps:[]}
            if(foreign)plan.context={start:"focused-frame",base_depth:1,owner:childOwner,receiver_origin:leaf.origin}
            else plan.context.focused_document=metadata
        }
        world(Scripts.hintCollector("links"),owner);wait(60)
        var ordinary=prepare(plan,undefined,owner),completed=finish(plan,ordinary,undefined,owner)
        verify(!completed.error,JSON.stringify(completed))
        var wire=JSON.parse(completed.annotation_json)
        compare(wire.outcomes[0].result["anchor-observed"].targets.length,1)
        done=false
        if(data.kind==="entered")probe.validateEnteredObjects(broker,childProbe,completed.annotation_batch,JSON.stringify(wire.stamp),
            completed.revision,completed.annotation_json,"null",function(result) { reply=result;done=true })
        else {
            var required=[wire.roots[0].start.document],validator=foreign?childProbe:probe
            validator.validateObjects(broker,childProbe,completed.annotation_batch,JSON.stringify(wire.stamp),completed.revision,
                completed.annotation_json,"null",required,function(result) { reply=result;done=true })
        }
        tryVerify(function() { return done },10000)
        compare(reply,true)
    }
}

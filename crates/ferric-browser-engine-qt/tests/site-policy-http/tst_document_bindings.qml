import QtQuick
import QtTest
import QtWebEngine
import io.github.ferricbrowser 1.0
import Ferric.Testing 1.0
import "../../qml/scripts/BrowserScripts.js" as Scripts

// Native receipts seal the collector's exact local Document identities. A
// successful association here is still not a root/witness/action permit.
TestCase {
    id: tests
    name: "PolicyDocumentBindings"
    when: windowShown
    width: 1000
    height: 800
    property bool done: false
    property var reply
    property string owner: ""
    property bool blockOnBindingProbe: false
    property bool sawBindingProbe: false
    FerricHintFrameBroker { id: broker }
    FrameReceiptProbe { id: probe }
    WebEngineView {
        id: page
        anchors.fill: parent
        property bool loaded: false
        onLoadingChanged: function(request) { loaded=request.status===WebEngineView.LoadSucceededStatus }
        onJavaScriptConsoleMessage: function(level,message,lineNumber,sourceID) {
            if (tests.blockOnBindingProbe && message === "fixture-document-binding-deadline") {
                tests.blockOnBindingProbe=false
                tests.sawBindingProbe=true
                probe.blockGui(1100)
            }
        }
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
        done=false
        broker.attach(page.mainFrame,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000)
        verify(!reply.error,JSON.stringify(reply))
        owner=reply.owner
        done=false
        probe.capture(broker,owner,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000)
        verify(reply.ok,JSON.stringify(reply))
        evaluate(Scripts.hintCollector("links"),WebEngineScript.ApplicationWorld)
        wait(50)
    }
    function cleanup() {
        blockOnBindingProbe=false
        evaluate(Scripts.hintStopTracking(),WebEngineScript.ApplicationWorld)
        broker.clear()
        probe.reset()
    }
    function stamp() {
        return {profile:"profileid-1",window:"windowid-1",tab:"tabid-2",document:"documentid-3",
            profile_revision:"1",configuration_revision:"1",policy_revision:"1",target_generation:"1",
            selector_revision:"0",view_generation:"1",viewport_generation:"0",collection_epoch:"1",
            provider_epoch:"1",request_id:"1",scope_lease:"",family:"links"}
    }
    function selector(css) { return {steps:[[null,css,1]]} }
    function collect(worldOwner,receiverOrigin,baseDepth) {
        var request={version:1,observations:true,stamp:stamp(),context:{start:"top-document",base_depth:0,owner:""},rules:[
            {rule:{kind:"anchor",id:"child"},root:{start:"top-document",steps:[{kind:"same-origin-frame",host_selector:selector("#same-origin")}]},target_selector:selector("a")}]}
        if (worldOwner) {
            receiverOrigin=receiverOrigin||"http://localhost:8766"
            request.context={start:"focused-frame",base_depth:baseDepth===undefined?1:baseDepth,owner:worldOwner,receiver_origin:receiverOrigin}
            request.rules[0].root={start:"focused-frame",receiver_origin:receiverOrigin,steps:[{kind:"same-origin-frame",host_selector:selector("#nested")}]}
            request.rules[0].target_selector=selector("button")
        }
        var script="JSON.stringify("+Scripts.hintCollector("links",false,undefined,"",false,undefined,request)+")"
        var encoded
        if (worldOwner) {
            done=false
            broker.run(worldOwner,script,function(value){reply=value;done=true})
            tryVerify(function(){return done},10000)
            encoded=reply
        } else encoded=evaluate(script,WebEngineScript.ApplicationWorld)
        var value=JSON.parse(encoded)
        verify(!value.error,JSON.stringify(value))
        value.annotation=JSON.parse(value.annotation_json)
        return value
    }
    function validateDocuments(value,documents,batch,requestStamp,revision) {
        done=false
        probe.validateDocuments(broker,batch===undefined?value.annotation_batch:batch,
            requestStamp===undefined?JSON.stringify(stamp()):requestStamp,
            revision===undefined?value.revision:revision,
            JSON.stringify(documents===undefined?value.annotation.documents:documents),
            function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        return reply
    }
    function test_native_top_receipt_binds_unfocused_same_origin_documents() {
        var value=collect()
        compare(validateDocuments(value),true)
        // No native child token was needed to establish its actual Document.
        compare(evaluate("typeof document.getElementById('same-origin').contentWindow.__ferric_hintFrameDocument",WebEngineScript.ApplicationWorld),"undefined")
    }
    function test_manual_slot_aba_preserves_document_identity_until_recollection_data() {
        return [{tag:"visited-shadow",capped:false},{tag:"unvisited-shadow",capped:true}]
    }
    function test_manual_slot_aba_preserves_document_identity_until_recollection(data) {
        compare(evaluate("(()=>{document.body.textContent='';const host=document.createElement('div');host.id='manual-host';host.style='position:fixed;left:500px;top:500px;width:250px;height:60px;z-index:99';host.innerHTML='<a id=manual-a href=#manual-a>Manual A</a><a id=manual-b href=#manual-b>Manual B</a>';document.body.append(host);const root=host.attachShadow({mode:'open',slotAssignment:'manual'});root.innerHTML='<slot></slot>';root.querySelector('slot').assign(host.querySelector('#manual-a'));return root.slotAssignment==='manual'})()"),true)
        var request={version:1,observations:true,stamp:stamp(),context:{start:"top-document",base_depth:0,owner:""},rules:[
            {rule:{kind:"anchor",id:"manual"},root:{start:"top-document",steps:[
            ]},scope_selector:selector("#manual-host"),target_selector:selector("#manual-a")}]}
        var script="JSON.stringify("+Scripts.hintCollector("links",false,undefined,"",false,data.capped?{candidates:1}:undefined,request)+")"
        JSON.parse(evaluate(script,WebEngineScript.ApplicationWorld))
        wait(50)
        var value=JSON.parse(evaluate(script,WebEngineScript.ApplicationWorld))
        value.annotation=JSON.parse(value.annotation_json)
        compare(value.annotation.outcomes[0].result["anchor-observed"].targets.length,1)
        compare(evaluate("window.__ferric_browserHintState.roots.has(document.getElementById('manual-host').shadowRoot)",WebEngineScript.ApplicationWorld),!data.capped)
        if(data.capped) {
            compare(value.candidates.length,1)
            compare(value.truncated,true)
        }
        compare(validateDocuments(value),true)
        compare(evaluate("(()=>{const host=document.getElementById('manual-host'),slot=host.shadowRoot.querySelector('slot');slot.assign(host.querySelector('#manual-b'));slot.assign(host.querySelector('#manual-a'));return slot.assignedElements({flatten:false})[0].id})()"),"manual-a")
        wait(50)
        // This seam associates Documents, not root/target/action freshness.
        // Restoring a slot does not replace any Document. The shipping graph
        // and geometry checks independently validate the captured targets.
        compare(validateDocuments(value),true)
        // Recollection replaces the batch even when Document identities match.
        var fresh=JSON.parse(evaluate(script,WebEngineScript.ApplicationWorld))
        fresh.annotation=JSON.parse(fresh.annotation_json)
        compare(validateDocuments(fresh),true)
        compare(validateDocuments(value),false)
    }
    function test_untrusted_document_metadata_cannot_mint_a_native_association_data() {
        return [{tag:"id",field:"id",value:999},
                {tag:"epoch",field:"epoch",value:"11111111-1111-4111-8111-111111111111"},
                {tag:"path",field:"frame_path",value:"0.99"},
                {tag:"origin",field:"origin",value:"https://spoof.test"},
                {tag:"owner",field:"frame_owner",value:"frame-22222222-2222-4222-8222-222222222222"},
                {tag:"document-node",field:"document_node",value:999},
                {tag:"parent-host",field:"parent_host",value:{document:1,node:999}},
                {tag:"extra-field",field:"authority",value:true}]
    }
    function test_untrusted_document_metadata_cannot_mint_a_native_association(data) {
        var value=collect()
        compare(validateDocuments(value),true)
        var documents=JSON.parse(JSON.stringify(value.annotation.documents))
        documents[1][data.field]=data.value
        compare(validateDocuments(value,documents),false)
        compare(validateDocuments(value),true)
    }
    function test_parent_graph_must_reach_the_captured_world() {
        var value=collect(),documents=JSON.parse(JSON.stringify(value.annotation.documents))
        compare(validateDocuments(value,documents.slice().reverse()),true)
        compare(validateDocuments(value,[documents[1]]),false)
        compare(validateDocuments(value,[documents[0],documents[0]]),false)
        documents[1].parent_host={document:documents[1].id,node:documents[1].parent_host.node}
        compare(validateDocuments(value,documents),false)
    }
    function test_batch_stamp_and_revision_are_all_required() {
        var value=collect()
        compare(validateDocuments(value,undefined,"11111111-1111-4111-8111-111111111111"),false)
        compare(validateDocuments(value,undefined,undefined,"{}"),false)
        compare(validateDocuments(value,undefined,undefined,undefined,value.revision+1),false)
        compare(validateDocuments(value),true)
    }
    function test_new_collection_invalidates_an_old_batch_even_with_same_dom_ids() {
        var first=collect(),second=collect()
        verify(first.annotation_batch!==second.annotation_batch)
        compare(JSON.stringify(first.annotation.documents),JSON.stringify(second.annotation.documents))
        compare(validateDocuments(first),false)
        compare(validateDocuments(second),true)
    }
    function test_ordinary_collection_and_stop_release_the_proof_batch_data() {
        return [{tag:"ordinary",stop:false},{tag:"stop",stop:true}]
    }
    function test_ordinary_collection_and_stop_release_the_proof_batch(data) {
        var value=collect()
        compare(validateDocuments(value),true)
        evaluate(data.stop?Scripts.hintStopTracking():Scripts.hintCollector("links"),WebEngineScript.ApplicationWorld)
        compare(evaluate("window.__ferric_browserHintState.interactionProof===null",WebEngineScript.ApplicationWorld),true)
        compare(validateDocuments(value),false)
    }
    function test_unrelated_dom_mutation_preserves_document_identity() {
        var value=collect()
        compare(validateDocuments(value),true)
        evaluate("document.getElementById('same-origin').dataset.changed='yes';true")
        // A data attribute on the host leaves the actual Document association
        // intact; this is not an activation or geometry permit.
        compare(validateDocuments(value),true)
    }
    function test_same_url_child_document_replacement_cannot_reuse_weak_ids() {
        var value=collect()
        compare(validateDocuments(value),true)
        compare(evaluate("window.fixtureOldDocument=document.getElementById('same-origin').contentDocument;document.getElementById('same-origin').contentWindow.location.reload();true"),true)
        tryVerify(function(){return evaluate("(()=>{const doc=document.getElementById('same-origin').contentDocument;return doc!==window.fixtureOldDocument&&doc.readyState==='complete'})()")===true},10000)
        compare(validateDocuments(value),false)
        // Even an isolated fixture forcing revision back cannot repair the old
        // object's association with this live frame's new Document.
        evaluate("window.__ferric_browserHintState.dirty=false;window.__ferric_browserHintState.revision="+value.revision,WebEngineScript.ApplicationWorld)
        compare(validateDocuments(value),false)
    }
    function test_same_srcdoc_host_replacement_invalidates_object_association() {
        var value=collect()
        compare(validateDocuments(value),true)
        evaluate("const frame=document.getElementById('same-origin');window.fixtureOldHost=frame;window.fixtureOldDocument=frame.contentDocument;frame.replaceWith(frame.cloneNode(true));true")
        tryVerify(function(){return evaluate("(()=>{const frame=document.getElementById('same-origin');return frame!==window.fixtureOldHost&&frame.contentDocument!==window.fixtureOldDocument&&frame.contentDocument.readyState==='complete'})()")===true},10000)
        evaluate("window.__ferric_browserHintState.dirty=false;window.__ferric_browserHintState.revision="+value.revision,WebEngineScript.ApplicationWorld)
        compare(validateDocuments(value),false)
    }
    function test_main_world_cannot_replace_the_isolated_reverse_catalogue() {
        var value=collect()
        evaluate("window.__ferric_browserHintState={interactionProof:null};window.__ferric_hintFrameDocument='spoof';true")
        compare(validateDocuments(value),true)
    }
    function test_foreign_world_binds_only_its_exact_native_owner() {
        evaluate("const frame=document.createElement('iframe');frame.id='foreign';frame.src='http://localhost:8766/native-receipt-child.html';document.body.append(frame);true")
        var child=""
        for (var attempt=0;attempt<30;attempt++) {
            evaluate("document.getElementById('foreign').contentWindow.postMessage('receipt-focus','*');true")
            wait(20)
            done=false
            broker.focusedChild(owner,function(value){reply=value;done=true})
            tryVerify(function(){return done},10000)
            if (reply&&!reply.error) {child=reply.owner;break}
        }
        verify(child.length>0,JSON.stringify(reply))
        done=false
        broker.run(child,"(()=>{const frame=document.createElement('iframe');frame.id='nested';frame.srcdoc='<button>Nested target</button>';document.body.append(frame);return true})()",function(value){reply=value;done=true})
        tryVerify(function(){return done},10000)
        compare(reply,true)
        var ready=false
        for (var step=0;step<30;step++) {
            done=false
            broker.run(child,"(()=>{const doc=document.getElementById('nested').contentDocument;return !!doc&&!!doc.querySelector('button')})()",function(value){reply=value;done=true})
            tryVerify(function(){return done},10000)
            if (reply===true) {ready=true;break}
            wait(20)
        }
        verify(ready)
        done=false
        probe.capture(broker,child,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000)
        compare(reply.ok,true)
        var value=collect(child)
        compare(value.annotation.documents.length,2)
        compare(value.annotation.documents[0].frame_owner,child)
        compare(value.annotation.documents[0].origin,"http://localhost:8766")
        compare(validateDocuments(value),true)
        var documents=JSON.parse(JSON.stringify(value.annotation.documents))
        documents[0].frame_owner=""
        compare(validateDocuments(value,documents),false)
        documents[0].frame_owner=owner
        compare(validateDocuments(value,documents),false)
    }
    function test_post_native_receipt_work_cannot_finish_a_retired_batch() {
        var value=collect()
        compare(validateDocuments(value),true)
        // Reading the local catalogue installs a fence on the NEXT native
        // document-token probe. That probe retires the catalogue but returns
        // its unchanged native token, so native receipt checks alone still pass.
        evaluate("(()=>{const state=window.__ferric_browserHintState;let stored=state.interactionProof,armed=true;state.fixtureRetired=false;Object.defineProperty(state,'interactionProof',{configurable:true,get:function(){if(armed){armed=false;const epoch=window.__ferric_hintFrameDocument;Object.defineProperty(window,'__ferric_hintFrameDocument',{configurable:true,get:function(){stored=null;state.fixtureRetired=true;return epoch;}});}return stored;},set:function(value){stored=value;}});return true})()",WebEngineScript.ApplicationWorld)
        var accepted=validateDocuments(value)
        compare(evaluate("window.__ferric_browserHintState.fixtureRetired",WebEngineScript.ApplicationWorld),true)
        compare(accepted,false)
    }
    function test_native_document_json_byte_bound_is_independent_of_candidate_payload() {
        var value=collect()
        var limit=2*1024*1024
        function raw(documents) {
            done=false
            probe.validateDocuments(broker,value.annotation_batch,JSON.stringify(stamp()),value.revision,documents,
                function(result){reply=result;done=true})
            tryVerify(function(){return done},10000)
            return reply
        }
        compare(raw(" ".repeat(limit-2)+"[]"),true)
        compare(raw(" ".repeat(limit-1)+"[]"),false)
        compare(raw(JSON.stringify(["🚀".repeat(limit/4)])),false)
        compare(raw("{}"),false)
        compare(raw("[null]"),false)
        compare(validateDocuments(value),true)
    }
    function test_native_association_does_not_establish_inventory_completeness() {
        var value=collect()
        compare(validateDocuments(value,[]),true)
        compare(validateDocuments(value,[value.annotation.documents[0]]),true)
        compare(validateDocuments(value,[value.annotation.documents[1]]),false)
    }
    function test_native_base_and_local_document_depth_share_the_eight_hop_cap_data() {
        return [{tag:"global-eight",depth:7,accepted:true},{tag:"global-nine",depth:8,accepted:false}]
    }
    function test_native_base_and_local_document_depth_share_the_eight_hop_cap(data) {
        page.loaded=false
        page.url="http://127.0.0.1:8766/native-document-chain.html?remaining="+data.depth
        tryVerify(function(){return page.loaded},10000)
        page.forceActiveFocus()
        broker.clear()
        done=false
        broker.attach(page.mainFrame,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000)
        verify(!reply.error,JSON.stringify(reply))
        owner=reply.owner
        tryVerify(function(){return evaluate("(()=>{let doc=document;for(let i=0;i<"+data.depth+";i++){const frame=doc.getElementById('chain');if(!frame||!frame.contentDocument)return false;doc=frame.contentDocument;}const button=doc.getElementById('leaf-focus');if(!button)return false;button.focus();return doc.hasFocus()})()")===true},10000)
        var leaf=owner
        for (var depth=0;depth<data.depth;depth++) {
            done=false
            broker.focusedChild(leaf,function(value){reply=value;done=true})
            tryVerify(function(){return done},10000)
            verify(!reply.error,JSON.stringify(reply))
            leaf=reply.owner
        }
        done=false
        probe.capture(broker,leaf,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000)
        compare(reply.ok,true,JSON.stringify(reply))
        compare(reply.documents.length,data.depth+1)
        // Deliberately lie about the producer's base depth. The native receipt
        // must independently debit the real ancestor chain, without ordinals.
        var value=collect(leaf,"http://127.0.0.1:8766",0)
        compare(value.annotation.documents.length,2)
        compare(validateDocuments(value),data.accepted)
    }
    function test_complete_binding_uses_one_absolute_deadline_through_gui_delay() {
        var value=collect()
        compare(validateDocuments(value),true)
        evaluate("(()=>{const state=window.__ferric_browserHintState,batch=state.interactionProof;Object.defineProperty(state,'interactionProof',{configurable:true,get:function(){console.info('fixture-document-binding-deadline');return batch;}});return true})()",WebEngineScript.ApplicationWorld)
        blockOnBindingProbe=true
        sawBindingProbe=false
        var accepted=validateDocuments(value)
        compare(sawBindingProbe,true)
        compare(accepted,false)
    }
    function test_unfocused_same_origin_documents_have_a_request_batch() {
        compare(evaluate("typeof document.getElementById('same-origin').contentWindow.__ferric_hintFrameDocument",WebEngineScript.ApplicationWorld),"undefined")
        var value=collect()
        verify(/^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/.test(value.annotation_batch),"A native postcheck needs the exact collector batch, including unfocused documents")
        compare(value.annotation.documents.length,2)
        compare(value.annotation.documents[1].parent_host.document,value.annotation.documents[0].id)
        var local=evaluate("(()=>{const state=window.__ferric_browserHintState,batch=state.interactionProof;return {batch:batch&&batch.batch,stamp:batch&&batch.stamp,revision:batch&&batch.revision,count:batch&&batch.documents.size,weak:batch&&Array.from(batch.documents.values()).every(function(entry){return entry.document instanceof WeakRef&&(!entry.host||entry.host instanceof WeakRef)})}})()",WebEngineScript.ApplicationWorld)
        compare(local.batch,value.annotation_batch)
        compare(local.stamp,JSON.stringify(stamp()))
        compare(local.revision,value.revision)
        compare(local.count,2)
        compare(local.weak,true)
    }
}

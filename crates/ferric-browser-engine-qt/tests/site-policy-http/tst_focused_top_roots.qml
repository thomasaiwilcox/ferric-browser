import QtQuick
import QtTest
import QtWebEngine
import io.github.ferricbrowser 1.0
import Ferric.Testing 1.0
import "../../qml/scripts/BrowserScripts.js" as Scripts

TestCase {
    id: tests
    name: "PolicyFocusedTopRoots"
    when: windowShown
    width: 1000
    height: 800
    property bool done: false
    property var reply
    property string owner: ""
    property var focused
    FerricHintFrameBroker { id: broker }
    FrameReceiptProbe { id: leafProbe }
    FrameReceiptProbe { id: topProbe }
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
        page.forceActiveFocus()
        done=false
        broker.attach(page.mainFrame,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000)
        verify(!reply.error,JSON.stringify(reply));owner=reply.owner
        var child=""
        for(var attempt=0;attempt<30;attempt++) {
            evaluate("document.getElementById('same-origin').contentDocument.querySelector('a').focus();true")
            wait(20)
            done=false
            broker.focusedChild(owner,function(value){reply=value;done=true})
            tryVerify(function(){return done},10000)
            if(reply&&!reply.error){child=reply.owner;break}
        }
        verify(!!child,JSON.stringify(reply))
        done=false
        leafProbe.capture(broker,child,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000)
        verify(reply.ok,JSON.stringify(reply))
        var documents=reply.documents,leaf=documents[documents.length-1]
        focused={epoch:leaf.epoch,origin:leaf.origin,depth:documents.length-1}
        done=false
        topProbe.capture(broker,owner,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000)
        verify(reply.ok,JSON.stringify(reply))
        evaluate(Scripts.hintCollector("links"),WebEngineScript.ApplicationWorld)
        wait(50)
    }
    function cleanup() {
        evaluate(Scripts.hintStopTracking(),WebEngineScript.ApplicationWorld)
        broker.clear();leafProbe.reset();topProbe.reset()
    }
    function stamp() {
        return {profile:"profileid-1",window:"windowid-1",tab:"tabid-2",document:"documentid-3",
            profile_revision:"1",configuration_revision:"1",policy_revision:"1",target_generation:"1",
            selector_revision:"0",view_generation:"1",viewport_generation:"0",collection_epoch:"1",
            provider_epoch:"1",request_id:"1",scope_lease:"",family:"links"}
    }
    function selector(css) { return {steps:[[null,css,1]]} }
    function collect(metadata,rules,budget,lease,snapshot) {
        var request={version:1,observations:true,stamp:stamp(),context:{start:"top-document",base_depth:0,owner:"",focused_document:metadata||focused},
            rules:rules||[{rule:{kind:"anchor",id:"focused"},root:{start:"focused-frame",receiver_origin:focused.origin,steps:[]},target_selector:selector("#frame-primary")}]}
        request.stamp.scope_lease=lease||""
        var value=JSON.parse(evaluate("JSON.stringify("+Scripts.hintCollector("links",false,snapshot,lease||"",false,budget,request)+")",WebEngineScript.ApplicationWorld))
        verify(!value.error,JSON.stringify(value))
        value.annotation=JSON.parse(value.annotation_json)
        return value
    }
    function anchor(id,start,steps,css) {
        var root={start:start,steps:steps}
        if(start==="focused-frame")root.receiver_origin=focused.origin
        return {rule:{kind:"anchor",id:id},root:root,target_selector:selector(css)}
    }
    function shadowTree(child,count,prefix) {
        compare(evaluate("(()=>{const doc="+(child?"document.getElementById('same-origin').contentDocument":"document")+";let root=doc.body;for(let i=0;i<"+count+";i++){const host=doc.createElement('div');host.id="+JSON.stringify(prefix)+"+i;root.append(host);root=host.attachShadow({mode:'open'});}const link=doc.createElement('a');link.id="+JSON.stringify(prefix+"terminal")+";link.href='#deep';link.textContent='Deep target';link.style='display:block;width:200px;height:40px';root.append(link);return true})()"),true)
    }
    function shadowSteps(count,prefix,self) {
        var steps=[]
        for(var i=0;i<count;i++)steps.push(i===0&&self?{kind:"open-shadow",self:true}:{kind:"open-shadow",host_selector:selector("#"+prefix+i)})
        return steps
    }
    function test_invalid_native_focused_metadata_is_unavailable_data() {
        return [{tag:"wrong-epoch",field:"epoch",value:"11111111-1111-4111-8111-111111111111",reason:"detached"},
                {tag:"bad-epoch",field:"epoch",value:"bad",reason:"unsupported-root"},
                {tag:"wrong-origin",field:"origin",value:"https://unrelated.test",reason:"unsupported-root"},
                {tag:"wrong-depth",field:"depth",value:2,reason:"detached"},
                {tag:"zero-depth",field:"depth",value:0,reason:"unsupported-root"},
                {tag:"fraction-depth",field:"depth",value:1.5,reason:"unsupported-root"},
                {tag:"extra-field",field:"authority",value:true,reason:"unsupported-root"}]
    }
    function test_invalid_native_focused_metadata_is_unavailable(data) {
        var metadata=JSON.parse(JSON.stringify(focused));metadata[data.field]=data.value
        var value=collect(metadata)
        compare(value.annotation.outcomes[0].result.unavailable.reason,data.reason)
        compare(value.candidates.filter(function(row){return row.text==="Frame primary"}).length,1)
    }
    function test_focus_loss_does_not_turn_an_ordinary_child_row_into_a_focused_proof() {
        evaluate("document.getElementById('slotted-primary').focus();true")
        var value=collect()
        compare(value.annotation.outcomes[0].result.unavailable.reason,"detached")
        compare(value.candidates.filter(function(row){return row.text==="Frame primary"}).length,1)
    }
    function test_focused_lookup_uses_shared_predicate_and_ordinary_admission_budgets_data() {
        return [{tag:"predicate",budget:{predicate_steps:0},reason:"predicate-limit"},
                {tag:"candidate",budget:{candidates:1},reason:"truncated"}]
    }
    function test_focused_lookup_uses_shared_predicate_and_ordinary_admission_budgets(data) {
        var value=collect(undefined,undefined,data.budget)
        compare(value.annotation.outcomes[0].result.unavailable.reason,data.reason)
        if(data.budget.candidates!==undefined)compare(value.candidates.length,1)
        else verify(value.candidates.length>1)
    }
    function test_native_document_depth_is_debited_for_outer_and_per_item_paths_data() {
        return [{tag:"outer-eight",count:7,primary:false,complete:true},
                {tag:"outer-nine",count:8,primary:false,complete:false},
                {tag:"primary-eight",count:7,primary:true,complete:true},
                {tag:"primary-nine",count:8,primary:true,complete:false}]
    }
    function test_native_document_depth_is_debited_for_outer_and_per_item_paths(data) {
        shadowTree(true,data.count,"hop")
        var steps=shadowSteps(data.count,"hop",data.primary)
        var rule
        if(data.primary)rule={rule:{kind:"collection",id:"focused"},root:{start:"focused-frame",receiver_origin:focused.origin,steps:[]},
            container_self:true,item_selector:selector("#hop0"),primary_root_steps:steps,primary_selector:selector("#hopterminal"),slot_count:1}
        else rule=anchor("focused","focused-frame",steps,"#hopterminal")
        var value=collect(undefined,[rule]),result=value.annotation.outcomes[0].result
        if(data.primary) {
            var items=result["collection-observed"].items
            compare(items.length,1)
            if(data.complete)compare(items[0].primary["targets-complete"].targets.length,1)
            else compare(items[0].primary.unavailable.reason,"unsupported-root")
        } else if(data.complete)compare(result["anchor-observed"].targets.length,1)
        else compare(result.unavailable.reason,"unsupported-root")
    }
    function test_focused_depth_does_not_leak_into_the_next_top_rule() {
        shadowTree(true,7,"child")
        shadowTree(false,8,"top")
        var rules=[anchor("focused","focused-frame",shadowSteps(7,"child",false),"#childterminal"),
            anchor("top","top-document",shadowSteps(8,"top",false),"#topterminal")]
        var value=collect(undefined,rules)
        compare(value.annotation.outcomes.length,2)
        compare(value.annotation.outcomes[0].result["anchor-observed"].targets.length,1)
        compare(value.annotation.outcomes[1].result["anchor-observed"].targets.length,1)
    }
    function test_child_element_scope_keeps_the_focused_document_identity_data() {
        return [{tag:"inside",selector:"#frame-primary",count:1},{tag:"outside-sibling",selector:"#outside",count:0}]
    }
    function test_child_element_scope_keeps_the_focused_document_identity(data) {
        compare(evaluate("(()=>{const doc=document.getElementById('same-origin').contentDocument,link=doc.getElementById('frame-primary'),section=doc.createElement('section');section.setAttribute('aria-label','Focused scope');section.setAttribute('role','region');section.style='display:block;width:300px;height:70px';doc.body.prepend(section);section.append(link);const outside=doc.createElement('a');outside.id='outside';outside.href='#outside';outside.textContent='Outside scope';doc.body.append(outside);link.focus();return true})()"),true)
        var snapshot={family:"regions",configuration_revision:0,selector_revision:"0000000000000000",policies:{}}
        var regions=JSON.parse(evaluate("JSON.stringify("+Scripts.hintCollector("regions",false,snapshot)+")",WebEngineScript.ApplicationWorld))
        var region=regions.candidates.filter(function(row){return row.text==="Focused scope"})[0]
        verify(!!region,JSON.stringify(regions))
        var lease={lease_id:"focused-child-scope",element_id:region.element_id,frame_path:region.frame_path,kind:region.kind,
            selector_revision:snapshot.selector_revision,configuration_revision:0,parent_scope_lease_id:null}
        compare(evaluate(Scripts.hintAcquireScopeLease(lease,region,snapshot),WebEngineScript.ApplicationWorld).ok,true)
        evaluate(Scripts.hintStopTracking(lease.lease_id),WebEngineScript.ApplicationWorld)
        snapshot.family="links"
        var value=collect(undefined,[anchor("focused","focused-frame",[],data.selector)],undefined,lease.lease_id,snapshot)
        compare(value.candidates.length,1)
        var result=value.annotation.outcomes[0].result
        verify(!!result["anchor-observed"],JSON.stringify(result))
        compare(result["anchor-observed"].targets.length,data.count)
        if(data.count)compare(result["anchor-observed"].targets[0].candidate.element_id,value.candidates[0].element_id)
    }
    function focusedDocument(value) {
        var result=value.annotation.outcomes[0].result["anchor-observed"]
        return value.annotation.roots.filter(function(root){return root.id===result.root})[0].start.document
    }
    function bindFocused(value,required,selected) {
        done=false
        topProbe.validateFocusedDocuments(broker,selected||leafProbe,value.annotation_batch,JSON.stringify(stamp()),value.revision,
            JSON.stringify(value.annotation.documents),required,function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        return reply
    }
    function validateReceiver(probe) {
        done=false
        probe.validateFocused(broker,function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        return reply
    }
    function test_native_collector_admission_requires_a_selected_terminal_receiver() {
        compare(validateReceiver(leafProbe),true)
        compare(validateReceiver(topProbe),false)
        compare(evaluate("document.getElementById('slotted-primary').focus();true"),true)
        compare(validateReceiver(leafProbe),false)
    }
    function test_native_collector_admission_seals_focus_after_the_last_receipt_pass() {
        compare(evaluate("(()=>{const doc=document.getElementById('same-origin').contentDocument,frame=doc.createElement('iframe');frame.id='deeper';frame.srcdoc='<button>Nested receiver</button>';doc.body.append(frame);return true})()"),true)
        tryVerify(function(){return evaluate("(()=>{const doc=document.getElementById('same-origin').contentDocument.getElementById('deeper').contentDocument;return !!doc&&!!doc.querySelector('button')})()")===true},10000)
        compare(validateReceiver(leafProbe),true,"An unfocused existing child does not prevent leaf admission")
        // Two epoch reads capture/revalidate the first receipt, then two prove
        // terminal focus. The fifth begins the LAST receipt pass. Trigger only
        // there so the previous terminal scan sees a valid selected leaf.
        compare(evaluate("(()=>{const view=document.getElementById('same-origin').contentWindow,epoch=view.__ferric_hintFrameDocument;view.__receiverProbeReads=0;Object.defineProperty(view,'__ferric_hintFrameDocument',{configurable:true,get(){if(++view.__receiverProbeReads===5)view.document.getElementById('deeper').contentDocument.querySelector('button').focus();return epoch;}});return typeof epoch==='string'})()",WebEngineScript.ApplicationWorld),true)
        compare(validateReceiver(leafProbe),false)
        verify(evaluate("document.getElementById('same-origin').contentWindow.__receiverProbeReads",WebEngineScript.ApplicationWorld)>=5)
        compare(evaluate("document.getElementById('same-origin').contentDocument.getElementById('deeper').contentDocument.hasFocus()"),true)
    }
    function test_native_receiver_binding_requires_the_exact_selected_document() {
        var value=collect(),id=focusedDocument(value)
        compare(validateReceiver(leafProbe),true)
        compare(bindFocused(value,[id]),true)
        var top=value.annotation.documents.filter(function(doc){return doc.frame_path==="0"})[0].id
        compare(bindFocused(value,[top]),false)
        compare(bindFocused(value,[id,top]),false)
        compare(bindFocused(value,[id,id]),false)
        compare(bindFocused(value,[]),false)
        compare(bindFocused(value,[4294967295]),false)
        compare(bindFocused(value,[String(id)]),false)
        compare(bindFocused(value,[id+0.5]),false)
        compare(bindFocused(value,[true]),false)
        compare(bindFocused(value,[id],topProbe),false,"A native ancestor receipt is not selected leaf authority")
        compare(bindFocused(value,[id]),true)
    }
    function test_complete_empty_focused_rule_still_binds_its_receiver() {
        var rule=anchor("focused","focused-frame",[],"#does-not-exist")
        var value=collect(undefined,[rule])
        compare(value.annotation.outcomes[0].result["anchor-observed"].targets.length,0)
        compare(bindFocused(value,[focusedDocument(value)]),true)
    }
    function test_same_origin_sibling_cannot_supply_focused_document_identity() {
        compare(evaluate("(()=>{const frame=document.createElement('iframe');frame.id='sibling';frame.srcdoc='<a href=#sibling>Sibling</a>';document.body.append(frame);return true})()"),true)
        tryVerify(function(){return evaluate("(()=>{const doc=document.getElementById('sibling').contentDocument;return !!doc&&!!doc.querySelector('a')})()")===true},10000)
        evaluate(Scripts.hintCollector("links"),WebEngineScript.ApplicationWorld)
        wait(50)
        var sibling={rule:{kind:"anchor",id:"sibling"},root:{start:"top-document",steps:[{kind:"same-origin-frame",host_selector:selector("#sibling")}]},target_selector:selector("a")}
        var value=collect(undefined,[anchor("focused","focused-frame",[],"#frame-primary"),sibling])
        var siblingResult=value.annotation.outcomes[1].result["anchor-observed"]
        var siblingId=value.annotation.roots.filter(function(root){return root.id===siblingResult.root})[0].resolved.document
        compare(bindFocused(value,[siblingId]),false)
        compare(bindFocused(value,[focusedDocument(value)]),true)
    }
    function test_focused_ancestor_is_rejected_without_retiring_the_local_document_table() {
        compare(evaluate("(()=>{const doc=document.getElementById('same-origin').contentDocument,frame=doc.createElement('iframe');frame.id='deeper';frame.style='position:fixed;left:220px;top:0;width:80px;height:60px';frame.srcdoc='<button>Deeper</button>';doc.body.append(frame);return true})()"),true)
        tryVerify(function(){return evaluate("(()=>{const doc=document.getElementById('same-origin').contentDocument.getElementById('deeper').contentDocument;return !!doc&&!!doc.querySelector('button')})()")===true},10000)
        evaluate(Scripts.hintCollector("links"),WebEngineScript.ApplicationWorld)
        wait(50)
        var value=collect(),id=focusedDocument(value)
        compare(bindFocused(value,[id]),true)
        compare(evaluate("document.getElementById('same-origin').contentDocument.getElementById('deeper').contentDocument.querySelector('button').focus();true"),true)
        // The old selected Document remains focused as an ancestor. Its native
        // children, not URL/origin equivalence, must disprove terminal ownership.
        done=false
        topProbe.validateDocuments(broker,value.annotation_batch,JSON.stringify(stamp()),value.revision,
            JSON.stringify(value.annotation.documents),function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        compare(reply,true,"Document association alone does not establish focused-leaf authority")
        compare(validateReceiver(leafProbe),false)
        compare(bindFocused(value,[id]),false)
    }
    function test_focused_same_origin_root_reuses_top_candidate_and_document_ids() {
        var value=collect(),outcome=value.annotation.outcomes[0].result
        verify(!!outcome["anchor-observed"],JSON.stringify(outcome))
        var raw=outcome["anchor-observed"],rows=value.candidates.filter(function(row){return row.text==="Frame primary"})
        compare(rows.length,1);compare(raw.targets.length,1)
        var target=raw.targets[0].candidate
        compare(value.candidates[target.index].element_id,rows[0].element_id)
        compare(target.frame_owner,"");compare(target.frame_path,rows[0].frame_path)
        compare(value.annotation.documents.length,2)
        var root=value.annotation.roots.filter(function(root){return root.id===raw.root})[0]
        var child=value.annotation.documents.filter(function(doc){return doc.id===root.start.document})[0]
        compare(child.frame_owner,"");compare(child.frame_path,rows[0].frame_path)
        verify(!!child.parent_host)
        done=false
        topProbe.validateDocuments(broker,value.annotation_batch,JSON.stringify(stamp()),value.revision,
            JSON.stringify(value.annotation.documents),function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        compare(reply,true)
    }
}

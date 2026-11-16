import QtQuick
import QtTest
import QtWebEngine
import "../../qml/scripts/BrowserScripts.js" as Scripts
import "../../qml/scripts/InteractionWitnessRuntime.js" as Witness

TestCase {
    id: tests
    name: "PolicyWitnessObservations"
    when: windowShown
    width: 1000
    height: 800
    property bool done: false
    property var reply
    WebEngineView {
        id: page
        anchors.fill: parent
        property bool loaded: false
        onLoadingChanged: function(request) { loaded=request.status===WebEngineView.LoadSucceededStatus }
    }
    function evaluate(script,world) {
        done=false
        page.runJavaScript(script,world,function(value){reply=value;done=true})
        tryVerify(function(){return done},10000)
        return reply
    }
    function main(script) { return evaluate(script,WebEngineScript.MainWorld) }
    function init() {
        page.loaded=false
        page.url="http://127.0.0.1:8766/roots.html"
        tryVerify(function(){return page.loaded},10000)
        compare(main("(()=>{document.body.replaceChildren();document.body.style='margin:20px;font:16px sans-serif';return true})()"),true)
    }
    function cleanup() { evaluate(Scripts.hintStopTracking(),WebEngineScript.ApplicationWorld) }
    function selector(css) { return {steps:[[null,css,1]]} }
    function stamp(family) {
        return {profile:"profileid-1",window:"windowid-1",tab:"tabid-2",document:"documentid-3",
            profile_revision:"18446744073709551615",configuration_revision:"9007199254740993",policy_revision:"3",
            target_generation:"18446744073709551615",selector_revision:"0",view_generation:"0",
            viewport_generation:"0",collection_epoch:"0",provider_epoch:"0",request_id:"18446744073709551615",
            scope_lease:"",family:family||"links"}
    }
    function query(css) {
        return {rule:{kind:"anchor",id:"observed"},root:{start:"top-document",steps:[]},target_selector:selector(css||".target")}
    }
    function collect(q,budget,snapshot,family,proxy,lease) {
        var request={version:1,observations:true,stamp:stamp(family),context:{start:"top-document",base_depth:0,owner:""},rules:[q||query()]}
        request.stamp.scope_lease=lease||""
        var script=Scripts.hintCollector(family||"links",proxy===true,snapshot,lease||"",false,budget,request)
        var value=JSON.parse(evaluate("JSON.stringify("+script+")",WebEngineScript.ApplicationWorld))
        verify(!value.error,JSON.stringify(value))
        verify(typeof value.annotation_json==="string","The isolated collector must serialize bounded raw witness observations")
        var annotation=JSON.parse(value.annotation_json)
        compare(JSON.stringify(annotation.stamp),JSON.stringify(request.stamp))
        compare(annotation.version,1)
        compare(annotation.outcomes.length,1)
        compare(value.root_proofs,undefined,"Stamped production results retain only the bounded annotation string")
        return {value:value,annotation:annotation,result:annotation.outcomes[0].result}
    }
    function observed(result) {
        verify(!!result["anchor-observed"],JSON.stringify(result))
        return result["anchor-observed"]
    }
    function links(count) {
        compare(main("(()=>{for(let i=0;i<"+count+";i++){const link=document.createElement('a');link.className='target';link.href='#item-'+i;link.textContent='Item '+i;link.style='display:block;width:200px;height:40px';document.body.append(link)}return true})()"),true)
    }
    function test_invisible_actions_do_not_spend_target_name_budget_data() {
        return [{tag:"hidden",style:"display:none"},
            {tag:"offscreen",style:"position:absolute;top:100000px"}]
    }
    function test_invisible_actions_do_not_spend_target_name_budget(data) {
        compare(main("(()=>{const caption=document.createElement('span');caption.id='caption';caption.style.display='none';caption.innerHTML='<span>Caption</span>'.repeat(60);document.body.append(caption);for(let i=0;i<200;i++){const button=document.createElement('button');button.setAttribute('aria-labelledby','caption');button.style="+JSON.stringify(data.style)+";document.body.append(button)}const visible=document.createElement('button');visible.id='visible';visible.textContent='Visible target';document.body.append(visible);return true})()"),true)
        var result=collect(query("#visible"),{inspections:3000},undefined,"actions")
        compare(result.value.truncated,false)
        // Offscreen absolute controls can make the document itself scrollable;
        // that independently eligible actions target must remain available.
        var buttons=result.value.candidates.filter(function(row){return row.kind==="button"})
        compare(buttons.length,1)
        compare(buttons[0].text,"Visible target")
        var targets=observed(result.result).targets
        compare(targets.length,1)
        compare(result.value.candidates[targets[0].candidate.index].element_id,buttons[0].element_id)
        compare(targets[0].name["complete-present"].value,"Visible target")
    }
    function test_known_non_targets_do_not_spend_selector_match_budget() {
        compare(main("(()=>{const fragment=document.createDocumentFragment();for(let i=0;i<1000;i++)fragment.append(document.createElement('div'));const button=document.createElement('button');button.id='visible';button.textContent='Visible target';fragment.append(button);document.body.append(fragment);return true})()"),true)
        var result=collect(query("#visible"),{predicate_steps:2000},undefined,"actions")
        compare(result.value.truncated,false)
        var targets=observed(result.result).targets
        compare(targets.length,1)
        compare(result.value.candidates[targets[0].candidate.index].text,"Visible target")
        verify(result.annotation.discovery_work.predicate_steps<2000)
    }
    function test_excluded_domain_still_rejects_malformed_target_selectors_data() {
        return [{tag:"relation",step:["child","div",1]},
            {tag:"empty-compound",step:[null,"",1]},
            {tag:"invalid-atom-count",step:[null,"div",0]}]
    }
    function test_excluded_domain_still_rejects_malformed_target_selectors(data) {
        var q=query()
        q.target_selector={steps:[data.step]}
        var result=collect(q,undefined,undefined,"actions")
        compare(result.value.candidates.length,0)
        compare(result.result.unavailable.reason,"invalid-predicate")
    }
    function test_complete_target_sets_data() { return [{tag:"absent",count:0},{tag:"one",count:1},{tag:"two",count:2}] }
    function privateObjectBindings(result) {
        return evaluate("(()=>{const expected="+JSON.stringify(result.annotation)+",raw="+JSON.stringify(result.value.annotation_json)+",state=window.__ferric_browserHintState,batch=state.interactionProof;if(!batch||!(batch.nodes instanceof Map)||!(batch.targets instanceof Map))return {ok:false};let checked=0;function node(ref){const record=batch.nodes.get(ref.node),object=record&&WeakRef.prototype.deref.call(record.object),doc=object&&(object.nodeType===9?object:object.ownerDocument);if(!record||record.document!==ref.document||!object||state.weak.get(object)!==ref.node||!doc||state.weak.get(doc)!==ref.document||WeakRef.prototype.deref.call(record.root)!==object.getRootNode())throw Error('wrong-node');if((record.assigned?WeakRef.prototype.deref.call(record.assigned):null)!==(object.assignedSlot||null))throw Error('wrong-assignment');checked++;return object;}for(const root of expected.roots){node(root.start);node(root.resolved);for(const hop of root.hops){const value=Object.values(hop)[0];for(const field of Object.values(value))if(field&&typeof field==='object'&&field.document)node(field);}}for(const outcome of expected.outcomes){const result=outcome.result;if(result['anchor-observed'])node(result['anchor-observed'].scope);if(result['collection-observed']){const facts=result['collection-observed'];node(facts.container);for(const item of facts.items){node(item.item);if(item.parent_item)node(item.parent_item);}}if(result['search-complete']){const stage=result['search-complete'].stage;if(stage['assigned-element'])node(stage['assigned-element'].slot);}}for(const entry of batch.targets){const index=entry[0],target=entry[1],record=state.elements.get(target.reference.element_id);if(!record||target.reference.index!==index||WeakRef.prototype.deref.call(target.element)!==record.element||(target.related?WeakRef.prototype.deref.call(target.related):null)!==record.relatedElement||target.document!==state.weak.get(record.ownerDocument))throw Error('wrong-target');node({document:target.document,node:target.reference.element_id});if(target.reference.related_element_id!==null)node({document:target.document,node:target.reference.related_element_id});}return {ok:true,annotation:batch.annotation===raw,nodes:batch.nodes.size,targets:batch.targets.size,checked:checked};})()",WebEngineScript.ApplicationWorld)
    }
    function test_private_batch_binds_exact_shadow_slot_and_target_objects() {
        compare(main("(()=>{const host=document.createElement('div');host.id='host';host.innerHTML='<a id=assigned class=target slot=primary href=#assigned>Assigned</a>';document.body.append(host);host.attachShadow({mode:'open'}).innerHTML='<slot name=primary></slot>';return true})()"),true)
        var q=query("#assigned")
        q.root.steps=[{kind:"open-shadow",host_selector:selector("#host")},
            {kind:"assigned-slot",host_selector:selector("slot"),assigned_selector:selector("#assigned")}]
        var result=collect(q)
        compare(observed(result.result).targets.length,1)
        var bindings=privateObjectBindings(result)
        compare(bindings.ok,true)
        compare(bindings.annotation,true)
        compare(bindings.targets,1)
        verify(bindings.nodes>=4)
        verify(bindings.checked>bindings.nodes)
        // Ordinary collection and stop each release the private batch. Tests
        // do not infer a measured heap result from registry retirement alone.
        evaluate(Scripts.hintCollector("links"),WebEngineScript.ApplicationWorld)
        compare(evaluate("window.__ferric_browserHintState.interactionProof===null",WebEngineScript.ApplicationWorld),true)
        collect(q)
        evaluate(Scripts.hintStopTracking(),WebEngineScript.ApplicationWorld)
        compare(evaluate("window.__ferric_browserHintState.interactionProof===null",WebEngineScript.ApplicationWorld),true)
    }
    function test_private_batch_binds_the_final_proxy_related_object() {
        main("document.body.innerHTML='<input id=check type=checkbox style=\"position:absolute;opacity:0\"><label class=target for=check style=\"display:block;width:200px;height:40px\">Proxy label</label>';true")
        var result=collect(undefined,undefined,undefined,"actions",true)
        compare(observed(result.result).targets.length,1)
        var bindings=privateObjectBindings(result)
        compare(bindings.ok,true)
        compare(bindings.annotation,true)
        compare(bindings.targets,1)
        verify(bindings.nodes>=3)
    }
    function fragmentCatalogue(result) {
        var saved=evaluate("(()=>{const batch=window.__ferric_browserHintState.interactionProof;return {encoded:batch.fragment_catalogue,debit:batch.annotation_work,annotation:batch.annotation}})()",WebEngineScript.ApplicationWorld)
        compare(saved.annotation,result.value.annotation_json)
        verify(typeof saved.encoded==="string","The original batch must retain a bounded fragment catalogue")
        verify(Number.isSafeInteger(saved.debit)&&saved.debit>=0,"The batch must retain its charged annotation reservation debit")
        return {value:JSON.parse(saved.encoded),encoded:saved.encoded,debit:saved.debit}
    }
    function test_private_fragment_catalogue_keeps_eligible_original_indices_data() {
        return [{tag:"wrapped",cover:false},{tag:"covered-first",cover:true}]
    }
    function test_private_fragment_catalogue_keeps_eligible_original_indices(data) {
        compare(main("(()=>{const host=document.createElement('div');host.style='position:fixed;left:40px;top:40px;width:70px;font:20px/30px monospace';host.innerHTML='<a id=wrapped class=target href=#wrapped>word word word word</a>';document.body.append(host);const target=host.firstElementChild;"
            +(data.cover?"const first=target.getClientRects()[0],cover=document.createElement('div');cover.style='position:fixed;z-index:2;background:white;left:'+first.left+'px;top:'+first.top+'px;width:'+first.width+'px;height:'+first.height+'px';document.body.append(cover);":"")
            +"return target.getClientRects().length})()"),4)
        var result=collect(query("#wrapped")),witness=observed(result.result).targets[0],catalogue=fragmentCatalogue(result)
        compare(catalogue.value.version,1)
        compare(catalogue.value.targets.length,1)
        var target=catalogue.value.targets[0],facts=target.fragments
        compare(JSON.stringify(target.candidate),JSON.stringify(witness.candidate))
        compare(target.document,result.annotation.roots[0].resolved.document)
        compare(facts.complete,true)
        compare(facts.kind,"link")
        compare(facts.policy,"link")
        compare(facts.fragment_index,data.cover?1:0)
        compare(JSON.stringify(facts.projected_primary_fragment_css),JSON.stringify(witness.projected_primary_fragment_css))
        compare(facts.eligible_fragments.length,data.cover?3:4)
        for(var i=0;i<facts.eligible_fragments.length;i++)compare(facts.eligible_fragments[i].index,i+(data.cover?1:0))
        compare(evaluate("(()=>{const entry=window.__ferric_browserHintState.interactionProof.targets.get("+witness.candidate.index+").fragment;return Object.isFrozen(entry)&&Object.isFrozen(entry.candidate)&&Object.isFrozen(entry.fragments)&&Object.isFrozen(entry.fragments.eligible_fragments)&&entry.fragments.eligible_fragments.every(value=>Object.isFrozen(value)&&Object.isFrozen(value.rect))})()",WebEngineScript.ApplicationWorld),true)
    }
    function test_private_fragment_catalogue_reuses_a_candidate_across_rules() {
        links(1)
        var first=collect(),single=fragmentCatalogue(first)
        var rules=[query(),query()];rules[1].rule.id="second"
        var request={version:1,observations:true,stamp:stamp(),context:{start:"top-document",base_depth:0,owner:""},rules:rules}
        var value=JSON.parse(evaluate("JSON.stringify("+Scripts.hintCollector("links",false,undefined,"",false,undefined,request)+")",WebEngineScript.ApplicationWorld))
        verify(!value.error,JSON.stringify(value))
        var result={value:value,annotation:JSON.parse(value.annotation_json)},repeated=fragmentCatalogue(result)
        compare(result.annotation.outcomes.length,2)
        compare(repeated.value.targets.length,1)
        compare(repeated.encoded,single.encoded)
        var sizes=evaluate("(()=>{const bytes=value=>new TextEncoder().encode(value).length;return {first:bytes("+JSON.stringify(first.value.annotation_json)+"),second:bytes("+JSON.stringify(value.annotation_json)+"),catalogue:bytes("+JSON.stringify(repeated.encoded)+")}})()",WebEngineScript.ApplicationWorld)
        compare(single.debit-sizes.first,sizes.catalogue,"The private catalogue shares the original byte grant")
        compare(repeated.debit-sizes.second,sizes.catalogue,"A second rule cannot debit the same snapshot twice")
    }
    function test_private_fragment_catalogue_keeps_discarded_reservation_debit() {
        links(2)
        compare(main("document.querySelectorAll('a')[1].setAttribute('data-key','x'.repeat(10000));true"),true)
        var q=query();q.predicates={attributes:[{name:"data-key",present:true}]}
        var result=collect(q,{annotation_bytes:4096}),catalogue=fragmentCatalogue(result)
        compare(result.value.candidates.length,2)
        compare(result.result.unavailable.reason,"annotation-limit")
        compare(catalogue.value.targets.length,0,"Discarded raw outcomes cannot retain positive fragment snapshots")
        compare(catalogue.debit,4096,"Discarding positive evidence must not refund an exhausted grant")
        compare(evaluate("new TextEncoder().encode("+JSON.stringify(result.value.annotation_json)+").length+new TextEncoder().encode("+JSON.stringify(catalogue.encoded)+").length<=4096",WebEngineScript.ApplicationWorld),true)
    }
    function test_private_fragment_catalogue_shares_the_utf8_carrier_bound_data() {
        return [{tag:"ascii",text:"ordinary"},{tag:"escaped-unicode",text:"é🚀\"\\\u0000"}]
    }
    function test_private_fragment_catalogue_shares_the_utf8_carrier_bound(data) {
        links(1)
        compare(main("document.querySelector('a').setAttribute('data-token',"+JSON.stringify(data.text)+");true"),true)
        var q=query();q.predicates={attributes:[{name:"data-token",present:true}]}
        var first=collect(q),saved=fragmentCatalogue(first),limit=saved.debit
        compare(observed(first.result).targets[0].attributes[0].fact["complete-present"].value,data.text)
        compare(evaluate("new TextEncoder().encode("+JSON.stringify(first.value.annotation_json)+").length+new TextEncoder().encode("+JSON.stringify(saved.encoded)+").length",WebEngineScript.ApplicationWorld),limit)
        var exact=collect(q,{annotation_bytes:limit}),exactCatalogue=fragmentCatalogue(exact)
        compare(observed(exact.result).targets.length,1)
        compare(exactCatalogue.debit,limit)
        compare(exactCatalogue.encoded,saved.encoded)
        var below=collect(q,{annotation_bytes:limit-1}),exhausted=fragmentCatalogue(below)
        compare(below.result.unavailable.reason,"annotation-limit")
        compare(exhausted.debit,limit-1)
        compare(exhausted.value.targets.length,0)
        compare(below.value.candidates.length,1,"A policy carrier cap cannot erase ordinary fallback")
    }
    function test_private_batch_keeps_raw_scopes_items_and_excluded_parents() {
        compare(main("(()=>{document.body.innerHTML='<div id=items><article class=item id=first data-include=yes></article><article class=item id=excluded data-include=no><article class=item id=nested data-include=yes></article></article></div>';for(const item of document.querySelectorAll('.item')){item.setAttribute('data-id',item.id);item.attachShadow({mode:'open'}).innerHTML='<slot></slot><a href=#'+item.id+'>'+item.id+'</a>';}return true})()"),true)
        var result=collect(collectionQuery()),items=result.result["collection-observed"].items
        compare(items.length,3)
        compare(items[1].attributes[0].fact["complete-present"].value,"no")
        compare(items[2].parent_item.node,items[1].item.node)
        var bindings=privateObjectBindings(result)
        compare(bindings.ok,true)
        compare(bindings.annotation,true)
        compare(bindings.targets,3)
        verify(bindings.nodes>=11)
    }
    function test_private_batch_keeps_the_exact_slot_for_a_complete_negative_search() {
        compare(main("(()=>{const host=document.createElement('div');host.id='host';host.innerHTML='<a class=target slot=primary href=#one>One</a><a class=target slot=primary href=#two>Two</a>';document.body.append(host);host.attachShadow({mode:'open'}).innerHTML='<slot name=primary></slot>';return true})()"),true)
        var q=query()
        q.root.steps=[{kind:"open-shadow",host_selector:selector("#host")},
            {kind:"assigned-slot",host_selector:selector("slot"),assigned_selector:selector(".target")}]
        var result=collect(q)
        compare(result.result["search-complete"].cardinality,"ambiguous")
        verify(!!result.result["search-complete"].stage["assigned-element"].slot)
        compare(result.value.candidates.length,2)
        var bindings=privateObjectBindings(result)
        compare(bindings.ok,true)
        compare(bindings.annotation,true)
        compare(bindings.targets,0)
        verify(bindings.nodes>=4)
    }
    function test_complete_target_sets(data) {
        links(data.count)
        var result=collect(),facts=observed(result.result)
        compare(facts.targets.length,data.count)
        for(var witness of facts.targets) {
            var row=result.value.candidates[witness.candidate.index]
            compare(row.element_id,witness.candidate.element_id)
            compare(row.related_element_id,witness.candidate.related_element_id)
            compare(row.frame_path,witness.candidate.frame_path)
            compare(witness.name["complete-present"].value,row.text)
            compare(witness.href["complete-present"].value,row.href)
            compare(witness.role["complete-absent"]!==undefined,true)
            compare(witness.target_border_box_css.width,200)
            verify(witness.projected_primary_fragment_css.width>0)
            compare(witness.fragment_index,0)
        }
        if(data.count===1)console.info("P05_RAW_WITNESS_FIXTURE",JSON.stringify({annotation:result.annotation,candidates:result.value.candidates}))
    }
    function test_candidate_cap_is_unknown_even_after_a_single_admission() {
        links(2)
        var result=collect(undefined,{candidates:1})
        compare(result.value.candidates.length,1)
        verify(!!result.result.unavailable,JSON.stringify(result.result))
        compare(result.result.unavailable.reason,"truncated")
    }
    function test_replace_exclusion_is_deliberate_and_does_not_resurrect_targets() {
        links(2)
        main("document.querySelectorAll('a')[0].id='allowed';true")
        var snapshot={family:"links",configuration_revision:0,selector_revision:"0000000000000000",policies:{links:{policy:{mode:"replace",selectors:["#allowed"]}}}}
        var result=collect(undefined,undefined,snapshot)
        compare(result.value.candidates.length,1)
        compare(observed(result.result).targets.length,1)
    }
    function test_hidden_disabled_and_inert_targets_are_known_exclusions() {
        links(3)
        main("(()=>{const rows=document.querySelectorAll('a');rows[0].style.display='none';rows[1].setAttribute('aria-disabled','true');rows[2].inert=true;return true})()")
        var result=collect()
        compare(result.value.candidates.length,0)
        compare(observed(result.result).targets.length,0)
    }
    function test_proxy_uses_final_index_and_exact_related_identity() {
        main("document.body.innerHTML='<button>Before</button><input id=check type=checkbox style=\"position:absolute;opacity:0\"><label class=target onclick=\"\" for=check style=\"display:block;width:200px;height:40px\">Proxy label</label><button>After</button>';true")
        var result=collect(undefined,undefined,undefined,"actions",true),targets=observed(result.result).targets
        compare(targets.length,1)
        var witness=targets[0],row=result.value.candidates[witness.candidate.index]
        compare(row.kind,"label-control")
        compare(row.element_id,witness.candidate.element_id)
        verify(witness.candidate.related_element_id!==null)
        compare(row.related_element_id,witness.candidate.related_element_id)
        compare(witness.candidate.index,2,"Proxy replacement must not retain its earlier normal-row index")
        compare(result.value.candidates[1].text,"After")
    }
    function test_disclosure_suppression_cannot_reappear_as_a_policy_target() {
        main("document.body.innerHTML='<details class=target tabindex=0 open><summary>Receiver</summary><p>Details body</p></details>';true")
        var result=collect(undefined,undefined,undefined,"all")
        verify(result.value.candidates.length>0)
        compare(observed(result.result).targets.length,0)
    }
    function test_truncated_name_stays_a_truncated_fact() {
        links(1)
        main("document.querySelector('a').setAttribute('aria-label','x'.repeat(513));true")
        var result=collect(),targets=observed(result.result).targets
        compare(targets.length,1)
        compare(targets[0].name.truncated!==undefined,true)
        compare(result.value.candidates[0].text.length,512)
    }
    function test_requested_raw_attributes_and_empty_name_are_complete() {
        links(1)
        main("(()=>{const link=document.querySelector('a');link.textContent='';link.setAttribute('data-key',' e\\u0301  ');return true})()")
        var q=query();q.predicates={attributes:[{name:"data-key",present:true}]}
        var target=observed(collect(q).result).targets[0]
        compare(target.name["complete-present"].value,"")
        compare(target.attributes.length,1)
        compare(target.attributes[0].name,"data-key")
        compare(target.attributes[0].fact["complete-present"].value," e\u0301  ")
    }
    function test_cap_history_survives_a_later_disclosure_removal() {
        main("document.body.innerHTML='<details tabindex=0 open><summary>Receiver</summary><p>Body</p></details><button class=target>Not evaluated</button>';true")
        var result=collect(undefined,{candidates:2},undefined,"all")
        compare(result.value.candidates.length,1,"Disclosure filtering must reduce the final row count")
        compare(result.value.candidates[0].text,"Receiver")
        compare(result.result.unavailable.reason,"truncated","A reduced final count does not undo earlier starvation")
    }
    function test_late_facet_cap_leaves_the_second_target_unknown() {
        main("document.body.innerHTML='<section class=target aria-label=First style=\"width:200px;height:50px\"></section><section class=target aria-label=Second style=\"width:200px;height:50px\"></section>';true")
        var result=collect(undefined,{candidates:1},undefined,"regions")
        compare(result.value.candidates.length,1)
        compare(result.result.unavailable.reason,"truncated")
    }
    function test_entire_proxy_loop_skipped_at_the_real_candidate_cap() {
        main("(()=>{const nodes=document.createDocumentFragment();for(let i=0;i<5000;i++){const button=document.createElement('button');button.textContent='X';button.style='position:fixed;padding:0;border:0;font:1px monospace;left:'+(i%100*8)+'px;top:'+Math.floor(i/100)*8+'px;width:7px;height:7px';nodes.append(button)}const input=document.createElement('input');input.id='hidden-check';input.type='checkbox';input.style='position:fixed;opacity:0;pointer-events:none;width:0;height:0';nodes.append(input);const label=document.createElement('label');label.htmlFor=input.id;label.className='target';label.textContent='Pending proxy';label.style='position:fixed;left:20px;top:500px;width:200px;height:40px';nodes.append(label);document.body.append(nodes);return true})()")
        var result=collect(undefined,undefined,undefined,"actions",true)
        compare(result.value.candidates.length,5000)
        compare(result.result.unavailable.reason,"truncated")
    }
    function test_oversized_retained_attributes_make_annotations_unavailable() {
        links(2)
        main("(()=>{for(const link of document.querySelectorAll('a'))link.setAttribute('data-key','x'.repeat(1100000));return true})()")
        var q=query();q.predicates={attributes:[{name:"data-key",present:true}]}
        var result=collect(q)
        compare(result.value.candidates.length,2)
        compare(result.result.unavailable.reason,"annotation-limit")
        compare(result.annotation.documents.length,0)
        compare(result.annotation.roots.length,0)
    }
    function test_annotation_utf8_and_escaping_boundaries_data() {
        return [{tag:"ascii",text:"plain"},{tag:"escapes",text:"\"\\\n\u0000"},
            {tag:"unicode",text:"é🚀\u2028"},{tag:"lone-high",text:String.fromCharCode(0xd800)},
            {tag:"lone-low",text:String.fromCharCode(0xdc00)}]
    }
    function test_annotation_utf8_and_escaping_boundaries(data) {
        var script="(()=>{"+Witness.source()+";const value={text:"+JSON.stringify(data.text)+"},expected=JSON.stringify(value),bytes=new TextEncoder().encode(expected).length;let rejected=false;try{ftInteractionAnnotationJson(value,bytes-1)}catch(error){rejected=error.ferricRootReason==='annotation-limit'}return {exact:ftInteractionAnnotationJson(value,bytes),expected:expected,rejected:rejected};})()"
        var value=evaluate(script,WebEngineScript.ApplicationWorld)
        compare(value.exact,value.expected)
        compare(value.rejected,true)
    }
    function test_exact_two_mib_annotation_boundary_data() {
        return [{tag:"ascii",expression:"'a'.repeat(2097152-11)"},
            {tag:"four-byte",expression:"String.fromCodePoint(0x1f680).repeat(Math.floor((2097152-11)/4))+'a'.repeat((2097152-11)%4)"},
            {tag:"escaped-controls",expression:"String.fromCharCode(0).repeat(Math.floor((2097152-11)/6))+'a'.repeat((2097152-11)%6)"}]
    }
    function test_exact_two_mib_annotation_boundary(data) {
        var script="(()=>{"+Witness.source()+";const value={text:"+data.expression+"},expected=JSON.stringify(value),bytes=new TextEncoder().encode(expected).length;let rejected=false;try{ftInteractionAnnotationJson(value,2097151)}catch(error){rejected=error.ferricRootReason==='annotation-limit'}return {exact:ftInteractionAnnotationJson(value,2097152)===expected,bytes:bytes,rejected:rejected};})()"
        var value=evaluate(script,WebEngineScript.ApplicationWorld)
        compare(value.bytes,2097152)
        compare(value.exact,true)
        compare(value.rejected,true)
    }
    function collectionQuery() {
        return {rule:{kind:"collection",id:"observed"},root:{start:"top-document",steps:[]},
            container_selector:selector("#items"),item_selector:selector(".item"),primary_root_steps:[{kind:"open-shadow",self:true}],
            primary_selector:selector("a"),item_attributes:[{name:"data-include",equals:"yes"}],identity_attributes:["data-id"],slot_count:1,order:"dom"}
    }
    function test_collection_retains_raw_items_and_individual_unavailable_primaries() {
        main("(()=>{const list=document.createElement('div');list.id='items';for(let i=0;i<2;i++){const item=document.createElement('article');item.className='item';item.setAttribute('data-include',i===0?'yes':'no');if(i===0){item.setAttribute('data-id','first');item.attachShadow({mode:'open'}).innerHTML='<a href=#first>Primary</a>'}list.append(item)}document.body.append(list);return true})()")
        var result=collect(collectionQuery()),facts=result.result["collection-observed"]
        verify(!!facts,JSON.stringify(result.result))
        compare(facts.items.length,2,"Raw observations are not prefiltered by JS predicates or code slots")
        compare(facts.items[0].attributes[0].fact["complete-present"].value,"yes")
        compare(facts.items[1].attributes[0].fact["complete-present"].value,"no")
        compare(facts.items[1].identity_attributes[0].fact["complete-absent"]!==undefined,true)
        compare(facts.items[1].primary.unavailable.reason,"unsupported-root")
        var primary=facts.items[0].primary["targets-complete"]
        compare(primary.targets.length,1)
        var root=result.annotation.roots.filter(function(root){return root.id===primary.root})[0]
        compare(root.start.node,facts.items[0].item.node)
        compare(root.start_kind,"element")
        compare(root.resolved_kind,"open-shadow")
        compare(facts.items[0].parent_item,null)
        compare(result.value.candidates[primary.targets[0].candidate.index].text,"Primary")
    }
    function test_collection_primary_predicates_remain_raw_and_conjunctive_in_rust() {
        main("document.body.innerHTML='<div id=items><article class=item data-include=yes data-id=one><a href=#one>First</a><a href=#two>Second</a></article></div>';true")
        var q=collectionQuery();q.primary_root_steps=[];q.primary_predicates={hint_name:{equals:"Second"}}
        var result=collect(q),items=result.result["collection-observed"].items
        compare(items.length,1)
        compare(items[0].primary["targets-complete"].targets.length,2,"Rust receives both targets before canonical name matching")
    }
    function test_nested_item_topology_is_retained_before_attribute_filtering() {
        main("document.body.innerHTML='<div id=items><article class=item data-include=no><article class=item data-include=yes data-id=child><a href=#child>Child</a></article></article></div>';true")
        var q=collectionQuery();q.primary_root_steps=[]
        var result=collect(q),items=result.result["collection-observed"].items
        compare(items.length,2)
        compare(items[0].parent_item,null)
        compare(items[1].parent_item.node,items[0].item.node)
        compare(items[1].parent_item.document,items[0].item.document)
    }
    function test_primary_self_preserves_the_admitted_item_and_witness_identity() {
        main("document.body.innerHTML='<div id=items><a class=item data-include=yes data-id=self href=#self>Self</a></div>';true")
        var q=collectionQuery();q.primary_root_steps=[];delete q.primary_selector;q.primary_self=true
        var result=collect(q),item=result.result["collection-observed"].items[0]
        var primary=item.primary["targets-complete"]
        compare(primary.targets.length,1)
        compare(primary.targets[0].candidate.element_id,item.item.node)
    }
    function test_assigned_element_anchor_scope_includes_the_exact_scope_element() {
        main("(()=>{const host=document.createElement('div');host.id='host';const link=document.createElement('a');link.className='target';link.slot='primary';link.href='#slot';link.textContent='Slotted';host.append(link);host.attachShadow({mode:'open'}).innerHTML='<slot name=primary></slot>';document.body.append(host);return true})()")
        var q=query();q.root.steps=[{kind:"open-shadow",host_selector:selector("#host")},{kind:"assigned-slot",host_selector:selector("slot"),assigned_selector:selector("a")}]
        var result=collect(q),facts=observed(result.result)
        compare(facts.targets.length,1)
        compare(facts.targets[0].candidate.element_id,facts.scope.node)
        compare(facts.scope_kind,"element")
    }
    function test_complete_small_scope_survives_an_unrelated_capped_shadow_branch() {
        main("(()=>{const small=document.createElement('section');small.id='small';small.innerHTML='<a class=target aria-label=Small href=#small>Small</a>';document.body.append(small);const large=document.createElement('div');large.attachShadow({mode:'open'}).innerHTML='<div>Padding</div>'.repeat(1000);document.body.append(large);return true})()")
        var q=query();q.scope_selector=selector("#small")
        var result=collect(q,{inspections:50}),targets=observed(result.result).targets
        compare(result.value.truncated,true)
        compare(result.value.inspected_nodes,50)
        compare(targets.length,1)
        compare(targets[0].name["complete-present"].value,"Small")
        verify(targets[0].projected_primary_fragment_css.width>0)
    }
    function test_cached_child_geometry_does_not_bypass_parent_fragment_proof() {
        main("(()=>{const small=document.createElement('iframe');small.id='small-frame';small.style='width:300px;height:100px';small.srcdoc='<a class=target aria-label=Small href=#small style=\"display:block;width:120px;height:40px\">Small</a>';const large=document.createElement('iframe');large.id='large-frame';large.srcdoc='<div>Padding</div>'.repeat(1000);document.body.append(small,large);return true})()")
        tryVerify(function(){return main("!!document.getElementById('small-frame').contentDocument.querySelector('a')&&!!document.getElementById('large-frame').contentDocument.querySelector('div')")},10000)
        var q=query();q.root.steps=[{kind:"same-origin-frame",host_selector:selector("#small-frame")}]
        var result=collect(q,{inspections:100})
        verify(result.value.candidates.some(function(row){return row.frame_path==='0.0'&&row.text==='Small'}),JSON.stringify(result.value))
        compare(result.value.inspected_nodes,100)
        compare(result.result.unavailable.reason,"inspection-limit","The child border box cannot replace its parent-fragment hit proof")
    }
    function test_cached_border_cannot_bypass_the_actual_scope_crop() {
        main("document.body.innerHTML='<section aria-label=Lease style=\"position:fixed;left:40px;top:40px;width:100px;height:80px\"><a class=target href=#crop style=\"position:fixed;left:40px;top:40px;display:block;width:200px;height:40px\">Cropped target</a></section><div style=\"position:fixed;z-index:2;left:40px;top:40px;width:100px;height:40px;background:white\"></div>';true")
        var snapshot={family:"regions",configuration_revision:0,selector_revision:"0000000000000000",policies:{}}
        var regions=JSON.parse(evaluate("JSON.stringify("+Scripts.hintCollector("regions",false,snapshot)+")",WebEngineScript.ApplicationWorld))
        var region=regions.candidates.filter(function(row){return row.text==='Lease'})[0]
        verify(!!region,JSON.stringify(regions))
        var request={lease_id:"witness-scope",element_id:region.element_id,frame_path:region.frame_path,kind:region.kind,
            selector_revision:snapshot.selector_revision,configuration_revision:0,parent_scope_lease_id:null}
        var receipt=evaluate(Scripts.hintAcquireScopeLease(request,region,snapshot),WebEngineScript.ApplicationWorld)
        compare(receipt.ok,true)
        evaluate(Scripts.hintStopTracking(request.lease_id),WebEngineScript.ApplicationWorld)
        snapshot.family="links"
        var result=collect(undefined,undefined,snapshot,undefined,undefined,request.lease_id)
        compare(result.value.candidates.length,1,"The uncropped action has an uncovered part outside the lease")
        compare(result.value.candidates[0].geometry.width,100)
        compare(observed(result.result).targets.length,0,"The covered scope crop cannot reuse the uncropped hit proof")
    }
    function test_primary_self_hidden_button_is_a_complete_empty_target_set() {
        main("document.body.innerHTML='<div id=items><button class=item data-include=yes data-id=visible>Visible</button><button class=item data-include=yes data-id=hidden style=\"display:none\">Hidden</button></div>';true")
        var q=collectionQuery();q.primary_root_steps=[];delete q.primary_selector;q.primary_self=true
        var result=collect(q,undefined,undefined,"buttons"),items=result.result["collection-observed"].items
        compare(items.length,2)
        compare(items[0].primary["targets-complete"].targets.length,1)
        compare(items[1].primary["targets-complete"].targets.length,0)
        compare(result.value.candidates.length,1)
    }
}

import QtQuick
import QtTest
import QtWebEngine
import io.github.ferricbrowser 1.0
import "../../qml/scripts/BrowserScripts.js" as Scripts

TestCase {
    id: tests
    name: "PolicyRootDiscovery"
    when: windowShown
    width: 1000
    height: 800
    property bool done: false
    property var reply
    FerricHintFrameBroker { id: broker }
    WebEngineView {
        id: page
        anchors.fill: parent
        property bool loaded: false
        onLoadingChanged: function(request) { loaded = request.status === WebEngineView.LoadSucceededStatus }
    }
    function evaluate(script, world) {
        done = false
        page.runJavaScript(script, world, function(value) { reply = value; done = true })
        tryVerify(function() { return done }, 10000)
        return reply
    }
    function main(script) { return evaluate(script, WebEngineScript.MainWorld) }
    // Compact checked-selector projection from Rust. There is no
    // fixture-side selector parser or root resolver.
    function selector(css, atoms) { return {steps:[[null,css,atoms || 1]]} }
    function id(value) { return selector("#"+value) }
    function type(name) { return selector(name) }
    function relation(left, right, kind) {
        var steps = left.steps.concat(right.steps)
        steps[left.steps.length][0] = kind
        return {steps:steps}
    }
    function shadow(host) { return {kind:"open-shadow",host_selector:host} }
    function assigned() {
        return [shadow(id("catalog")),{kind:"assigned-slot",host_selector:id("primary-slot"),assigned_selector:type("a")}]
    }
    function collect(steps, scope, budget, start, lease) {
        var rule = {rule:{kind:"anchor",id:"fixture"},root:{start:start || "top-document",steps:steps}}
        if (scope) rule.scope_selector = scope
        var request = {version:1,context:{start:"top-document",base_depth:0,owner:""},rules:[rule]}
        var value = evaluate(Scripts.hintCollector(lease ? "all" : "links",false,undefined,lease || "",false,budget,request),WebEngineScript.ApplicationWorld)
        verify(!value.error, JSON.stringify(value))
        verify(!!value.root_proofs, "Production collector must retain explicit rooted discovery facts")
        compare(value.root_proofs.results.length, 1)
        return value
    }
    function resolved(value) {
        compare(value.root_proofs.results[0].state, "resolved", JSON.stringify(value.root_proofs))
        var key = value.root_proofs.results[0].root
        return value.root_proofs.roots.filter(function(root) { return root.id === key })[0]
    }
    function init() {
        page.loaded = false
        page.url = "http://127.0.0.1:8766/roots.html"
        tryVerify(function() { return page.loaded }, 10000)
        tryVerify(function() {
            return main("!!document.querySelector('#catalog').shadowRoot && !!document.querySelector('iframe').contentDocument.querySelector('#frame-card').shadowRoot")
        },10000)
    }
    function cleanup() {
        evaluate(Scripts.hintStopTracking(),WebEngineScript.ApplicationWorld)
        broker.clear()
    }
    function collectRules(rules, budget, context, lease) {
        var request = {version:1,context:context || {start:"top-document",base_depth:0,owner:""},rules:rules}
        var value = evaluate(Scripts.hintCollector(lease?"all":"links",false,undefined,lease||"",false,budget,request),WebEngineScript.ApplicationWorld)
        verify(!value.error,JSON.stringify(value))
        verify(!!value.root_proofs,JSON.stringify(value))
        return value
    }
    function anchor(name, steps, scope) {
        var query = {rule:{kind:"anchor",id:name},root:{start:"top-document",steps:steps}}
        if (scope) query.scope_selector = scope
        return query
    }
    function test_repeated_selector_validation_does_not_exhaust_search_budget() {
        compare(main("(()=>{document.body.replaceChildren();const fragment=document.createDocumentFragment();for(let i=0;i<4000;i++)fragment.append(document.createElement('div'));const goal=document.createElement('div');goal.id='goal';fragment.append(goal);document.body.append(fragment);return true})()"),true)
        var value=collectRules([anchor("goal",[],relation(type("body"),id("goal"),"child"))],{predicate_steps:15000})
        compare(value.root_proofs.results[0].state,"resolved",JSON.stringify(value.root_proofs))
        verify(value.root_proofs.predicate_steps<15000)
        compare(value.truncated,false)
    }
    function test_malformed_selector_relations_are_rejected_before_matching() {
        var malformed={steps:[["child","body",1],[null,"#absent",1]]}
        var value=collectRules([anchor("invalid",[],malformed)])
        compare(value.root_proofs.results[0].state,"unavailable")
        compare(value.root_proofs.results[0].reason,"invalid-predicate")
    }
    function test_predicate_budget_is_shared_across_rules() {
        var first = anchor("first",[shadow(id("catalog"))],id("structural"))
        var second = anchor("second",[shadow(id("catalog"))],id("primary-slot"))
        var single = collectRules([first])
        compare(single.root_proofs.results[0].state,"resolved")
        var limit = single.root_proofs.predicate_steps
        verify(limit > 0)
        var value = collectRules([first,second],{predicate_steps:limit})
        compare(value.root_proofs.results[0].state,"resolved")
        compare(value.root_proofs.results[1].state,"unavailable")
        compare(value.root_proofs.results[1].reason,"predicate-limit")
        compare(value.root_proofs.predicate_steps,limit)
        verify(value.candidates.length > 0)
    }
    function test_remaining_depth_and_global_depth_both_bound_explicit_hops() {
        // Constructed collector contexts qualify the remaining budget, not
        // native association of an actual seven/eight-frame focus chain.
        for (var data of [
            {base:0,depth:0,steps:[shadow(id("catalog"))],expected:"unavailable"},
            {base:7,depth:1,steps:[shadow(id("catalog"))],expected:"resolved"},
            {base:8,depth:0,steps:[],expected:"resolved"},
            {base:8,depth:0,steps:[shadow(id("catalog"))],expected:"unavailable"},
            {base:8,depth:1,steps:[shadow(id("catalog"))],expected:"unavailable"},
            {base:7,depth:1,steps:assigned(),expected:"unavailable"}
        ]) {
            var value = collectRules([anchor("depth",data.steps)],{depth:data.depth},
                {start:"top-document",base_depth:data.base,owner:""})
            compare(value.root_proofs.results[0].state,data.expected,JSON.stringify(data))
            if (data.expected === "unavailable") compare(value.root_proofs.results[0].reason,"unsupported-root")
        }
    }
    function test_complete_frame_branch_survives_another_inspection_capped_branch() {
        main("const frame=document.createElement('iframe');frame.id='large';frame.style='position:fixed;left:500px;top:10px;width:400px;height:120px';frame.srcdoc='<a class=late href=#early>Early large link</a>'+ '<div>Padding</div>'.repeat(1000)+'<a class=late href=#duplicate>Duplicate</a>';document.body.append(frame);true")
        tryVerify(function() { return main("!!document.getElementById('large').contentDocument.querySelectorAll('.late')[1]") },10000)
        var small = anchor("small",[{kind:"same-origin-frame",host_selector:id("same-origin")}],id("frame-primary"))
        var large = anchor("large",[{kind:"same-origin-frame",host_selector:id("large")}],selector(".late"))
        var value = collectRules([small,large],{inspections:200})
        verify(value.truncated)
        compare(value.inspected_nodes,200)
        compare(value.root_proofs.results[0].state,"resolved",JSON.stringify(value.root_proofs))
        compare(value.root_proofs.results[1].state,"unavailable",JSON.stringify(value.root_proofs))
        compare(value.root_proofs.results[1].reason,"inspection-limit")
        // Name extraction can exhaust the same inspection budget and fall
        // back to "Link". Physical frame + href still identify the visited
        // early candidate without assuming unrelated name completeness.
        verify(value.candidates.some(function(candidate) {
            return candidate.frame_path === "0.1" && candidate.href && candidate.href.endsWith("#early")
        }),JSON.stringify(value))
    }
    function test_near_limit_selector_policy_fits_and_executes_through_native_broker_data() {
        return [{tag:"short-compounds",predicates:false},{tag:"escaped-predicates",predicates:true}]
    }
    function test_near_limit_selector_policy_fits_and_executes_through_native_broker(data) {
        // Rust composes sixteen anchors plus sixteen collections through two
        // authored layers and checks these full rules. Lists replace lists.
        // Raw item-root discovery now shares the actual predicate budget.
        // This deliberately huge selector inventory must fit the broker
        // script bound and preserve ordinary fallback. More efficient checked
        // selector validation may allow the whole inventory to complete.
        var steps = []
        for (var i=0;i<512;i++) {
            var step = type("a").steps[0]
            step[0] = i === 0 ? null : "descendant"
            steps.push(step)
        }
        var rules = []
        for (var n=0;n<32;n++) {
            var rule = anchor("max"+n,[])
            if (n < 16) rule.target_selector = {steps:steps}
            else {
                rule.rule.kind = "collection"
                rule.slot_count = 1
                rule.container_self = true
                rule.item_selector = {steps:steps}
                rule.primary_root_steps = []
                rule.primary_self = true
                rule.order = "dom"
            }
            if (data.predicates) {
                var match = '\\"'.repeat(128)
                var predicates = {hint_name:{equals:match},attributes:[]}
                for (var a=0;a<4;a++) predicates.attributes.push({name:"data-"+a,equals:match})
                rule[n < 16 ? "predicates" : "primary_predicates"] = predicates
            }
            rules.push(rule)
        }
        var plan = {version:1,context:{start:"top-document",base_depth:0,owner:""},rules:rules}
        var control = evaluate(Scripts.hintCollector("links",false),WebEngineScript.ApplicationWorld)
        verify(!control.error,JSON.stringify(control))
        verify(control.candidates.length>0,"The ordinary control must contain usable targets")
        var script = Scripts.hintCollector("links",false,undefined,"",false,undefined,plan)
        done = false
        broker.attach(page.mainFrame,function(value) { reply=value;done=true })
        tryVerify(function() { return done },10000)
        verify(!reply.error,JSON.stringify(reply))
        var owner = reply.owner
        done = false
        broker.run(owner,script,function(value) { reply=value;done=true })
        tryVerify(function() { return done },10000)
        verify(!reply.error,"Full script UTF-16 length="+script.length+": "+JSON.stringify(reply))
        verify(script.length <= 1024*1024)
        console.info("Collector full script UTF-16 units:",script.length)
        compare(reply.root_proofs.results.length,32)
        var exhausted=0
        for(var index=0;index<reply.root_proofs.results.length;index++) {
            var result=reply.root_proofs.results[index]
            if(index<16||result.state==="resolved")compare(result.state,"resolved")
            else {
                compare(result.state,"unavailable")
                compare(result.reason,"predicate-limit")
                exhausted++
            }
        }
        verify(reply.root_proofs.predicate_steps<=100000)
        if(exhausted>0)compare(reply.root_proofs.predicate_steps,100000)
        compare(reply.candidates.length,control.candidates.length,
            "Maximum selector work must preserve every ordinary candidate")
        function identity(candidate) {
            return [candidate.element_id,candidate.related_element_id,candidate.kind,
                candidate.frame_path,candidate.href,candidate.geometry]
        }
        compare(JSON.stringify(reply.candidates.map(identity)),JSON.stringify(control.candidates.map(identity)),
            "Maximum selector work must preserve ordinary identity, geometry and order")
    }
    function test_maximum_special_rule_count_preserves_every_ordinary_candidate() {
        // Maximum effective rule cardinality and reservation inventory:
        // 64 anchors + 16 collections * 28 codes = 512 reservations.
        // Codes stay native; the renderer receives their slot counts only.
        var rules = [], reservations = 0
        for (var index=0;index<64;index++) {
            var query = anchor("maximum-anchor-"+index,[])
            query.target_selector = id("slotted-primary")
            rules.push(query)
            reservations++
        }
        for (var collection=0;collection<16;collection++) {
            rules.push({rule:{kind:"collection",id:"maximum-collection-"+collection},
                root:{start:"top-document",steps:[]},container_selector:id("catalog"),
                item_selector:id("slotted-primary"),primary_root_steps:[],primary_self:true,
                order:"dom",slot_count:28})
            reservations += 28
        }
        compare(rules.length,80)
        compare(reservations,512)
        var control = evaluate(Scripts.hintCollector("links",false),WebEngineScript.ApplicationWorld)
        verify(!control.error,JSON.stringify(control))
        verify(!control.truncated)
        verify(control.candidates.length>0)
        var plan = {version:1,context:{start:"top-document",base_depth:0,owner:""},rules:rules}
        done = false
        broker.attach(page.mainFrame,function(value) { reply=value;done=true })
        tryVerify(function() { return done },10000)
        verify(!reply.error,JSON.stringify(reply))
        var owner = reply.owner
        done = false
        broker.run(owner,Scripts.hintCollector("links",false,undefined,"",false,undefined,plan),
            function(value) { reply=value;done=true })
        tryVerify(function() { return done },10000)
        verify(!reply.error,JSON.stringify(reply))
        verify(!reply.truncated,"Maximum rule cardinality must preserve complete ordinary discovery")
        compare(reply.root_proofs.results.length,80)
        for (var result=0;result<80;result++) {
            compare(reply.root_proofs.results[result].state,"resolved")
            if (result>=64) verify(reply.root_proofs.results[result].item_roots.length>0,
                "Every maximum-count collection must perform real item-root discovery")
        }
        function identity(candidate) {
            return [candidate.element_id,candidate.related_element_id,candidate.kind,
                candidate.frame_path,candidate.href,candidate.geometry]
        }
        compare(reply.candidates.length,control.candidates.length)
        compare(JSON.stringify(reply.candidates.map(identity)),JSON.stringify(control.candidates.map(identity)),
            "Maximum special rules must preserve ordinary identity, geometry and order")
    }
    function test_default_document_and_nonhittable_structural_shadow_scope() {
        compare(resolved(collect([])).resolved_kind, "document")
        var value = collect([shadow(id("catalog"))],id("structural"))
        compare(resolved(value).resolved_kind, "open-shadow")
        compare(value.root_proofs.results[0].scope_kind, "element")
    }
    function test_duplicate_host_is_ambiguous_even_when_first_host_is_usable() {
        main("let host=document.createElement('catalog-shell');host.attachShadow({mode:'open'});document.body.append(host);true")
        var result = collect([shadow(type("catalog-shell"))]).root_proofs.results[0]
        compare(result.state,"search-complete")
        compare(result.cardinality,"ambiguous")
        compare(result.stage["root-host"].step,0)
    }
    function test_direct_slot_enters_the_original_light_dom_action_identity() {
        var value = collect(assigned())
        var root = resolved(value)
        compare(root.resolved_kind,"element")
        compare(root.hops[1]["assigned-slot"].assigned.node,root.resolved.node)
        var ordinary = value.candidates.filter(function(candidate) { return candidate.text === "Slotted primary" })
        compare(ordinary.length,1)
        compare(ordinary[0].element_id,root.resolved.node)
        compare(root.resolved.document,root.start.document)
    }
    function test_second_direct_assignment_is_ambiguous_and_reassignment_is_absent() {
        main("rootFixture.addAssignedDuplicate();true")
        var ambiguous = collect(assigned()).root_proofs.results[0]
        compare(ambiguous.state,"search-complete")
        compare(ambiguous.cardinality,"ambiguous")
        verify(!!ambiguous.stage["assigned-element"])
        main("document.querySelector('#slotted-duplicate').remove();rootFixture.reassignSlot();true")
        var absent = collect(assigned()).root_proofs.results[0]
        compare(absent.state,"search-complete")
        compare(absent.cardinality,"absent")
    }
    function test_selector_ancestry_cannot_escape_an_entered_element() {
        main("document.querySelector('#slotted-primary').innerHTML='<span id=inner>Slotted primary</span>';true")
        var outside = collect(assigned(),relation(type("body"),type("span"),"descendant")).root_proofs.results[0]
        compare(outside.state,"search-complete")
        compare(outside.cardinality,"absent")
        resolved(collect(assigned(),relation(type("a"),type("span"),"child")))
    }
    function test_same_origin_frame_then_explicit_shadow_keeps_document_identity() {
        var value = collect([{kind:"same-origin-frame",host_selector:id("same-origin")},shadow(id("frame-card"))])
        var root = resolved(value)
        compare(root.resolved_kind,"open-shadow")
        verify(root.resolved.document !== root.start.document)
        compare(root.hops[0]["same-origin-frame"].document.document,root.resolved.document)
    }
    function collectionQuery(steps,primarySteps,primarySelf) {
        return {rule:{kind:"collection",id:"items"},root:{start:"top-document",steps:steps||[]},
            container_selector:id("item-list"),item_selector:selector(".item"),
            primary_root_steps:primarySteps||[],primary_self:primarySelf===true}
    }
    function itemFixture() {
        main("(()=>{document.body.replaceChildren();const list=document.createElement('div');list.id='item-list';for(let i=0;i<2;i++){const item=document.createElement('article');item.className='item';item.id='item-'+i;item.style='display:block;width:200px;height:50px';item.attachShadow({mode:'open'}).innerHTML='<a href=#primary-'+i+'>Primary '+i+'</a>';list.append(item)}document.body.append(list);return true})()")
    }
    function test_per_item_shadow_self_starts_at_exact_logical_element() {
        itemFixture()
        var value=collectRules([collectionQuery([],[{kind:"open-shadow",self:true}],false)])
        var result=value.root_proofs.results[0]
        compare(result.state,"resolved",JSON.stringify(result))
        verify(result.item_roots!==undefined,"Per-item root paths must use the integrated resolver: "+JSON.stringify(result))
        compare(result.item_roots.length,2)
        var first=result.item_roots[0],second=result.item_roots[1]
        verify(first.item.node!==second.item.node)
        for(var entry of result.item_roots) {
            var root=value.root_proofs.roots.filter(function(root){return root.id===entry.root})[0]
            compare(root.start_kind,"element")
            compare(JSON.stringify(root.start),JSON.stringify(entry.item))
            compare(root.resolved_kind,"open-shadow")
            compare(root.hops.length,1)
            compare(root.hops[0]["open-shadow"].host.node,entry.item.node)
        }
    }
    function test_per_item_empty_path_preserves_item_identity_for_primary_self() {
        itemFixture()
        var value=collectRules([collectionQuery([],[],true)])
        var result=value.root_proofs.results[0]
        compare(result.state,"resolved",JSON.stringify(result))
        verify(result.item_roots!==undefined,JSON.stringify(result))
        compare(result.item_roots.length,2)
        for(var entry of result.item_roots) {
            var root=value.root_proofs.roots.filter(function(root){return root.id===entry.root})[0]
            compare(root.start_kind,"element")
            compare(root.resolved_kind,"element")
            compare(root.hops.length,0)
            compare(JSON.stringify(root.start),JSON.stringify(root.resolved))
        }
    }
    function test_primary_self_cannot_turn_a_shadow_root_into_its_host() {
        itemFixture()
        var value=collectRules([collectionQuery([],[{kind:"open-shadow",self:true}],true)])
        compare(value.root_proofs.results[0].state,"unavailable")
        compare(value.root_proofs.results[0].reason,"unsupported-root")
    }
    function test_per_item_paths_share_already_spent_depth_data() {
        return [{tag:"one-left",base:7,depth:1,expected:"resolved"},
            {tag:"none-left",base:8,depth:0,expected:"unavailable"}]
    }
    function test_per_item_paths_share_already_spent_depth(data) {
        itemFixture()
        var value=collectRules([collectionQuery([],[{kind:"open-shadow",self:true}],false)],{depth:data.depth},
            {start:"top-document",base_depth:data.base,owner:""})
        compare(value.root_proofs.results[0].state,data.expected,JSON.stringify(value.root_proofs))
        if(data.expected==="resolved")compare(value.root_proofs.results[0].item_roots.length,2)
        else compare(value.root_proofs.results[0].reason,"unsupported-root")
    }
    function test_per_item_path_cannot_reset_the_outer_root_depth_data() {
        return [{tag:"exact-combined",depth:2,expected:"resolved"},
            {tag:"combined-over",depth:1,expected:"unavailable"}]
    }
    function test_per_item_path_cannot_reset_the_outer_root_depth(data) {
        itemFixture()
        main("(()=>{const shell=document.createElement('div');shell.id='list-shell';const list=document.getElementById('item-list');shell.attachShadow({mode:'open'}).append(list);document.body.append(shell);return true})()")
        var value=collectRules([collectionQuery([shadow(id("list-shell"))],[{kind:"open-shadow",self:true}],false)],{depth:data.depth})
        compare(value.root_proofs.results[0].state,data.expected,JSON.stringify(value.root_proofs))
        if(data.expected==="resolved")compare(value.root_proofs.results[0].item_roots.length,2)
        else compare(value.root_proofs.results[0].reason,"unsupported-root")
    }
    function test_per_item_slot_reuses_the_ordinary_physical_action_identity() {
        itemFixture()
        main("(()=>{for(const item of document.querySelectorAll('.item')){item.shadowRoot.innerHTML='<slot name=primary></slot>';const link=document.createElement('a');link.slot='primary';link.href='#slotted-'+item.id;link.textContent='Slot '+item.id;item.append(link)}return true})()")
        var steps=[{kind:"open-shadow",self:true},{kind:"assigned-slot",host_selector:type("slot"),assigned_selector:type("a")}]
        var value=collectRules([collectionQuery([],steps,true)])
        var result=value.root_proofs.results[0]
        compare(result.state,"resolved",JSON.stringify(value.root_proofs))
        verify(result.item_roots!==undefined,JSON.stringify(result))
        compare(result.item_roots.length,2)
        for(var entry of result.item_roots) {
            var root=value.root_proofs.roots.filter(function(root){return root.id===entry.root})[0]
            compare(root.resolved_kind,"element")
            var rows=value.candidates.filter(function(candidate){return candidate.element_id===root.resolved.node})
            compare(rows.length,1,"Assigned primary must retain its one ordinary physical identity")
            compare(root.hops[1]["assigned-slot"].assigned.node,rows[0].element_id)
        }
    }
    function test_per_item_same_origin_frame_self_enters_its_real_document() {
        main("(()=>{document.body.replaceChildren();const list=document.createElement('div');list.id='item-list';const frame=document.createElement('iframe');frame.className='item';frame.id='item-frame';frame.srcdoc='<a href=#item-frame>Frame item</a>';list.append(frame);document.body.append(list);return true})()")
        tryVerify(function(){return main("!!document.getElementById('item-frame').contentDocument.querySelector('a')")},10000)
        var value=collectRules([collectionQuery([],[{kind:"same-origin-frame",self:true}],false)])
        var result=value.root_proofs.results[0]
        compare(result.state,"resolved",JSON.stringify(value.root_proofs))
        verify(result.item_roots!==undefined,JSON.stringify(result))
        compare(result.item_roots.length,1)
        var entry=result.item_roots[0],root=value.root_proofs.roots.filter(function(root){return root.id===entry.root})[0]
        compare(root.start_kind,"element")
        compare(root.resolved_kind,"document")
        verify(root.start.document!==root.resolved.document)
        compare(root.start.node,entry.item.node)
        compare(root.hops[0]["same-origin-frame"].host.node,entry.item.node)
    }
    function test_per_item_failure_discards_earlier_unused_captures() {
        itemFixture()
        main("(()=>{const old=document.getElementById('item-1');old.replaceWith(old.cloneNode(true));return true})()")
        var value=collectRules([collectionQuery([],[{kind:"open-shadow",self:true}],false),anchor("after",[])])
        compare(value.root_proofs.results[0].state,"unavailable")
        compare(value.root_proofs.results[0].reason,"unsupported-root")
        compare(value.root_proofs.results[1].state,"resolved")
        compare(value.root_proofs.roots.length,1,"Failed partial item captures cannot become orphan roots")
        compare(value.root_proofs.results[1].root,value.root_proofs.roots[0].id)
        compare(value.root_proofs.roots[0].start_kind,"document")
    }
    function test_per_item_enumeration_is_exhaustive_and_not_limited_by_code_slots() {
        itemFixture()
        main("(()=>{for(let i=2;i<5;i++){const item=document.createElement('article');item.className='item';item.id='item-'+i;item.attachShadow({mode:'open'}).innerHTML='<a href=#primary-'+i+'>Primary '+i+'</a>';document.getElementById('item-list').append(item)}return true})()")
        var query=collectionQuery([],[{kind:"open-shadow",self:true}],false)
        query.slot_count=1
        var value=collectRules([query])
        compare(value.root_proofs.results[0].state,"resolved")
        compare(value.root_proofs.results[0].item_roots.length,5)
        compare(value.root_proofs.roots.length,6)
    }
    function test_per_item_predicate_work_is_shared_across_collections() {
        itemFixture()
        var first=collectionQuery([],[{kind:"open-shadow",self:true}],false)
        var baseline=collectRules([first]),limit=baseline.root_proofs.predicate_steps
        verify(limit>0)
        var second=collectionQuery([],[{kind:"open-shadow",self:true}],false)
        second.rule.id="second"
        var value=collectRules([first,second],{predicate_steps:limit})
        compare(value.root_proofs.results[0].state,"resolved")
        compare(value.root_proofs.results[1].state,"unavailable")
        compare(value.root_proofs.results[1].reason,"predicate-limit")
        compare(value.root_proofs.predicate_steps,limit)
        compare(value.root_proofs.roots.length,baseline.root_proofs.roots.length)
    }
    function test_document_and_shadow_self_are_unavailable() {
        for (var steps of [[{kind:"open-shadow",self:true}],[shadow(id("catalog")),{kind:"open-shadow",self:true}]]) {
            var result = collect(steps).root_proofs.results[0]
            compare(result.state,"unavailable")
            compare(result.reason,"unsupported-root")
        }
    }
    function test_predicate_exhaustion_preserves_ordinary_candidates() {
        var value = collect([shadow(id("catalog"))],undefined,{predicate_steps:0})
        compare(value.root_proofs.results[0].state,"unavailable")
        compare(value.root_proofs.results[0].reason,"predicate-limit")
        verify(value.candidates.length > 0)
    }
    function test_ordinary_six_argument_collector_keeps_its_own_budget_and_no_policy_facts() {
        var value = evaluate(Scripts.hintCollector("links",false,undefined,"",false,{inspections:50}),WebEngineScript.ApplicationWorld)
        verify(!value.error,JSON.stringify(value))
        compare(value.root_proofs,undefined)
        verify(value.inspected_nodes <= 50)
        verify(value.candidates.length > 0)
    }
    function test_invalid_collection_budget_is_rejected_data() {
        var result = []
        for (var field of [{name:"depth",max:8},{name:"inspections",max:50000},{name:"candidates",max:5000},{name:"predicate_steps",max:100000}]) {
            for (var invalid of [{name:"negative",value:-1},{name:"fraction",value:0.5},{name:"string",value:"1"},{name:"nan",value:NaN},{name:"overflow",value:field.max+1}]) {
                var budget = {};budget[field.name] = invalid.value
                result.push({tag:field.name+"-"+invalid.name,budget:budget})
            }
        }
        return result
    }
    function test_invalid_collection_budget_is_rejected(data) {
        var value = evaluate(Scripts.hintCollector("links",false,undefined,"",false,data.budget),WebEngineScript.ApplicationWorld)
        compare(value.error,"invalid-collection-budget")
        compare(value.candidates.length,0)
    }
    function test_root_replacement_never_reuses_the_shadow_identity() {
        var first = resolved(collect([shadow(id("catalog"))])).resolved.node
        main("const old=document.querySelector('#catalog');const next=old.cloneNode(true);next.attachShadow({mode:'open'}).innerHTML='<slot name=primary></slot>';old.replaceWith(next);true")
        var second = resolved(collect([shadow(id("catalog"))])).resolved.node
        verify(first !== second)
    }
    function emptyTrackedFrame() {
        // No old child candidates/ResizeObserver targets: child self-navigation
        // must not be detected merely as a candidate losing its old geometry.
        main("document.getElementById('same-origin').contentDocument.body.replaceChildren();true")
        var steps = [{kind:"same-origin-frame",host_selector:id("same-origin")}]
        collect(steps)
        wait(100)
        var value = collect(steps)
        wait(50)
        compare(evaluate(Scripts.hintDirtyRevision(),WebEngineScript.ApplicationWorld).dirty,false)
        return value
    }
    function navigateEmptyFrame(index) {
        var suffix = "/roots-empty-child.html?document="+index
        // The child changes its own location. The parent's iframe src/srcdoc
        // attributes stay unchanged, so parent attribute observers cannot win.
        main("(()=>{const frame=document.getElementById('same-origin');window.fixtureChildLoaded='';frame.addEventListener('load',function(){window.fixtureChildLoaded="+JSON.stringify(suffix)+";},{once:true});frame.contentWindow.location.replace("+JSON.stringify(suffix)+");return true})()")
        tryVerify(function() {
            return main("(()=>{const doc=document.getElementById('same-origin').contentDocument;return window.fixtureChildLoaded==="+JSON.stringify(suffix)+"&&!!doc&&doc.readyState==='complete'&&doc.URL.endsWith("+JSON.stringify(suffix)+")})()")
        },10000)
    }
    function rawTrackingState() {
        return evaluate("(()=>{const s=window.__ferric_browserHintState;return {revision:s.revision,dirty:s.dirty,observers:s.observers.length,windows:s.windows.length}})()",WebEngineScript.ApplicationWorld)
    }
    function manualSlotFixture(child) {
        compare(main("(()=>{const doc="+(child?"document.getElementById('same-origin').contentDocument":"document")+";const host=doc.createElement('div');host.id='manual-host';host.style='position:fixed;left:30px;top:400px;width:250px;height:60px';host.innerHTML='<a id=assigned-a href=#manual-a>Manual A</a><a id=assigned-b href=#manual-b>Manual B</a>';doc.body.append(host);const root=host.attachShadow({mode:'open',slotAssignment:'manual'});root.innerHTML='<slot></slot>';root.querySelector('slot').assign(host.querySelector('#assigned-a'));window.__manualSlotRoot=root;window.__manualSlotHost=host;return root.slotAssignment==='manual'})()"),true)
        var steps=child?[{kind:"same-origin-frame",host_selector:id("same-origin")}]:[]
        steps.push(shadow(id("manual-host")))
        // Track the ShadowRoot itself: slotchange does not compose across its
        // boundary, and manual assign() need not mutate any DOM attribute.
        resolved(collect(steps))
        wait(80)
        resolved(collect(steps))
        wait(30)
        compare(rawTrackingState().dirty,false)
        compare(main("(()=>{window.__manualSlotMutations=0;const observer=new MutationObserver(()=>window.__manualSlotMutations++);observer.observe(window.__manualSlotHost,{subtree:true,childList:true,attributes:true,characterData:true});observer.observe(window.__manualSlotRoot,{subtree:true,childList:true,attributes:true,characterData:true});window.__manualSlotObserver=observer;return true})()"),true)
        return steps
    }
    function reassignManualSlot(restore) {
        compare(main("(()=>{const root=window.__manualSlotRoot,host=window.__manualSlotHost,slot=root.querySelector('slot');slot.assign(host.querySelector('#assigned-b'));"+(restore?"slot.assign(host.querySelector('#assigned-a'));":"")+"return slot.assignedElements({flatten:false})[0].id})()"),restore?"assigned-a":"assigned-b")
        wait(50)
        compare(main("window.__manualSlotMutations"),0,"The regression must not be covered by MutationObserver")
    }
    function test_manual_slot_assignment_invalidates_the_collected_batch_data() {
        return [{tag:"top-change",child:false,restore:false},{tag:"top-aba",child:false,restore:true},{tag:"child-aba",child:true,restore:true}]
    }
    function test_manual_slot_assignment_invalidates_the_collected_batch(data) {
        manualSlotFixture(data.child)
        var before=rawTrackingState()
        reassignManualSlot(data.restore)
        var after=rawTrackingState()
        compare(after.dirty,true)
        verify(after.revision>before.revision)
    }
    function test_stop_removes_manual_slot_assignment_listeners() {
        manualSlotFixture(false)
        compare(evaluate(Scripts.hintStopTracking(),WebEngineScript.ApplicationWorld),true)
        var stopped=rawTrackingState()
        reassignManualSlot(true)
        compare(rawTrackingState().revision,stopped.revision)
        compare(rawTrackingState().dirty,false)
        compare(rawTrackingState().observers,0)
    }
    function test_pruning_a_detached_shadow_root_removes_its_slot_listener() {
        manualSlotFixture(false)
        compare(main("window.__manualSlotHost.remove();true"),true)
        wait(50)
        collect([])
        wait(50)
        compare(rawTrackingState().dirty,false)
        var current=rawTrackingState()
        // The fixture retains the detached root, so a leaked listener would
        // still fire even though production only keeps weak root references.
        reassignManualSlot(true)
        compare(rawTrackingState().revision,current.revision)
        compare(rawTrackingState().dirty,false)
    }
    function test_scoped_light_dom_still_observes_its_owning_slot_data() {
        return [{tag:"assigned-scope",nested:false},{tag:"nested-scope",nested:true}]
    }
    function test_scoped_light_dom_still_observes_its_owning_slot(data) {
        compare(main("(()=>{const host=document.createElement('div');host.id='manual-scope-host';host.style='position:fixed;left:500px;top:450px;width:300px;height:80px';host.innerHTML='<section id=manual-scope role=region aria-label=Manual-scope><button>Scoped manual action</button></section><section id=manual-other><button>Other manual action</button></section>';document.body.append(host);const root=host.attachShadow({mode:'open',slotAssignment:'manual'});root.innerHTML='<slot></slot>';let assigned=host.querySelector('#manual-scope');if("+data.nested+"){const wrapper=document.createElement('div');host.insertBefore(wrapper,assigned);wrapper.append(assigned);assigned=wrapper;}window.__manualScopeAssigned=assigned;root.querySelector('slot').assign(assigned);return true})()"),true)
        var snapshot={family:"regions",configuration_revision:0,selector_revision:"1111111111111111",policies:{}}
        var regions=evaluate(Scripts.hintCollector("regions",false,snapshot),WebEngineScript.ApplicationWorld)
        var scope=regions.candidates.filter(function(candidate){return candidate.text==="Manual-scope"})[0]
        verify(!!scope,JSON.stringify(regions))
        var lease={lease_id:"manual-scope",element_id:scope.element_id,frame_path:scope.frame_path,kind:scope.kind,
            selector_revision:snapshot.selector_revision,configuration_revision:0,parent_scope_lease_id:null}
        compare(evaluate(Scripts.hintAcquireScopeLease(lease,scope,snapshot),WebEngineScript.ApplicationWorld).ok,true)
        evaluate(Scripts.hintStopTracking(lease.lease_id),WebEngineScript.ApplicationWorld)
        // Collection starts at the light-DOM section, not at its host or the
        // ShadowRoot that owns its assigned slot. An ancestor Document rule
        // must not make the private slot invisible to invalidation.
        collect([],undefined,undefined,undefined,lease.lease_id)
        wait(80)
        var value=collect([],undefined,undefined,undefined,lease.lease_id)
        verify(value.candidates.some(function(row){return row.text==="Scoped manual action"}))
        compare(rawTrackingState().dirty,false)
        compare(main("(()=>{const host=document.getElementById('manual-scope-host'),slot=host.shadowRoot.querySelector('slot');slot.assign(host.querySelector('#manual-other'));slot.assign(window.__manualScopeAssigned);return slot.assignedElements()[0]===window.__manualScopeAssigned})()"),true)
        wait(50)
        compare(rawTrackingState().dirty,true)
    }
    function test_child_self_navigation_invalidates_root_and_tracks_the_new_window() {
        var first = resolved(emptyTrackedFrame()).resolved.document
        navigateEmptyFrame(1)
        compare(evaluate(Scripts.hintDirtyRevision(),WebEngineScript.ApplicationWorld).dirty,true)
        var second = resolved(collect([{kind:"same-origin-frame",host_selector:id("same-origin")}])).resolved.document
        verify(first !== second)
        compare(evaluate(Scripts.hintDirtyRevision(),WebEngineScript.ApplicationWorld).dirty,false)
        compare(main("(()=>{const child=document.getElementById('same-origin').contentWindow;child.dispatchEvent(new child.Event('scroll'));return true})()"),true)
        compare(evaluate(Scripts.hintDirtyRevision(),WebEngineScript.ApplicationWorld).dirty,true)
        evaluate(Scripts.hintStopTracking(),WebEngineScript.ApplicationWorld)
        var stopped = rawTrackingState()
        compare(main("(()=>{const child=document.getElementById('same-origin').contentWindow;child.dispatchEvent(new child.Event('scroll'));window.dispatchEvent(new Event('scroll'));return true})()"),true)
        compare(rawTrackingState().revision,stopped.revision)
        compare(rawTrackingState().dirty,false)
    }
    function test_repeated_child_navigation_retires_observers_and_stop_removes_load_tracking() {
        emptyTrackedFrame()
        var original = rawTrackingState()
        verify(original.observers > 0)
        for (var index=1;index<=3;index++) {
            navigateEmptyFrame(index)
            collect([{kind:"same-origin-frame",host_selector:id("same-origin")}])
            compare(rawTrackingState().observers,original.observers,"Old Document observers must be disconnected")
            compare(rawTrackingState().windows,original.windows)
        }
        evaluate(Scripts.hintStopTracking(),WebEngineScript.ApplicationWorld)
        var stopped = rawTrackingState()
        compare(stopped.observers,0)
        compare(stopped.windows,0)
        navigateEmptyFrame(4)
        compare(rawTrackingState().revision,stopped.revision)
        compare(rawTrackingState().dirty,false)
    }
    function test_shadow_frame_self_navigation_and_stopped_load_listener() {
        main("document.getElementById('catalog').shadowRoot.innerHTML='<iframe id=shadow-child srcdoc=\"<!doctype html><title>Empty shadow child</title>\"></iframe>';true")
        tryVerify(function() {
            return main("document.getElementById('catalog').shadowRoot.getElementById('shadow-child').contentDocument.readyState==='complete'")
        },10000)
        var steps = [shadow(id("catalog")),{kind:"same-origin-frame",host_selector:id("shadow-child")}]
        collect(steps)
        wait(100)
        collect(steps)
        wait(50)
        compare(evaluate(Scripts.hintDirtyRevision(),WebEngineScript.ApplicationWorld).dirty,false)
        function navigate(index) {
            var suffix = "/roots-empty-child.html?shadow="+index
            main("(()=>{const frame=document.getElementById('catalog').shadowRoot.getElementById('shadow-child');window.fixtureShadowLoaded='';frame.addEventListener('load',function(){window.fixtureShadowLoaded="+JSON.stringify(suffix)+";},{once:true});frame.contentWindow.location.replace("+JSON.stringify(suffix)+");return true})()")
            tryVerify(function() {
                return main("(()=>{const doc=document.getElementById('catalog').shadowRoot.getElementById('shadow-child').contentDocument;return window.fixtureShadowLoaded==="+JSON.stringify(suffix)+"&&!!doc&&doc.readyState==='complete'&&doc.URL.endsWith("+JSON.stringify(suffix)+")})()")
            },10000)
        }
        navigate(1)
        compare(evaluate(Scripts.hintDirtyRevision(),WebEngineScript.ApplicationWorld).dirty,true)
        collect(steps)
        evaluate(Scripts.hintStopTracking(),WebEngineScript.ApplicationWorld)
        var stopped = rawTrackingState()
        navigate(2)
        compare(rawTrackingState().revision,stopped.revision)
        compare(rawTrackingState().dirty,false)
    }
    function test_child_navigation_releases_old_candidates_before_recollection() {
        var value = collect([{kind:"same-origin-frame",host_selector:id("same-origin")}])
        var child = value.candidates.filter(function(candidate) { return candidate.frame_path === "0.0" })
        verify(child.length > 0)
        main("window.retainedChildDocument=document.getElementById('same-origin').contentDocument;true")
        navigateEmptyFrame(1)
        for (var candidate of child) {
            compare(evaluate("window.__ferric_browserHintState.elements.has("+candidate.element_id+")",WebEngineScript.ApplicationWorld),false)
        }
    }
    function test_detached_frame_roots_are_retired_even_when_the_page_keeps_old_objects() {
        collect([{kind:"same-origin-frame",host_selector:id("same-origin")}])
        compare(evaluate(`(()=>{
            const frame=document.getElementById('same-origin'),doc=frame.contentDocument;
            window.__retirementFixture={frame:frame,doc:doc,view:frame.contentWindow,
                shadow:doc.getElementById('frame-card').shadowRoot};
            return true;
        })()`,WebEngineScript.ApplicationWorld),true)
        main("window.retainedFrame=document.getElementById('same-origin');window.retainedChildDocument=retainedFrame.contentDocument;retainedFrame.remove();true")
        collect([])
        var tracked=evaluate(`(()=>{
            const state=window.__ferric_browserHintState,old=window.__retirementFixture;
            const shadow=document.getElementById('catalog').shadowRoot;
            const expected=new Set([document,shadow,shadow.getElementById('primary-slot')]);
            const roots=state.rootTracking.map(entry=>entry.root.deref());
            return {
                exactRoots:roots.length===expected.size&&new Set(roots).size===expected.size
                    &&roots.every(root=>expected.has(root)),
                onlyTop:roots.every(root=>(root.nodeType===9?root:root.ownerDocument)===document),
                exactWindows:state.windows.length===1&&state.windows[0]===window,
                retiredWindow:!state.windowDocuments.has(old.view),
                retiredObjects:!roots.includes(old.doc)&&!roots.includes(old.shadow)
                    &&Array.from(state.elements.values()).every(record=>record.ownerDocument!==old.doc),
                liveObservers:state.observers.length===expected.size,
                retainedDetached:!old.frame.isConnected&&!!old.doc&&!!old.shadow
            };
        })()`,WebEngineScript.ApplicationWorld)
        for(var key of Object.keys(tracked))compare(tracked[key],true,key+": "+JSON.stringify(tracked))
        // Live slot tracking adds an observer of its own. Test exact subjects
        // and disconnection, rather than assuming the old two-observer count.
        wait(50)
        collect([])
        var before=rawTrackingState()
        compare(evaluate(`(()=>{
            const old=window.__retirementFixture;
            old.doc.body.append(old.doc.createElement('button'));
            old.shadow.append(old.doc.createElement('button'));
            return true;
        })()`,WebEngineScript.ApplicationWorld),true)
        wait(50)
        compare(rawTrackingState().revision,before.revision)
        compare(rawTrackingState().dirty,before.dirty)
        compare(evaluate("delete window.__retirementFixture",WebEngineScript.ApplicationWorld),true)
    }
    function test_compounds_preserve_native_html_and_svg_semantics_data() {
        return [
            {tag:"enumerated-attribute",html:"<input id=css-probe type=text>",css:"input[type=TEXT]",tests:[{kind:"type",name:"input"},{kind:"attribute",name:"type",operator:"equals",value:"TEXT"}]},
            {tag:"empty-prefix",html:"<input id=css-probe type=text>",css:'input[type^=""]',tests:[{kind:"type",name:"input"},{kind:"attribute",name:"type",operator:"prefix",value:""}]},
            {tag:"data-case",html:"<div id=css-probe data-code=lower></div>",css:"[data-code=LOWER]",tests:[{kind:"attribute",name:"data-code",operator:"equals",value:"LOWER"}]},
            {tag:"html-type-case",html:"<div id=css-probe></div>",css:"DIV#css-probe",tests:[{kind:"type",name:"DIV"},{kind:"id",value:"css-probe"}]},
            {tag:"svg-type-case",html:"<svg><linearGradient id=css-probe></linearGradient></svg>",css:"linearGradient#css-probe",tests:[{kind:"type",name:"linearGradient"},{kind:"id",value:"css-probe"}]}
        ]
    }
    function test_compounds_preserve_native_html_and_svg_semantics(data) {
        main("document.body.insertAdjacentHTML('beforeend',"+JSON.stringify(data.html)+");true")
        var nativeCount = main("document.querySelectorAll("+JSON.stringify(data.css)+").length")
        var ast = selector(data.css,data.tests.length)
        var result = collect([],ast).root_proofs.results[0]
        if (nativeCount === 1) compare(result.state,"resolved",JSON.stringify(result))
        else {
            compare(result.state,"search-complete",JSON.stringify(result))
            compare(result.cardinality,nativeCount === 0 ? "absent" : "ambiguous")
        }
    }
    function acquireScope() {
        main("document.body.insertAdjacentHTML('beforeend','<section id=lease aria-label=Root-lease style=\"display:block;width:400px;height:220px\"><scope-shell id=inside-host></scope-shell><div id=inside-scope class=mixed>Inside</div></section><scope-shell id=outside-host></scope-shell><div id=outside-scope class=mixed>Outside</div>');for(const id of ['inside-host','outside-host'])document.getElementById(id).attachShadow({mode:'open'}).innerHTML='<a href=#inner>Inner</a>';true")
        var snapshot = {family:"regions",configuration_revision:0,selector_revision:"1111111111111111",policies:{}}
        var regions = evaluate(Scripts.hintCollector("regions",false,snapshot),WebEngineScript.ApplicationWorld)
        var scope = regions.candidates.filter(function(candidate) { return candidate.text === "Root-lease" })[0]
        verify(!!scope,JSON.stringify(regions))
        var request = {lease_id:"root-lease",element_id:scope.element_id,frame_path:scope.frame_path,kind:scope.kind,
            selector_revision:snapshot.selector_revision,configuration_revision:0,parent_scope_lease_id:null}
        var receipt = evaluate(Scripts.hintAcquireScopeLease(request,scope,snapshot),WebEngineScript.ApplicationWorld)
        verify(receipt.ok,JSON.stringify(receipt))
        evaluate(Scripts.hintStopTracking(request.lease_id),WebEngineScript.ApplicationWorld)
        return request.lease_id
    }
    function test_scope_allows_ancestor_document_and_inside_structural_root() {
        var lease = acquireScope()
        compare(resolved(collect([],undefined,undefined,undefined,lease)).resolved_kind,"document")
        resolved(collect([shadow(id("inside-host"))],undefined,undefined,undefined,lease))
    }
    function test_per_item_scope_intersection_excludes_a_known_disjoint_sibling() {
        var lease=acquireScope()
        var query={rule:{kind:"collection",id:"scope-items"},root:{start:"top-document",steps:[]},
            container_self:true,item_selector:selector(".mixed"),primary_root_steps:[],primary_self:true}
        var value=collectRules([query],undefined,undefined,lease)
        compare(value.root_proofs.results[0].state,"resolved",JSON.stringify(value.root_proofs))
        compare(value.root_proofs.results[0].item_roots.length,1)
        var inside=evaluate("window.__ferric_browserHintState.weak.get(document.getElementById('inside-scope'))",WebEngineScript.ApplicationWorld)
        compare(value.root_proofs.results[0].item_roots[0].item.node,inside)
        var outside=evaluate("window.__ferric_browserHintState.weak.get(document.getElementById('outside-scope'))",WebEngineScript.ApplicationWorld)
        verify(outside===undefined,"Disjoint item capture must not allocate a proof identity")
    }
    function test_scope_rejects_disjoint_host_before_tracking_it() {
        var lease = acquireScope()
        var value = collect([shadow(id("outside-host"))],undefined,undefined,undefined,lease)
        compare(value.root_proofs.results[0].state,"unavailable")
        compare(value.root_proofs.results[0].reason,"outside-scope")
        compare(value.root_proofs.roots.length,0)
        compare(evaluate("window.__ferric_browserHintState.roots.has(document.getElementById('outside-host').shadowRoot)",WebEngineScript.ApplicationWorld),false)
    }
    function test_scope_does_not_filter_a_competing_host_into_a_singleton() {
        var lease = acquireScope()
        var value = collect([shadow(type("scope-shell"))],undefined,undefined,undefined,lease)
        compare(value.root_proofs.results[0].state,"unavailable")
        compare(value.root_proofs.results[0].reason,"outside-scope")
        compare(value.root_proofs.roots.length,0)
    }
    function test_scope_disjoint_or_mixed_scope_selection_is_unavailable() {
        var lease = acquireScope()
        for (var scope of [id("outside-scope"),selector(".mixed")]) {
            var value = collect([],scope,undefined,undefined,lease)
            compare(value.root_proofs.results[0].state,"unavailable")
            compare(value.root_proofs.results[0].reason,"outside-scope")
            compare(value.root_proofs.roots.length,0)
        }
    }
    function test_complete_root_lookup_survives_unrelated_candidate_truncation() {
        var value = collect([shadow(id("catalog"))],id("structural"),{candidates:1})
        resolved(value)
        compare(value.candidates.length,1)
        verify(value.truncated)
        verify(value.inspected_nodes <= 50000)
    }
    function test_explicit_nested_slot_steps_remain_direct_and_support_element_self() {
        main("const root=document.getElementById('catalog').shadowRoot;root.innerHTML='<inner-shell><slot id=outer-slot slot=nested name=primary></slot></inner-shell>';root.querySelector('inner-shell').attachShadow({mode:'open'}).innerHTML='<slot id=inner-slot name=nested></slot>';true")
        var value = collect([shadow(id("catalog")),shadow(type("inner-shell")),
            {kind:"assigned-slot",host_selector:id("inner-slot"),assigned_selector:type("slot")},
            {kind:"assigned-slot",self:true,assigned_selector:type("a")}])
        var root = resolved(value)
        compare(root.resolved_kind,"element")
        compare(root.hops[2]["assigned-slot"].assigned.node,root.hops[3]["assigned-slot"].slot.node)
        compare(root.hops[3]["assigned-slot"].assigned.node,root.resolved.node)
    }
    function test_checked_rust_plan_fixtures_execute_without_another_selector_parser() {
        done = false
        var request = new XMLHttpRequest()
        request.onreadystatechange = function() {
            if (request.readyState === XMLHttpRequest.DONE) { reply = request.responseText; done = true }
        }
        request.open("GET","http://127.0.0.1:8766/root-query-plans.json")
        request.send()
        tryVerify(function() { return done }, 10000)
        compare(request.status,200)
        // Rust's collector_plan_fixtures test checks these exact rule values
        // against serialization of the checked policy and selector compiler.
        // This is not a substitute for the future retained BrowserUi request.
        var fixtures = JSON.parse(reply)
        compare(fixtures.length,3)
        for (var fixture of fixtures) {
            main(fixture.setup)
            var plan = {version:1,context:{start:"top-document",base_depth:0,owner:""},rules:fixture.rules,
                stamp:{request_id:"18446744073709551615",configuration_revision:"9007199254740993"}}
            var value = evaluate(Scripts.hintCollector("links",false,undefined,"",false,undefined,plan),WebEngineScript.ApplicationWorld)
            verify(!!value.root_proofs,JSON.stringify(value))
            compare(value.root_proofs.results.length,1)
            resolved(value)
            compare(value.root_proofs.stamp.request_id,plan.stamp.request_id)
            compare(value.root_proofs.stamp.configuration_revision,plan.stamp.configuration_revision)
        }
    }
}

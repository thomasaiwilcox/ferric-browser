import QtQuick
import QtTest
import QtWebEngine
import "../../qml/components" as Components
import "../../qml/scripts/BrowserScripts.js" as Scripts

// Real broker leases, isolated-world collector and provider projection. This
// qualifies the component boundary, not the later enhanced BrowserUi consumer.
TestCase {
    id: tests
    name: "PolicyFrameMapping"
    when: windowShown
    width: 800
    height: 600
    property bool done: false
    property var reply
    QtObject {
        id: host
        property int hintCollectionEpoch: 1
        function runBrowserScript(view, script, callback, world) {
            view.runJavaScript(script, WebEngineScript.ApplicationWorld, callback)
            return true
        }
    }
    WebEngineView {
        id: page
        anchors.fill: parent
        property bool loaded: false
        onLoadingChanged: function(request) {
            loaded = request.status === WebEngineView.LoadSucceededStatus
        }
    }
    Components.FerricHintFrameProvider { id: provider; browserWindow: host }

    function main(script) {
        done = false
        page.runJavaScript(script, WebEngineScript.MainWorld, function(value) {
            reply = value
            done = true
        })
        tryVerify(function() { return done }, 5000)
        return reply
    }
    function collect(options) {
        done = false
        provider.collect(page, Scripts.hintCollector("links"), options || {family:"links"}, function(value) {
            reply = value
            done = true
        })
        tryVerify(function() { return done }, 10000)
        return reply
    }
    function init() {
        page.loaded = false
        page.url = "http://127.0.0.1:8766/provider-race-parent.html"
        tryVerify(function() { return page.loaded }, 10000)
        page.forceActiveFocus()
        done = false
        provider.ensure(page, function(value) { reply = value; done = true })
        tryVerify(function() { return done }, 5000)
        verify(!reply.error, JSON.stringify(reply))
        var found
        for (var attempt = 0; attempt < 20; ++attempt) {
            main("document.getElementById('subject').contentWindow.postMessage('fixture-focus','*');true")
            wait(30)
            done = false
            provider.discover(provider.rootOwner, 0, false, false, function(value) {
                reply = value
                done = true
            })
            tryVerify(function() { return done }, 5000)
            if (reply.foreign && reply.owner !== provider.rootOwner) { found = reply; break }
        }
        verify(!!found, JSON.stringify(reply))
        provider.scopedOwner = found.owner
        var initial = collect()
        verify(!initial.error, JSON.stringify(initial))
        compare(initial.candidates.length, 1)
        compare(initial.candidates[0].frame_owner, found.owner)
        compare(initial.candidates[0].text, "Foreign target")
    }
    function cleanup() {
        provider.stop()
        compare(provider.leaseCount, 0)
    }
    function test_stable_chain_still_collects() {
        for (var attempt = 0; attempt < 3; ++attempt) {
            var value = collect()
            verify(!value.error, JSON.stringify(value))
            compare(value.candidates.length, 1)
        }
    }
    function test_captured_leaf_requires_no_focused_descendant_data() {
        return [{tag:"unfocused-child",focused:false},{tag:"focus-moved-deeper",focused:true}]
    }
    function test_captured_leaf_requires_no_focused_descendant(data) {
        var owner = provider.scopedOwner
        done = false
        provider.run(owner, "(()=>{const frame=document.createElement('iframe');frame.id='nested';frame.srcdoc='<a href=\"#nested\">Nested target</a>';document.body.append(frame);return true})()", function(value) { reply=value;done=true })
        tryVerify(function() { return done }, 5000)
        compare(reply,true)
        var observed = false
        for (var attempt=0;attempt<30;++attempt) {
            done = false
            provider.run(owner, "(()=>{const child=document.getElementById('nested').contentDocument;if(!child||!child.querySelector('a'))return null;"+(data.focused ? "child.querySelector('a').focus();" : "document.querySelector('a').focus();")+"return {parent:document.hasFocus(),child:child.hasFocus()}})()", function(value) { reply=value;done=true })
            tryVerify(function() { return done }, 5000)
            if (reply && reply.parent === true && reply.child === data.focused) { observed=true;break }
            wait(20)
        }
        verify(observed,JSON.stringify(reply))
        var leases = provider.leaseCount
        done = false
        provider.verifyChain(owner, function(value) { reply=value;done=true })
        tryVerify(function() { return done }, 5000)
        if (data.focused) compare(reply.error,"frame-focus-changed",JSON.stringify(reply))
        else verify(!reply.error,JSON.stringify(reply))
        // Verification must not attach a new receiver while testing absence.
        compare(provider.leaseCount,leases)
    }
    function test_actual_collector_viewport_receipt_must_match_the_chain() {
        done = false
        provider.verifyChain(provider.scopedOwner, function(value) { reply = value; done = true })
        tryVerify(function() { return done }, 5000)
        var proof = reply
        verify(!proof.error, JSON.stringify(proof))
        done = false
        provider.run(provider.scopedOwner, Scripts.hintCollector("links"), function(value) {
            reply = value
            done = true
        })
        tryVerify(function() { return done }, 5000)
        var child = reply
        verify(!child.error, JSON.stringify(child))
        compare(child.candidates.length, 1)
        var base = {viewport:main("({width:innerWidth,height:innerHeight})")}
        verify(provider.collectionFitsProof(base, child, proof))
        // Equal pre/post proofs cannot validate an intervening collector's
        // different viewport, or missing receipt fields. Perturb the actual
        // untrusted payload rather than replacing native chain discovery.
        var changedChild = JSON.parse(JSON.stringify(child))
        changedChild.viewport.width += 20
        verify(!provider.collectionFitsProof(base, changedChild, proof))
        var changedBase = JSON.parse(JSON.stringify(base))
        changedBase.viewport.height += 20
        verify(!provider.collectionFitsProof(changedBase, child, proof))
        verify(!provider.collectionFitsProof(base, {candidates:child.candidates}, proof))
        verify(!provider.collectionFitsProof({}, child, proof))
    }
    function test_invalid_accounting_is_rejected_before_child_collection_data() {
        return [
            {tag:"string",mutation:"value.inspected_nodes='bad'"},
            {tag:"null",mutation:"value.inspected_nodes=null"},
            {tag:"negative",mutation:"value.inspected_nodes=-1"},
            {tag:"fraction",mutation:"value.inspected_nodes=0.5"},
            {tag:"over-limit",mutation:"value.inspected_nodes=50001"},
            {tag:"nonfinite",mutation:"value.inspected_nodes=NaN"},
            {tag:"omitted",mutation:"delete value.inspected_nodes"},
            {tag:"null-candidates",mutation:"value.candidates=null"},
            {tag:"array-like",mutation:"value.candidates={length:0}"},
            {tag:"excess-candidates",mutation:"value.candidates=Array(5001).fill({})"},
            {tag:"missing-completeness",mutation:"delete value.truncated"},
            {tag:"string-completeness",mutation:"value.truncated='false'"},
            {tag:"string-revision",mutation:"value.revision='1'"},
            {tag:"unsafe-revision",mutation:"value.revision=Number.MAX_SAFE_INTEGER+1"},
            {tag:"missing-selector-revision",mutation:"delete value.selector_revision"}
        ]
    }
    function test_invalid_accounting_is_rejected_before_child_collection(data) {
        provider.scopedOwner = ""
        var script = "(()=>{const value={candidates:[],inspected_nodes:0,truncated:false,revision:0,selector_revision:'',viewport:{width:innerWidth,height:innerHeight}};"+data.mutation+";return value})()"
        var childRequests = 0
        var options = {family:"links"}
        Object.defineProperty(options, "proxy", {get:function() { childRequests++; return false }})
        done = false
        provider.collect(page, script, options, function(value) { reply = value; done = true })
        tryVerify(function() { return done }, 5000)
        compare(reply.error, "invalid-frame-collection", JSON.stringify(reply))
        compare(childRequests, 0, "Malformed accounting must fail before constructing a child budget")
    }
    function test_exact_shared_inspection_budget_data() {
        return [{tag:"zero",inspections:0},{tag:"limit",inspections:50000}]
    }
    function test_exact_shared_inspection_budget(data) {
        provider.scopedOwner = ""
        var script = "({candidates:[],inspected_nodes:"+data.inspections+",truncated:false,revision:0,selector_revision:'',viewport:{width:innerWidth,height:innerHeight}})"
        done = false
        provider.collect(page, script, {family:"links"}, function(value) { reply = value; done = true })
        tryVerify(function() { return done }, 5000)
        verify(!reply.error, JSON.stringify(reply))
        verify(Number.isSafeInteger(reply.inspected_nodes) && reply.inspected_nodes <= 50000)
        compare(reply.candidates.length, data.inspections === 50000 ? 0 : 1)
    }
    function test_projection_never_coerces_raw_geometry_or_discards_bad_presentation_data() {
        return [
            {tag:"string-x",field:"x",value:"8"},
            {tag:"string-y",field:"y",value:"8"},
            {tag:"string-width",field:"width",value:"180"},
            {tag:"boolean-height",field:"height",value:true},
            {tag:"infinite-width",field:"width",value:Infinity},
            {tag:"bad-anchor",presentation:"anchor"},
            {tag:"bad-fragments",presentation:"fragments"},
            {tag:"extra-presentation-field",presentation:"extra"}
        ]
    }
    function test_projection_never_coerces_raw_geometry_or_discards_bad_presentation(data) {
        done = false
        provider.verifyChain(provider.scopedOwner, function(value) { reply = value; done = true })
        tryVerify(function() { return done }, 5000)
        var proof = reply
        verify(!proof.error, JSON.stringify(proof))
        done = false
        provider.run(provider.scopedOwner, Scripts.hintCollector("links"), function(value) { reply = value; done = true })
        tryVerify(function() { return done }, 5000)
        verify(!reply.error, JSON.stringify(reply))
        compare(reply.candidates.length, 1)
        var candidate = reply.candidates[0]
        verify(provider.project(candidate, provider.scopedOwner, proof.mappings) !== null)
        if (data.presentation) {
            candidate.presentation = {anchor:{x:candidate.geometry.x,y:candidate.geometry.y},fragments:[JSON.parse(JSON.stringify(candidate.geometry))]}
            verify(provider.project(candidate, provider.scopedOwner, proof.mappings) !== null)
            if (data.presentation === "anchor") candidate.presentation.anchor.x = "invalid"
            if (data.presentation === "fragments") candidate.presentation.fragments = "invalid"
            if (data.presentation === "extra") candidate.presentation.command = "forbidden"
        } else candidate.geometry[data.field] = data.value
        var projected, threw = false
        try { projected = provider.project(candidate, provider.scopedOwner, proof.mappings) }
        catch (error) { threw = true }
        verify(!threw, "Malformed untrusted geometry must fail closed without throwing")
        compare(projected, null, "Projection must not normalize malformed source facts into accepted numbers")
    }
    function test_changed_mapping_during_collection_is_rejected_data() {
        return [
            {tag:"move",property:"left",initial:40,changed:120,script:"document.getElementById('subject').style.left='120px';true"},
            {tag:"resize",property:"width",initial:400,changed:460,script:"document.getElementById('subject').style.width='460px';true"}
        ]
    }
    function test_changed_mapping_during_collection_is_rejected(data) {
        compare(main("document.getElementById('subject').getBoundingClientRect()."+data.property), data.initial)
        var scheduled = 0, mutationDone = false
        var options = {family:"links"}
        // The production provider reads proxy while constructing the child
        // collector, after its first real chain proof. Queue a real parent DOM
        // change then; later parent proof queries share that frame's ordered
        // script queue. No broker, chain proof or collector result is mocked.
        Object.defineProperty(options, "proxy", {get:function() {
            scheduled++
            page.runJavaScript(data.script, WebEngineScript.MainWorld, function(value) {
                mutationDone = value === true
            })
            return false
        }})
        var value = collect(options)
        compare(scheduled, 1)
        tryVerify(function() { return mutationDone }, 5000)
        compare(main("document.getElementById('subject').getBoundingClientRect()."+data.property), data.changed)
        compare(value.error, "frame-geometry-changed", JSON.stringify(value))
    }
}

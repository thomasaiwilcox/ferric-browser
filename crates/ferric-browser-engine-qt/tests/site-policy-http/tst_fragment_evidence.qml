import QtQuick
import QtTest
import QtWebEngine
import "../../qml/scripts/BrowserScripts.js" as Scripts
import "../../qml/scripts/PageInteractionScripts.js" as Policy

TestCase {
    id: tests
    name: "PolicyFragmentEvidence"
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
    function init() {
        page.loaded=false
        page.url="http://127.0.0.1:8766/roots.html"
        tryVerify(function(){return page.loaded},10000)
    }
    function cleanup() { evaluate(Scripts.hintStopTracking(),WebEngineScript.ApplicationWorld) }
    function fixture(lines,setup) {
        return evaluate("(()=>{document.body.replaceChildren();const host=document.createElement('div');host.style='position:fixed;left:40px;top:40px;width:70px;font:20px/30px monospace';const target=document.createElement('a');target.id='fragment-target';target.href='#fragment';target.textContent=Array("+lines+").fill('word').join(' ');host.append(target);document.body.append(host);"+(setup||"")+";return {rects:target.getClientRects().length}})()",WebEngineScript.MainWorld)
    }
    function measure(limit,clip,policy,targetExpression) {
        var script="(()=>{"+Policy.scrollTargetRuntime()+Policy.hintNameRuntime()
            +";let remaining="+(limit===undefined?50000:limit)+";function inspect(){if(remaining===0)return false;remaining--;return true}"
            +"const target="+(targetExpression||"document.getElementById('fragment-target')")+",action="+JSON.stringify(policy||"link")+",local=ftLocalVisible(target,target.ownerDocument.defaultView,action);if(!local)return {admitted:false};"
            +"const topClip="+JSON.stringify(clip||{left:0,top:0,right:1000,bottom:800})+",evidence={};"
            +"const display=ftHintPresentation(target,local,action,topClip,null),proofDisplay=ftHintPresentation(target,local,action,topClip,null,evidence);"
            +"return {admitted:true,display:display,proofDisplay:proofDisplay,evidence:evidence,remaining:remaining,rects:target.getClientRects().length};})()"
        return JSON.parse(evaluate("JSON.stringify("+script+")",WebEngineScript.ApplicationWorld))
    }
    function sameDisplay(value) {
        verify(value.admitted,JSON.stringify(value))
        compare(JSON.stringify(value.proofDisplay),JSON.stringify(value.display),"Proof extraction preserves ordinary presentation")
        verify(typeof value.evidence.complete==="boolean","Fragment proof must report completeness")
    }
    function test_single_border_box_reuses_projected_geometry() {
        fixture(1,"target.style='display:block;width:120px;height:40px'")
        var value=measure()
        sameDisplay(value)
        compare(value.display,null)
        compare(value.evidence.complete,true)
        compare(value.evidence.primary.x,40)
        compare(value.evidence.primary.y,40)
        compare(value.evidence.primary.width,120)
        compare(value.evidence.primary.height,40)
        compare(value.evidence.primary_index,0)
        compare(value.remaining,49999)
        var ordinary=evaluate(Scripts.hintCollector("links"),WebEngineScript.ApplicationWorld)
        compare(ordinary.candidates.length,1)
        for(var field of ["x","y","width","height"])compare(ordinary.candidates[0].geometry[field],value.evidence.primary[field])
    }
    function test_multiple_fragment_bounds_data() {
        // Natural wrapping gives one native inline rectangle per line; BR
        // elements would add separate zero-width rectangles to this count.
        return [{tag:"exact-eight",lines:8,complete:true},{tag:"over-eight",lines:9,complete:false}]
    }
    function test_multiple_fragment_bounds(data) {
        compare(fixture(data.lines).rects,data.lines)
        var value=measure()
        sameDisplay(value)
        compare(value.evidence.complete,data.complete)
        compare(value.evidence.reason,data.complete?null:"truncated")
        compare(value.display.fragments.length,8)
        compare(value.evidence.primary.y,value.display.fragments[0].y)
        compare(value.remaining,49992)
    }
    function test_inspection_exhaustion_data() {
        return [{tag:"single",lines:1,limit:0},{tag:"multiple",lines:4,limit:2}]
    }
    function test_inspection_exhaustion(data) {
        fixture(data.lines,data.lines===1?"target.style='display:block;width:120px;height:40px'":"")
        var value=measure(data.limit)
        sameDisplay(value)
        compare(value.evidence.complete,false)
        compare(value.evidence.reason,"inspection-limit")
        compare(value.remaining,0)
    }
    function test_occluded_first_fragment_is_not_the_canonical_receiver() {
        compare(fixture(4,"const first=target.getClientRects()[0];const cover=document.createElement('div');cover.style='position:fixed;z-index:2;background:white;left:'+first.left+'px;top:'+first.top+'px;width:'+first.width+'px;height:'+first.height+'px';document.body.append(cover)").rects,4)
        var value=measure()
        sameDisplay(value)
        compare(value.evidence.complete,true)
        verify(value.evidence.primary.y>value.display.fragments[0].y,JSON.stringify(value))
        compare(value.evidence.primary.y,value.display.fragments[1].y)
        compare(value.evidence.primary_index,1)
    }
    function test_eligible_fragment_inventory_keeps_original_indices_data() {
        return [{tag:"wrapped",cover:false,vertical:false},{tag:"covered-first",cover:true,vertical:false},
            {tag:"visual-order-differs",cover:false,vertical:true}]
    }
    function test_eligible_fragment_inventory_keeps_original_indices(data) {
        var setup=data.vertical?"host.style.writingMode='vertical-rl';host.style.width='300px';host.style.height='70px'":""
        if(data.cover)setup="const first=target.getClientRects()[0],cover=document.createElement('div');cover.style='position:fixed;z-index:2;background:white;left:'+first.left+'px;top:'+first.top+'px;width:'+first.width+'px;height:'+first.height+'px';document.body.append(cover)"
        compare(fixture(4,setup).rects,4)
        var value=measure()
        sameDisplay(value)
        compare(value.evidence.complete,true)
        verify(Array.isArray(value.evidence.eligible_fragments),"Projection needs the complete eligible fragment alternatives")
        var eligible=value.evidence.eligible_fragments
        compare(eligible.length,data.cover?3:4)
        for(var i=0;i<eligible.length;i++) {
            var index=i+(data.cover?1:0)
            compare(eligible[i].index,index,"Keep getClientRects indices, including gaps left by covered fragments")
            compare(JSON.stringify(eligible[i].rect),JSON.stringify(value.display.fragments[index]))
        }
        var canonical=eligible.slice().sort(function(a,b){return a.rect.y-b.rect.y||a.rect.x-b.rect.x||a.index-b.index})[0]
        compare(value.evidence.primary_index,canonical.index)
        compare(JSON.stringify(value.evidence.primary),JSON.stringify(canonical.rect))
    }
    function test_eligible_fragment_inventory_does_not_reset_existing_limits_data() {
        return [{tag:"exact-eight",lines:8,limit:50000,complete:true,count:8},
            {tag:"ninth-incomplete",lines:9,limit:50000,complete:false,count:8},
            {tag:"inspection-prefix",lines:4,limit:2,complete:false,count:2},
            {tag:"single-no-work",lines:1,limit:0,complete:false,count:0}]
    }
    function test_eligible_fragment_inventory_does_not_reset_existing_limits(data) {
        compare(fixture(data.lines,data.lines===1?"target.style='display:block;width:120px;height:40px'":"").rects,data.lines)
        var value=measure(data.limit)
        sameDisplay(value)
        compare(value.evidence.complete,data.complete)
        verify(Array.isArray(value.evidence.eligible_fragments))
        compare(value.evidence.eligible_fragments.length,data.count)
        for(var i=0;i<data.count;i++)compare(value.evidence.eligible_fragments[i].index,i)
        compare(value.remaining,Math.max(0,data.limit-Math.min(8,data.lines)))
    }
    function test_same_y_uses_leftmost_fragment_before_dom_order() {
        compare(fixture(4,"host.style.writingMode='vertical-rl';host.style.width='300px';host.style.height='70px'").rects,4)
        var value=measure()
        sameDisplay(value)
        compare(value.evidence.complete,true)
        var first=value.display.fragments[0],last=value.display.fragments[3]
        compare(first.y,last.y)
        verify(last.x<first.x,"Vertical right-to-left lines must differ from DOM order")
        compare(value.evidence.primary.x,last.x)
        compare(value.evidence.primary.y,last.y)
        compare(value.evidence.primary_index,3)
    }
    function test_scope_clip_is_applied_before_primary_selection() {
        fixture(1,"target.style='display:block;width:120px;height:40px'")
        var value=measure(undefined,{left:60,top:50,right:140,bottom:70})
        sameDisplay(value)
        compare(value.evidence.complete,true)
        compare(value.evidence.primary.x,60)
        compare(value.evidence.primary.y,50)
        compare(value.evidence.primary.width,80)
        compare(value.evidence.primary.height,20)
        compare(value.evidence.eligible_fragments.length,1)
        compare(JSON.stringify(value.evidence.eligible_fragments[0]),JSON.stringify({index:0,rect:value.evidence.primary}))
    }
    function test_hit_testing_is_restricted_to_the_final_scope_clip() {
        fixture(1,"target.style='display:block;width:120px;height:40px';const cover=document.createElement('div');cover.style='position:fixed;z-index:2;left:40px;top:40px;width:40px;height:40px;background:white';document.body.append(cover)")
        var value=measure(undefined,{left:40,top:40,right:75,bottom:80})
        sameDisplay(value)
        compare(value.evidence.complete,true)
        compare(value.evidence.primary,null,"An uncovered part outside the scope clip cannot prove a hit")
        compare(value.evidence.eligible_fragments.length,0)
    }
    function test_projected_fragment_checks_each_parent_receiver() {
        evaluate("(()=>{document.body.replaceChildren();const frame=document.createElement('iframe');frame.id='fragment-frame';frame.style='position:fixed;left:40px;top:40px;width:300px;height:150px;border:0';frame.srcdoc='<a id=fragment-target href=#fragment style=\"position:fixed;left:20px;top:20px;display:block;width:120px;height:40px\">Frame target</a>';document.body.append(frame);return true})()",WebEngineScript.MainWorld)
        tryVerify(function(){return evaluate("!!document.getElementById('fragment-frame').contentDocument.getElementById('fragment-target')",WebEngineScript.MainWorld)},10000)
        var target="document.getElementById('fragment-frame').contentDocument.getElementById('fragment-target')"
        var value=measure(undefined,undefined,undefined,target)
        sameDisplay(value)
        compare(value.evidence.complete,true)
        compare(value.evidence.primary.x,60)
        compare(value.evidence.primary.y,60)
        compare(value.evidence.primary.width,120)
        compare(value.remaining,49998)
        evaluate("(()=>{const cover=document.createElement('div');cover.style='position:fixed;z-index:2;left:60px;top:60px;width:40px;height:40px;background:white';document.body.append(cover);return true})()",WebEngineScript.MainWorld)
        value=measure(undefined,{left:60,top:60,right:95,bottom:100},undefined,target)
        sameDisplay(value)
        compare(value.evidence.complete,true)
        compare(value.evidence.primary,null,"Another uncovered part of the parent iframe cannot prove the clipped fragment")
        compare(value.evidence.eligible_fragments.length,0)
    }
    function test_parent_and_target_require_the_same_hit_point_data() {
        return [{tag:"disjoint-receivers",opposite:true,visible:false},
            {tag:"common-receiver",opposite:false,visible:true}]
    }
    function test_parent_and_target_require_the_same_hit_point(data) {
        evaluate("(()=>{document.body.replaceChildren();const frame=document.createElement('iframe');frame.id='fragment-frame';frame.style='position:fixed;left:40px;top:40px;width:200px;height:100px;border:0';frame.srcdoc='<a id=fragment-target href=#fragment style=\"position:fixed;left:0;top:0;display:block;width:200px;height:100px\">Frame target</a>';document.body.append(frame);return true})()",WebEngineScript.MainWorld)
        var target="document.getElementById('fragment-frame').contentDocument.getElementById('fragment-target')"
        tryVerify(function(){return evaluate("!!("+target+")",WebEngineScript.MainWorld)},10000)
        evaluate("(()=>{const doc=document.getElementById('fragment-frame').contentDocument,child=doc.createElement('div'),parent=document.createElement('div');child.style='position:fixed;z-index:2;left:"+(data.opposite?0:100)+"px;top:0;width:100px;height:100px;background:white';doc.body.append(child);parent.style='position:fixed;z-index:2;left:140px;top:40px;width:100px;height:100px;background:white';document.body.append(parent);return true})()",WebEngineScript.MainWorld)
        var value=measure(undefined,undefined,undefined,target)
        sameDisplay(value)
        compare(value.evidence.complete,true)
        compare(value.evidence.eligible_fragments.length,data.visible?1:0,
            "Separate uncovered points in parent and child cannot prove one exposed target")
        compare(value.evidence.primary!==null,data.visible)
    }
    function test_nested_frame_projection_data() {
        return [{tag:"unscaled",scale:1},{tag:"scaled-parent",scale:1.25}]
    }
    function test_nested_frame_projection(data) {
        evaluate("(()=>{document.body.replaceChildren();const frame=document.createElement('iframe');frame.id='outer';frame.style='position:fixed;left:40px;top:40px;width:300px;height:200px;border:0;transform-origin:0 0;transform:scale("+data.scale+")';frame.srcdoc='<body></body>';document.body.append(frame);return true})()",WebEngineScript.MainWorld)
        tryVerify(function(){return evaluate("(()=>{const doc=document.getElementById('outer').contentDocument;return !!doc&&doc.URL==='about:srcdoc'&&doc.readyState==='complete'&&!!doc.body})()",WebEngineScript.MainWorld)},10000)
        evaluate("(()=>{const frame=document.createElement('iframe');frame.id='inner';frame.style='position:fixed;left:20px;top:20px;width:200px;height:120px;border:0';frame.srcdoc='<a id=fragment-target href=#fragment style=\"position:fixed;left:10px;top:10px;display:block;width:100px;height:30px\">Nested target</a>';document.getElementById('outer').contentDocument.body.append(frame);return true})()",WebEngineScript.MainWorld)
        var target="document.getElementById('outer').contentDocument.getElementById('inner').contentDocument.getElementById('fragment-target')"
        tryVerify(function(){return evaluate("!!("+target+")",WebEngineScript.MainWorld)},10000)
        var value=measure(undefined,undefined,undefined,target)
        sameDisplay(value)
        compare(value.evidence.complete,true)
        compare(value.evidence.primary.x,40+30*data.scale)
        compare(value.evidence.primary.y,40+30*data.scale)
        compare(value.evidence.primary.width,100*data.scale)
        compare(value.evidence.primary.height,30*data.scale)
        compare(value.remaining,49997)
        var exhausted=measure(2,undefined,undefined,target)
        sameDisplay(exhausted)
        compare(exhausted.evidence.complete,false)
        compare(exhausted.evidence.reason,"inspection-limit")
        compare(exhausted.evidence.primary,null)
        compare(exhausted.remaining,0)
    }
    function test_focus_policy_retains_its_existing_nonpointer_visibility() {
        fixture(1,"target.remove();const input=document.createElement('input');input.id='fragment-target';input.style='display:block;width:120px;height:40px;box-sizing:border-box';host.append(input);const cover=document.createElement('div');cover.style='position:fixed;z-index:2;left:40px;top:40px;width:120px;height:40px;background:white';document.body.append(cover)")
        var value=measure(undefined,undefined,"focus")
        sameDisplay(value)
        compare(value.evidence.complete,true)
        compare(value.evidence.primary.x,40)
        compare(value.evidence.primary.y,40)
        compare(value.evidence.primary.width,120)
        compare(value.evidence.eligible_fragments.length,1,"Focus retains its existing covered-target eligibility")
    }
}

import QtQuick
import QtTest
import QtWebEngine
import "../../qml/scripts/SitePolicyCapture.js" as Capture

TestCase {
    id: test
    name: "SitePolicyCaptureFacts"
    when: windowShown
    width: 1000; height: 800
    property bool ready: false
    property bool done: false
    property var result
    WebEngineView {
        id: view; anchors.fill: parent
        onLoadingChanged: function(request) { if(request.status===WebEngineView.LoadSucceededStatus)test.ready=true }
    }
    function js(source) {
        done=false
        view.runJavaScript(source,function(value){result=value;done=true})
        tryVerify(function(){return done},5000)
        return result
    }
    function init() {
        ready=false
        view.loadHtml("<!doctype html><body><section id='cards'><article class='card' data-id='one'><a id='alpha' href='https://user:secret@example.test/page?token=secret#frag'>Alpha</a></article><article class='card' data-id='two'><a>Beta</a></article></section><x-card id='shadow-card'></x-card><x-slot id='slot-host'><a id='slotted' slot='entry'>Slot</a></x-slot><input id='password' type='password' value='do-not-read'><input id='text-input' value='also-secret'><iframe id='frame' srcdoc='<button id=child>Child</button>'></iframe>","https://capture.test/")
        tryVerify(function(){return ready},10000)
        verify(js("(()=>{document.getElementById('shadow-card').attachShadow({mode:'open'}).innerHTML='<button class=go>Inside</button>';document.getElementById('slot-host').attachShadow({mode:'open'}).innerHTML='<slot name=entry></slot>';window.captureEvents={click:0,focus:0,key:0};document.addEventListener('click',()=>captureEvents.click++);document.addEventListener('focusin',()=>captureEvents.focus++);document.addEventListener('keydown',()=>captureEvents.key++);return true})()"))
    }
    function facts(expression, text, href) {
        return js("("+Capture.factsSource()+")("+expression+","+JSON.stringify({text:text||"",href:href||null,geometry:{x:10,y:10,width:100,height:20}})+")")
    }
    function test_selected_facts_are_bounded_and_exclude_values_and_url_secrets() {
        verify(js("(()=>{const el=document.getElementById('alpha');el.setAttribute('aria-label','<img src=x onerror=alert(1)>; quit');el.setAttribute('data-testid','x'.repeat(1024));return true})()"))
        var value=facts("document.getElementById('alpha')","<b>Alpha</b>; quit","https://user:secret@example.test/page?token=secret#frag")
        compare(value.href.origin,"https://example.test");compare(value.href.path,"/page")
        verify(!JSON.stringify(value).includes("secret"))
        compare(value.attributes.filter(function(attr){return attr.name==="data-testid"})[0].oversized,true)
        verify(!JSON.stringify(value).includes("x".repeat(300)))
        compare(facts("document.getElementById('password')","do-not-read").hint_name,"")
        verify(!JSON.stringify(facts("document.getElementById('text-input')","also-secret")).includes("also-secret"))
        compare(js("JSON.stringify(captureEvents)"),'{"click":0,"focus":0,"key":0}')
        compare(value.activation_permitted,false)
    }
    function test_shadow_primary_is_relative_to_the_item_and_repeated_selector_has_counts() {
        var value=facts("document.getElementById('shadow-card').shadowRoot.querySelector('button')","Inside")
        compare(value.root.steps[0].kind,"open-shadow")
        var item=value.ancestors.filter(function(item){return item.tag==="x-card"})[0]
        compare(item.primary.primary_root_steps[0].kind,"open-shadow")
        compare(item.primary.primary_root_steps[0].self,true)
        var button=item.primary.suggestions.filter(function(row){return row.selector==="button"})[0]
        compare(button.count,1);compare(button.complete,true)
        var ordinary=facts("document.getElementById('alpha')","Alpha")
        var card=ordinary.ancestors.filter(function(item){return item.tag==="article"})[0]
        compare(card.selectors.filter(function(row){return row.selector==="article.card"})[0].count,2)
        compare(card.primary.primary_root_steps.length,0)
    }
    function test_same_origin_and_assigned_slot_paths_remain_explicit() {
        tryVerify(function(){return js("!!document.getElementById('frame').contentDocument.getElementById('child')")},5000)
        var framed=facts("document.getElementById('frame').contentDocument.getElementById('child')","Child")
        compare(framed.root.steps[0].kind,"same-origin-frame")
        compare(framed.root.start,"top-document")
        var slotted=facts("document.getElementById('slotted')","Slot")
        compare(slotted.slot_paths.length,1)
        compare(slotted.slot_paths[0].root.steps[0].kind,"open-shadow")
        compare(slotted.slot_paths[0].root.steps[1].kind,"assigned-slot")
        compare(slotted.slot_paths[0].complete,false)
    }
    function test_slot_collection_counts_are_relative_to_the_assigned_container() {
        verify(js("(()=>{document.getElementById('slot-host').innerHTML='<article id=assigned class=card slot=entry><article class=card><a id=chosen>Chosen</a></article><article class=card><a>Other</a></article></article>';return true})()"))
        var value=facts("document.getElementById('chosen')","Chosen"),slot=value.slot_paths[0]
        compare(slot.ancestor_level,2)
        compare(slot.relative_ancestors[1].selectors.filter(function(row){return row.selector==='article.card'})[0].count,2)
        compare(value.ancestors[1].selectors.filter(function(row){return row.selector==='article.card'})[0].count,5)
        verify(slot.relative_ancestors[3]==null)
    }
}

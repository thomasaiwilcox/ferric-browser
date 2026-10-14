import QtQuick
import QtTest
import QtWebEngine
import "../../qml/scripts/BrowserScripts.js" as BrowserScripts

TestCase {
    id: test
    name: "HintQuality"
    when: windowShown
    visible: true
    width: 1000
    height: 800
    property bool done: false
    property var result: null
    WebEngineProfile { id: privateProfile; offTheRecord: true }
    WebEngineView {
        id: page
        anchors.fill: parent
        profile: privateProfile
        property bool loaded: false
        onLoadingChanged: function(request) { loaded=request.status===WebEngineView.LoadSucceededStatus }
    }
    function evaluate(source) {
        done=false
        page.runJavaScript(source,WebEngineScript.ApplicationWorld,function(value){result=value;done=true})
        tryVerify(function(){return done},10000)
        return result
    }
    function load(body,style) {
        page.loaded=false
        page.loadHtml("<!doctype html><meta charset=utf-8><style>body{margin:0;font:16px sans-serif}"+(style||"")+"</style>"+body,"https://fixture.test/")
        tryVerify(function(){return page.loaded},10000)
        wait(100)
    }
    function cleanup() { evaluate(BrowserScripts.hintStopTracking()) }
    function stretchedPost() {
        load("<article id=card><a id=cover slot=cover href=/posts/example/comments aria-label='Open whole post'></a><a id=community slot=credit href=/community>Community</a><a id=title slot=title href=/posts/example/comments>Article title</a></article>",
             "body{height:1800px}#card{position:relative;margin:450px 20px 0;width:600px;height:180px}#cover{position:absolute;inset:0;z-index:2}#community{position:absolute;left:16px;top:8px;width:140px;height:24px;z-index:3}#title{position:absolute;left:16px;top:40px;width:568px;height:48px}")
        evaluate("document.getElementById('card').attachShadow({mode:'open'}).innerHTML='<slot name=cover></slot><slot name=credit></slot><h2><slot name=title></slot></h2>';scrollTo(0,400);true")
        wait(100)
    }
    function test_stretched_link_title_data() {
        return [{tag:"zoom80",zoom:0.8},{tag:"zoom100",zoom:1},{tag:"zoom125",zoom:1.25}]
    }
    function test_stretched_link_title(data) {
        page.zoomFactor=data.zoom
        stretchedPost()
        var snapshot=evaluate(BrowserScripts.hintCollector("links"))
        var title=snapshot.candidates.filter(function(c){return c.text==='Article title'})[0]
        verify(!!title,"A visible title covered by its same-destination card link needs its own hint")
        verify(title.href.endsWith('/posts/example/comments'))
        verify(evaluate(BrowserScripts.hintFresh(title)).visible)
        // URL opening can resolve the title; native delivery must still identify
        // the actual pointer receiver, rather than dispatching to another node.
        verify(!evaluate(BrowserScripts.hintPointerPreflight(title,1000,800)).ok)
        verify(!evaluate(BrowserScripts.hintClick(title)))
        evaluate("document.getElementById('cover').href='/community';true")
        verify(!evaluate(BrowserScripts.hintFresh(title)).visible)
        evaluate("document.getElementById('cover').href='/posts/example/comments';document.getElementById('title').href='/changed';true")
        verify(!evaluate(BrowserScripts.hintFresh(title)).visible)
        page.zoomFactor=1
    }
    function test_stretched_link_excludes_unrelated_and_hidden_targets() {
        stretchedPost()
        for(var mutation of ["document.getElementById('cover').href='/community'",
                             "document.getElementById('cover').style.background='white'",
                             "document.getElementById('title').style.visibility='hidden'",
                             "document.getElementById('title').setAttribute('aria-disabled','true')",
                             "document.getElementById('card').inert=true"]) {
            evaluate(mutation+";true")
            var snapshot=evaluate(BrowserScripts.hintCollector("links"))
            verify(!snapshot.candidates.some(function(c){return c.text==='Article title'}))
            evaluate("document.getElementById('cover').href='/posts/example/comments';document.getElementById('cover').style.background='';document.getElementById('title').style.visibility='';document.getElementById('title').removeAttribute('aria-disabled');document.getElementById('card').inert=false;true")
        }
        // A separate dialog is not a card alias, even if its link has the same URL.
        evaluate("const dialog=document.createElement('div');dialog.innerHTML='<a href=/posts/example/comments style=\"position:fixed;inset:0;z-index:20\">Dialog link</a>';document.body.append(dialog);true")
        verify(!evaluate(BrowserScripts.hintCollector("links")).candidates.some(function(c){return c.text==='Article title'}))
    }
    function test_stretched_link_nested_title() {
        stretchedPost()
        evaluate("const title=document.getElementById('title'),wrapper=document.createElement('div');wrapper.slot='title';title.removeAttribute('slot');title.replaceWith(wrapper);wrapper.append(title);true")
        var title=evaluate(BrowserScripts.hintCollector("links")).candidates.filter(function(c){return c.text==='Article title'})[0]
        verify(!!title)
        verify(evaluate(BrowserScripts.hintFresh(title)).visible)
        verify(!evaluate(BrowserScripts.hintClick(title)))
    }
    function test_budget_starves_shadow_controls() {
        load("<main id=links></main><div id=shadow></div>","a{display:inline-block;width:36px;height:24px}#shadow{position:absolute;top:500px;left:30px}")
        evaluate("document.getElementById('links').innerHTML=Array.from({length:250},(_,i)=>'<a href=\"/target-'+i+'\">'+i+'</a>').join('');document.getElementById('shadow').attachShadow({mode:'open'}).innerHTML='<button>Shadow action</button>';true")
        var snapshot=evaluate(BrowserScripts.hintCollector("all"))
        var controls=snapshot.candidates.filter(function(c){return c.kind==='button'})
        console.log("BUDGET",JSON.stringify({fixture_links:250,unique_elements:evaluate("document.querySelectorAll('*').length+document.getElementById('shadow').shadowRoot.querySelectorAll('*').length"),candidates:snapshot.candidates.length,buttons:controls.length,inspected:snapshot.inspected_nodes,truncated:snapshot.truncated,work:snapshot.work}))
        verify(snapshot.inspected_nodes<50000);verify(!snapshot.truncated);compare(controls.length,1)
        // Isolating the same button proves it is eligible and visible.
        var buttons=evaluate(BrowserScripts.hintCollector("buttons"))
        compare(buttons.candidates.length,1)
    }
    function test_accessible_names() {
        load("<span id=search-name>Search</span><button id=labelled aria-labelledby=search-name><svg width=24 height=24><circle cx=10 cy=10 r=8 /></svg></button><button id=direct aria-label=Direct><svg width=24 height=24><circle cx=10 cy=10 r=8 /></svg></button><img id=alternative width=40 height=30 alt='Useful image' src='data:image/svg+xml,%3Csvg xmlns=%22http://www.w3.org/2000/svg%22 width=%2240%22 height=%2230%22/%3E'>", "button,img{display:block;margin:20px}")
        var snapshot=evaluate(BrowserScripts.hintCollector("all"))
        var names=snapshot.candidates.map(function(c){return {kind:c.kind,text:c.text}})
        console.log("NAMES",JSON.stringify(names))
        verify(names.some(function(c){return c.kind==='button'&&c.text==='Search'}))
        verify(names.some(function(c){return c.text==='Direct'}))
        verify(names.some(function(c){return c.kind==='image'&&c.text==='Useful image'}))
    }
    function test_transformed_frame_geometry() {
        load("<iframe id=frame srcdoc=\"<style>body{margin:0}button{position:absolute;left:30px;top:20px;width:80px;height:20px}</style><button>Scaled frame control</button>\"></iframe>","iframe{position:absolute;left:40px;top:40px;width:250px;height:120px;border:0;transform-origin:0 0;transform:scale(2)}")
        tryVerify(function(){return evaluate("!!document.getElementById('frame').contentDocument.querySelector('button')")},5000)
        var snapshot=evaluate(BrowserScripts.hintCollector("buttons"))
        var candidate=snapshot.candidates.filter(function(c){return c.text==='Scaled frame control'})[0]
        verify(!!candidate)
        console.log("SCALED_FRAME",JSON.stringify({reported:candidate.geometry,expected:{x:100,y:80,width:160,height:40}}))
        compare(candidate.geometry.x,100);compare(candidate.geometry.y,80);compare(candidate.geometry.width,160);compare(candidate.geometry.height,40)
    }
    function test_padded_frame_geometry() {
        load("<iframe id=frame srcdoc=\"<style>body{margin:0}button{position:absolute;left:30px;top:20px;width:80px;height:20px}</style><button>Padded frame control</button>\"></iframe>","iframe{position:absolute;left:40px;top:40px;width:250px;height:120px;border:3px solid;padding:10px;transform-origin:0 0;transform:scale(2)}")
        tryVerify(function(){return evaluate("!!document.getElementById('frame').contentDocument.querySelector('button')")},5000)
        var snapshot=evaluate(BrowserScripts.hintCollector("buttons"))
        var candidate=snapshot.candidates.filter(function(c){return c.text==='Padded frame control'})[0]
        verify(!!candidate)
        compare(candidate.geometry.x,126);compare(candidate.geometry.y,106)
        compare(candidate.geometry.width,160);compare(candidate.geometry.height,40)
        // Native preflight retains the existing transform rejection safeguard.
        verify(!evaluate(BrowserScripts.hintPointerPreflight(candidate,1000,800)).ok)
    }
    function test_padded_frame_pointer_inverse() {
        load("<iframe id=frame srcdoc=\"<style>body{margin:0}button{position:absolute;left:30px;top:20px;width:8px;height:8px;padding:0}</style><button aria-label='Small framed control'></button>\"></iframe>","iframe{position:absolute;left:40px;top:40px;width:250px;height:120px;border:3px solid;padding:10px}")
        tryVerify(function(){return evaluate("!!document.getElementById('frame').contentDocument.querySelector('button')")},5000)
        var snapshot=evaluate(BrowserScripts.hintCollector("buttons"))
        var candidate=snapshot.candidates[0]
        verify(!!candidate)
        compare(candidate.geometry.x,83);compare(candidate.geometry.y,73)
        verify(evaluate(BrowserScripts.hintPointerPreflight(candidate,1000,800)).ok)
    }
    function test_nested_frame_pointer_inverse() {
        load("<iframe id=outer></iframe>","iframe{position:absolute;left:40px;top:40px;width:400px;height:250px;border:3px solid;padding:10px}")
        evaluate("document.getElementById('outer').srcdoc='<style>body{margin:0}iframe{position:absolute;left:20px;top:20px;width:200px;height:100px;border:0}</style><iframe id=inner></iframe>';true")
        tryVerify(function(){return evaluate("!!document.getElementById('outer').contentDocument.getElementById('inner')")},5000)
        evaluate("document.getElementById('outer').contentDocument.getElementById('inner').srcdoc='<style>body{margin:0}button{position:absolute;left:30px;top:20px;width:8px;height:8px;padding:0}</style><button aria-label=Nested></button>';true")
        tryVerify(function(){return evaluate("!!document.getElementById('outer').contentDocument.getElementById('inner').contentDocument.querySelector('button')")},5000)
        var snapshot=evaluate(BrowserScripts.hintCollector("buttons"))
        compare(snapshot.candidates.length,1)
        var candidate=snapshot.candidates[0]
        compare(candidate.frame_path,'0.0.0')
        compare(candidate.geometry.x,103);compare(candidate.geometry.y,93)
        verify(evaluate(BrowserScripts.hintPointerPreflight(candidate,1000,800)).ok)
    }
    function test_wrapped_fragments_and_single_box_fallback() {
        load("<div style='width:100px'><a href=/wrapped>Several words form multiple visible fragments here</a></div><button>Single box</button>","a{line-height:24px}")
        var snapshot=evaluate(BrowserScripts.hintCollector("all"))
        var wrapped=snapshot.candidates.filter(function(c){return c.kind==='link'})[0]
        var single=snapshot.candidates.filter(function(c){return c.kind==='button'})[0]
        verify(!!wrapped&&!!single)
        verify(wrapped.presentation.fragments.length>1)
        compare(wrapped.presentation.anchor.x,wrapped.presentation.fragments[0].x)
        compare(wrapped.presentation.anchor.y,wrapped.presentation.fragments[0].y)
        verify(single.presentation===null||single.presentation===undefined)
    }
    function test_focusable_container_intent() {
        load("<div id=focus-only tabindex=0 aria-label='Focusable widget'>Keyboard region</div><button>Native button</button>","#focus-only{width:200px;height:50px;margin:20px}")
        var snapshot=evaluate(BrowserScripts.hintCollector("all"))
        var candidate=snapshot.candidates.filter(function(c){return c.text==='Focusable widget'})[0]
        compare(candidate.kind,"focusable")
        var outcome=evaluate(BrowserScripts.hintFocus(candidate))
        var focused=evaluate("document.activeElement.id")
        console.log("FOCUS_ONLY",JSON.stringify({kind:candidate.kind,click_result:outcome,active_id:focused}))
        compare(focused,'focus-only')
    }
    function test_focusable_structural_role_intent() {
        load("<div id=region role=region tabindex=0 aria-label='Named keyboard region'>Region</div>","#region{width:200px;height:50px;margin:20px}")
        var snapshot=evaluate(BrowserScripts.hintCollector("all"))
        var candidate=snapshot.candidates.filter(function(c){return c.text==='Named keyboard region'})[0]
        compare(candidate.kind,"focusable")
        verify(evaluate(BrowserScripts.hintFocus(candidate)))
        compare(evaluate("document.activeElement.id"),'region')
    }
    function test_slotted_non_controls_bypass_selectors() {
        load("<div id=host><div id=plain slot=body>Plain slotted content</div><button id=action slot=body>Actual control</button></div>","#plain{display:block;width:200px;height:60px}")
        evaluate("document.getElementById('host').attachShadow({mode:'open'}).innerHTML='<slot name=body></slot>';true")
        var result=[]
        for(var family of ['all','buttons']) {
            var snapshot=evaluate(BrowserScripts.hintCollector(family))
            compare(snapshot.candidates.length,1)
            verify(snapshot.candidates.every(function(c){return c.kind==='button'}))
            for(var candidate of snapshot.candidates) verify(evaluate(BrowserScripts.hintFresh(candidate)).visible)
        }
    }
    function test_coincident_distinct_actions() {
        load("<div id=host role=button tabindex=0 onclick='document.body.dataset.outer=Number(document.body.dataset.outer||0)+1'><button id=inner onclick='event.stopPropagation();document.body.dataset.inner=Number(document.body.dataset.inner||0)+1'>Inner action</button></div>","#host{position:absolute;left:40px;top:50px;width:160px;height:32px}#inner{width:100%;height:100%}")
        var snapshot=evaluate(BrowserScripts.hintCollector("buttons"))
        compare(snapshot.candidates.length,2)
        for(var c of snapshot.candidates) evaluate(BrowserScripts.hintClick(c))
        var effects=evaluate("({outer:Number(document.body.dataset.outer||0),inner:Number(document.body.dataset.inner||0)})")
        console.log("DISTINCT_COINCIDENT_ACTIONS",JSON.stringify(effects))
        compare(effects.outer,1);compare(effects.inner,1)
    }
    function test_names_are_unicode_bounded_and_exclude_editable_contents() {
        load("<button id=name>Label</button><input value='fixture-secret'><div role=button>Visible name<div contenteditable=true>editable-secret</div></div>","button,input,[role=button]{display:block;width:240px;height:50px;margin:15px}")
        evaluate("document.getElementById('name').setAttribute('aria-label','A'+'😀'.repeat(600));true")
        // Count Unicode scalars in Chromium, not the Qt JS string iterator.
        var names=evaluate("(function(){const s="+BrowserScripts.hintCollector('all')+";return s.candidates.map(c=>({text:c.text,kind:c.kind,characters:Array.from(c.text).length,bytes:new TextEncoder().encode(c.text).length}));})()")
        for(var candidate of names) {
            verify(candidate.characters<=512)
            verify(candidate.bytes<=2048)
            verify(candidate.text.indexOf('fixture-secret')<0)
            verify(candidate.text.indexOf('editable-secret')<0)
        }
        var name=names.filter(function(c){return c.kind==='button'&&c.text.indexOf('A😀')===0})[0]
        verify(!!name)
        compare(name.characters,512)
    }
    function test_rotated_frame_children_are_not_misplaced() {
        load("<iframe id=frame srcdoc=\"<button>Rotated child</button>\"></iframe>","iframe{position:absolute;left:100px;top:100px;width:250px;height:120px;border:0;transform:rotate(10deg)}")
        tryVerify(function(){return evaluate("!!document.getElementById('frame').contentDocument.querySelector('button')")},5000)
        var snapshot=evaluate(BrowserScripts.hintCollector('buttons'))
        compare(snapshot.candidates.length,0)
    }

}

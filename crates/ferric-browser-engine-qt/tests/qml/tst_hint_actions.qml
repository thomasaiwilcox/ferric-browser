import QtQuick
import QtTest
import QtWebEngine
import "../../qml/scripts/BrowserScripts.js" as BrowserScripts
import "../../qml/scripts/HintPresentation.js" as HintPresentation

TestCase {
    name: "HintActions"
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
        onLoadingChanged: function(request) { loaded = request.status === WebEngineView.LoadSucceededStatus }
    }
    function evaluate(source) {
        done = false
        page.runJavaScript(source, WebEngineScript.ApplicationWorld, function(value) { result = value; done = true })
        tryVerify(function() { return done }, 10000)
        return result
    }
    function load() {
        page.loaded = false
        page.url = Qt.resolvedUrl("../fixtures/webapp-interaction/native-hint-controls.html")
        tryVerify(function() { return page.loaded }, 10000)
        tryVerify(function() { return evaluate("document.body.dataset.ready") === "true" }, 5000)
    }
    function named(values, name) { return values.filter(function(c) { return c.text === name })[0] }
    function collect(family, snapshot, budget) {
        var value = evaluate(BrowserScripts.hintCollector(family, false, snapshot, null, false, budget))
        verify(!!value && !value.error, JSON.stringify(value))
        return value.candidates
    }
    function cleanup() { evaluate(BrowserScripts.hintStopTracking()) }

    function test_action_and_focus_views() {
        load()
        var actions = collect("actions")
        verify(!actions.some(function(c) { return c.kind === "focusable" }))
        verify(!!named(actions, "Expand or collapse thread"))
        verify(!!named(actions, "Enable feature"))
        verify(!!named(actions, "Text entry"))
        verify(!!named(collect("all"), "Keyboard region"))
        var focus = collect("focus")
        verify(focus.length > 0)
        verify(focus.every(function(c) { return c.kind === "focusable" }))
        var keyboard = named(focus, "Keyboard region")
        verify(!!keyboard)
        verify(evaluate(BrowserScripts.hintFocus(keyboard)))
        compare(evaluate("document.activeElement.id"), "focus-only")
        verify(!evaluate(BrowserScripts.hintClick(keyboard)))
    }
    function test_css_descendant_overrides_data() {
        var rows=[]
        for (var property of ["pointer-events", "visibility"])
            for (var fallback of [false,true])
                for (var location of ["document","shadow","slot","frame"])
                    rows.push({tag:property+"-"+location+(fallback?"-fallback":"-native"),property:property,fallback:fallback,location:location})
        return rows
    }
    function test_css_descendant_overrides(data) {
        load()
        evaluate("document.body.innerHTML='<div id=outer><button id=target>Quote</button><button id=inherited>Inherited control</button><button id=disabled disabled>Disabled control</button></div>';true")
        if (data.location === "shadow") {
            evaluate("document.getElementById('outer').attachShadow({mode:'open'}).innerHTML='<button id=target>Quote</button><button id=inherited>Inherited control</button><button id=disabled disabled>Disabled control</button>';window.scope=document.getElementById('outer').shadowRoot;true")
        } else if (data.location === "slot") {
            evaluate("document.getElementById('outer').attachShadow({mode:'open'}).innerHTML='<slot></slot>';window.scope=document;true")
        } else if (data.location === "frame") {
            evaluate("window.saved=document.getElementById('outer').outerHTML;document.body.innerHTML='<iframe id=frame style=\"width:700px;height:500px\"></iframe>';const doc=document.getElementById('frame').contentDocument;doc.open();doc.write(window.saved);doc.close();window.scope=doc;true")
        } else evaluate("window.scope=document;true")
        evaluate("window.outer=window.scope.querySelector('#outer')||document.getElementById('outer');window.target=window.scope.getElementById('target');window.outer.style.setProperty("+JSON.stringify(data.property)+","+JSON.stringify(data.property==='pointer-events'?'none':'hidden')+");window.target.style.setProperty("+JSON.stringify(data.property)+","+JSON.stringify(data.property==='pointer-events'?'auto':'visible')+");true")
        if (data.fallback) evaluate("window.target.ownerDocument.defaultView.Element.prototype.checkVisibility=undefined;true")
        var values=collect("actions"), candidate=named(values,"Quote")
        verify(!!candidate, "An explicitly exposed descendant must survive inherited CSS on its ancestor")
        verify(!named(values,"Inherited control"))
        verify(!named(values,"Disabled control"))
        verify(evaluate(BrowserScripts.hintFresh(candidate)).visible)
        verify(evaluate(BrowserScripts.hintClick(candidate)))
        evaluate("window.target.style.setProperty("+JSON.stringify(data.property)+","+JSON.stringify(data.property==='pointer-events'?'none':'hidden')+");true")
        verify(!evaluate(BrowserScripts.hintFresh(candidate)).visible)
    }
    function test_programmatic_button_receiver() {
        load()
        evaluate("document.body.innerHTML='<div type=button tabindex=-1 id=react aria-label=React>React</div><div tabindex=-1>Programmatic region</div><div type=button>Unannotated container</div>';document.getElementById('react').addEventListener('click',()=>document.body.dataset.reacted='true');true")
        var values=collect("actions"), candidate=named(values,"React")
        verify(!!candidate)
        compare(candidate.kind,"button")
        verify(!named(values,"Programmatic region"))
        verify(!named(values,"Unannotated container"))
        verify(evaluate(BrowserScripts.hintFresh(candidate)).visible)
        verify(evaluate(BrowserScripts.hintClick(candidate)))
        compare(evaluate("document.body.dataset.reacted"),"true")
        evaluate("document.getElementById('react').removeAttribute('type');true")
        verify(!evaluate(BrowserScripts.hintFresh(candidate)).visible)
    }
    function test_actions_filter_before_budget() {
        load()
        evaluate("document.body.innerHTML='<div tabindex=0>Wrapper one</div><div tabindex=0>Wrapper two</div><button>Actual action</button>';true")
        var actions = collect("actions", null, {candidates:1})
        compare(actions.length, 1)
        compare(actions[0].text, "Actual action")
    }
    function test_views_inherit_all_selector_policy() {
        load()
        var policy = {selector_revision:"view-policy",policies:{all:{policy:{mode:"replace",selectors:["#focus-only", "#button"]}}}}
        var actions = collect("actions", policy)
        compare(actions.length, 1)
        compare(actions[0].text, "Run action")
        var focus = collect("focus", policy)
        compare(focus.length, 1)
        compare(focus[0].text, "Keyboard region")
        verify(evaluate(BrowserScripts.hintFresh(focus[0], policy)).visible)
        verify(!evaluate(BrowserScripts.hintFresh(focus[0], {selector_revision:"changed",policies:policy.policies})).visible)
    }
    function test_rendering_data() {
        var rows = []
        for (var location of ["document", "shadow", "slot", "frame"])
            for (var fallback of [false, true]) rows.push({tag:location + (fallback ? "-fallback" : "-native"),location:location,fallback:fallback})
        return rows
    }
    function test_rendering(data) {
        load()
        evaluate("document.body.innerHTML='<div id=host><div id=inside tabindex=0 slot=body aria-label=\"Hidden keyboard region\">Content</div></div>';window.inside=document.getElementById('inside');window.host=document.getElementById('host');true")
        if (data.location === "slot") {
            evaluate("window.host.attachShadow({mode:'open'}).innerHTML='<details open><summary>Toggle disclosure</summary><slot name=body></slot></details>';window.disclosure=window.host.shadowRoot.querySelector('details');true")
        } else {
            evaluate("const d=document.createElement('details');d.open=true;d.innerHTML='<summary>Toggle disclosure</summary>';window.disclosure=d;d.append(window.inside);window.host.append(d);true")
            if (data.location === "shadow") evaluate("window.host.attachShadow({mode:'open'}).append(window.disclosure);true")
            if (data.location === "frame") {
                evaluate("window.frame=document.createElement('iframe');window.frame.style='width:700px;height:250px';document.body.prepend(window.frame);true")
                tryVerify(function() { return evaluate("!!window.frame.contentDocument.body") }, 5000)
                evaluate("window.frame.contentDocument.body.append(window.disclosure);true")
            }
        }
        if (data.fallback) evaluate("window.inside.checkVisibility=undefined;true")
        var visible = named(collect("all"), "Hidden keyboard region")
        verify(!!visible)
        evaluate("window.disclosure.open=false;true")
        verify(!evaluate(BrowserScripts.hintFresh(visible)).visible)
        verify(!evaluate(BrowserScripts.hintFocus(visible)))
        verify(!named(collect("all"), "Hidden keyboard region"))
        verify(!named(collect("focus"), "Hidden keyboard region"))
        verify(!!named(collect("actions"), "Toggle disclosure"))
        evaluate("window.disclosure.open=true;true")
        verify(!!named(collect("focus"), "Hidden keyboard region"))
    }
    function test_layoutless_ancestors_and_hidden_content_data() {
        return [{tag:"native",fallback:false},{tag:"fallback",fallback:true}]
    }
    function test_layoutless_ancestors_and_hidden_content(data) {
        load()
        evaluate("document.body.innerHTML='<div style=\"display:contents\"><div id=host><div id=target tabindex=0 aria-label=\"Slotted keyboard region\">Region</div></div></div>';document.getElementById('host').attachShadow({mode:'open'}).innerHTML='<slot></slot>';true")
        if (data.fallback) evaluate("document.getElementById('target').checkVisibility=undefined;true")
        var target = named(collect("focus"), "Slotted keyboard region")
        verify(!!target)
        evaluate("document.getElementById('host').style.contentVisibility='hidden';true")
        verify(!evaluate(BrowserScripts.hintFresh(target)).visible)
        verify(!named(collect("focus"), "Slotted keyboard region"))
        evaluate("document.getElementById('host').style.contentVisibility='visible';true")
        verify(!!named(collect("focus"), "Slotted keyboard region"))
    }
    function test_nested_distinct_actions() {
        load()
        evaluate("document.body.innerHTML='<details tabindex=0><summary>Thread <a href=\"https://example.test/author\">Author</a></summary><p>Replies</p></details><div role=button tabindex=0 onclick=\"this.dataset.outer=1\"><button onclick=\"event.stopPropagation();this.dataset.inner=1\">Inner action</button></div>';true")
        var actions = collect("actions")
        verify(actions.some(function(c) { return c.kind === "link" && c.text === "Author" }))
        verify(actions.some(function(c) { return c.kind === "aria" && c.text.indexOf("Thread") >= 0 }))
        var buttons = actions.filter(function(c) { return c.kind === "button" })
        compare(buttons.length, 2)
        for (var button of buttons) verify(evaluate(BrowserScripts.hintClick(button)))
        compare(evaluate("document.querySelector('[role=button]').dataset.outer"), "1")
        compare(evaluate("document.querySelector('button').dataset.inner"), "1")
    }
    function test_focus_target_mutation_data() {
        return [
            {tag:"replaced",mutation:"e.replaceWith(e.cloneNode(true))"},
            {tag:"inert",mutation:"e.inert=true"},
            {tag:"disabled",mutation:"e.setAttribute('aria-disabled','true')"},
            {tag:"action-role",mutation:"e.setAttribute('role','button')"},
            {tag:"removed-tabindex",mutation:"e.removeAttribute('tabindex')"}
        ]
    }
    function test_focus_target_mutation(data) {
        load()
        var focus = named(collect("focus"), "Keyboard region")
        verify(!!focus)
        evaluate("{const e=document.getElementById('focus-only');" + data.mutation + ";}true")
        verify(!evaluate(BrowserScripts.hintFresh(focus)).visible)
        verify(!evaluate(BrowserScripts.hintFocus(focus)))
    }
    function test_actions_keep_validated_editor_proxy() {
        load()
        evaluate("document.body.innerHTML='<div class=ace_editor role=textbox tabindex=0 aria-label=Editor style=\"width:300px;height:100px\"><textarea class=ace_text-input style=\"opacity:0;width:1px;height:1px\"></textarea></div>';true")
        var values = evaluate(BrowserScripts.hintCollector("actions", "validated")).candidates
        var editor = named(values, "Editor")
        verify(!!editor)
        compare(editor.kind, "editor-focus")
        verify(evaluate(BrowserScripts.hintFocus(editor)))
        compare(evaluate("document.activeElement.localName"), "textarea")
    }
    function test_action_descriptions() {
        compare(HintPresentation.description({label:"a",kind:"focusable",text:"Keyboard region",action:"Focus"}), "a · Focus · Keyboard region")
        compare(HintPresentation.description({label:"s",kind:"input",text:"Search",action:"Edit"}), "s · Edit · Search")
    }
}

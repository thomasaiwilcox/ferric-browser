import QtQuick
import QtTest
import QtWebEngine
import "../../qml/scripts/BrowserScripts.js" as BrowserScripts

TestCase {
    name: "HintControls"
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
    function main(source) {
        done = false
        page.runJavaScript(source, WebEngineScript.MainWorld, function(value) { result = value; done = true })
        tryVerify(function() { return done }, 10000)
        return result
    }
    function load() {
        page.loaded = false
        page.url = Qt.resolvedUrl("../fixtures/webapp-interaction/native-hint-controls.html")
        tryVerify(function() { return page.loaded }, 10000)
        tryVerify(function() { return evaluate("document.body.dataset.ready") === "true" }, 5000)
    }
    function collect(family, snapshot, budget) {
        return evaluate(BrowserScripts.hintCollector(family || "all", false, snapshot, null, false, budget)).candidates
    }
    function named(values, name) {
        return values.filter(function(c) { return c.text === name })[0]
    }
    function policy(selectors, revision) {
        return {selector_revision: revision || "controls-1", policies:{all:{policy:{mode:"replace", selectors:selectors}}}}
    }
    function cleanup() { evaluate(BrowserScripts.hintStopTracking()) }

    function test_disclosure_data() {
        var rows = []
        for (var location of ["document", "shadow", "frame"])
            for (var open of [false, true]) rows.push({tag:location + "-" + open, location:location, open:open})
        return rows
    }
    function test_disclosure(data) {
        load()
        evaluate("window.threadTarget=document.getElementById('thread');window.threadTarget.open=" + data.open + ";true")
        if (data.location === "shadow")
            evaluate("const h=document.createElement('div');document.body.prepend(h);h.attachShadow({mode:'open'}).append(window.threadTarget);true")
        if (data.location === "frame") {
            evaluate("const f=document.createElement('iframe');f.style='width:700px;height:350px';document.body.prepend(f);window.testFrame=f;true")
            tryVerify(function() { return evaluate("!!window.testFrame.contentDocument.body") }, 5000)
            evaluate("window.testFrame.contentDocument.body.append(window.threadTarget);true")
        }
        for (var cycle = 0; cycle < 3; cycle++) {
            var values = collect("all")
            verify(!named(values, "Thread container"), "A native summary replaces the redundant focus-only details hint")
            var summary = named(values, "Expand or collapse thread")
            verify(!!summary)
            compare(summary.kind, "aria")
            verify(evaluate(BrowserScripts.hintFresh(summary)).visible)
            verify(evaluate(BrowserScripts.hintClick(summary)))
            compare(evaluate("window.threadTarget.open"), cycle % 2 === 0 ? !data.open : data.open)
            verify(!evaluate("Array.from(window.__ferric_browserHintState.elements.values()).some(r=>r.element===window.threadTarget)"), "Suppressed records are retired")
        }
        if (data.location === "document" && !data.open) {
            var nested = named(collect(), "Expand nested replies")
            verify(!!nested)
            verify(evaluate(BrowserScripts.hintClick(nested)))
            verify(evaluate("document.getElementById('nested').open"))
        }
    }
    function test_disclosure_fallback_data() {
        return [
            {tag:"missing", mutation:"document.getElementById('thread-toggle').remove()"},
            {tag:"hidden", mutation:"document.getElementById('thread-toggle').style.display='none';document.getElementById('thread').style.minHeight='40px'"},
            {tag:"covered", mutation:"const s=document.getElementById('thread-toggle'),r=s.getBoundingClientRect(),cover=document.createElement('div');cover.style='position:fixed;z-index:999;background:white;left:'+r.x+'px;top:'+r.y+'px;width:'+r.width+'px;height:'+r.height+'px';document.body.append(cover)"},
            {tag:"excluded", selectors:["details"]},
            {tag:"budget", budget:{candidates:1}},
            {tag:"second-summary", mutation:"document.getElementById('thread').open=true;document.getElementById('thread-toggle').style.display='none';const s=document.createElement('summary');s.textContent='Second summary';document.getElementById('thread').append(s)"}
        ]
    }
    function test_disclosure_fallback(data) {
        load()
        if (data.mutation) evaluate(data.mutation + ";true")
        var values = collect("all", data.selectors ? policy(data.selectors) : undefined, data.budget)
        verify(!!named(values, "Thread container"), "Keep focus when its first native summary was not admitted")
    }
    function test_input_families_data() {
        return [{tag:"all",family:"all"},{tag:"inputs",family:"inputs"},{tag:"buttons",family:"buttons"}]
    }
    function test_input_families(data) {
        load()
        var values = collect(data.family)
        for (var caption of ["Run action", "Send form", "Reset form", "Image submit"]) {
            var c = named(values, caption)
            verify(!!c, caption + " needs a hint in " + data.family)
            compare(c.kind, "button")
        }
        for (var name of ["Enable feature", "First option", "Second option"]) {
            var c = named(values, name)
            verify(!!c)
            compare(c.kind, "aria")
        }
        if (data.family === "buttons") {
            verify(!!named(values, "Expand or collapse thread"))
            verify(!named(values, "Text entry"))
        } else compare(named(values, "Text entry").kind, "input")
        for (var c of values) verify(c.text.indexOf("private-") < 0, "Control values must stay private")
    }
    function test_native_effects_data() {
        return [{tag:"checkbox",name:"Enable feature",id:"checkbox"},
                {tag:"radio",name:"Second option",id:"radio-b"},
                {tag:"button",name:"Run action",id:"button"},
                {tag:"submit",name:"Send form",id:"submit"},
                {tag:"reset",name:"Reset form",id:"reset"},
                {tag:"image",name:"Image submit",id:"image"}]
    }
    function test_native_effects(data) {
        load()
        evaluate("document.getElementById('text').value='modified';true")
        var c = named(collect(), data.name)
        verify(!!c)
        verify(evaluate(BrowserScripts.hintClick(c)))
        compare(main("window.controlEvents.clicks[" + JSON.stringify(data.id) + "]"), 1)
        if (data.id === "checkbox") {
            verify(evaluate("document.getElementById('checkbox').checked"))
            compare(main("window.controlEvents.changes.checkbox"), 1)
            verify(evaluate(BrowserScripts.hintClick(c)))
            verify(!evaluate("document.getElementById('checkbox').checked"))
            compare(main("window.controlEvents.clicks.checkbox"), 2)
        } else if (data.id === "radio-b") {
            verify(evaluate("document.getElementById('radio-b').checked && !document.getElementById('radio-a').checked"))
            verify(evaluate(BrowserScripts.hintClick(c)))
            compare(main("window.controlEvents.clicks['radio-b']"), 2)
            compare(main("window.controlEvents.changes['radio-b']"), 1)
        } else if (data.id === "reset") {
            compare(main("window.controlEvents.resets"), 1)
            compare(evaluate("document.getElementById('text').value"), "private-text-value")
        } else if (data.id === "submit" || data.id === "image") {
            compare(main("window.controlEvents.submits"), 1)
            compare(main("window.controlEvents.submitter"), data.id)
            evaluate("document.getElementById('text').value='';true")
            verify(evaluate(BrowserScripts.hintClick(c)))
            compare(main("window.controlEvents.submits"), 1, "Native constraint validation must still apply")
        }
    }
    function test_cancelled_click_and_focus() {
        load()
        evaluate("document.getElementById('checkbox').dataset.cancel='true';true")
        var values = collect()
        verify(evaluate(BrowserScripts.hintClick(named(values, "Enable feature"))))
        verify(!evaluate("document.getElementById('checkbox').checked"))
        compare(main("window.controlEvents.clicks.checkbox"), 1)
        compare(main("window.controlEvents.changes.checkbox || 0"), 0)
        for (var name of ["Text entry", "Text area", "Keyboard region"]) {
            verify(evaluate(BrowserScripts.hintFocus(named(values, name))))
        }
        compare(evaluate("document.activeElement.id"), "focus-only")
    }
    function test_stale_input_type_data() {
        return [{tag:"checkbox-radio",name:"Enable feature",id:"checkbox",type:"radio"},
                {tag:"submit-reset",name:"Send form",id:"submit",type:"reset"},
                {tag:"button-file",name:"Run action",id:"button",type:"file"}]
    }
    function test_stale_input_type(data) {
        load()
        var c = named(collect(), data.name)
        verify(!!c)
        evaluate("document.getElementById(" + JSON.stringify(data.id) + ").type=" + JSON.stringify(data.type) + ";true")
        verify(!evaluate(BrowserScripts.hintFresh(c)).visible)
        verify(!evaluate(BrowserScripts.hintClick(c)))
        verify(!evaluate(BrowserScripts.hintPointerPreflight(c, width, height)).ok)
        compare(main("window.controlEvents.clicks[" + JSON.stringify(data.id) + "] || 0"), 0)
    }
    function test_disabled_stale_and_selectors() {
        load()
        var c = named(collect(), "Enable feature")
        evaluate("document.getElementById('checkbox').disabled=true;document.getElementById('button').inert=true;true")
        verify(!evaluate(BrowserScripts.hintClick(c)))
        var values = collect()
        verify(!named(values, "Enable feature")); verify(!named(values, "Run action"))
        c = named(values, "Second option")
        evaluate("document.getElementById('radio-b').replaceWith(document.getElementById('radio-b').cloneNode(true));true")
        verify(!evaluate(BrowserScripts.hintClick(c)))
        var p = policy(["#submit"])
        values = collect("all", p)
        compare(values.length, 1)
        c = values[0]
        verify(!evaluate(BrowserScripts.hintFresh(c, policy(["#submit"], "controls-2"))).visible)
        evaluate("document.body.innerHTML='<button>Replaced document</button>';true")
        verify(!evaluate(BrowserScripts.hintClick(c, p)))
    }
    function test_disclosure_refresh_and_scope() {
        load()
        var p = policy(["details"])
        var old = named(collect("all", p), "Thread container")
        verify(!!old)
        var values = collect()
        var summary = named(values, "Expand or collapse thread")
        verify(!!summary)
        verify(!evaluate(BrowserScripts.hintFocus(old)), "The removed focus record cannot be reused")
        var id = summary.element_id
        evaluate("document.getElementById('thread').dataset.refresh='1';true")
        compare(named(collect(), "Expand or collapse thread").element_id, id)

        var regions = {selector_revision:"controls-regions",policies:{}}
        var scope = named(evaluate(BrowserScripts.hintCollector("regions", false, regions)).candidates, "Control section")
        verify(!!scope)
        var request = {lease_id:"controls-scope",element_id:scope.element_id,frame_path:scope.frame_path,
                       kind:scope.kind,selector_revision:regions.selector_revision,configuration_revision:0,parent_scope_lease_id:null}
        verify(evaluate(BrowserScripts.hintAcquireScopeLease(request, scope, regions)).ok)
        values = evaluate(BrowserScripts.hintCollector("all", false, undefined, request.lease_id)).candidates
        verify(!named(values, "Thread container"))
        summary = named(values, "Expand or collapse thread")
        verify(!!summary)
        verify(evaluate(BrowserScripts.hintClick(summary)))
        verify(evaluate("document.getElementById('thread').open"))
    }
    function test_input_names_and_proxy_identity() {
        load()
        evaluate("document.getElementById('button').type='BuTtOn';document.getElementById('button').setAttribute('aria-label','Explicit action');true")
        var values = collect("buttons")
        var explicit = named(values, "Explicit action")
        verify(!!explicit)
        compare(explicit.kind, "button")
        verify(!named(values, "Run action"))
        evaluate("document.getElementById('button').removeAttribute('aria-label');document.getElementById('button').value='A'+'😀'.repeat(600);true")
        var sizes = evaluate("(" + BrowserScripts.hintCollector("all") + ").candidates.map(c=>({length:Array.from(c.text).length,bytes:new TextEncoder().encode(c.text).length,text:c.text}))")
        for (var c of sizes) {
            verify(c.length <= 512); verify(c.bytes <= 2048)
            verify(c.text.indexOf("private-") < 0)
        }
        // The invisible native receiver still uses one validated label pair.
        evaluate("document.getElementById('checkbox').style='position:absolute;width:0;height:0;opacity:0';true")
        values = evaluate(BrowserScripts.hintCollector("all", true)).candidates
        var proxy = named(values, "Enable feature")
        verify(!!proxy)
        compare(proxy.kind, "label-control")
        compare(values.filter(function(c) { return c.text === "Enable feature" }).length, 1)
        evaluate("document.getElementById('checkbox').type='radio';true")
        verify(!evaluate(BrowserScripts.hintClick(proxy)), "A label cannot silently acquire a different native input type")
    }
    function test_input_shadow_and_navigation() {
        load()
        evaluate("const h=document.createElement('div');document.body.prepend(h);h.attachShadow({mode:'open'}).append(document.getElementById('checkbox').parentElement);true")
        var c = named(collect(), "Enable feature")
        verify(!!c)
        compare(c.kind, "aria")
        verify(evaluate(BrowserScripts.hintClick(c)))
        verify(evaluate("document.querySelector('div').shadowRoot.querySelector('input').checked"))
        load()
        verify(!evaluate(BrowserScripts.hintClick(c)), "Navigation retires the old document's receiver")
    }
}

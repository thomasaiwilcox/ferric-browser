import QtQuick
import QtTest
import QtWebEngine
import "../../qml/scripts/BrowserScripts.js" as BrowserScripts

TestCase {
    id: tests
    name: "WebAppInteraction"
    when: windowShown
    width: 640
    height: 480

    WebEngineView {
        id: page
        anchors.fill: parent
        property bool loaded: false
        property int mainFrameNavigationRequests: 0
        onLoadingChanged: function(request) {
            loaded = request.status === WebEngineView.LoadSucceededStatus
        }
        onNavigationRequested: function(request) {
            if (request.isMainFrame) mainFrameNavigationRequests += 1
        }
    }
    property bool done: false
    property var result

    function evaluate(source) {
        done = false
        page.runJavaScript(source, WebEngineScript.ApplicationWorld, function(value) {
            result = value
            done = true
        })
        tryVerify(function() { return done }, 5000)
        return result
    }
    function evaluateMain(source) {
        done = false
        page.runJavaScript(source, WebEngineScript.MainWorld, function(value) {
            result = value
            done = true
        })
        tryVerify(function() { return done }, 5000)
        return result
    }
    function load(name) {
        page.loaded = false
        page.url = Qt.resolvedUrl("../fixtures/webapp-interaction/" + name)
        tryVerify(function() { return page.loaded }, 5000)
        tryVerify(function() { return evaluate("document.body.dataset.ready") === "true" }, 5000)
    }
    function candidate(family, text) {
        var found
        // Layout/hit-test readiness, rather than a sleep, gates collection.
        tryVerify(function() {
            found = evaluate(BrowserScripts.hintCollector(family)).candidates.filter(
                        function(item) { return item.text === text })[0]
            return !!found
        }, 5000)
        return found
    }
    function selectorSnapshot(family, mode, selectors, revision, extraPolicies) {
        var policies = ({})
        policies[family] = {
            mode: mode,
            selectors: selectors,
            provenance: ["site-rule:fixture"]
        }
        if (extraPolicies) {
            for (var key in extraPolicies) policies[key] = extraPolicies[key]
        }
        return {
            family: family,
            configuration_revision: 0,
            selector_revision: revision,
            policies: policies
        }
    }
    function cleanup() {
        evaluate(BrowserScripts.hintStopTracking())
    }

    // T019: extracted resources execute through the browser-owned ApplicationWorld.
    function test_T019_extracted_script_world_isolation() {
        load("shadow-targets.html")
        candidate("buttons", "Shadow button")
        verify(evaluate("!!window.__ferric_browserHintState"))
        compare(evaluateMain("typeof window.__ferric_browserHintState"), "undefined")
        compare(evaluateMain("typeof window.__ferric_browserHintElements"), "undefined")
    }

    function test_T026_document_identity_survives_spa_urls_and_changes_on_replacement() {
        page.userScripts.clear()
        var identityScript = WebEngine.script()
        identityScript.name = "ferric-test-document-identity"
        identityScript.sourceCode = BrowserScripts.focusObserverSource()
        identityScript.injectionPoint = WebEngineScript.DocumentCreation
        identityScript.worldId = WebEngineScript.ApplicationWorld
        page.userScripts.insert(identityScript)

        page.loaded = false
        page.loadHtml("<!doctype html><html><body>Identity fixture</body></html>",
                      "https://identity.test/one")
        tryVerify(function() { return page.loaded }, 5000)
        var initial = evaluate(BrowserScripts.historyTraversalProbe())
        verify(initial && initial.document.length > 0)
        compare(evaluateMain("typeof window.__ferric_browserHistoryTraversal"), "undefined")

        var requestsBeforeSpa = page.mainFrameNavigationRequests
        evaluateMain("history.pushState({}, '', '/one#push'); true")
        tryVerify(function() { return page.url.toString() === "https://identity.test/one#push" }, 5000)
        var afterPush = evaluate(BrowserScripts.historyTraversalProbe())
        compare(afterPush.document, initial.document)
        compare(page.mainFrameNavigationRequests, requestsBeforeSpa)

        evaluateMain("location.hash = 'fragment'; true")
        tryVerify(function() { return page.url.toString() === "https://identity.test/one#fragment" }, 5000)
        var afterFragment = evaluate(BrowserScripts.historyTraversalProbe())
        compare(afterFragment.document, initial.document)
        compare(page.mainFrameNavigationRequests, requestsBeforeSpa)

        page.loaded = false
        page.loadHtml("<!doctype html><html><body>Replacement</body></html>",
                      "https://identity.test/two")
        tryVerify(function() { return page.loaded }, 5000)
        var replacement = evaluate(BrowserScripts.historyTraversalProbe())
        verify(replacement.document !== initial.document)
    }

    // T002: production collection -> fresh -> final click, exact counter.
    function test_T002_shadow_click() {
        load("shadow-targets.html")
        var selected = candidate("buttons", "Shadow button")
        verify(evaluate(BrowserScripts.hintFresh(selected)).visible)
        verify(evaluate(BrowserScripts.hintClick(selected)), "Final shadow click rejected")
        compare(evaluate("document.getElementById('host').shadowRoot.getElementById('shadow-button').dataset.clicks"), "1")
    }
    function test_T002_shadow_focus() {
        load("shadow-targets.html")
        var selected = candidate("inputs", "Shadow input")
        verify(evaluate(BrowserScripts.hintFresh(selected)).visible)
        verify(evaluate(BrowserScripts.hintFocus(selected)), "Final shadow focus rejected")
        verify(evaluate("document.activeElement===document.getElementById('host') && document.activeElement.shadowRoot.activeElement.id==='shadow-input'"))
    }
    // T003: assigned-slot ancestry and ancestor inertness.
    function test_T003_slot_and_inert() {
        load("shadow-targets.html")
        var selected = candidate("buttons", "Slotted control")
        verify(evaluate(BrowserScripts.hintFresh(selected)).visible)
        evaluate("document.getElementById('slot-host').shadowRoot.getElementById('nested').inert=true;true")
        verify(!evaluate(BrowserScripts.hintFresh(selected)).visible, "Assigned slot must inherit inert ancestor")
        verify(!evaluate(BrowserScripts.hintClick(selected)))
        compare(evaluate("document.getElementById('slotted').dataset.clicks||'0'"), "0")
    }
    function test_T003_slot_clipping() {
        load("shadow-targets.html")
        evaluate("document.getElementById('slot-host').shadowRoot.getElementById('nested').style='overflow:hidden;width:90px;height:16px';true")
        var selected = candidate("buttons", "Slotted control")
        compare(selected.geometry.width, 90)
        compare(selected.geometry.height, 16)
        compare(evaluate(BrowserScripts.hintFresh(selected)).geometry.height, 16)
    }
    function test_T003_excessive_ancestry() {
        load("shadow-targets.html")
        evaluate("{let parent=document.body;for(let i=0;i<300;i++){const node=document.createElement('div');parent.appendChild(node);parent=node;}const button=document.createElement('button');button.textContent='Deep ancestry';parent.appendChild(button);}true")
        var values = evaluate(BrowserScripts.hintCollector("buttons")).candidates
        verify(!values.some(function(item) { return item.text === "Deep ancestry" }), "Excessive composed ancestry is bounded")
    }
    function test_T004_frame_border_projection() {
        load("frame-targets.html")
        var selected = candidate("buttons", "Frame button")
        compare(selected.frame_path, "0.0")
        compare(selected.geometry.x, 57, "Owner CSS x includes the frame border")
        compare(selected.geometry.y, 72, "Owner CSS y includes the frame border")
        verify(evaluate(BrowserScripts.hintFresh(selected)).visible)
        verify(evaluate(BrowserScripts.hintClick(selected)))
        compare(evaluate("document.getElementById('child').contentDocument.getElementById('frame-button').dataset.clicks"), "1")
    }
    function test_T004_frame_replaced() {
        load("frame-targets.html")
        var selected = candidate("buttons", "Frame button")
        evaluate("document.getElementById('child').srcdoc='<body data-replaced=true><button id=frame-button>Frame button</button>';true")
        tryVerify(function() { return evaluate("document.getElementById('child').contentDocument.body.dataset.replaced==='true'") }, 5000)
        verify(!evaluate(BrowserScripts.hintFresh(selected)).visible)
        verify(!evaluate(BrowserScripts.hintClick(selected)))
    }
    function test_T004_adopted_element() {
        load("frame-targets.html")
        var selected = candidate("buttons", "Frame button")
        evaluate("document.body.appendChild(document.adoptNode(document.getElementById('child').contentDocument.getElementById('frame-button')));true")
        verify(!evaluate(BrowserScripts.hintFresh(selected)).visible, "A retained object adopted into a different document is stale")
        verify(!evaluate(BrowserScripts.hintClick(selected)))
    }
    function test_T005_ineligible_data() {
        return [
            {tag:"hidden", mutation:"target.style.display='none'"},
            {tag:"opacity", mutation:"target.style.opacity='0'"},
            {tag:"disabled", mutation:"target.disabled=true"},
            {tag:"inert", mutation:"target.inert=true"},
            {tag:"detached", mutation:"target.remove()"},
            {tag:"covered", mutation:"document.getElementById('cover').style.display='block'"},
            {tag:"replacement", mutation:"target.replaceWith(target.cloneNode(true))"}
        ]
    }
    function test_T005_ineligible(data) {
        load("adversarial-targets.html")
        var selected = candidate("buttons", "Target button")
        evaluate("{const target=document.getElementById('target');" + data.mutation + ";}true")
        verify(!evaluate(BrowserScripts.hintFresh(selected)).visible)
        verify(!evaluate(BrowserScripts.hintClick(selected)))
        compare(evaluate("document.getElementById('target')?.dataset.clicks||'0'"), "0")
    }
    function test_T005_disabled_fieldset() {
        load("adversarial-targets.html")
        var values = evaluate(BrowserScripts.hintCollector("buttons")).candidates
        verify(!values.some(function(item) { return item.text === "Disabled fieldset" }), "Disabled fieldset descendants are ineligible")
    }
    function test_T005_transparent_ancestor() {
        load("adversarial-targets.html")
        var values = evaluate(BrowserScripts.hintCollector("buttons")).candidates
        verify(!values.some(function(item) { return item.text === "Invisible ancestor" }), "Ancestor opacity excludes ordinary targets")
    }
    function test_T006_viewport_overflow() {
        load("viewport-overflow.html")
        var selected = candidate("links", "Video")
        compare(evaluate("document.body.getBoundingClientRect().height"), 0)
        verify(evaluate(BrowserScripts.hintFresh(selected)).visible)
        var values = evaluate(BrowserScripts.hintCollector("buttons")).candidates
        var partial = values.filter(function(item) { return item.text === "Partial" })[0]
        verify(partial)
        compare(partial.geometry.height, 10)
    }
    function test_T008_observed_focus() {
        load("adversarial-targets.html")
        evaluate("document.getElementById('redirect').focus=function(){document.getElementById('other').focus()};true")
        var selected = candidate("inputs", "Redirecting input")
        verify(evaluate(BrowserScripts.hintFresh(selected)).visible)
        var focusResult = evaluate(BrowserScripts.hintFocus(selected))
        compare(evaluate("document.activeElement.id"), "other")
        verify(!focusResult, "Redirected focus must not report success")
    }
    function test_T008_normal_focus() {
        load("adversarial-targets.html")
        var selected = candidate("inputs", "Focus target")
        verify(evaluate(BrowserScripts.hintFocus(selected)))
        compare(evaluate("document.activeElement.id"), "focus")
    }
    function test_T008_focus_uses_focus_eligibility() {
        load("adversarial-targets.html")
        var selected = candidate("inputs", "Pointerless input")
        verify(evaluate(BrowserScripts.hintFresh(selected)).visible)
        verify(evaluate(BrowserScripts.hintFocus(selected)))
        compare(evaluate("document.activeElement.id"), "pointerless")
    }
    function test_T007_same_url_cover() {
        load("adversarial-targets.html")
        evaluate("{const target=document.getElementById('target');const link=document.createElement('a');link.id='exact-link';link.href='https://fixture.test/same';link.textContent='Exact link';link.style='display:block;width:180px;height:32px;margin:8px';target.replaceWith(link);}true")
        var selected = candidate("links", "Exact link")
        evaluate("{const original=document.getElementById('exact-link'),rect=original.getBoundingClientRect(),cover=original.cloneNode(true);cover.id='same-url-cover';cover.style='position:fixed;left:'+rect.left+'px;top:'+rect.top+'px;width:'+rect.width+'px;height:'+rect.height+'px;z-index:100;background:white';document.body.appendChild(cover);}true")
        verify(!evaluate(BrowserScripts.hintFresh(selected)).visible, "Equivalent URL cannot authorise a covering sibling")
        verify(!evaluate(BrowserScripts.hintClick(selected)))
    }
    function test_T009_control_value_privacy() {
        load("adversarial-targets.html")
        evaluate("{const password=document.createElement('input');password.id='secret';password.type='password';password.value='FIXTURE_PRIVATE_VALUE';const text=document.createElement('input');text.id='field-buffer';text.type='text';text.value='FIXTURE_FIELD_BUFFER';document.body.prepend(password,text);}true")
        var values = evaluate(BrowserScripts.hintCollector("inputs")).candidates
        verify(!values.some(function(item) { return item.text.indexOf("FIXTURE_PRIVATE_VALUE") >= 0 }), "Control values must not become labels")
        verify(!values.some(function(item) { return item.text.indexOf("FIXTURE_FIELD_BUFFER") >= 0 }), "Editable field buffers must not become labels")
    }
    function test_T009_label_is_literal() {
        load("adversarial-targets.html")
        evaluate("document.getElementById('focus').setAttribute('aria-label','<img src=x onerror=alert(1)>');true")
        var selected = candidate("inputs", "<img src=x onerror=alert(1)>")
        compare(selected.text, "<img src=x onerror=alert(1)>")
    }
    // T015: structural ACE-like pairing is opt-in and focuses only the retained receiver.
    function test_T015_editor_focus_pair() {
        load("proxy-targets.html")
        var disabled = evaluate(BrowserScripts.hintCollector("inputs", false)).candidates
        verify(!disabled.some(function(item) { return item.kind === "editor-focus" }))
        var values = evaluate(BrowserScripts.hintCollector("inputs", true)).candidates
        var selected = values.filter(function(item) { return item.kind === "editor-focus" && item.text === "Source editor" })[0]
        verify(selected, "Validated setting admits the visible structural editor pair")
        verify(selected.related_element_id > 0)
        verify(!JSON.stringify(values).includes("FIXTURE_PRIVATE_EDITOR_BUFFER"))
        verify(!JSON.stringify(values).includes("FIXTURE_PRIVATE_PASSWORD"))
        verify(evaluate(BrowserScripts.hintFresh(selected)).visible)
        verify(evaluate(BrowserScripts.hintFocus(selected)), "Exact retained editor receiver receives focus")
        verify(evaluate("document.activeElement.id==='editor-receiver'"))
        var allValues = evaluate(BrowserScripts.hintCollector("all", true)).candidates
        verify(allValues.some(function(item) { return item.kind === "editor-focus" && item.text === "Source editor" }), "The all family includes validated editor pairs")
    }
    // T016: native label activation remains a single label click with checked state.
    function test_T016_label_control_pair() {
        load("proxy-targets.html")
        var disabled = evaluate(BrowserScripts.hintCollector("inputs", false)).candidates
        verify(!disabled.some(function(item) { return item.kind === "label-control" }))
        var values = evaluate(BrowserScripts.hintCollector("inputs", true)).candidates
        var selected = values.filter(function(item) { return item.kind === "label-control" && item.text === "Enable feature" })[0]
        verify(selected, "Exact native label.control pair is collected")
        verify(evaluate(BrowserScripts.hintFresh(selected)).visible)
        verify(evaluate(BrowserScripts.hintClick(selected)), "Native label action succeeds")
        verify(evaluate("document.getElementById('check-control').checked"))
        compare(evaluate("document.documentElement.dataset.labelClicks"), "1")
        compare(evaluate("document.documentElement.dataset.controlClicks"), "1")
        compare(evaluate("document.documentElement.dataset.controlChanges"), "1")
        var radio = values.filter(function(item) { return item.kind === "label-control" && item.text === "Choose option" })[0]
        verify(radio, "Radio labels use the same exact native association contract")
        verify(evaluate(BrowserScripts.hintClick(radio)))
        verify(evaluate("document.getElementById('radio-control').checked"))
        compare(evaluate("document.documentElement.dataset.radioClicks"), "1")
        compare(evaluate("document.documentElement.dataset.radioChanges"), "1")
        var visibleControl = values.filter(function(item) { return item.text === "Visible option" })
        compare(visibleControl.length, 1)
        compare(visibleControl[0].kind, "input")
        verify(!values.some(function(item) { return item.kind === "label-control" && item.text === "Visible option" }), "Normally visible checkbox is deduplicated")
        var allValues = evaluate(BrowserScripts.hintCollector("all", true)).candidates
        verify(allValues.some(function(item) { return item.kind === "label-control" && item.text === "Enable feature" }), "The all family includes validated label pairs")
    }
    // T017: structural, receiver, label, and retained-object exclusions fail closed.
    function test_T017_proxy_exclusions_and_identity() {
        load("proxy-targets.html")
        var values = evaluate(BrowserScripts.hintCollector("inputs", true)).candidates
        for (var excluded of ["Ambiguous editor", "Class only editor", "Password editor", "File editor", "Read only editor", "Hidden editor", "Inert editor", "Disabled option", "Inert option"]) {
            verify(!values.some(function(item) { return item.text === excluded || item.text.indexOf(excluded) === 0 }), excluded + " stays excluded")
        }
        var selected = values.filter(function(item) { return item.kind === "editor-focus" && item.text === "Source editor" })[0]
        evaluate("document.getElementById('editor-receiver').replaceWith(document.getElementById('editor-receiver').cloneNode(true));true")
        verify(!evaluate(BrowserScripts.hintFresh(selected)).visible, "Replacement receiver cannot inherit retained identity")
        verify(!evaluate(BrowserScripts.hintFocus(selected)))
        verify(evaluate("document.activeElement.id!=='editor-receiver'"))

        var label = values.filter(function(item) { return item.kind === "label-control" && item.text === "Enable feature" })[0]
        evaluate("document.getElementById('check-label').htmlFor='visible-control';true")
        verify(!evaluate(BrowserScripts.hintFresh(label)).visible, "Retargeted label.control invalidates the original pair")
        verify(!evaluate(BrowserScripts.hintClick(label)))
        verify(!evaluate("document.getElementById('check-control').checked"))
    }
    function test_T020_map_teardown() {
        load("adversarial-targets.html")
        var selected = candidate("buttons", "Target button")
        verify(evaluate(BrowserScripts.hintClick(selected)))
        compare(evaluate("document.getElementById('target').dataset.clicks"), "1")
        verify(evaluate(BrowserScripts.hintStopTracking()))
        verify(!evaluate(BrowserScripts.hintClick(selected)))
        compare(evaluate("document.getElementById('target').dataset.clicks"), "1")
        verify(evaluate("!window.__ferric_browserHintElements && window.__ferric_browserHintState.elements.size===0 && window.__ferric_browserHintState.observers.length===0 && window.__ferric_browserHintState.windows.length===0 && !window.__ferric_browserHintState.tracking"))
    }
    function test_T020_proxy_pair_teardown() {
        load("proxy-targets.html")
        var values = evaluate(BrowserScripts.hintCollector("inputs", true)).candidates
        var selected = values.filter(function(item) { return item.kind === "editor-focus" && item.text === "Source editor" })[0]
        verify(selected)
        verify(evaluate(BrowserScripts.hintFresh(selected)).visible)
        verify(evaluate(BrowserScripts.hintStopTracking()))
        verify(!evaluate(BrowserScripts.hintFresh(selected)).visible, "Teardown drops the representative and related receiver records")
        verify(evaluate("window.__ferric_browserHintState.elements.size===0 && window.__ferric_browserHintState.observers.length===0 && window.__ferric_browserHintState.windows.length===0 && !window.__ferric_browserHintState.tracking"))
    }
    // T031/T032: replacement selects discovery; captured policy survives fresh/final checks.
    function test_T031_T032_custom_selector_and_action_boundary() {
        load("adversarial-targets.html")
        evaluate("document.body.innerHTML='<button id=custom class=ferric-custom>Custom action</button><div id=lookalike class=ferric-custom href=\"https://invalid.test/\">Lookalike</div>';document.getElementById('custom').onclick=function(){this.dataset.clicks=String(Number(this.dataset.clicks||0)+1)};true")
        var snapshot = selectorSnapshot("all", "replace", [".ferric-custom"], "s12-custom-all", {
            scrollables: {mode:"replace", selectors:[], provenance:["site-rule:fixture"]}
        })
        var value = evaluate(BrowserScripts.hintCollector("all", false, snapshot))
        compare(value.selector_revision, "s12-custom-all")
        compare(value.truncated, false)
        var custom = value.candidates.filter(function(item) { return item.text === "Custom action" })[0]
        var lookalike = value.candidates.filter(function(item) { return item.text === "Lookalike" })[0]
        verify(custom, "Custom selector discovers the semantic button")
        verify(lookalike, "A matching generic element remains discovery-only")
        compare(lookalike.kind, "aria")
        verify(lookalike.href === null || typeof lookalike.href === "undefined",
               "A generic custom match carries no URL")
        verify(evaluate(BrowserScripts.hintFresh(custom, snapshot)).visible)
        verify(!evaluate(BrowserScripts.hintFresh(custom,
                            selectorSnapshot("all", "replace", [".ferric-custom"], "stale-revision"))).visible,
               "A changed site-policy revision invalidates retained identity")
        verify(!evaluate(BrowserScripts.hintClick(lookalike, snapshot)),
               "A generic custom match cannot gain click authority")
        verify(!evaluate(BrowserScripts.hintFocus(lookalike, snapshot)),
               "A generic custom match cannot gain focus authority")
        var pointerPreflight = evaluate(
                    BrowserScripts.hintPointerPreflight(lookalike, 640, 480, snapshot))
        verify(!pointerPreflight.ok,
               "A generic custom match cannot gain native pointer authority")
        verify(evaluate(BrowserScripts.hintClick(custom, snapshot)),
               "A real button retains its semantic final-action check")
        compare(evaluate("document.getElementById('custom').dataset.clicks"), "1")
    }
    // T031: empty replacement disables implicit scrollable and proxy additions.
    function test_T031_empty_replace_disables_secondary_discovery() {
        load("proxy-targets.html")
        var scrollablesDisabled = selectorSnapshot("all", "extend", [], "s12-disabled-scrollables", {
            scrollables: {mode:"replace", selectors:[], provenance:["site-rule:fixture"]}
        })
        evaluate("document.body.innerHTML+='<div id=scroll style=\"width:80px;height:40px;overflow:auto\"><div style=\"width:400px;height:120px\"></div></div>';true")
        var values = evaluate(BrowserScripts.hintCollector("all", true, scrollablesDisabled)).candidates
        verify(!values.some(function(item) { return item.kind === "scrollable" }),
               "The implicit all-family pass honors an empty scrollables replacement")
        var allDisabled = selectorSnapshot("all", "replace", [], "s12-disabled-all")
        values = evaluate(BrowserScripts.hintCollector("all", true, allDisabled)).candidates
        verify(!values.length, "An empty all replacement also disables its implicit scrollable pass")
        var inputsDisabled = selectorSnapshot("inputs", "replace", [], "s12-disabled-inputs")
        var inputValues = evaluate(BrowserScripts.hintCollector("inputs", true, inputsDisabled)).candidates
        verify(!inputValues.some(function(item) {
            return item.kind === "editor-focus" || item.kind === "label-control"
        }), "An empty inputs replacement disables opt-in proxy additions")
    }
    // T033: traversal and output stay bounded and report visible truncation.
    function test_T033_selector_collection_resource_caps() {
        load("adversarial-targets.html")
        evaluate("document.body.innerHTML='';document.body.style='margin:0';const fragment=document.createDocumentFragment();for(let i=0;i<5001;i++){const input=document.createElement('input');input.setAttribute('aria-label','Stress '+i);input.style='position:fixed;left:4px;top:4px;width:12px;height:12px';fragment.appendChild(input)}for(let i=0;i<50000;i++){fragment.appendChild(document.createElement('div'))}document.body.appendChild(fragment);true")
        var selectors = ["input[aria-label^='Stress']"]
        for (var selectorIndex = 0; selectorIndex < 31; ++selectorIndex) {
            selectors.push("div:not(:has(> [data-expensive='v" + selectorIndex + "']))")
        }
        var snapshot = selectorSnapshot("inputs", "replace", selectors, "s12-resource-stress")
        var collection = BrowserScripts.hintCollector("inputs", false, snapshot)
        var value = evaluate("(()=>{const started=performance.now();const result=" + collection
                           + ";result.collection_ms=performance.now()-started;return result;})()")
        verify(value.candidates.length <= 5000, "Output candidate limit is enforced")
        compare(value.candidates.length, 5000)
        verify(value.inspected_nodes <= 50000, "Per-snapshot inspected-element limit is enforced")
        verify(value.truncated, "Budget exhaustion is reported rather than shown as complete")
        verify(value.collection_ms >= 0, "Renderer timing is available for the bounded selector stress")
        console.info("FERRIC_SELECTOR_STRESS_JSON " + JSON.stringify({
            selector_count: selectors.length,
            inspected_nodes: value.inspected_nodes,
            candidates: value.candidates.length,
            truncated: value.truncated,
            collection_ms: value.collection_ms,
            user_agent: evaluate("navigator.userAgent"),
            viewport: evaluate("({width:innerWidth,height:innerHeight,device_pixel_ratio:devicePixelRatio})")
        }))
    }
}

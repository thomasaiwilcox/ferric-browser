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

    function evaluate(source, timeout) {
        done = false
        page.runJavaScript(source, WebEngineScript.ApplicationWorld, function(value) {
            result = value
            done = true
        })
        tryVerify(function() { return done }, timeout || 5000)
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
        page.zoomFactor = 1
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
    // T034: explicit facets share retained identities and exclude visual floods.
    function test_T034_region_and_surface_classification() {
        load("regions-surfaces.html")
        var regions = evaluate(BrowserScripts.hintCollector("regions"))
        compare(regions.candidates.length <= 256, true)
        verify(regions.candidates.some(function(item) { return item.text === "Main content" && item.kind === "region" }))
        verify(regions.candidates.some(function(item) { return item.text === "Named section" && item.kind === "region" }))
        verify(regions.candidates.some(function(item) { return item.text === "Scrollable results" && item.kind === "scrollable-region" }))
        verify(!regions.candidates.some(function(item) {
            return ["Unnamed section", "Unnamed ARIA region", "Large generic div"].includes(item.text)
        }), "Generic containers and unnamed sections do not become regions by size")
        var namedRegion = regions.candidates.filter(function(item) { return item.text === "Main content" })[0]
        verify(evaluate(BrowserScripts.hintFresh(namedRegion)).visible, "Region identity and visibility are rechecked")

        var surfaces = evaluate(BrowserScripts.hintCollector("surfaces"))
        verify(surfaces.candidates.length <= 256)
        var drawingBoard = surfaces.candidates.filter(function(item) { return item.text === "Drawing board" })[0]
        verify(drawingBoard, "Visible canvas missing from surfaces: " + JSON.stringify(surfaces.candidates))
        verify(["surface", "focusable-surface"].includes(drawingBoard.kind),
               "Canvas kind must be one of the closed surface types")
        verify(surfaces.candidates.some(function(item) { return item.text === "Small named surface" }),
               "A named visual surface may be smaller than 16 CSS pixels")
        verify(surfaces.candidates.some(function(item) {
            return item.text === "Canvas surface" && item.geometry.width >= 16 && item.geometry.height >= 16
        }), "An unnamed 16-by-16 CSS-pixel surface is eligible")
        verify(!evaluate("window.__ferric_browserHintState.weak.has(document.getElementById('below-minimum'))"),
               "Unnamed visual surfaces below 16 by 16 CSS pixels are excluded")
        verify(!evaluate("window.__ferric_browserHintState.weak.has(document.getElementById('hidden-canvas'))"),
               "Display-none canvas buffers are excluded")
        verify(!evaluate("window.__ferric_browserHintState.weak.has(document.getElementById('offscreen'))"),
               "Offscreen visual surfaces are excluded")
        var focusable = surfaces.candidates.filter(function(item) { return item.text === "Keyboard canvas" })[0]
        verify(focusable && focusable.kind === "focusable-surface", "Focus capability requires actual focus eligibility")
        verify(evaluate(BrowserScripts.hintFresh(focusable)).visible)
        verify(!evaluate(BrowserScripts.hintClick(focusable)), "Surface selection never authorizes semantic click")
        verify(evaluate(BrowserScripts.hintFocus(focusable)), "Explicit focus targets the exact focusable surface")
        verify(evaluate("document.activeElement===document.getElementById('focusable-canvas')"))

        regions = evaluate(BrowserScripts.hintCollector("regions"))
        var scrollableRegion = regions.candidates.filter(function(item) { return item.kind === "scrollable-region" })[0]
        verify(scrollableRegion)
        var scrollResult = evaluate(BrowserScripts.hintSetScrollTarget(
                                       scrollableRegion.element_id, scrollableRegion.frame_path))
        verify(scrollResult && scrollResult.ok && scrollResult.policy === "element",
               "Only an independently scrollable region can use the existing weak scroll target")
        var ordinary = evaluate(BrowserScripts.hintCollector("all")).candidates
        verify(!ordinary.some(function(item) { return ["region", "scrollable-region", "surface", "focusable-surface"].includes(item.kind) }),
               "The default all family does not gain Region or Surface kinds")
        cleanup()
    }
    function test_T034_region_and_surface_caps() {
        load("regions-surfaces.html")
        evaluate("{const fragment=document.createDocumentFragment();for(let i=0;i<300;i++){const region=document.createElement('section');region.setAttribute('aria-label','Cap region '+i);region.style='position:fixed;left:10px;top:10px;width:30px;height:20px';fragment.appendChild(region);const canvas=document.createElement('canvas');canvas.setAttribute('aria-label','Cap surface '+i);canvas.style='position:fixed;left:40px;top:10px;width:18px;height:18px';fragment.appendChild(canvas)}document.getElementById('main').appendChild(fragment);}true")
        var regions = evaluate(BrowserScripts.hintCollector("regions"))
        compare(regions.candidates.length, 256)
        verify(regions.truncated, "Region cap reports truncation")
        var surfaces = evaluate(BrowserScripts.hintCollector("surfaces"))
        compare(surfaces.candidates.length, 256)
        verify(surfaces.truncated, "Surface cap reports truncation")
        verify(regions.inspected_nodes <= 50000 && surfaces.inspected_nodes <= 50000)
        evaluate("{document.body.innerHTML='';const fragment=document.createDocumentFragment();for(let i=0;i<256;i++){const region=document.createElement('section');region.setAttribute('aria-label','Exact region '+i);region.style='position:fixed;left:2px;top:2px;width:20px;height:20px';fragment.appendChild(region);const canvas=document.createElement('canvas');canvas.setAttribute('aria-label','Exact surface '+i);canvas.style='position:fixed;left:2px;top:2px;width:20px;height:20px';fragment.appendChild(canvas)}document.body.appendChild(fragment);}true")
        regions = evaluate(BrowserScripts.hintCollector("regions"))
        compare(regions.candidates.length, 256)
        compare(regions.truncated, false, "Exactly 256 regions do not imply omitted candidates")
        surfaces = evaluate(BrowserScripts.hintCollector("surfaces"))
        compare(surfaces.candidates.length, 256)
        compare(surfaces.truncated, false, "Exactly 256 surfaces do not imply omitted candidates")
        cleanup()
    }
    // T035: parent collection retains only the cross-origin exterior and open roots.
    function test_T035_cross_origin_and_closed_root_boundaries() {
        load("cross-origin-surface.html")
        verify(evaluate("!Boolean(document.getElementById('cross-frame').contentDocument)"),
               "The cross-origin child document is not readable from its parent")
        var surfaces = evaluate(BrowserScripts.hintCollector("surfaces"))
        var exterior = surfaces.candidates.filter(function(item) { return item.text === "Cross-origin frame surface" })[0]
        verify(exterior && exterior.kind === "focusable-surface" && exterior.frame_path === "0",
               "The visible iframe exterior is represented in the parent document")
        verify(evaluate(BrowserScripts.hintFresh(exterior)).visible)
        var regions = evaluate(BrowserScripts.hintCollector("regions")).candidates
        verify(!regions.some(function(item) { return item.text === "Closed root secret" || item.text === "Unavailable child" }),
               "Closed-root content and cross-origin child semantics stay unavailable")
        var ordinary = evaluate(BrowserScripts.hintCollector("all")).candidates
        verify(!ordinary.some(function(item) { return item.kind === "region" || item.kind === "surface" }),
               "The default family remains unchanged for frame and closed-root content")
        var regionSnapshot = selectorSnapshot("regions", "merge", [], "0123456789abcdef")
        var inaccessible = {
            element_id: 2147483000,
            related_element_id: null,
            kind: "region",
            frame_path: "0.0",
            text: "Cross-origin child",
            href: null
        }
        var request = {
            lease_id: "cross-origin-scope",
            element_id: inaccessible.element_id,
            frame_path: inaccessible.frame_path,
            kind: inaccessible.kind,
            selector_revision: regionSnapshot.selector_revision,
            configuration_revision: 0,
            parent_scope_lease_id: null
        }
        var receipt = evaluate(BrowserScripts.hintAcquireScopeLease(
                                  request, inaccessible, regionSnapshot))
        verify(receipt && !receipt.ok,
               "An inaccessible cross-origin child cannot mint a semantic scope lease")
        cleanup()
    }

    // T020/T036/T037: the destination weak lease exists before source records are released.
    function test_T036_scoped_lease_transfer_collects_only_descendants() {
        load("scoped-hint.html")
        tryVerify(function() {
            return evaluate("!!(document.getElementById('same-origin-frame').contentDocument.body&&document.getElementById('shadow-host').shadowRoot.querySelector('iframe').contentDocument.body)")
        }, 5000)
        var regionSnapshot = selectorSnapshot("regions", "merge", [], "0123456789abcdef")
        var allSnapshot = selectorSnapshot("all", "merge", [], "fedcba9876543210")
        var regionCollection = evaluate(BrowserScripts.hintCollector(
                                           "regions", false, regionSnapshot))
        var scope = regionCollection.candidates.filter(function(item) {
            return item.text === "Scoped region"
        })[0]
        verify(scope, "Named scope fixture was not collected")
        var request = {
            lease_id: "test-scope-transfer",
            element_id: scope.element_id,
            frame_path: scope.frame_path,
            kind: scope.kind,
            selector_revision: regionSnapshot.selector_revision,
            configuration_revision: 0,
            parent_scope_lease_id: null
        }
        var receipt = evaluate(BrowserScripts.hintAcquireScopeLease(
                                  request, scope, regionSnapshot))
        verify(receipt && receipt.ok, "A fresh eligible Region must mint its destination lease: "
               + JSON.stringify(receipt))
        compare(evaluate("window.__ferric_browserHintState.elements.size"),
                regionCollection.candidates.length,
                "Source records stay alive until the destination lease validates")
        verify(evaluate("window.__ferric_browserHintState.scopeLease.elementRef instanceof WeakRef"))
        verify(!evaluate("Object.prototype.hasOwnProperty.call(window.__ferric_browserHintState.scopeLease,'element')"),
               "The scope lease stores no strong DOM element")
        var replay = evaluate(BrowserScripts.hintAcquireScopeLease(request, scope, regionSnapshot))
        verify(!replay.ok, "A local scope lease request cannot be replayed")
        var unscoped = evaluate(BrowserScripts.hintCollector("all", false, allSnapshot))
        compare(unscoped.error, "scope-lease-required")
        compare(unscoped.candidates.length, 0, "An active lease cannot be omitted to widen collection")

        verify(evaluate(BrowserScripts.hintStopTracking(request.lease_id)),
               "Source tracking teardown failed")
        compare(evaluate("window.__ferric_browserHintState.elements.size"), 0,
                "Source candidate map is cleared after lease acquisition")
        verify(evaluate("window.__ferric_browserHintState.scopeLease.elementRef.deref()===document.getElementById('scope')"),
               "The connected exact target remains available through its weak reference")

        evaluate("document.getElementById('scope').scrollTop=8;document.getElementById('outside').focus();true")
        var before = evaluate("({focus:document.activeElement.id,scrollX:scrollX,scrollY:scrollY,scopeTop:document.getElementById('scope').scrollTop})")
        var scoped = evaluate(BrowserScripts.hintCollector(
                                 "all", false, allSnapshot, request.lease_id))
        verify(scoped && scoped.candidates.length > 0,
               "Visible descendants should be collected inside the selected Region")
        verify(!scoped.candidates.some(function(item) {
            return item.element_id === scope.element_id || item.text === "Scoped region"
        }), "The scope root is excluded")
        verify(!scoped.candidates.some(function(item) { return item.text === "Outside target" }),
               "Candidates outside the selected Region are excluded")
        verify(scoped.candidates.some(function(item) { return item.text === "Inside button" }))
        verify(scoped.candidates.some(function(item) { return item.text === "Shadow child" }),
               "Open shadow-root descendants remain in scope")
        verify(scoped.candidates.some(function(item) { return item.text === "Frame child" }),
               "Same-origin frame descendants rooted inside the scope remain in scope")
        verify(scoped.candidates.some(function(item) { return item.text === "Shadow frame child" }),
               "Same-origin frames in an open shadow root remain in scope")
        var child = scoped.candidates.filter(function(item) { return item.text === "Inside button" })[0]
        verify(evaluate(BrowserScripts.hintFresh(child, allSnapshot)).visible,
               "A scoped child remains freshly revalidatable")
        var after = evaluate("({focus:document.activeElement.id,scrollX:scrollX,scrollY:scrollY,scopeTop:document.getElementById('scope').scrollTop})")
        compare(after, before, "Scoped entry and collection do not change focus or scroll")
        cleanup()
    }

    function test_T037_empty_and_replaced_scopes_never_fall_back() {
        load("scoped-hint.html")
        var regionSnapshot = selectorSnapshot("regions", "merge", [], "0123456789abcdef")
        var allSnapshot = selectorSnapshot("all", "merge", [], "fedcba9876543210")
        var scope = evaluate(BrowserScripts.hintCollector("regions", false, regionSnapshot))
                      .candidates.filter(function(item) { return item.text === "Scoped region" })[0]
        verify(scope)
        var request = {
            lease_id: "test-empty-scope",
            element_id: scope.element_id,
            frame_path: scope.frame_path,
            kind: scope.kind,
            selector_revision: regionSnapshot.selector_revision,
            configuration_revision: 0,
            parent_scope_lease_id: null
        }
        verify(evaluate(BrowserScripts.hintAcquireScopeLease(request, scope, regionSnapshot)).ok)
        verify(evaluate(BrowserScripts.hintStopTracking(request.lease_id)))
        evaluate("document.getElementById('scope').innerHTML='<div style=\"height:300px\"></div>';true")
        var empty = evaluate(BrowserScripts.hintCollector("all", false, allSnapshot, request.lease_id))
        compare(empty.candidates.length, 0)
        verify(!empty.candidates.some(function(item) { return item.text === "Outside target" }),
               "An empty scope never broadens to the page")
        cleanup()

        load("scoped-hint.html")
        scope = evaluate(BrowserScripts.hintCollector("regions", false, regionSnapshot))
                    .candidates.filter(function(item) { return item.text === "Scoped region" })[0]
        request.lease_id = "test-replaced-scope"
        request.element_id = scope.element_id
        request.frame_path = scope.frame_path
        request.kind = scope.kind
        verify(evaluate(BrowserScripts.hintAcquireScopeLease(request, scope, regionSnapshot)).ok)
        verify(evaluate(BrowserScripts.hintStopTracking(request.lease_id)))
        var descendants = evaluate(BrowserScripts.hintCollector(
                                      "all", false, allSnapshot, request.lease_id))
        var oldChild = descendants.candidates.filter(function(item) {
            return item.text === "Inside button"
        })[0]
        verify(oldChild, "The replacement case needs a live scoped child")
        evaluate("const old=document.getElementById('scope'),next=document.createElement('section');next.id='scope';next.setAttribute('aria-label','Replacement scope');old.replaceWith(next);true")
        var stale = evaluate(BrowserScripts.hintCollector("all", false, allSnapshot, request.lease_id))
        compare(stale.error, "stale-scope")
        compare(stale.candidates.length, 0)
        verify(!evaluate(BrowserScripts.hintFresh(oldChild, allSnapshot)).visible,
               "A replacement root cannot authorize a retained child")
        cleanup()
    }

    function test_T037_scroll_target_keeps_existing_weak_setter() {
        load("scoped-hint.html")
        var regionSnapshot = selectorSnapshot("regions", "merge", [], "0123456789abcdef")
        var regions = evaluate(BrowserScripts.hintCollector("regions", false, regionSnapshot))
        var scope = regions.candidates.filter(function(item) { return item.text === "Scoped region" })[0]
        verify(scope && scope.kind === "scrollable-region")
        var selected = evaluate(BrowserScripts.hintSetScrollTarget(scope.element_id, scope.frame_path))
        verify(selected && selected.ok && selected.policy === "element")
        verify(evaluate("window.__ferric_browserScrollTargetState.elementRef instanceof WeakRef"))
        verify(evaluate("window.__ferric_browserScrollTargetState.elementRef.deref()===document.getElementById('scope')"),
               "Set scroll target keeps the exact selected element in its existing weak policy")
        cleanup()
    }

    // T038: the Grid scope is projected through frame borders, CSS zoom, and
    // browser zoom once, then Rust maps the top CSS viewport to ViewLocal.
    function test_T038_nested_frame_scope_geometry_uses_measured_spaces() {
        load("geometry-spaces.html")
        page.zoomFactor = 1.25
        tryVerify(function() {
            return evaluate("document.getElementById('outer-frame').contentDocument"
                            + "?.getElementById('inner-frame').contentDocument"
                            + "?.getElementById('scope')?.isConnected") === true
        }, 5000)
        var regions = selectorSnapshot("regions", "merge", [], "0123456789abcdef")
        var values = evaluate(BrowserScripts.hintCollector("regions", false, regions))
        var scope = values.candidates.filter(function(item) {
            return item.text === "Nested scoped region"
        })[0]
        verify(scope, "Nested region should be discoverable in the frame tree")
        compare(scope.frame_path, "0.0.0")
        var request = {
            lease_id: "geometry-space-scope",
            element_id: scope.element_id,
            frame_path: scope.frame_path,
            kind: scope.kind,
            selector_revision: regions.selector_revision,
            configuration_revision: 0,
            parent_scope_lease_id: null
        }
        var receipt = evaluate(BrowserScripts.hintAcquireScopeLease(request, scope, regions))
        verify(receipt && receipt.ok, "Exact scoped target should grant a weak lease")
        verify(evaluate(BrowserScripts.hintStopTracking(request.lease_id)))
        var geometry = evaluate(BrowserScripts.spatialScopeGeometry(request.lease_id))
        verify(geometry && geometry.ok, "Fresh nested frame geometry should validate: "
               + JSON.stringify(geometry))
        verify(geometry.geometry.x > 0 && geometry.geometry.y > 0,
               "The scoped root keeps its nonzero origin in the page viewport")

        var expected = evaluate("(function(){const scope=document.getElementById('outer-frame')"
                + ".contentDocument.getElementById('inner-frame').contentDocument.getElementById('scope');"
                + "let r=scope.getBoundingClientRect(),left=r.left,top=r.top,right=r.right,bottom=r.bottom;"
                + "let view=scope.ownerDocument.defaultView;while(view!==window){const frame=view.frameElement;"
                + "const parent=frame.ownerDocument.defaultView,box=frame.getBoundingClientRect();"
                + "const bx=box.width/frame.offsetWidth,by=box.height/frame.offsetHeight;"
                + "const sx=frame.clientWidth*bx/view.innerWidth,sy=frame.clientHeight*by/view.innerHeight;"
                + "left=box.left+frame.clientLeft*bx+left*sx;right=box.left+frame.clientLeft*bx+right*sx;"
                + "top=box.top+frame.clientTop*by+top*sy;bottom=box.top+frame.clientTop*by+bottom*sy;view=parent;}"
                + "return {x:left,y:top,width:right-left,height:bottom-top};})()")
        compare(Math.abs(geometry.geometry.x - expected.x) < 0.75, true,
                "Nested frame borders and zoom use measured origins once")
        compare(Math.abs(geometry.geometry.y - expected.y) < 0.75, true)
        compare(Math.abs(geometry.geometry.width - expected.width) < 0.75, true)
        compare(Math.abs(geometry.geometry.height - expected.height) < 0.75, true)
        verify(Math.abs(geometry.viewport_width - evaluate("window.innerWidth")) < 0.01)
        cleanup()
        page.zoomFactor = 1
    }

    function test_T038_transformed_scope_is_rejected() {
        load("geometry-spaces.html")
        var regions = selectorSnapshot("regions", "merge", [], "0123456789abcdef")
        var scope = evaluate(BrowserScripts.hintCollector("regions", false, regions))
                      .candidates.filter(function(item) {
                          return item.text === "Nested scoped region"
                      })[0]
        verify(scope)
        var request = {
            lease_id: "transformed-geometry-scope",
            element_id: scope.element_id,
            frame_path: scope.frame_path,
            kind: scope.kind,
            selector_revision: regions.selector_revision,
            configuration_revision: 0,
            parent_scope_lease_id: null
        }
        verify(evaluate(BrowserScripts.hintAcquireScopeLease(request, scope, regions)).ok)
        verify(evaluate(BrowserScripts.hintStopTracking(request.lease_id)))
        evaluate("document.getElementById('outer-frame').contentDocument"
                 + ".getElementById('inner-frame').contentDocument.getElementById('scope')"
                 + ".style.transform='translateX(2px)';true")
        var geometry = evaluate(BrowserScripts.spatialScopeGeometry(request.lease_id))
        verify(!geometry.ok, "Unsupported transforms must be rejected instead of guessed")
        cleanup()
    }

    function test_T035_cross_origin_frame_scopes_only_its_outer_surface() {
        load("cross-origin-surface.html")
        var surfaces = selectorSnapshot("surfaces", "merge", [], "89abcdef01234567")
        var candidates = evaluate(BrowserScripts.hintCollector("surfaces", false, surfaces))
        var frameSurface = candidates.candidates.filter(function(item) {
            return item.frame_path === "0"
                    && (item.kind === "surface" || item.kind === "focusable-surface")
        })[0]
        verify(frameSurface, "The parent can identify the outer iframe surface")
        verify(frameSurface.kind === "surface" || frameSurface.kind === "focusable-surface")
        compare(frameSurface.frame_path, "0")
        verify(!candidates.candidates.some(function(item) {
            return item.frame_path !== "0"
        }), "Cross-origin child contents are not collected semantically")
        var request = {
            lease_id: "cross-origin-surface-grid",
            element_id: frameSurface.element_id,
            frame_path: frameSurface.frame_path,
            kind: frameSurface.kind,
            selector_revision: surfaces.selector_revision,
            configuration_revision: 0,
            parent_scope_lease_id: null
        }
        var receipt = evaluate(BrowserScripts.hintAcquireScopeLease(
                                  request, frameSurface, surfaces))
        verify(receipt && receipt.ok,
               "Grid may scope raw navigation to the parent-owned iframe surface")
        verify(evaluate(BrowserScripts.hintStopTracking(request.lease_id)))
        var geometry = evaluate(BrowserScripts.spatialScopeGeometry(request.lease_id))
        verify(geometry && geometry.ok,
               "Outer iframe content geometry does not require cross-origin access: "
               + JSON.stringify(geometry))
        var expected = evaluate("(function(){const frame=document.getElementById('cross-frame'),r=frame.getBoundingClientRect();return {x:r.left+frame.clientLeft,y:r.top+frame.clientTop,width:frame.clientWidth,height:frame.clientHeight};})()")
        compare(Math.abs(geometry.geometry.x - expected.x) < 0.75, true)
        compare(Math.abs(geometry.geometry.y - expected.y) < 0.75, true)
        compare(Math.abs(geometry.geometry.width - expected.width) < 0.75, true)
        compare(Math.abs(geometry.geometry.height - expected.height) < 0.75, true)
        cleanup()
    }

    function test_T039_scope_scroll_revision_and_reflow_are_observable() {
        load("geometry-spaces.html")
        var regions = selectorSnapshot("regions", "merge", [], "0123456789abcdef")
        var scope = evaluate(BrowserScripts.hintCollector("regions", false, regions))
                      .candidates.filter(function(item) {
                          return item.text === "Nested scoped region"
                      })[0]
        verify(scope)
        var request = {
            lease_id: "scope-lifecycle-geometry",
            element_id: scope.element_id,
            frame_path: scope.frame_path,
            kind: scope.kind,
            selector_revision: regions.selector_revision,
            configuration_revision: 0,
            parent_scope_lease_id: null
        }
        verify(evaluate(BrowserScripts.hintAcquireScopeLease(request, scope, regions)).ok)
        verify(evaluate(BrowserScripts.hintStopTracking(request.lease_id)))
        var before = evaluate(BrowserScripts.spatialScopeGeometry(request.lease_id))
        verify(before.ok, "Initial scope geometry should be available: "
               + JSON.stringify(before))
        var scrollSetup = evaluate("(function(){const scope=document.getElementById('outer-frame').contentDocument"
                 + ".getElementById('inner-frame').contentDocument.getElementById('scope');"
                 + "scope.scrollTop=24;return {scrollTop:scope.scrollTop,max:scope.scrollHeight-scope.clientHeight};})()")
        verify(scrollSetup.scrollTop > 0 && scrollSetup.max > 0,
               "The fixture scope has a real scrollable range")
        var scrollEvent = evaluate("(function(){const scope=document.getElementById('outer-frame').contentDocument"
                 + ".getElementById('inner-frame').contentDocument.getElementById('scope');"
                 + "scope.dispatchEvent(new Event('scroll'));return window.__ferric_browserHintState.scopeLease.scrollRevision;})()")
        verify(scrollEvent > before.scroll_revision,
               "Scroll events on the exact scope advance its revision")
        var scrolled = evaluate(BrowserScripts.spatialScopeGeometry(request.lease_id))
        evaluate("document.getElementById('outer-frame').contentDocument"
                 + ".getElementById('inner-frame').contentDocument.getElementById('scope')"
                 + ".style.left='48px';true")
        var reflowed = evaluate(BrowserScripts.spatialScopeGeometry(request.lease_id))
        verify(reflowed.ok)
        verify(Math.abs(reflowed.geometry.x - scrolled.geometry.x) > 1,
               "A reflow changes the fresh scope projection")
        cleanup()
    }

    // T044: exercise the production builders through repeated teardown and weak-target checks.
    function test_T044_scope_start_cancel_stress_and_forced_gc() {
        load("scoped-hint.html")
        var regionSnapshot = selectorSnapshot("regions", "merge", [], "0123456789abcdef")
        var source = evaluate(BrowserScripts.hintCollector("regions", false, regionSnapshot))
        var scope = source.candidates.filter(function(item) { return item.text === "Scoped region" })[0]
        verify(scope)
        var request = {
            lease_id: "hint-scope-stress",
            element_id: scope.element_id,
            frame_path: scope.frame_path,
            kind: scope.kind,
            selector_revision: regionSnapshot.selector_revision,
            configuration_revision: 0,
            parent_scope_lease_id: null
        }
        var collectScript = BrowserScripts.hintCollector("regions", false, regionSnapshot)
        var acquireScript = BrowserScripts.hintAcquireScopeLease(request, scope, regionSnapshot)
        var stopScript = BrowserScripts.hintStopTracking()
        var stressScript = "(function(){const started=Date.now();let cycles=0;for(let index=0;index<1000;index++){const source="
                + collectScript + ";if(!source.candidates.some(function(item){return item.element_id==="
                + scope.element_id + "&&item.text==='Scoped region'}))return {ok:false,at:index};const receipt="
                + acquireScript + ";if(!receipt.ok)return {ok:false,at:index,error:receipt.error};const stopped="
                + stopScript + ";if(!stopped)return {ok:false,at:index,reason:'teardown'};cycles++;}const gcAvailable=typeof gc==='function';if(gcAvailable)gc();const s=window.__ferric_browserHintState;return {ok:cycles===1000&&s.elements.size===0&&s.observers.length===0&&s.windows.length===0&&!s.tracking&&!s.scopeLease,gc_available:gcAvailable,cycles:cycles,elements:s.elements.size,observers:s.observers.length,windows:s.windows.length,elapsed_ms:Date.now()-started};})()"
        var stress = evaluate(stressScript, 30000)
        console.log("S14 T044 scope stress: " + JSON.stringify(stress))
        verify(stress && stress.ok, "1000 source-map/lease teardown cycles must remain bounded: "
               + JSON.stringify(stress))
        verify(Number.isFinite(stress.elapsed_ms) && stress.elapsed_ms >= 0,
               "The 1000-cycle teardown measurement must report elapsed time")
        verify(stress.gc_available, "The qualified WebEngine test process must expose forced GC")
        var finalCandidate = evaluate(BrowserScripts.hintCollector("regions", false, regionSnapshot))
                               .candidates.filter(function(item) { return item.text === "Scoped region" })[0]
        request.element_id = finalCandidate.element_id
        request.frame_path = finalCandidate.frame_path
        request.kind = finalCandidate.kind
        var finalReceipt = evaluate(BrowserScripts.hintAcquireScopeLease(
                                        request, finalCandidate, regionSnapshot))
        verify(finalReceipt.ok)
        verify(evaluate("window.__ferric_browserHintState.scopeLease.elementRef.deref()===document.getElementById('scope')"),
               "A legitimate connected weak target remains usable after GC")
        cleanup()
    }
}

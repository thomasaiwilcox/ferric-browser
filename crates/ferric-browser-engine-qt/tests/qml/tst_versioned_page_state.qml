import QtQuick
import QtQuick.Controls
import QtTest
import QtWebEngine
import "../../qml/scripts/BrowserScripts.js" as BrowserScripts
import "../../qml/scripts/VersionedPageState.js" as VersionedPageState

TestCase {
    id: probe
    name: "VersionedPageState"
    when: windowShown
    width: 480
    height: 320

    // The xtask native admission probe substitutes a live ephemeral loopback
    // fixture URL into a temporary copy of this test file. Direct QML runs may
    // omit it; the same-origin frame contract is then explicitly skipped.
    property string sameOriginFrameFixtureUrl: "__FERRIC_FRAME_FIXTURE_URL__"

    ApplicationWindow {
        id: testWindow
        width: 480
        height: 320
        visible: true
    }

    WebEngineProfilePrototype {
        id: profilePrototype
        storageName: ""
    }

    property var disposableProfile: null

    function initTestCase() {
        disposableProfile = profilePrototype.instance()
        verify(disposableProfile)
    }

    Component {
        id: viewFactory
        WebEngineView {
            profile: probe.disposableProfile
            width: 460
            height: 300
            visible: true
            property bool loaded: false
            property string loadFailure: ""
            onLoadingChanged: function(request) {
                if (request.status === WebEngineView.LoadSucceededStatus) {
                    loaded = true
                } else if (request.status === WebEngineView.LoadFailedStatus) {
                    loadFailure = request.errorString
                }
            }
        }
    }

    function evaluate(view, script, world) {
        var result = { done: false, value: null }
        view.runJavaScript(script, world === undefined
                           ? WebEngineScript.ApplicationWorld : world,
                           function(value) {
            result.value = value
            result.done = true
        })
        tryVerify(function() { return result.done }, 10000)
        return result.value
    }

    function geometryRequest() {
        return {
            nonce: "11111111111111111111111111111111",
            expected_origin: "https://example.test",
            expected_route: "/geometry",
            route_fingerprint: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            capture_fingerprint: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            max_nodes: 256,
            max_candidates: 16,
            max_ms: 50
        }
    }

    function loadGeometryView(html) {
        var view = createTemporaryObject(viewFactory, testWindow.contentItem)
        verify(view)
        view.loadHtml(html, "https://example.test/geometry")
        tryVerify(function() { return view.loaded || view.loadFailure.length > 0 }, 15000)
        compare(view.loadFailure, "")
        return view
    }

    function loadSameOriginFrameFixture() {
        var view = createTemporaryObject(viewFactory, testWindow.contentItem)
        verify(view)
        verify(sameOriginFrameFixtureUrl.indexOf("http://127.0.0.1:") === 0,
               "the native Rust admission harness must provide a loopback fixture")
        view.url = sameOriginFrameFixtureUrl
        tryVerify(function() { return view.loaded || view.loadFailure.length > 0 }, 15000)
        compare(view.loadFailure, "")
        tryVerify(function() {
            if (!view.mainFrame || !view.mainFrame.children
                    || view.mainFrame.children.length !== 2) return false
            var hasSameOriginChild = false
            var hasCrossOriginChild = false
            for (var index = 0; index < view.mainFrame.children.length; index++) {
                var childUrl = String(view.mainFrame.children[index].url)
                if (childUrl.indexOf("/versioning/frame-child?kind=same") >= 0)
                    hasSameOriginChild = true
                if (childUrl.indexOf("/versioning/frame-child?cross=1") >= 0)
                    hasCrossOriginChild = true
            }
            return hasSameOriginChild && hasCrossOriginChild
        }, 15000)
        return view
    }

    function sameOriginFrameRequest() {
        var request = geometryRequest()
        request.expected_origin = sameOriginFrameFixtureUrl
            .replace(/\/versioning\/frames$/, "")
        request.expected_route = "/versioning/frames"
        return request
    }

    function finishMutationProbe(capture) {
        verify(capture.probe)
        var result = {done: false, value: ""}
        capture.probe(function(value) {
            result.value = String(value)
            result.done = true
        })
        tryVerify(function() { return result.done }, 10000)
        return result.value
    }

    function collect(view, request) {
        return JSON.parse(evaluate(view, VersionedPageState.geometryScript(request)))
    }

    function collectFrames(view, request) {
        var result = { done: false, value: null }
        verify(VersionedPageState.collectFrameTree(view, request, function(value) {
            result.value = value
            result.done = true
        }) !== false)
        tryVerify(function() { return result.done }, 15000)
        return JSON.parse(result.value)
    }

    function collectFramesWithMutationProbe(view, request) {
        var result = { done: false, value: null, probe: null }
        verify(VersionedPageState.collectFrameTree(view, request, function(value, probe) {
            result.value = value
            result.probe = probe
            result.done = true
        }) !== false)
        tryVerify(function() { return result.done }, 15000)
        return {sidecar: JSON.parse(result.value), probe: result.probe}
    }

    function restore(view, request) {
        return JSON.parse(evaluate(view, VersionedPageState.restoreGeometryScript(request)))
    }

    function geometryRestoreRequest(capture, x, y) {
        var request = geometryRequest()
        request.include_controls = false
        request.restore_geometry = {
            top_x: null,
            top_y: null,
            targets: [{
                candidate_id: capture.candidates[0].candidate_id,
                id_fingerprint: capture.candidates[0].id_fingerprint,
                name_fingerprint: capture.candidates[0].name_fingerprint,
                label_fingerprint: capture.candidates[0].label_fingerprint,
                context_fingerprint: capture.candidates[0].context_fingerprint,
                x: x,
                y: y
            }]
        }
        return request
    }

    function test_load_html_has_qualifiable_origin() {
        var view = createTemporaryObject(viewFactory, testWindow.contentItem)
        verify(view)
        view.loadHtml("<!doctype html><html><body>synthetic geometry</body></html>",
                      "https://example.test/geometry")
        tryVerify(function() { return view.loaded || view.loadFailure.length > 0 }, 15000)
        compare(view.loadFailure, "")
        compare(evaluate(view, "location.origin"), "https://example.test")
    }

    function test_same_url_document_replacement_loses_application_world_challenge() {
        var html = "<!doctype html><html><body>synthetic challenge</body></html>"
        var view = loadGeometryView(html)
        var request = geometryRequest()
        compare(collect(view, request).schema_version, 1)
        compare(evaluate(view, VersionedPageState.documentChallengeScript(request)), "same")

        view.loaded = false
        view.loadHtml(html, "https://example.test/geometry")
        tryVerify(function() { return view.loaded || view.loadFailure.length > 0 }, 15000)
        compare(view.loadFailure, "")
        compare(evaluate(view, VersionedPageState.documentChallengeScript(request)), "changed")
    }

    function test_geometry_collects_top_and_nested_without_changing_scroll_policy() {
        var html = "<!doctype html><html><body style='height:2500px'>"
                 + "<div id='nested' style='height:100px;width:200px;overflow:auto'>"
                 + "<div style='height:1000px'>synthetic nested scroll</div></div>"
                 + "</body></html>"
        var view = loadGeometryView(html)
        compare(evaluate(view,
            "document.scrollingElement.scrollTop=120;"
            + "document.getElementById('nested').scrollTop=55;true"), true)
        var before = evaluate(view, BrowserScripts.scrollTargetPolicy("document"))
        compare(before.policy, "document")
        var result = collect(view, geometryRequest())
        compare(result.schema_version, 1)
        compare(result.nonce, geometryRequest().nonce)
        compare(result.frames[0].origin, "https://example.test")
        verify(result.positions.some(function(position) {
            return position.kind === "top-scroll-y" && position.value >= 100
        }))
        verify(result.positions.some(function(position) {
            return position.kind === "element-scroll-y" && position.value >= 50
        }))
        compare(result.candidates[0].id_fingerprint,
                "424c3a61930e8f21191af0f5cba9a6a8514c42d29900757f2607f4bc71ef935d")
        verify(JSON.stringify(result).indexOf("nested") < 0,
               "candidate IDs must be hashed rather than returned raw")
        compare(evaluate(view, BrowserScripts.scrollTargetStatus()).policy, "document")
        // This fixture contains no editable values or real profile data. The
        // xtask native contract check passes the exact output to Rust's DTO.
        console.log("FERRIC_GEOMETRY_RESULT:" + JSON.stringify(result))
    }

    function test_bounded_traversal_and_mutating_dom() {
        var view = loadGeometryView("<!doctype html><html><body></body></html>")
        compare(evaluate(view,
            "var prior=TreeWalker.prototype.nextNode;"
            + "TreeWalker.prototype.nextNode=function(){"
            + "var extra=document.createElement('div');extra.textContent='synthetic';"
            + "document.body.appendChild(extra);return prior.call(this)};true"), true)
        var request = geometryRequest()
        request.max_nodes = 10
        var result = collect(view, request)
        compare(result.coverage[0].truncated, true)
        verify(result.exclusions.some(function(exclusion) {
            return exclusion.reason === "limit"
        }))
        verify(result.candidates.length <= request.max_candidates)
        compare(evaluate(view, "document.querySelectorAll('div').length"), 10)
    }

    function test_detached_scroller_and_non_finite_offset_are_excluded() {
        var html = "<!doctype html><html><body>"
                 + "<div id='nested' style='height:100px;overflow:auto'>"
                 + "<div style='height:1000px'>synthetic</div></div></body></html>"
        var view = loadGeometryView(html)
        compare(evaluate(view,
            "var nested=document.getElementById('nested');"
            + "Object.defineProperty(nested,'scrollTop',{get:function(){return NaN}});"
            + "true"), true)
        var invalid = collect(view, geometryRequest())
        verify(invalid.exclusions.some(function(exclusion) {
            return exclusion.reason === "unsupported"
        }))
        verify(!invalid.positions.some(function(position) {
            return position.kind === "element-scroll-y"
        }))
        compare(evaluate(view,
            "var prior=TreeWalker.prototype.nextNode;"
            + "TreeWalker.prototype.nextNode=function(){var next=prior.call(this);"
            + "if(next&&next.id==='nested')next.remove();return next};true"), true)
        var detached = collect(view, geometryRequest())
        verify(!detached.candidates.some(function(candidate) {
            return candidate.id_fingerprint
                   === "424c3a61930e8f21191af0f5cba9a6a8514c42d29900757f2607f4bc71ef935d"
        }))
    }

    function test_simple_controls_are_bounded_and_closed() {
        var html = "<!doctype html><html><body>"
                 + "<details id='facts' open><summary>synthetic</summary><p>body</p></details>"
                 + "<select id='choice'><option value='first'>first</option>"
                 + "<option value='second' selected>second</option></select>"
                 + "</body></html>"
        var view = loadGeometryView(html)
        var request = geometryRequest()
        request.include_controls = true
        var result = collect(view, request)
        compare(result.requested_categories.length, 2)
        verify(result.controls.some(function(control) {
            return control.value.kind === "details" && control.value.open
        }))
        verify(result.controls.some(function(control) {
            return control.value.kind === "select"
                   && control.value.option_fingerprints.length === 1
        }))
        verify(JSON.stringify(result).indexOf("second") < 0)
        request.max_nodes = 5000
        compare(collect(view, request).error, "invalid-request")
    }

    function test_editable_and_selection_are_opt_in_and_sensitive_values_are_excluded() {
        var html = "<!doctype html><html><body>"
                 + "<input id='note' value='synthetic draft'>"
                 + "<input id='password' type='password' value='synthetic-password'>"
                 + "<input id='csrf_token' value='synthetic-csrf'>"
                 + "<input id='otp' autocomplete='one-time-code' value='synthetic-otp'>"
                 + "<input id='hidden' type='hidden' name='session-token' value='synthetic-session'>"
                 + "<textarea id='area'>selection draft</textarea>"
                 + "<input id='checked' type='checkbox' checked>"
                 + "<div id='rich' contenteditable='true'>rich draft</div>"
                 + "</body></html>"
        var view = loadGeometryView(html)
        var defaultResult = collect(view, geometryRequest())
        compare(defaultResult.sensitive_editable.length, 0)
        verify(defaultResult.requested_categories.indexOf("editable") < 0)
        verify(defaultResult.requested_categories.indexOf("selection") < 0)

        compare(evaluate(view,
            "var area=document.getElementById('area');area.focus();"
            + "area.setSelectionRange(2,6);true"), true)
        var request = geometryRequest()
        request.include_editable = true
        request.include_selection = true
        var result = collect(view, request)
        verify(result.requested_categories.indexOf("editable") >= 0)
        verify(result.requested_categories.indexOf("selection") >= 0)
        verify(result.sensitive_editable.some(function(item) {
            return item.value.kind === "text" && item.value.value === "synthetic draft"
        }))
        verify(result.sensitive_editable.some(function(item) {
            return item.value.kind === "checked" && item.value.value === true
        }))
        verify(result.sensitive_editable.some(function(item) {
            return item.value.kind === "text" && item.value.value === "rich draft"
        }))
        verify(result.positions.some(function(position) {
            return position.kind === "selection-start" && position.value === 2
        }))
        verify(result.positions.some(function(position) {
            return position.kind === "selection-end" && position.value === 6
        }))
        var geometryCoverage = result.coverage.find(function(item) {
            return item.category === "geometry"
        })
        var selectionCoverage = result.coverage.find(function(item) {
            return item.category === "selection"
        })
        compare(geometryCoverage.captured, 2)
        compare(selectionCoverage.captured, 2)
        var encoded = JSON.stringify(result)
        verify(encoded.indexOf("synthetic-password") < 0)
        verify(encoded.indexOf("synthetic-csrf") < 0)
        verify(encoded.indexOf("synthetic-otp") < 0)
        verify(encoded.indexOf("synthetic-session") < 0)
        verify(result.exclusions.some(function(exclusion) {
            return exclusion.reason === "sensitive-control"
        }))
        console.log("FERRIC_EDITABLE_RESULT:" + encoded)
    }

    function test_oversized_editable_value_is_skipped_with_partial_coverage() {
        var value = new Array(4098).join("x")
        var view = loadGeometryView("<!doctype html><html><body>"
                                    + "<textarea id='draft'>" + value + "</textarea>"
                                    + "</body></html>")
        var request = geometryRequest()
        request.include_editable = true
        var result = collect(view, request)
        compare(result.sensitive_editable.length, 0)
        var coverage = result.coverage.filter(function(item) {
            return item.category === "editable"
        })[0]
        verify(coverage)
        compare(coverage.captured, 0)
        compare(coverage.truncated, true)
        verify(result.exclusions.some(function(exclusion) { return exclusion.reason === "limit" }))
    }

    function test_overlong_editable_metadata_fails_closed() {
        var longPrefix = new Array(258).join("x") + "-password"
        var html = "<!doctype html><html><body>"
                 + "<input id='" + longPrefix + "' value='synthetic-long-id'>"
                 + "<input name='" + longPrefix + "' value='synthetic-long-name'>"
                 + "<input autocomplete='" + longPrefix + "' value='synthetic-long-autocomplete'>"
                 + "<input aria-label='" + longPrefix + "' value='synthetic-long-label'>"
                 + "</body></html>"
        var view = loadGeometryView(html)
        var request = geometryRequest()
        request.include_editable = true
        var result = collect(view, request)
        compare(result.sensitive_editable.length, 0)
        var encoded = JSON.stringify(result)
        verify(encoded.indexOf("synthetic-long-id") < 0)
        verify(encoded.indexOf("synthetic-long-name") < 0)
        verify(encoded.indexOf("synthetic-long-autocomplete") < 0)
        verify(encoded.indexOf("synthetic-long-label") < 0)
        compare(result.exclusions.filter(function(item) {
            return item.reason === "sensitive-control"
        }).length, 4)
        var coverage = result.coverage.find(function(item) {
            return item.category === "editable"
        })
        verify(coverage)
        compare(coverage.requested, 4)
        compare(coverage.captured, 0)
    }

    function test_open_shadow_editor_is_bounded_and_included_by_explicit_editable_opt_in() {
        var view = loadGeometryView("<!doctype html><html><body>"
                                    + "<x-editor id='editor-host' style='display:block;width:300px;height:80px'></x-editor>"
                                    + "</body></html>")
        compare(evaluate(view,
            "customElements.define('x-editor',class extends HTMLElement{});"
            + "var root=document.getElementById('editor-host').attachShadow({mode:'open'});"
            + "root.innerHTML='<div id=\"shadow-draft\" contenteditable=\"true\">synthetic shadow draft</div>';true",
            WebEngineScript.MainWorld),
            true)
        var request = geometryRequest()
        request.include_editable = true
        var result = collectFrames(view, request)
        compare(result.helper_version, "ferric-page-state-5")
        verify(result.sensitive_editable.some(function(item) {
            return item.value.kind === "text" && item.value.value === "synthetic shadow draft"
        }))
        verify(result.candidates.some(function(candidate) {
            return candidate.kind === "content-editable"
                   && candidate.id_fingerprint !== null
                   && candidate.context_fingerprint !== null
        }))
        verify(JSON.stringify(result).indexOf("shadow-draft") < 0)
        verify(JSON.stringify(result).indexOf("editor-host") < 0)
        var editableCoverage = result.coverage.filter(function(item) {
            return item.category === "editable"
        })[0]
        verify(editableCoverage)
        compare(editableCoverage.captured, 1)
        compare(editableCoverage.truncated, false)
    }

    function test_registered_closed_shadow_component_is_explicitly_partial() {
        var view = loadGeometryView("<!doctype html><html><body>"
                                    + "<x-closed-editor id='closed-host' style='display:block;width:300px;height:80px'></x-closed-editor>"
                                    + "</body></html>")
        compare(evaluate(view,
            "customElements.define('x-closed-editor',class extends HTMLElement{"
            + "constructor(){super();this.attachShadow({mode:'closed'}).innerHTML="
            + "'<div contenteditable=\"true\">synthetic closed shadow secret</div>';}});true",
            WebEngineScript.MainWorld),
            true)
        var request = geometryRequest()
        request.include_editable = true
        var result = collectFrames(view, request)
        compare(result.sensitive_editable.length, 0)
        verify(JSON.stringify(result).indexOf("synthetic closed shadow secret") < 0)
        verify(result.exclusions.some(function(exclusion) {
            return exclusion.reason === "uninspectable-component"
        }))
        var editableCoverage = result.coverage.filter(function(item) {
            return item.category === "editable"
        })[0]
        verify(editableCoverage)
        compare(editableCoverage.truncated, true)
    }

    function test_contenteditable_selection_uses_bounded_absolute_text_offsets() {
        var view = loadGeometryView("<!doctype html><html><body>"
                                    + "<div id='editor' contenteditable='true'>"
                                    + "<span>abc</span><span>def</span></div>"
                                    + "</body></html>")
        compare(evaluate(view,
            "var editor=document.getElementById('editor');editor.focus();"
            + "var spans=editor.querySelectorAll('span');var range=document.createRange();"
            + "range.setStart(spans[0].firstChild,1);range.setEnd(spans[1].firstChild,2);"
            + "var selection=window.getSelection();selection.removeAllRanges();"
            + "selection.addRange(range);true"), true)
        var request = geometryRequest()
        request.include_selection = true
        var result = collectFrames(view, request)
        var start = result.positions.filter(function(position) {
            return position.kind === "selection-start"
        })[0]
        var end = result.positions.filter(function(position) {
            return position.kind === "selection-end"
        })[0]
        verify(start)
        verify(end)
        compare(start.value, 1)
        compare(end.value, 5)
        verify(JSON.stringify(result).indexOf("abc") < 0)
        var selectionCoverage = result.coverage.filter(function(item) {
            return item.category === "selection"
        })[0]
        verify(selectionCoverage)
        compare(selectionCoverage.captured, 2)
        compare(selectionCoverage.truncated, false)
    }

    function test_media_capture_is_bounded_and_restore_never_starts_playback() {
        var html = "<!doctype html><html><body>"
                 + "<video id='saved' style='width:320px;height:180px'></video>"
                 + "<video id='playing' style='width:320px;height:180px'></video>"
                 + "<video id='live' style='width:320px;height:180px'></video>"
                 + "<video id='dvr' style='width:320px;height:180px'></video>"
                 + "<video id='drm' style='width:320px;height:180px'></video>"
                 + "<video id='blob' style='width:320px;height:180px'></video>"
                 + "<video id='invalid-duration' style='width:320px;height:180px'></video>"
                 + "<video id='invalid-time' style='width:320px;height:180px'></video>"
                 + "</body></html>"
        var view = loadGeometryView(html)
        compare(evaluate(view,
            "window.__mediaCalls={play:0,pause:0,userMedia:0,displayMedia:0};"
            + "HTMLMediaElement.prototype.play=function(){window.__mediaCalls.play++;};"
            + "HTMLMediaElement.prototype.pause=function(){window.__mediaCalls.pause++;};"
            + "var countUserMedia=function(){window.__mediaCalls.userMedia++;};"
            + "var countDisplayMedia=function(){window.__mediaCalls.displayMedia++;};"
            + "try{if(navigator.mediaDevices){Object.defineProperty(navigator.mediaDevices,'getUserMedia',{configurable:true,value:countUserMedia});Object.defineProperty(navigator.mediaDevices,'getDisplayMedia',{configurable:true,value:countDisplayMedia});}}catch(error){}"
            + "try{Object.defineProperty(navigator,'getUserMedia',{configurable:true,value:countUserMedia});}catch(error){}"
            + "function mockMedia(id,time,duration,paused,source,protectedMedia){"
            + "var e=document.getElementById(id);e.__time=time;"
            + "Object.defineProperty(e,'currentTime',{configurable:true,get:function(){return this.__time;},set:function(v){this.__time=v;this.__seekCalls=(this.__seekCalls||0)+1;}});"
            + "Object.defineProperty(e,'duration',{configurable:true,value:duration});"
            + "Object.defineProperty(e,'paused',{configurable:true,value:paused});"
            + "Object.defineProperty(e,'readyState',{configurable:true,value:1});"
            + "Object.defineProperty(e,'currentSrc',{configurable:true,value:source});"
            + "Object.defineProperty(e,'mediaKeys',{configurable:true,value:protectedMedia?{}:null});"
            + "Object.defineProperty(e,'srcObject',{configurable:true,value:null});return e;};"
            + "mockMedia('saved',10,120,true,'https://example.test/saved.mp4',false);"
            + "mockMedia('playing',8,120,false,'https://example.test/playing.mp4',false);"
            + "mockMedia('live',8,Infinity,true,'https://example.test/live.m3u8',false);"
            + "var dvr=mockMedia('dvr',130,300,true,'https://example.test/dvr.mp4',false);"
            + "Object.defineProperty(dvr,'seekable',{configurable:true,value:{length:1,start:function(){return 100;},end:function(){return 300;}}});"
            + "mockMedia('drm',8,120,true,'https://example.test/drm.mp4',true);"
            + "mockMedia('blob',8,120,true,'blob:https://example.test/session',false);"
            + "mockMedia('invalid-duration',8,NaN,true,'https://example.test/invalid-duration.mp4',false);"
            + "mockMedia('invalid-time',-1,120,true,'https://example.test/invalid-time.mp4',false);true"), true)
        var defaultCapture = collect(view, geometryRequest())
        verify(defaultCapture.requested_categories.indexOf("media") < 0)
        verify(!defaultCapture.candidates.some(function(candidate) {
            return candidate.kind === "media"
        }))
        compare(evaluate(view, "document.getElementById('saved').__time"), 10)
        var request = geometryRequest()
        request.include_media = true
        var captured = collect(view, request)
        compare(captured.helper_version, "ferric-page-state-5")
        verify(captured.requested_categories.indexOf("media") >= 0)
        var capturedJson = JSON.stringify(captured)
        // These are transient native-callback inputs. Rust replaces them with
        // profile-keyed HMAC values before sidecar parsing or persistence.
        verify(capturedJson.indexOf("https://example.test/saved.mp4") >= 0)
        verify(capturedJson.indexOf("https://example.test/playing.mp4") >= 0)
        verify(capturedJson.indexOf("invalid-duration.mp4") < 0)
        verify(capturedJson.indexOf("invalid-time.mp4") < 0)
        verify(capturedJson.indexOf("key_bytes") < 0)
        var mediaPositions = captured.positions.filter(function(position) {
            return position.kind === "media-time"
        })
        compare(mediaPositions.length, 2)
        verify(mediaPositions.some(function(position) { return position.paused === true }))
        verify(mediaPositions.some(function(position) { return position.paused === false }))
        var mediaCoverage = captured.coverage.filter(function(item) {
            return item.category === "media"
        })[0]
        verify(mediaCoverage)
        compare(mediaCoverage.captured, 2)
        verify(captured.exclusions.length >= 4)
        compare(evaluate(view, "window.__mediaCalls.play + window.__mediaCalls.pause"), 0)
        compare(evaluate(view, "window.__mediaCalls.userMedia + window.__mediaCalls.displayMedia"), 0)
        compare(evaluate(view, "document.getElementById('saved').__time"), 10)

        var savedCandidate = captured.candidates.filter(function(candidate) {
            return candidate.kind === "media"
                   && candidate.id_fingerprint !== null
                   && captured.positions.some(function(position) {
                       return position.candidate_id === candidate.candidate_id
                              && position.kind === "media-time"
                              && position.paused === true
                   })
        })[0]
        verify(savedCandidate)
        var restoreRequest = geometryRequest()
        restoreRequest.include_controls = false
        restoreRequest.include_media = true
        restoreRequest.restore_geometry = {
            top_x: null,
            top_y: null,
            targets: [],
            media_targets: [{
                candidate_id: savedCandidate.candidate_id,
                id_fingerprint: savedCandidate.id_fingerprint,
                name_fingerprint: savedCandidate.name_fingerprint,
                label_fingerprint: savedCandidate.label_fingerprint,
                context_fingerprint: savedCandidate.context_fingerprint,
                source_fingerprint: "hmac-sha256-v1-k1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                expected_source_url: "https://example.test/saved.mp4",
                time_seconds: 35
            }]
        }
        var restored = restore(view, restoreRequest)
        compare(restored.ok, true)
        compare(restored.media_applied, 1)
        compare(restored.media_skipped, 0)
        compare(evaluate(view, "document.getElementById('saved').__time"), 35)
        compare(evaluate(view, "window.__mediaCalls.play + window.__mediaCalls.pause"), 0)
        compare(evaluate(view, "window.__mediaCalls.userMedia + window.__mediaCalls.displayMedia"), 0)

        compare(evaluate(view,
            "var saved=document.getElementById('saved');"
            + "Object.defineProperty(saved,'paused',{configurable:true,value:false});"
            + "saved.__time=10;true"), true)
        var nowPlaying = restore(view, restoreRequest)
        compare(nowPlaying.media_applied, 0)
        compare(nowPlaying.media_skipped, 1)
        compare(evaluate(view, "document.getElementById('saved').__time"), 10)
        compare(evaluate(view, "window.__mediaCalls.play + window.__mediaCalls.pause"), 0)

        // A source can change after candidate collection but before the seek.
        // The final check must reject the new file even when DOM identity matches.
        compare(evaluate(view,
            "var saved=document.getElementById('saved');"
            + "Object.defineProperty(saved,'paused',{configurable:true,value:true});"
            + "saved.__time=10;saved.__sourceReads=0;"
            + "Object.defineProperty(saved,'currentSrc',{configurable:true,get:function(){"
            + "this.__sourceReads++;return this.__sourceReads===1"
            + "?'https://example.test/saved.mp4':'https://example.test/replaced.mp4';}});true"), true)
        var changedDuringRestore = restore(view, restoreRequest)
        compare(changedDuringRestore.media_applied, 0)
        compare(changedDuringRestore.media_skipped, 1)
        verify(evaluate(view, "document.getElementById('saved').__sourceReads") >= 2)
        compare(evaluate(view, "document.getElementById('saved').__time"), 10)
        compare(evaluate(view, "window.__mediaCalls.play + window.__mediaCalls.pause"), 0)

        // Replacing the actual media node during live observation must not
        // transfer the old candidate's position to the new element.
        compare(evaluate(view,
            "var saved=document.getElementById('saved');"
            + "Object.defineProperty(saved,'currentSrc',{configurable:true,get:function(){"
            + "var parent=this.parentNode;var replacement=document.createElement('video');"
            + "replacement.id='saved';replacement.style.width='320px';replacement.style.height='180px';replacement.__time=10;"
            + "Object.defineProperty(replacement,'currentTime',{configurable:true,get:function(){return this.__time;},set:function(v){this.__time=v;this.__seekCalls=(this.__seekCalls||0)+1;}});"
            + "Object.defineProperty(replacement,'duration',{configurable:true,value:120});"
            + "Object.defineProperty(replacement,'paused',{configurable:true,value:true});"
            + "Object.defineProperty(replacement,'readyState',{configurable:true,value:1});"
            + "Object.defineProperty(replacement,'currentSrc',{configurable:true,value:'https://example.test/saved.mp4'});"
            + "Object.defineProperty(replacement,'mediaKeys',{configurable:true,value:null});"
            + "Object.defineProperty(replacement,'srcObject',{configurable:true,value:null});"
            + "parent.replaceChild(replacement,this);window.__replacedMedia=replacement;"
            + "return 'https://example.test/saved.mp4';}});true"), true)
        var replacedElement = restore(view, restoreRequest)
        compare(replacedElement.media_applied, 0)
        verify(replacedElement.media_skipped >= 1)
        compare(evaluate(view, "window.__replacedMedia.__time"), 10)
        compare(evaluate(view, "window.__replacedMedia.__seekCalls || 0"), 0)
        compare(evaluate(view, "window.__mediaCalls.play + window.__mediaCalls.pause"), 0)
        compare(evaluate(view, "window.__mediaCalls.userMedia + window.__mediaCalls.displayMedia"), 0)
    }

    function test_editable_restore_only_fills_unique_blank_native_properties_without_events() {
        var html = "<!doctype html><html><body>"
                 + "<input id='blank' value='' style='width:200px'>"
                 + "<input id='refused' value='' style='width:200px'>"
                 + "<input id='occupied' value='live value' style='width:200px'>"
                 + "<input id='check' type='checkbox'>"
                 + "</body></html>"
        var view = loadGeometryView(html)
        compare(evaluate(view,
            "window.__formEvents=0;"
            + "var countFormEvent=function(){window.__formEvents++;};"
            + "document.addEventListener('input',countFormEvent,true);"
            + "document.addEventListener('change',countFormEvent,true);"
            + "document.addEventListener('focus',countFormEvent,true);"
            + "document.addEventListener('blur',countFormEvent,true);"
            + "document.addEventListener('submit',countFormEvent,true);"
            + "window.__attemptedRefill='';"
            + "Object.defineProperty(document.getElementById('refused'),'value',"
            + "{configurable:true,get:function(){return '';},set:function(value){window.__attemptedRefill=String(value);}});true"), true)
        var captureRequest = geometryRequest()
        captureRequest.include_editable = true
        var observation = collect(view, captureRequest)
        var candidates = observation.candidates
        var blank = candidates.filter(function(candidate) {
            return candidate.kind === "text"
                   && observation.sensitive_editable.some(function(entry) {
                       return entry.candidate_id === candidate.candidate_id
                              && entry.value.value === ""
                   })
        })[0]
        var occupied = candidates.filter(function(candidate) {
            return candidate.kind === "text"
                   && observation.sensitive_editable.some(function(entry) {
                       return entry.candidate_id === candidate.candidate_id
                              && entry.value.value === "live value"
                   })
        })[0]
        var refused = candidates.filter(function(candidate) {
            return candidate.kind === "text"
                   && observation.sensitive_editable.some(function(entry) {
                       return entry.candidate_id === candidate.candidate_id
                              && entry.value.value === ""
                   })
                   && candidate.id_fingerprint !== blank.id_fingerprint
                   && candidate.id_fingerprint !== occupied.id_fingerprint
        })[0]
        var check = candidates.filter(function(candidate) {
            return candidate.kind === "checkbox"
        })[0]
        verify(blank)
        verify(refused)
        verify(occupied)
        verify(check)
        function editableTarget(candidate, kind, value) {
            return {
                candidate_id: candidate.candidate_id,
                id_fingerprint: candidate.id_fingerprint,
                name_fingerprint: candidate.name_fingerprint,
                label_fingerprint: candidate.label_fingerprint,
                context_fingerprint: candidate.context_fingerprint,
                kind: kind,
                value: value
            }
        }
        var restoreRequest = geometryRequest()
        restoreRequest.include_controls = false
        restoreRequest.include_editable = true
        restoreRequest.restore_geometry = {
            top_x: null,
            top_y: null,
            targets: [],
            media_targets: [],
            editable_targets: [
                editableTarget(blank, "text", {kind: "text", value: "saved draft"}),
                editableTarget(refused, "text", {kind: "text", value: "rejected draft"}),
                editableTarget(occupied, "text", {kind: "text", value: "saved replacement"}),
                editableTarget(check, "checkbox", {kind: "checked", value: true})
            ]
        }
        var result = restore(view, restoreRequest)
        compare(result.ok, true)
        compare(result.editable_applied, 2)
        compare(result.editable_skipped, 2)
        compare(result.editable_unaccepted, 1)
        compare(evaluate(view, "document.getElementById('blank').value"), "saved draft")
        compare(evaluate(view, "document.getElementById('refused').value"), "")
        compare(evaluate(view, "window.__attemptedRefill"), "rejected draft")
        compare(evaluate(view, "document.getElementById('occupied').value"), "live value")
        compare(evaluate(view, "document.getElementById('check').checked"), true)
        // Native property assignment must not trigger page handlers or steal
        // focus, blur a control, or submit the form.
        compare(evaluate(view, "window.__formEvents"), 0)
    }

    function test_editable_restore_skips_duplicate_name_matches() {
        var html = "<!doctype html><html><body>"
                 + "<section><input id='first' name='shared-draft' value=''></section>"
                 + "<section><input id='second' name='shared-draft' value=''></section>"
                 + "</body></html>"
        var view = loadGeometryView(html)
        var captureRequest = geometryRequest()
        captureRequest.include_editable = true
        var observation = collect(view, captureRequest)
        var first = observation.candidates.filter(function(candidate) {
            return candidate.kind === "text"
                   && observation.sensitive_editable.some(function(entry) {
                       return entry.candidate_id === candidate.candidate_id
                              && entry.value.kind === "text"
                              && entry.value.value === ""
                   })
        })[0]
        verify(first)
        var second = observation.candidates.filter(function(candidate) {
            return candidate.kind === "text"
                   && candidate.candidate_id !== first.candidate_id
                   && observation.sensitive_editable.some(function(entry) {
                       return entry.candidate_id === candidate.candidate_id
                              && entry.value.kind === "text"
                              && entry.value.value === ""
                   })
        })[0]
        verify(second)
        compare(first.name_fingerprint, second.name_fingerprint)

        var request = geometryRequest()
        request.include_controls = false
        request.include_editable = true
        request.restore_geometry = {
            top_x: null,
            top_y: null,
            targets: [],
            media_targets: [],
            editable_targets: [{
                candidate_id: first.candidate_id,
                id_fingerprint: first.id_fingerprint,
                name_fingerprint: first.name_fingerprint,
                label_fingerprint: first.label_fingerprint,
                context_fingerprint: first.context_fingerprint,
                kind: "text",
                value: {kind: "text", value: "recovered draft"}
            }]
        }
        var result = restore(view, request)
        compare(result.ok, true)
        compare(result.editable_applied, 0)
        compare(result.editable_skipped, 1)
        compare(result.editable_unaccepted, 0)
        compare(evaluate(view, "document.getElementById('first').value"), "")
        compare(evaluate(view, "document.getElementById('second').value"), "")
    }

    function test_origin_route_and_nonce_request_boundary_fail_closed() {
        var view = loadGeometryView("<!doctype html><html><body>synthetic</body></html>")
        var request = geometryRequest()
        request.expected_origin = "https://foreign.test"
        compare(collect(view, request).error, "origin-changed")
        request = geometryRequest()
        request.expected_route = "/other"
        compare(collect(view, request).error, "route-changed")
        request = geometryRequest()
        request.nonce = "not-a-nonce"
        compare(collect(view, request).error, "invalid-request")
    }

    function test_shutdown_probe_never_calls_plaintext_editor_or_child_frame_clean() {
        var script = BrowserScripts.shutdownPageProbe()
        var ordinary = loadGeometryView("<!doctype html><html><body>"
                                        + "<input id='draft' value='original'>"
                                        + "</body></html>")
        compare(evaluate(ordinary, script), "clean")
        compare(evaluate(ordinary,
            "document.getElementById('draft').value='changed';true"), true)
        compare(evaluate(ordinary, script), "dirty")

        var editor = loadGeometryView("<!doctype html><html><body>"
                                      + "<div contenteditable='plaintext-only'>draft</div>"
                                      + "</body></html>")
        compare(evaluate(editor, script), "unknown")

        var frame = loadGeometryView("<!doctype html><html><body>"
                                     + "<iframe srcdoc=\"<input value='draft'>\"></iframe>"
                                     + "</body></html>")
        tryVerify(function() { return frame.mainFrame.children.length === 1 }, 10000)
        compare(evaluate(frame, script), "unknown")

        var shadow = loadGeometryView("<!doctype html><html><body>"
                                      + "<div id='shadow-host'></div>"
                                      + "</body></html>")
        compare(evaluate(shadow,
            "var root=document.getElementById('shadow-host')"
            + ".attachShadow({mode:'open'});"
            + "root.innerHTML='<input value=\"original\">';"
            + "root.querySelector('input').value='changed';true"), true)
        compare(evaluate(shadow, script), "unknown")

        var custom = loadGeometryView("<!doctype html><html><body>"
                                      + "<x-private-editor></x-private-editor>"
                                      + "</body></html>")
        compare(evaluate(custom, script), "unknown")

        var large = loadGeometryView("<!doctype html><html><body>"
                                     + new Array(4098).join("<span></span>")
                                     + "</body></html>")
        compare(evaluate(large, script), "unknown")
    }

    function test_frame_collection_denies_cross_origin_without_exposing_frame_url() {
        var html = "<!doctype html><html><body>synthetic top"
                 + "<iframe src='https://other.test/private-path?synthetic=1'></iframe>"
                 + "</body></html>"
        var view = loadGeometryView(html)
        tryVerify(function() { return view.mainFrame.children.length === 1 }, 10000)
        var result = collectFrames(view, geometryRequest())
        compare(result.schema_version, 1)
        compare(result.frames.length, 1)
        compare(result.frames[0].id, 0)
        verify(result.exclusions.some(function(item) { return item.reason === "cross-origin" }))
        var encoded = JSON.stringify(result)
        verify(encoded.indexOf("other.test") < 0)
        verify(encoded.indexOf("private-path") < 0)
        console.log("FERRIC_FRAME_RESULT:" + encoded)
    }

    function test_frame_collector_rejects_over_budget_renderer_results_before_merging() {
        var request = geometryRequest()
        var baseResult = {
            schema_version: 1,
            helper_version: "ferric-page-state-5",
            nonce: request.nonce,
            capture_fingerprint: request.capture_fingerprint,
            requested_categories: ["geometry"],
            nodes_visited: 1,
            frames: [{id: 0, parent: null, origin: request.expected_origin,
                      route_fingerprint: request.route_fingerprint}],
            candidates: [], positions: [], controls: [], coverage: [],
            exclusions: [], sensitive_editable: []
        }
        function collectMocked(rawResult) {
            var frame = {
                isValid: true,
                url: "https://example.test/geometry",
                children: [],
                calls: 0,
                runJavaScript: function(script, world, callback) {
                    this.calls++
                    callback(this.calls === 1 ? rawResult : "same")
                }
            }
            return collectFrames({url: frame.url, mainFrame: frame}, request)
        }

        var candidateOverflow = JSON.parse(JSON.stringify(baseResult))
        for (var index = 0; index < 65; index++) {
            candidateOverflow.candidates.push({})
        }
        compare(collectMocked(JSON.stringify(candidateOverflow)).error,
                "invalid-frame-result")
        compare(collectMocked(JSON.stringify(baseResult)
                              + new Array(262145).join(" ")).error,
                "result-too-large")

        var injectedFrame = JSON.parse(JSON.stringify(baseResult))
        injectedFrame.frames[0].native_command = "open-file"
        compare(collectMocked(JSON.stringify(injectedFrame)).error,
                "invalid-frame-result")

        var injectedCandidate = JSON.parse(JSON.stringify(baseResult))
        injectedCandidate.candidates.push({
            frame_id: 0, candidate_id: 1, kind: "text",
            id_fingerprint: null, name_fingerprint: null,
            label_fingerprint: null, context_fingerprint: request.route_fingerprint,
            source_fingerprint: null, url: "https://example.test/private-frame-url"
        })
        compare(collectMocked(JSON.stringify(injectedCandidate)).error,
                "invalid-frame-result")

        var injectedControl = JSON.parse(JSON.stringify(baseResult))
        injectedControl.controls.push({frame_id: 0, candidate_id: 1,
                                       value: {kind: "details", open: true,
                                               native_command: "open-file"}})
        compare(collectMocked(JSON.stringify(injectedControl)).error,
                "invalid-frame-result")

        var injectedEditable = JSON.parse(JSON.stringify(baseResult))
        injectedEditable.sensitive_editable.push({frame_id: 0, candidate_id: 1,
                                                   value: {kind: "text", value: "synthetic",
                                                           url: "https://example.test/private"}})
        compare(collectMocked(JSON.stringify(injectedEditable)).error,
                "invalid-frame-result")

        var injectedRoot = JSON.parse(JSON.stringify(baseResult))
        injectedRoot.native_command = "open-file"
        compare(collectMocked(JSON.stringify(injectedRoot)).error,
                "invalid-frame-result")
    }

    function test_frame_collector_excludes_child_with_unreviewed_metadata() {
        var request = geometryRequest()
        var childUrl = "https://example.test/private-child-frame"
        var rootResult = {
            schema_version: 1, helper_version: "ferric-page-state-5",
            nonce: request.nonce, capture_fingerprint: request.capture_fingerprint,
            requested_categories: ["geometry"], nodes_visited: 1,
            frames: [{id: 0, parent: null, origin: request.expected_origin,
                      route_fingerprint: request.route_fingerprint}],
            candidates: [], positions: [], controls: [], coverage: [],
            exclusions: [], sensitive_editable: []
        }
        var childResult = JSON.parse(JSON.stringify(rootResult))
        childResult.frames[0].id = 1
        childResult.frames[0].parent = 0
        childResult.frames[0].native_command = "open-file"
        var child = {
            isValid: true, url: childUrl, children: [], calls: 0,
            runJavaScript: function(script, world, callback) {
                this.calls++
                callback(this.calls === 1 ? JSON.stringify(childResult) : "same")
            }
        }
        var root = {
            isValid: true, url: "https://example.test/geometry", children: [child], calls: 0,
            runJavaScript: function(script, world, callback) {
                this.calls++
                callback(this.calls === 1 ? JSON.stringify(rootResult) : "same")
            }
        }
        var result = collectFrames({url: root.url, mainFrame: root}, request)
        compare(result.frames.length, 1)
        verify(result.exclusions.some(function(item) { return item.reason === "unsupported" }))
        var encoded = JSON.stringify(result)
        verify(encoded.indexOf("native_command") < 0)
        verify(encoded.indexOf("open-file") < 0)
        verify(encoded.indexOf("private-child-frame") < 0)
    }

    function test_same_origin_child_is_admitted_and_cross_origin_sibling_is_categorical() {
        if (sameOriginFrameFixtureUrl.indexOf("http://127.0.0.1:") !== 0) {
            skip("requires the xtask-injected live loopback frame fixture")
        }
        var view = loadSameOriginFrameFixture()
        var request = sameOriginFrameRequest()
        var result = collectFrames(view, request)
        compare(result.schema_version, 1)
        compare(result.frames.length, 2)
        compare(result.frames[0].id, 0)
        compare(result.frames[0].parent, null)
        compare(result.frames[0].origin, request.expected_origin)
        compare(result.frames[1].id, 1)
        compare(result.frames[1].parent, 0)
        compare(result.frames[1].origin, request.expected_origin)
        verify(result.exclusions.some(function(item) {
            return item.reason === "cross-origin"
        }))
        verify(result.positions.filter(function(position) {
            return position.kind === "top-scroll-x" || position.kind === "top-scroll-y"
        }).every(function(position) { return position.frame_id === 0 }),
            "only the top document contributes restorable page scroll coordinates")
        var encoded = JSON.stringify(result)
        verify(encoded.indexOf("Synthetic child frame") < 0)
        verify(encoded.indexOf("/versioning/frame-child") < 0)
        console.log("FERRIC_FRAME_TREE_RESULT:" + encoded)
    }

    function test_post_result_collects_under_current_route_without_replay_proof() {
        if (sameOriginFrameFixtureUrl.indexOf("http://127.0.0.1:") !== 0) {
            skip("requires the xtask-injected live loopback frame fixture")
        }
        var origin = sameOriginFrameFixtureUrl.replace(/\/versioning\/frames$/, "")
        var view = createTemporaryObject(viewFactory, testWindow.contentItem)
        verify(view)
        view.url = origin + "/versioning/form"
        tryVerify(function() { return view.loaded || view.loadFailure.length > 0 }, 15000)
        compare(view.loadFailure, "")
        view.loaded = false
        view.runJavaScript("document.querySelector('form').submit()",
                           WebEngineScript.MainWorld)
        tryVerify(function() { return view.loaded || view.loadFailure.length > 0 }, 15000)
        compare(view.loadFailure, "")
        compare(String(view.url), origin + "/versioning/form/result")
        var request = geometryRequest()
        request.expected_origin = origin
        request.expected_route = "/versioning/form/result"
        request.include_controls = true
        var result = collectFrames(view, request)
        compare(result.schema_version, 1)
        compare(result.frames.length, 1)
        compare(result.frames[0].origin, origin)
        compare(result.frames[0].route_fingerprint, request.route_fingerprint)
        console.log("FERRIC_POST_RESULT_SIDECAR:" + JSON.stringify(result))
    }

    function test_live_frame_depth_limit_reports_partial_coverage() {
        if (sameOriginFrameFixtureUrl.indexOf("http://127.0.0.1:") !== 0) {
            skip("requires the xtask-injected live loopback frame fixture")
        }
        var view = createTemporaryObject(viewFactory, testWindow.contentItem)
        verify(view)
        view.url = sameOriginFrameFixtureUrl.replace(
            /\/versioning\/frames$/, "/versioning/frame-depth?level=0")
        tryVerify(function() { return view.loaded || view.loadFailure.length > 0 }, 15000)
        compare(view.loadFailure, "")
        tryVerify(function() {
            var frame = view.mainFrame
            for (var level = 0; level < 9; level++) {
                if (!frame || !frame.isValid || !frame.children
                        || frame.children.length !== 1) return false
                frame = frame.children[0]
            }
            return frame && frame.isValid
        }, 15000)
        var request = sameOriginFrameRequest()
        request.expected_route = "/versioning/frame-depth?level=0"
        var result = collectFrames(view, request)
        compare(result.frames.length, 9)
        for (var index = 1; index < result.frames.length; index++) {
            compare(result.frames[index].parent, index - 1)
        }
        verify(result.exclusions.some(function(item) { return item.reason === "limit" }))
        verify(result.coverage.every(function(item) { return item.truncated }))
        verify(JSON.stringify(result).indexOf("frame-depth") < 0)
        console.log("FERRIC_FRAME_DEPTH_RESULT:" + JSON.stringify(result))
    }

    function test_live_branched_frame_tree_keeps_parent_relationships() {
        if (sameOriginFrameFixtureUrl.indexOf("http://127.0.0.1:") !== 0) {
            skip("requires the xtask-injected live loopback frame fixture")
        }
        var origin = sameOriginFrameFixtureUrl.replace(/\/versioning\/frames$/, "")
        var view = createTemporaryObject(viewFactory, testWindow.contentItem)
        verify(view)
        view.url = origin + "/versioning/frame-branch"
        tryVerify(function() { return view.loaded || view.loadFailure.length > 0 }, 15000)
        compare(view.loadFailure, "")
        tryVerify(function() {
            var root = view.mainFrame
            if (!root || !root.isValid || !root.children || root.children.length !== 3)
                return false
            var left = null, right = null
            for (var index = 0; index < root.children.length; index++) {
                var child = root.children[index]
                var url = String(child.url)
                if (url.indexOf("/versioning/frame-branch-left") >= 0) left = child
                if (url.indexOf("/versioning/frame-branch-right") >= 0) right = child
            }
            return left && right && left.isValid && right.isValid
                && left.children && left.children.length === 2
                && right.children && right.children.length === 1
                && left.children[0].isValid && left.children[1].isValid
                && right.children[0].isValid
        }, 15000)
        var request = sameOriginFrameRequest()
        request.expected_route = "/versioning/frame-branch"
        var result = collectFrames(view, request)
        compare(result.schema_version, 1)
        compare(result.frames.length, 6)
        compare(result.frames.filter(function(frame) { return frame.parent === 0 }).length, 2)
        compare(result.frames.filter(function(frame) { return frame.parent !== null
            && frame.parent !== 0 }).length, 3)
        verify(result.exclusions.some(function(item) { return item.reason === "cross-origin" }))
        verify(JSON.stringify(result).indexOf("frame-branch") < 0)
        console.log("FERRIC_FRAME_BRANCH_RESULT:" + JSON.stringify(result))
    }

    function test_frame_removal_between_capture_and_probe_is_never_reported_stable() {
        if (sameOriginFrameFixtureUrl.indexOf("http://127.0.0.1:") !== 0) {
            skip("requires the xtask-injected live loopback frame fixture")
        }
        var view = loadSameOriginFrameFixture()
        var capture = collectFramesWithMutationProbe(view, sameOriginFrameRequest())
        compare(capture.sidecar.frames.length, 2)
        compare(evaluate(view, "document.querySelector('#same').remove();true"), true)
        verify(finishMutationProbe(capture) !== "observed-stable",
               "removing a captured frame must invalidate stable-activity evidence")
    }

    function test_frame_navigation_between_capture_and_probe_is_document_changed() {
        if (sameOriginFrameFixtureUrl.indexOf("http://127.0.0.1:") !== 0) {
            skip("requires the xtask-injected live loopback frame fixture")
        }
        var view = loadSameOriginFrameFixture()
        var capture = collectFramesWithMutationProbe(view, sameOriginFrameRequest())
        compare(capture.sidecar.frames.length, 2)
        compare(evaluate(view,
            "document.querySelector('#same').src="
            + "'/versioning/frame-child?navigated=1';true"), true)
        tryVerify(function() {
            if (!view.mainFrame || !view.mainFrame.children) return false
            for (var index = 0; index < view.mainFrame.children.length; index++) {
                if (String(view.mainFrame.children[index].url)
                        .indexOf("/versioning/frame-child?navigated=1") >= 0) return true
            }
            return false
        }, 10000)
        compare(finishMutationProbe(capture), "document-changed")
    }

    function test_frame_reordering_between_capture_and_probe_is_never_reported_stable() {
        if (sameOriginFrameFixtureUrl.indexOf("http://127.0.0.1:") !== 0) {
            skip("requires the xtask-injected live loopback frame fixture")
        }
        var view = loadSameOriginFrameFixture()
        var capture = collectFramesWithMutationProbe(view, sameOriginFrameRequest())
        compare(capture.sidecar.frames.length, 2)
        compare(evaluate(view,
            "document.querySelector('#frames').prepend("
            + "document.querySelector('#cross'));true"), true)
        verify(finishMutationProbe(capture) !== "observed-stable",
               "reordering the captured frame tree must not preserve stable evidence")
    }

    function test_frame_collection_is_bounded_and_reports_opaque_children() {
        var html = "<!doctype html><html><body>synthetic top"
                 + "<iframe srcdoc='<p>synthetic opaque document</p>'></iframe>"
                 + "<iframe src='https://other.test/second'></iframe>"
                 + "</body></html>"
        var view = loadGeometryView(html)
        tryVerify(function() { return view.mainFrame.children.length === 2 }, 10000)
        var request = geometryRequest()
        request.max_frames = 1
        var result = collectFrames(view, request)
        compare(result.frames.length, 1)
        verify(result.exclusions.some(function(item) {
            return item.reason === "opaque-origin" || item.reason === "limit"
        }))
        verify(result.coverage.every(function(item) { return item.truncated }))
        verify(JSON.stringify(result).indexOf("synthetic opaque document") < 0)
    }

    function test_frame_mutation_probe_reports_change_without_returning_page_content() {
        var view = loadGeometryView("<!doctype html><html><body>initial</body></html>")
        var capture = collectFramesWithMutationProbe(view, geometryRequest())
        verify(capture.sidecar.ok !== false)
        compare(capture.sidecar.helper_version, "ferric-page-state-5")
        verify(capture.probe)
        compare(evaluate(view,
            "document.body.appendChild(document.createTextNode('private mutation text'));true"), true)
        var observed = {done: false, value: ""}
        capture.probe(function(value) {
            observed.value = String(value)
            observed.done = true
        })
        tryVerify(function() { return observed.done }, 10000)
        compare(observed.value, "observed-changed")
        verify(JSON.stringify(capture.sidecar).indexOf("private mutation text") < 0)
    }

    function test_frame_mutation_probe_distinguishes_no_observation_from_stability() {
        var view = loadGeometryView("<!doctype html><html><body>unchanged</body></html>")
        var capture = collectFramesWithMutationProbe(view, geometryRequest())
        verify(capture.sidecar.ok !== false)
        verify(capture.probe)
        var observed = {done: false, value: ""}
        capture.probe(function(value) {
            observed.value = String(value)
            observed.done = true
        })
        tryVerify(function() { return observed.done }, 10000)
        compare(observed.value, "observed-stable")
    }

    function test_frame_mutation_probe_rejects_same_url_document_replacement() {
        var view = loadGeometryView("<!doctype html><html><body>same route</body></html>")
        var capture = collectFramesWithMutationProbe(view, geometryRequest())
        verify(capture.sidecar.ok !== false)
        verify(capture.probe)
        view.loaded = false
        view.reload()
        tryVerify(function() { return view.loaded || view.loadFailure.length > 0 }, 15000)
        compare(view.loadFailure, "")
        var observed = {done: false, value: ""}
        capture.probe(function(value) {
            observed.value = String(value)
            observed.done = true
        })
        tryVerify(function() { return observed.done }, 10000)
        compare(observed.value, "document-changed")
    }

    function test_ambiguous_selected_options_are_excluded_not_replayed() {
        var html = "<!doctype html><html><body><select id='duplicate' multiple>"
                 + "<option value='same' selected>one</option>"
                 + "<option value='same' selected>two</option></select></body></html>"
        var view = loadGeometryView(html)
        var request = geometryRequest()
        request.include_controls = true
        var result = collect(view, request)
        compare(result.controls.length, 0)
        verify(result.exclusions.some(function(exclusion) {
            return exclusion.reason === "unsupported"
        }))
    }

    function test_geometry_restore_applies_only_unique_live_scroll_area() {
        var html = "<!doctype html><html><body>"
                 + "<div id='nested' style='height:100px;width:150px;overflow:auto'>"
                 + "<div style='height:1000px;width:600px'>synthetic</div></div>"
                 + "</body></html>"
        var view = loadGeometryView(html)
        var capture = collect(view, geometryRequest())
        compare(capture.candidates.length, 1)
        var request = geometryRestoreRequest(capture, 35, 75)
        var result = restore(view, request)
        compare(result.ok, true)
        compare(result.applied, 1)
        verify(Math.abs(evaluate(view, "document.getElementById('nested').scrollTop") - 75) < 1)
        verify(Math.abs(evaluate(view, "document.getElementById('nested').scrollLeft") - 35) < 1)
        compare(evaluate(view, BrowserScripts.scrollTargetStatus()).policy, "auto")
    }

    function test_geometry_restore_matches_descriptor_after_candidate_order_changes() {
        var html = "<!doctype html><html><body>"
                 + "<section id='earlier'></section>"
                 + "<section><div id='target' style='height:100px;overflow:auto'>"
                 + "<div style='height:1000px'>synthetic</div></div></section>"
                 + "</body></html>"
        var view = loadGeometryView(html)
        var capture = collect(view, geometryRequest())
        compare(capture.candidates.length, 1)
        var request = geometryRestoreRequest(capture, null, 80)
        compare(evaluate(view,
            "var earlier=document.getElementById('earlier');"
            + "var inserted=document.createElement('div');"
            + "inserted.id='inserted';inserted.style.cssText='height:100px;overflow:auto';"
            + "inserted.innerHTML='<div style=\"height:1000px\">synthetic</div>';"
            + "earlier.appendChild(inserted);true"), true)
        var current = collect(view, geometryRequest())
        compare(current.candidates.length, 2)
        compare(current.candidates[1].id_fingerprint,
                capture.candidates[0].id_fingerprint)
        verify(current.candidates[1].candidate_id
               !== request.restore_geometry.targets[0].candidate_id)
        var result = restore(view, request)
        compare(result.ok, true)
        compare(result.applied, 1)
        verify(Math.abs(evaluate(view, "document.getElementById('target').scrollTop") - 80) < 1)
        compare(evaluate(view, "document.getElementById('inserted').scrollTop"), 0)

        var repeated = Object.assign({}, request.restore_geometry.targets[0],
                                     {candidate_id: 2})
        request.restore_geometry.targets.push(repeated)
        compare(restore(view, request).error, "invalid-request")
    }

    function test_geometry_restore_skips_duplicate_identity_and_changed_route() {
        var html = "<!doctype html><html><body>"
                 + "<div id='nested' style='height:100px;overflow:auto'>"
                 + "<div style='height:1000px'>synthetic</div></div>"
                 + "</body></html>"
        var view = loadGeometryView(html)
        var capture = collect(view, geometryRequest())
        var request = geometryRestoreRequest(capture, null, 80)
        compare(evaluate(view,
            "var copy=document.getElementById('nested').cloneNode(true);"
            + "document.body.appendChild(copy);true"), true)
        var ambiguous = restore(view, request)
        compare(ambiguous.applied, 0)
        compare(ambiguous.skipped, 1)
        compare(evaluate(view, "document.getElementById('nested').scrollTop"), 0)
        request.expected_route = "/other"
        compare(restore(view, request).error, "route-changed")
    }

    function test_geometry_restore_rejects_unbounded_or_changed_targets() {
        var html = "<!doctype html><html><body>"
                 + "<div id='nested' style='height:100px;overflow:auto'>"
                 + "<div style='height:1000px'>synthetic</div></div>"
                 + "</body></html>"
        var view = loadGeometryView(html)
        var capture = collect(view, geometryRequest())
        var request = geometryRestoreRequest(capture, null, 1000000000)
        var impossible = restore(view, request)
        compare(impossible.applied, 0)
        compare(impossible.skipped, 1)
        request = geometryRestoreRequest(capture, null, 50)
        request.max_nodes = 1
        compare(restore(view, request).error, "limit")
        request = geometryRestoreRequest(capture, null, 50)
        request.restore_geometry.targets[0].context_fingerprint = "f".repeat(64)
        compare(restore(view, request).applied, 0)
    }
}

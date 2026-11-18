import QtQuick
import QtTest
import QtWebEngine
import io.github.ferricbrowser 1.0
import Ferric.Testing 1.0

TestCase {
    id: tests
    name: "NativeDocumentReceipts"
    when: windowShown
    width: 1000
    height: 800
    property bool done: false
    property var reply
    property string owner: ""
    property bool blockOnProbe: false
    property bool blockedProbe: false
    FerricHintFrameBroker { id: broker }
    FrameReceiptProbe { id: probe }
    WebEngineView {
        id: page
        anchors.fill: parent
        property bool loaded: false
        onLoadingChanged: function(request) { loaded = request.status === WebEngineView.LoadSucceededStatus }
        onJavaScriptConsoleMessage: function(level,message,lineNumber,sourceID) {
            if (tests.blockOnProbe && message === "ferric-fixture-late-receipt") {
                tests.blockOnProbe = false
                tests.blockedProbe = true
                probe.blockGui(1100)
            }
        }
    }
    function evaluate(script, world) {
        done = false
        page.runJavaScript(script,world === undefined ? WebEngineScript.MainWorld : world,function(value) { reply=value;done=true })
        tryVerify(function() { return done },10000)
        return reply
    }
    function attach() {
        done = false
        broker.attach(page.mainFrame,function(value) { reply=value;done=true })
        tryVerify(function() { return done },10000)
        verify(!reply.error,JSON.stringify(reply))
        owner = reply.owner
    }
    function capture(selected) {
        done = false
        probe.capture(broker,selected || owner,function(value) { reply=value;done=true })
        tryVerify(function() { return done },10000)
        return reply
    }
    function validate() {
        done = false
        probe.validate(broker,function(value) { reply=value;done=true })
        tryVerify(function() { return done },10000)
        return reply
    }
    function focusedChild(focus) {
        var found = ""
        for (var attempt=0;attempt<30;attempt++) {
            evaluate(focus)
            wait(20)
            done = false
            broker.focusedChild(owner,function(value) { reply=value;done=true })
            tryVerify(function() { return done },10000)
            if (reply && !reply.error) { found=reply.owner;break }
        }
        verify(found.length > 0,JSON.stringify(reply))
        return found
    }
    function init() {
        page.loaded = false
        page.url = "http://127.0.0.1:8766/roots.html"
        tryVerify(function() { return page.loaded },10000)
        page.forceActiveFocus()
        attach()
    }
    function cleanup() { blockOnProbe=false;broker.clear();probe.reset() }
    function test_current_main_document_has_a_native_origin_and_epoch_receipt() {
        var value = capture()
        verify(value.ok,JSON.stringify(value))
        compare(value.documents.length,1)
        compare(value.documents[0].origin,"http://127.0.0.1:8766")
        compare(value.documents[0].owner,owner)
        compare(value.documents[0].parent,"")
        compare(value.documents[0].main,true)
        verify(/^[a-f0-9-]{36}$/.test(value.documents[0].epoch))
        verify(probe.matchesFrame(page.mainFrame))
        compare(validate(),true)
    }
    function test_main_world_cannot_supply_the_origin_or_focus_receipt() {
        var focused = evaluate("document.hasFocus()",WebEngineScript.ApplicationWorld)
        evaluate("window.origin='https://spoof.test';document.hasFocus=function(){return "+(!focused)+"};true")
        compare(evaluate("window.origin"),"https://spoof.test")
        var value = capture()
        verify(value.ok,JSON.stringify(value))
        compare(value.documents[0].origin,"http://127.0.0.1:8766")
        compare(value.documents[0].focused,focused)
    }
    function test_srcdoc_uses_effective_origin_and_exact_parent_chain() {
        var child = focusedChild("document.getElementById('same-origin').contentDocument.querySelector('a').focus();true")
        var value = capture(child)
        verify(value.ok,JSON.stringify(value))
        compare(value.documents.length,2)
        compare(value.documents[1].parent,owner)
        compare(value.documents[1].owner,child)
        compare(value.documents[1].origin,value.documents[0].origin)
        compare(value.documents[1].url,"about:srcdoc")
        compare(value.documents[1].focused,true)
        compare(validate(),true)
        // The ancestor native handle remains reachable, but its document token
        // no longer qualifies this old child receipt.
        evaluate("window.__ferric_hintFrameDocument='changed-parent'",WebEngineScript.ApplicationWorld)
        compare(validate(),false)
    }
    function test_foreign_and_sandbox_origins_are_observed_not_inferred_data() {
        return [{tag:"foreign",origin:"http://localhost:8766",sandbox:false},
                {tag:"opaque",origin:"null",sandbox:true}]
    }
    function test_foreign_and_sandbox_origins_are_observed_not_inferred(data) {
        var source = (data.sandbox ? "http://127.0.0.1:8766" : data.origin)+"/native-receipt-child.html"
        evaluate("const frame=document.createElement('iframe');frame.id='subject';"+(data.sandbox ? "frame.sandbox='allow-scripts';" : "")+"frame.src="+JSON.stringify(source)+";document.body.append(frame);true")
        var child = focusedChild("document.getElementById('subject').contentWindow.postMessage('receipt-focus','*');true")
        var value = capture(child)
        verify(value.ok,JSON.stringify(value))
        compare(value.documents.length,2)
        compare(value.documents[1].origin,data.origin)
        verify(value.documents[1].url.indexOf("http:") === 0)
        compare(value.documents[1].focused,true)
        // Capturing an opaque fact grants no focused receiver authority.
        compare(validate(),true)
    }
    function test_terminal_leaf_requires_complete_child_focus_data() {
        return [{tag:"unfocused",result:"false",accepted:true},
                {tag:"malformed",result:"undefined",accepted:false},
                {tag:"closed-focus-moved-after-reply",closed:true,result:"(()=>{document.querySelector('a').focus();return false})()",accepted:false},
                {tag:"focus-moved-after-reply",result:"(()=>{document.querySelector('a').focus();return false})()",accepted:false},
                {tag:"timed-out",result:"(()=>{const until=Date.now()+1300;while(Date.now()<until){}return false})()",accepted:false}]
    }
    function test_terminal_leaf_requires_complete_child_focus(data) {
        if (data.closed) evaluate("(()=>{const frame=document.getElementById('same-origin');const host=document.createElement('div');document.body.append(host);host.attachShadow({mode:'closed'}).append(frame);window.fixtureClosedFrame=frame;return true})()")
        var frame = data.closed ? "window.fixtureClosedFrame" : "document.getElementById('same-origin')"
        var child = focusedChild(frame+".contentDocument.querySelector('a').focus();true")
        done = false
        broker.run(child,"(()=>{document.hasFocus=function(){return "+data.result+"};return true})()",function(value) { reply=value;done=true })
        tryVerify(function() { return done },10000)
        compare(reply,true)
        compare(evaluate("const anchor=document.createElement('a');anchor.href='#parent';document.body.append(anchor);anchor.focus();document.hasFocus()"),true)
        var leases = broker.leaseCount
        done = false
        broker.verifyFocusedLeaf(owner,function(value) { reply=value;done=true })
        tryVerify(function() { return done },10000)
        if (data.accepted) compare(reply,true)
        else verify(!!reply.error,JSON.stringify(reply))
        compare(broker.leaseCount,leases)
        if (data.tag === "timed-out") wait(400)
    }
    function test_terminal_leaf_bounds_exposed_shadow_focus_data() {
        return [{tag:"at-depth",depth:8,accepted:true},{tag:"over-depth",depth:9,accepted:false}]
    }
    function test_terminal_leaf_bounds_exposed_shadow_focus(data) {
        compare(evaluate("(()=>{let root=document.body;for(let i=0;i<"+data.depth+";i++){const host=document.createElement('div');root.append(host);root=host.attachShadow({mode:'open'});}const input=document.createElement('input');root.append(input);input.focus();return document.hasFocus()})()"),true)
        done = false
        broker.verifyFocusedLeaf(owner,function(value) { reply=value;done=true })
        tryVerify(function() { return done },10000)
        if (data.accepted) compare(reply,true)
        else verify(!!reply.error,JSON.stringify(reply))
    }
    function test_terminal_leaf_preserves_unknown_shadow_capable_focus_data() {
        return [{tag:"body-with-children",element:"body",accepted:false},
                {tag:"div-with-children",element:"div",accepted:false},
                {tag:"span-with-children",element:"span",accepted:false},
                {tag:"custom-with-children",element:"fixture-host",accepted:false},
                {tag:"button-with-children",element:"button",accepted:true},
                {tag:"input-with-children",element:"input",accepted:true},
                {tag:"link-with-children",element:"a",accepted:true},
                {tag:"body-without-children",element:"body",empty:true,accepted:true}]
    }
    function test_terminal_leaf_preserves_unknown_shadow_capable_focus(data) {
        if (data.empty) {
            page.loaded = false
            page.url = "http://127.0.0.1:8766/roots-empty-child.html"
            tryVerify(function() { return page.loaded },10000)
            page.forceActiveFocus()
            broker.clear()
            attach()
        }
        compare(evaluate("(()=>{const element="+(data.element === "body" ? "document.body" : "document.createElement("+JSON.stringify(data.element)+")")+";element.tabIndex=0;if(element!==document.body)document.body.append(element);element.focus();return document.activeElement===element&&document.hasFocus()})()"),true)
        var leases = broker.leaseCount
        done = false
        broker.verifyFocusedLeaf(owner,function(value) { reply=value;done=true })
        tryVerify(function() { return done },10000)
        if (data.accepted) compare(reply,true)
        else compare(reply.error,"frame-focus-unavailable",JSON.stringify(reply))
        compare(broker.leaseCount,leases)
    }
    function test_terminal_leaf_rejects_stale_parent_document() {
        evaluate("window.__ferric_hintFrameDocument='replaced'",WebEngineScript.ApplicationWorld)
        done = false
        broker.verifyFocusedLeaf(owner,function(value) { reply=value;done=true })
        tryVerify(function() { return done },10000)
        verify(!!reply.error,JSON.stringify(reply))
    }
    function test_same_url_reload_invalidates_the_original_document_receipt() {
        verify(capture().ok)
        var url = page.url.toString()
        page.loaded = false
        page.reload()
        tryVerify(function() { return page.loaded },10000)
        compare(page.url.toString(),url)
        verify(probe.matchesFrame(page.mainFrame))
        compare(validate(),false)
        attach()
        verify(capture().ok)
    }
    function test_clear_invalidates_the_retained_receipt() {
        verify(capture().ok)
        broker.clear()
        compare(validate(),false)
    }
    function test_capture_absolute_deadline_survives_gui_event_delay() {
        evaluate("(()=>{const origin=window.origin;let reads=0;Object.defineProperty(window,'origin',{configurable:true,get:function(){if(++reads===2)console.info('ferric-fixture-late-receipt');return origin;}});return true})()",WebEngineScript.ApplicationWorld)
        blockOnProbe = true
        blockedProbe = false
        var value = capture()
        compare(blockedProbe,true)
        compare(value.ok,false,JSON.stringify(value))
    }
}

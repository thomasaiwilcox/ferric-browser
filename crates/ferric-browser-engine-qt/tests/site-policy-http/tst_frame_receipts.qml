import QtQuick
import QtTest
import QtWebEngine
import io.github.ferricbrowser 1.0
import Ferric.Testing 1.0
import "../../qml/scripts/BrowserScripts.js" as Scripts

// Shipping broker and real isolated-world execution. Token mutation tests the
// receipt boundary deterministically; the navigation test uses a new document.
TestCase {
    id: tests
    name: "PolicyFrameReceipts"
    when: windowShown
    width: 1000
    height: 800
    property bool done: false
    property int completionCount: 0
    property var reply
    property string owner: ""
    property bool clearOnProbe: false
    property bool sawProbe: false
    property bool completedBeforeProbe: false
    property bool blockOnProbe: false
    property bool blockedProbe: false
    FerricHintFrameBroker { id: broker }
    FrameReceiptProbe { id: nativeProbe }
    Component { id: temporaryBroker; FerricHintFrameBroker {} }
    WebEngineView {
        id: page
        anchors.fill: parent
        property bool loaded: false
        onLoadingChanged: function(request) { loaded = request.status === WebEngineView.LoadSucceededStatus }
        onJavaScriptConsoleMessage: function(level,message,lineNumber,sourceID) {
            if (tests.blockOnProbe && message === "ferric-fixture-late-post-probe") {
                tests.blockOnProbe = false
                tests.blockedProbe = true
                nativeProbe.blockGui(1100)
            }
            if (tests.clearOnProbe && message === "ferric-fixture-post-probe") {
                tests.completedBeforeProbe = tests.done
                tests.sawProbe = true
                tests.clearOnProbe = false
                broker.clear()
            }
        }
    }
    function evaluate(script, world) {
        done = false
        page.runJavaScript(script,world,function(value) { reply=value;done=true })
        tryVerify(function() { return done },10000)
        return reply
    }
    function load(suffix) {
        page.loaded = false
        page.url = "http://127.0.0.1:8766/roots.html"+(suffix || "")
        tryVerify(function() { return page.loaded },10000)
        page.forceActiveFocus()
    }
    function attach() {
        done = false
        broker.attach(page.mainFrame,function(value) { reply=value;done=true })
        tryVerify(function() { return done },10000)
        verify(!reply.error,JSON.stringify(reply))
        owner = reply.owner
    }
    function run(script, dispatch, selectedOwner) {
        done = false
        completionCount = 0
        var complete = function(value) { completionCount++;reply=value;done=true }
        if (dispatch) broker.dispatch(selectedOwner || owner,script,complete)
        else broker.run(selectedOwner || owner,script,complete)
        tryVerify(function() { return done },10000)
        return reply
    }
    function init() { load();attach() }
    function cleanup() {
        clearOnProbe = false
        blockOnProbe = false
        nativeProbe.reset()
        broker.clear()
        evaluate(Scripts.hintStopTracking(),WebEngineScript.ApplicationWorld)
    }
    function test_unchanged_document_still_returns_actual_collection() {
        var value = run(Scripts.hintCollector("links"))
        verify(!value.error,JSON.stringify(value))
        verify(value.candidates.length > 0)
    }
    function test_actual_collector_result_rejected_after_token_replacement() {
        var value = run("(()=>{const result=("+Scripts.hintCollector("links")+");window.__ferric_hintFrameDocument='replaced-after-collection';return result})()")
        compare(value.error,"stale-frame-document",JSON.stringify(value))
    }
    function test_only_boolean_true_dispatch_can_exempt_postcheck_data() {
        return [
            {tag:"run-true",dispatch:false,result:"true"},
            {tag:"dispatch-false",dispatch:true,result:"false"},
            {tag:"dispatch-one",dispatch:true,result:"1"},
            {tag:"dispatch-string",dispatch:true,result:"'true'"},
            {tag:"dispatch-object",dispatch:true,result:"({ok:true})"},
            {tag:"dispatch-true",dispatch:true,result:"true",acknowledged:true}
        ]
    }
    function test_only_boolean_true_dispatch_can_exempt_postcheck(data) {
        var value = run("(()=>{window.__ferric_hintFrameDocument='replaced';return "+data.result+"})()",data.dispatch)
        if (data.acknowledged) compare(value,true)
        else compare(value.error,"stale-frame-document",JSON.stringify(value))
    }
    function test_actual_navigation_retires_the_old_owner() {
        var old = owner
        load("?new-document")
        compare(run(Scripts.hintCollector("links"),false,old).error,"stale-frame-document")
        attach()
        verify(owner !== old)
        var value = run(Scripts.hintCollector("links"))
        verify(!value.error,JSON.stringify(value))
        verify(value.candidates.length > 0)
    }
    function test_frame_removal_without_native_ack_is_unavailable_data() {
        return [{tag:"js-true",result:"true"},{tag:"js-false",result:"false"}]
    }
    function test_frame_removal_without_native_ack_is_unavailable(data) {
        evaluate("document.getElementById('same-origin').contentWindow.document.querySelector('a').focus();true",WebEngineScript.MainWorld)
        tryVerify(function() { return evaluate("document.getElementById('same-origin').contentDocument.hasFocus()",WebEngineScript.MainWorld) },10000)
        done = false
        broker.focusedChild(owner,function(value) { reply=value;done=true })
        tryVerify(function() { return done },10000)
        verify(!reply.error,JSON.stringify(reply))
        var child = reply.owner
        done = false
        nativeProbe.capture(broker,child,function(value) { reply=value;done=true })
        tryVerify(function() { return done },10000)
        compare(reply.ok,true)
        var value = run("(()=>{window.frameElement.remove();return "+data.result+"})()",true,child)
        // On this SDK removing the frame loses its JS return at the native
        // boundary, even for `return true`. That is not a native Bool ack.
        // Require an unavailable result rather than manufacturing success.
        // DOM removal and native invalidation are asynchronous. Either native
        // invalidation or expiry rejects this lost acknowledgement.
        verify(value && (value.error === "invalid-frame" || value.error === "frame-timeout"),JSON.stringify(value))
        compare(evaluate("document.getElementById('same-origin')===null",WebEngineScript.MainWorld),true)
        compare(completionCount,1)
        // Force an observed native retirement, rather than assuming the handle
        // became invalid before the UI delivery deadline.
        load("?native-retirement")
        tryVerify(function() { return !nativeProbe.receiptFrameValid() },10000)
        attach()
        compare(nativeProbe.pendingScripts(broker),0,"Confirmed native retirement releases its dispatch slot")
        compare(run("true"),true)
        wait(50)
        compare(completionCount,1,"Late backend replies cannot deliver twice")
    }
    function delayedProbe(delay, marker) {
        var notify = marker ? "console.info('ferric-fixture-post-probe');" : ""
        return "const epoch=window.__ferric_hintFrameDocument;Object.defineProperty(window,'__ferric_hintFrameDocument',{configurable:true,get:function(){"+notify+"const until=Date.now()+"+delay+";while(Date.now()<until){}return epoch;}});"
    }
    function test_postcheck_uses_remaining_operation_deadline_data() {
        return [{tag:"first-only",probe:0,timeout:false},{tag:"combined",probe:600,timeout:true}]
    }
    function test_postcheck_uses_remaining_operation_deadline(data) {
        var value = run("(()=>{const until=Date.now()+600;while(Date.now()<until){}"+delayedProbe(data.probe)+"return true})()")
        if (data.timeout) compare(value.error,"frame-timeout",JSON.stringify(value))
        else compare(value,true)
    }
    function test_clear_during_postcheck_discards_old_completion() {
        done = false
        sawProbe = false
        completedBeforeProbe = false
        clearOnProbe = true
        broker.run(owner,"(()=>{"+delayedProbe(600,true)+"return true})()",function(value) { reply=value;done=true })
        tryVerify(function() { return sawProbe },10000)
        compare(completedBeforeProbe,false)
        wait(700)
        compare(done,false,"A cleared generation cannot deliver the retained result")
        load("?after-clear")
        attach()
        var value = run(Scripts.hintCollector("links"))
        verify(!value.error,JSON.stringify(value))
        verify(value.candidates.length > 0)
    }
    function test_timed_out_native_scripts_still_count_toward_the_dispatch_bound() {
        var completed = 0
        for (var index=0;index<128;index++) {
            var script = index === 0 ? "(()=>{const until=Date.now()+2500;while(Date.now()<until){}return true})()" : "true"
            broker.run(owner,script,function(value) { completed++ })
        }
        tryVerify(function() { return completed === 128 },5000)
        // UI delivery deadlines cannot cancel work already sent to WebEngine.
        // The still-running/queued native calls must continue to fill the cap.
        var value = run("true")
        compare(value.error,"frame-operation-limit",JSON.stringify(value))
        wait(1800)
        compare(completed,128,"Late engine replies cannot deliver twice")
        compare(run("true"),true,"Capacity returns after actual engine completion")
    }
    function test_stale_attachment_keeps_one_deadline_data() {
        return [{tag:"healthy-rebind",delay:0,timeout:false},{tag:"combined-deadline",delay:650,timeout:true}]
    }
    function test_stale_attachment_keeps_one_deadline(data) {
        compare(evaluate("(()=>{const state={epoch:window.__ferric_hintFrameDocument,reads:0,writes:0};window.attachmentWork=state;const delay="+data.delay+";function pause(){const until=Date.now()+delay;while(Date.now()<until){}}Object.defineProperty(window,'__ferric_hintFrameDocument',{configurable:true,get:function(){if(++state.reads===1){pause();return 'retired-document';}return state.epoch;},set:function(value){state.writes++;pause();state.epoch=value;}});return true})()",WebEngineScript.ApplicationWorld),true)
        try {
            done=false;completionCount=0
            broker.attach(page.mainFrame,function(value){reply=value;completionCount++;done=true})
            tryVerify(function(){return done},10000)
            if(data.timeout)compare(reply.error,"frame-timeout",JSON.stringify(reply))
            else {
                verify(!!reply.owner&&!reply.error,JSON.stringify(reply))
                verify(reply.owner!==owner,"A known old document gets a fresh native owner")
                owner=reply.owner
                compare(run("true"),true)
            }
            // The timed-out initialization can still finish in WebEngine;
            // its late native callback cannot publish a new owner or redeliver.
            wait(data.timeout?700:0)
            compare(completionCount,1)
            var work=evaluate("({reads:window.attachmentWork.reads,writes:window.attachmentWork.writes})",WebEngineScript.ApplicationWorld)
            verify(work.reads>=1,JSON.stringify(work));compare(work.writes,1)
        } finally {
            evaluate("(()=>{const state=window.attachmentWork;Object.defineProperty(window,'__ferric_hintFrameDocument',{configurable:true,writable:true,value:state.epoch});delete window.attachmentWork;return true})()",WebEngineScript.ApplicationWorld)
        }
    }
    function test_unknown_cached_attachment_cannot_retire_a_valid_owner() {
        var completed=0
        for(var index=0;index<128;index++)broker.run(owner,"true",function(){completed++})
        compare(nativeProbe.pendingScripts(broker),128)
        done=false
        broker.attach(page.mainFrame,function(value){reply=value;done=true})
        verify(done,"The full dispatcher refuses the probe synchronously")
        verify(!!reply.error,JSON.stringify(reply))
        tryVerify(function(){return completed===128},10000)
        compare(run("true"),true,"Unknown probe work cannot retire or replace the original native owner")
    }
    function test_attachment_notification_cannot_outlive_its_deadline() {
        broker.clear()
        var blocked=false
        var block=function(){
            broker.leasesChanged.disconnect(block)
            blocked=true
            nativeProbe.blockGui(1100)
        }
        broker.leasesChanged.connect(block)
        try {
            done=false;completionCount=0
            broker.attach(page.mainFrame,function(value){reply=value;completionCount++;done=true})
            tryVerify(function(){return done},10000)
            verify(blocked)
            compare(reply.error,"frame-timeout",JSON.stringify(reply))
            wait(50)
            compare(completionCount,1)
        } finally {
            if(!blocked)broker.leasesChanged.disconnect(block)
        }
    }
    function test_attachment_clear_reentry_discards_completion() {
        broker.clear()
        var cleared = false
        var cancel = function() {
            broker.leasesChanged.disconnect(cancel)
            cleared = true
            broker.clear()
        }
        broker.leasesChanged.connect(cancel)
        done = false
        broker.attach(page.mainFrame,function(value) { reply=value;done=true })
        tryVerify(function() { return cleared },10000)
        wait(50)
        compare(done,false,"An old attachment cannot finish after its signal observer clears it")
        compare(broker.leaseCount,0)
    }
    function test_attachment_destruction_reentry_discards_completion() {
        var target = temporaryBroker.createObject(tests)
        verify(!!target)
        nativeProbe.deleteOnLease(target)
        done = false
        target.attach(page.mainFrame,function(value) { reply=value;done=true })
        tryVerify(function() { return !nativeProbe.watchedBrokerAlive() },10000)
        wait(50)
        compare(done,false)
    }
    function test_postcheck_absolute_deadline_survives_gui_event_delay() {
        blockOnProbe = true
        blockedProbe = false
        var value = run("(()=>{const epoch=window.__ferric_hintFrameDocument;Object.defineProperty(window,'__ferric_hintFrameDocument',{configurable:true,get:function(){console.info('ferric-fixture-late-post-probe');return epoch;}});return true})()")
        compare(blockedProbe,true)
        compare(value.error,"frame-timeout",JSON.stringify(value))
    }
}

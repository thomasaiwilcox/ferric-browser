import QtQuick
import QtTest
import QtWebEngine
import io.github.ferricbrowser 1.0
import Ferric.Testing 1.0
import "../../qml/scripts/BrowserScripts.js" as Scripts

TestCase {
    id: tests
    name: "PolicyDocumentWork"
    when: windowShown
    width: 1000
    height: 800
    property bool done: false
    property var reply
    property string owner: ""
    FerricHintFrameBroker { id: broker }
    FrameReceiptProbe { id: probe }
    Component { id: temporaryBroker; FerricHintFrameBroker {} }
    WebEngineView {
        id: page
        anchors.fill: parent
        property bool loaded: false
        onLoadingChanged: function(request) { loaded=request.status===WebEngineView.LoadSucceededStatus }
        Component.onCompleted: {
            var script=WebEngine.script()
            script.name="ferric-document-work-lifecycle"
            script.sourceCode=Scripts.hintNativeAncestryLifecycle()
            script.injectionPoint=WebEngineScript.DocumentCreation
            script.worldId=WebEngineScript.ApplicationWorld
            script.runsOnSubFrames=true
            userScripts.insert(script)
        }
    }
    function evaluate(script,world) {
        done=false
        page.runJavaScript(script,world===undefined?WebEngineScript.ApplicationWorld:world,function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        return reply
    }
    function capture(target,selectedOwner) {
        done=false
        probe.capture(target||broker,selectedOwner||owner,function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        return reply
    }
    function validate(target) {
        done=false
        probe.validate(target||broker,function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        return reply
    }
    function completed() { return probe.documentEvents().filter(function(event){return event.kind==="complete"}) }
    function init() {
        page.loaded=false
        page.url="http://127.0.0.1:8766/roots.html"
        tryVerify(function(){return page.loaded},10000)
        page.forceActiveFocus()
        done=false
        broker.attach(page.mainFrame,function(result){reply=result;done=true})
        tryVerify(function(){return done},10000)
        verify(!reply.error,JSON.stringify(reply));owner=reply.owner
        verify(capture().ok,"Each case starts with a genuinely captured native receipt")
    }
    function cleanup() {
        probe.reset();broker.clear()
        evaluate(Scripts.hintStopTracking())
    }
    function test_capture_and_validation_have_separate_fixed_positions() {
        probe.configureDocumentAccounting(-1,-1)
        verify(capture().ok)
        var events=completed()
        compare(events.length,2)
        compare(validate(),true)
        events=completed();compare(events.length,4)
        for(var index=0;index<events.length;index++) {
            compare(events[index].checkpoint,index<2?"bind-top":"validation")
            compare(events[index].document,0)
            compare(events[index].pass,index%2)
            compare(events[index].outcome,"known");compare(events[index].units,1)
        }
        compare(validate(),false,"The native test ledger refuses a repeated validation slot")
        compare(completed().length,4)
    }
    function test_refusal_happens_before_document_probe_data() {
        return [{tag:"forward",pass:0},{tag:"reverse",pass:1}]
    }
    function test_refusal_happens_before_document_probe(data) {
        compare(evaluate("(()=>{window.documentWorkSpy={original:Document.prototype.hasFocus,reads:0};Document.prototype.hasFocus=function(){window.documentWorkSpy.reads++;return window.documentWorkSpy.original.call(this)};return true})()"),true)
        probe.configureDocumentAccounting(0,data.pass)
        try {
            compare(capture().ok,false)
            compare(evaluate("window.documentWorkSpy.reads"),data.pass,"A refused pass cannot invoke document.hasFocus")
            compare(completed().length,data.pass)
        } finally {
            evaluate("Document.prototype.hasFocus=window.documentWorkSpy.original;delete window.documentWorkSpy;true")
        }
    }
    function test_dispatched_rejection_and_exception_are_charged_data() {
        return [{tag:"epoch",throwing:false},{tag:"exception",throwing:true}]
    }
    function test_dispatched_rejection_and_exception_are_charged(data) {
        probe.configureDocumentAccounting(-1,-1)
        if(data.throwing)compare(evaluate("window.originalDocumentFocus=Document.prototype.hasFocus;Document.prototype.hasFocus=function(){throw Error('document-work-test')};true"),true)
        else compare(evaluate("window.__ferric_hintFrameDocument='replaced';true"),true)
        try {
            compare(capture().ok,false)
            var events=completed();compare(events.length,1)
            compare(events[0].outcome,"known");compare(events[0].units,1)
        } finally {
            if(data.throwing)evaluate("Document.prototype.hasFocus=window.originalDocumentFocus;delete window.originalDocumentFocus;true")
        }
    }
    function test_expired_admission_is_native_no_script() {
        probe.configureDocumentAccounting(-1,-1,"expire")
        compare(capture().ok,false)
        var events=completed();compare(events.length,1)
        compare(events[0].outcome,"no-script");compare(events[0].units,0)
    }
    function test_valid_receipt_cannot_bypass_rejected_settlement() {
        probe.configureDocumentAccounting(-1,-1,"reject-settlement")
        compare(capture().ok,false)
        var events=completed();compare(events.length,1)
        compare(events[0].outcome,"known");compare(events[0].units,1)
    }
    function test_cancel_and_destruction_settle_independently_data() {
        return [{tag:"clear",interruption:"clear"},{tag:"destruction",interruption:"destroy"}]
    }
    function test_cancel_and_destruction_settle_independently(data) {
        var target=temporaryBroker.createObject(tests)
        verify(target);probe.watchBroker(target)
        try {
            done=false
            target.attach(page.mainFrame,function(result){reply=result;done=true})
            tryVerify(function(){return done},10000)
            verify(!reply.error,JSON.stringify(reply))
            var selectedOwner=reply.owner
            verify(capture(target,selectedOwner).ok,"Temporary broker must first prove a valid receipt")
            probe.configureDocumentAccounting(-1,-1,data.interruption)
            var delivered=false
            probe.capture(target,selectedOwner,function(result){delivered=true})
            tryVerify(function(){return completed().length>0},3000)
            var events=completed();compare(events.length,1)
            compare(events[0].outcome,"unknown")
            if(data.interruption==="destroy")compare(probe.watchedBrokerAlive(),false)
            wait(1200)
            compare(completed().length,1,"A late engine reply cannot refund cancellation")
            compare(delivered,false)
        } finally {
            if(probe.watchedBrokerAlive())target.destroy()
        }
    }
    function test_timeout_burns_without_late_refund() {
        probe.configureDocumentAccounting(-1,-1)
        compare(evaluate("window.originalDocumentFocus=Document.prototype.hasFocus;Document.prototype.hasFocus=function(){const end=Date.now()+1400;while(Date.now()<end){}return window.originalDocumentFocus.call(this)};true"),true)
        try {
            compare(capture().ok,false)
            var events=completed();compare(events.length,1);compare(events[0].outcome,"unknown")
            wait(1600);compare(completed().length,1)
        } finally {
            evaluate("Document.prototype.hasFocus=window.originalDocumentFocus;delete window.originalDocumentFocus;true")
        }
    }
    function test_changed_catalogue_is_rejected_before_positional_grants() {
        probe.configureDocumentAccounting(-1,-1)
        page.loaded=false;page.url="http://127.0.0.1:8766/roots.html?new-catalogue"
        tryVerify(function(){return page.loaded},10000)
        compare(validate(),false)
        compare(probe.documentEvents().length,0)
    }
    function test_parent_leaf_and_composed_revalidation_use_native_positions() {
        compare(evaluate("document.getElementById('same-origin').contentDocument.querySelector('a').focus();true",WebEngineScript.MainWorld),true)
        tryVerify(function(){return evaluate("document.getElementById('same-origin').contentDocument.hasFocus()",WebEngineScript.MainWorld)},10000)
        done=false
        broker.focusedChild(owner,function(result){reply=result;done=true})
        tryVerify(function(){return done},10000);verify(!reply.error,JSON.stringify(reply))
        var child=reply.owner
        verify(capture(broker,child).ok)
        probe.configureDocumentAccounting(-1,-1)
        verify(capture(broker,child).ok)
        var events=completed();compare(events.length,4)
        var documents=[0,1,1,0],passes=[0,0,1,1]
        for(var index=0;index<4;index++) {
            compare(events[index].checkpoint,"bind-selected")
            compare(events[index].document,documents[index]);compare(events[index].pass,passes[index])
        }
        done=false
        probe.captureComposed(broker,function(result){reply=result;done=true})
        tryVerify(function(){return done},10000);verify(reply.ok,JSON.stringify(reply))
        events=completed();compare(events.length,16,"Two focused receipt passes and final composed seal each visit the exact two-document chain")
        var stages=["focused-before-first","focused-before-second","capture-final"]
        for(var stage=0;stage<3;stage++)for(var pass=0;pass<4;pass++) {
            var event=events[4+stage*4+pass]
            compare(event.checkpoint,stages[stage]);compare(event.document,documents[pass])
            compare(event.pass,passes[pass]);compare(event.outcome,"known");compare(event.units,1)
        }
    }
}

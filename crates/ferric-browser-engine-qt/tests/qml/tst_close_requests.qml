import QtQuick
import QtTest
import "../../qml/scripts/CloseRequests.js" as CloseRequests

TestCase {
    name: "CloseRequests"
    when: windowShown
    property var calls: []
    property var outcomes: []
    property bool current: true
    function init() { calls=[];outcomes=[];current=true }
    function run(views, request) {
        CloseRequests.probePages(views,function(){return current},request || function(view,reply){
            calls.push(view.name);reply(view.result);return true
        },function(result){outcomes.push(result)})
    }
    function test_native_pages_do_not_wait_for_a_web_document() {
        run([{nativeUrl:"rb://blank"},{nativeUrl:"rb://start",startPage:{closeNeedsConfirmation:false}}])
        compare(calls.length,0)
        compare(outcomes.length,1)
        compare(outcomes[0].state,"clean")
    }
    function test_many_native_tabs_do_not_need_a_recursive_stack() {
        var views=[]
        for(var i=0;i<5000;++i) views.push({nativeUrl:"rb://blank"})
        run(views)
        compare(calls.length,0)
        compare(outcomes.length,1)
        compare(outcomes[0].state,"clean")
    }
    function test_native_editor_requires_a_decision() {
        run([{nativeUrl:"rb://start",startPage:{closeNeedsConfirmation:true}}])
        compare(calls.length,0)
        compare(outcomes[0].state,"dirty")
        verify(outcomes[0].reason.indexOf("editor")>=0)
    }
    function test_unknown_or_retired_native_views_are_not_declared_clean() {
        run([{nativeUrl:"rb://unknown"}])
        compare(outcomes[0].state,"unknown")
        outcomes=[]
        run([{nativeUrl:"rb://blank",ownsTab:function(){return false}}])
        compare(outcomes[0].state,"unknown")
    }
    function test_native_pages_do_not_hide_a_later_dirty_webpage() {
        run([{nativeUrl:"rb://blank"},{name:"background page",result:"dirty"},{name:"active page",result:"clean"}])
        compare(calls,["background page"])
        compare(outcomes[0].state,"dirty")
    }
    function test_every_clean_page_is_checked_once() {
        run([{name:"first",result:"clean"},{nativeUrl:"rb://start"},{name:"last",result:"clean"}])
        compare(calls,["first","last"])
        compare(outcomes.length,1)
        compare(outcomes[0].state,"clean")
    }
    function test_rejected_script_admission_is_unknown_without_a_timeout() {
        run([{name:"closed page"}],function(){return false})
        compare(outcomes.length,1)
        compare(outcomes[0].state,"unknown")
    }
    function test_old_callbacks_cannot_finish_a_new_close_request() {
        var reply
        run([{name:"pending"}],function(view,callback){reply=callback;return true})
        compare(outcomes.length,0)
        current=false
        reply("clean")
        compare(outcomes.length,0)
    }
    function test_duplicate_callback_does_not_finish_twice() {
        var reply
        run([{name:"pending"}],function(view,callback){reply=callback;return true})
        reply("clean");reply("dirty")
        compare(outcomes.length,1)
        compare(outcomes[0].state,"clean")
    }
}

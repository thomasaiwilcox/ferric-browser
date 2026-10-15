import QtQuick
import QtTest
import QtWebEngine
import Ferric.Testing 1.0

TestCase {
    id: test
    name: "ScriptRunner"
    when: windowShown
    WebEngineView { id: page; width: 400; height: 300; url: "about:blank" }
    Component { id: runnerFixture; ScriptRunner {} }
    Component { id: ownerFixture; QtObject {} }
    property int receivedCount: 0
    property var result: null
    function init() {
        failOnWarning(/invalid context|TypeError|ReferenceError/)
        tryVerify(function() { return page.mainFrame.isValid && !page.loading })
        receivedCount = 0; result = null
    }
    function receive(value) { receivedCount++; result = value }
    function runner() { return createTemporaryObject(runnerFixture, test) }
    function test_result_arrives_once_in_the_requested_world() {
        var execution = runner()
        verify(execution.run(page, page.mainFrame, "window.runnerValue=42;({value:window.runnerValue})", 1, receive))
        tryCompare(test, "receivedCount", 1)
        compare(result.value, 42)
        verify(execution.run(page, page.mainFrame, "typeof window.runnerValue", 0, receive))
        tryCompare(test, "receivedCount", 2)
        compare(result, "undefined")
    }
    function test_cancelled_request_does_not_deliver() {
        var execution = runner()
        verify(execution.run(page, page.mainFrame, "42", 1, receive))
        execution.cancelFor(page)
        wait(30)
        compare(receivedCount, 0)
    }
    function test_destroyed_owner_does_not_deliver() {
        var execution = runner()
        var owner = createTemporaryObject(ownerFixture, test)
        verify(execution.run(owner, page.mainFrame, "(()=>{var end=Date.now()+60;while(Date.now()<end){};return 42})()", 1, receive))
        owner.destroy()
        // Retire the QObject before processing the renderer's reply.
        wait(0)
        wait(100)
        compare(receivedCount, 0)
    }
    function test_destroyed_runner_does_not_deliver() {
        var execution = runner()
        verify(execution.run(page, page.mainFrame, "(()=>{var end=Date.now()+60;while(Date.now()<end){};return 42})()", 1, receive))
        execution.destroy()
        wait(100)
        compare(receivedCount, 0)
    }
    function test_cancel_all_does_not_deliver() {
        var execution = runner()
        verify(execution.run(page, page.mainFrame, "42", 1, receive))
        execution.cancelAll()
        wait(30)
        compare(receivedCount, 0)
    }
    function test_cancel_does_not_affect_another_owner() {
        var execution = runner()
        var owner = createTemporaryObject(ownerFixture, test)
        verify(execution.run(owner, page.mainFrame, "42", 1, receive))
        verify(execution.run(page, page.mainFrame, "43", 1, receive))
        execution.cancelFor(owner)
        tryCompare(test, "receivedCount", 1)
        compare(result, 43)
    }
    function test_invalid_world_is_rejected_without_dispatch() {
        var execution = runner()
        verify(!execution.run(page, page.mainFrame, "42", -1, receive))
        verify(!execution.run(page, page.mainFrame, "42", 257, receive))
        wait(20)
        compare(receivedCount, 0)
    }
}

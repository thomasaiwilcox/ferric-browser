import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"

TestCase {
    id: test
    name: "FocusOverlay"
    when: windowShown
    ApplicationWindow {
        id: host
        width: 600
        height: 400
        visible: true
        property bool runtimeDisposing: false
        property bool runtimeReleased: false
        function activeWebView() { return pageField }
        TextField { id: pageField; y: 20; text: "Page draft" }
        TextField { id: firstField; y: 70; text: "First prompt draft" }
        TextField { id: secondField; y: 120; text: "Second prompt draft" }
    }
    Component { id: controllerFixture; FerricFocusOverlayController { browserWindow: host } }
    Component { id: transientField; TextField {} }
    Component {
        id: otherHostFixture
        ApplicationWindow {
            width: 600; height: 400; visible: true
            property bool runtimeReleased: false
            property alias pageField: otherPage
            property alias promptField: otherPrompt
            TextField { id: otherPage; y: 20; text: "Other page draft" }
            TextField { id: otherPrompt; y: 70; text: "Other prompt draft" }
        }
    }

    function init() {
        failOnWarning(/invalid context|TypeError|ReferenceError|Binding loop/)
        host.runtimeDisposing = false
        host.runtimeReleased = false
        host.requestActivate()
        tryCompare(host, "active", true)
        pageField.forceActiveFocus()
        tryCompare(pageField, "activeFocus", true)
    }
    function controller() {
        return createTemporaryObject(controllerFixture, test)
    }
    function test_independent_windows_can_close_in_either_order_data() {
        return [{tag:"primary-first", primaryFirst:true}, {tag:"secondary-first", primaryFirst:false}]
    }
    function test_independent_windows_can_close_in_either_order(data) {
        var focus = controller()
        var other = createTemporaryObject(otherHostFixture, test)
        verify(other)
        focus.captureOverlayFocus(host, pageField)
        firstField.forceActiveFocus()
        other.requestActivate(); tryCompare(other, "active", true)
        other.pageField.forceActiveFocus()
        focus.captureOverlayFocus(other, other.pageField)
        other.promptField.forceActiveFocus()
        var firstHost = data.primaryFirst ? host : other
        var lastHost = data.primaryFirst ? other : host
        firstHost.requestActivate(); tryCompare(firstHost, "active", true)
        if (data.primaryFirst) focus.restoreOverlayFocus() // Default belongs to the primary window.
        else focus.restoreOverlayFocus(other)
        compare(focus.focusReturnStack.length, 1)
        verify(focus.focusReturnStack[0].host === lastHost)
        tryCompare(data.primaryFirst ? pageField : other.pageField, "activeFocus", true)
        verify(data.primaryFirst ? other.promptField.focus : firstField.focus)
        lastHost.requestActivate(); tryCompare(lastHost, "active", true)
        focus.restoreOverlayFocus(lastHost)
        tryCompare(data.primaryFirst ? other.pageField : pageField, "activeFocus", true)
        compare(focus.focusReturnStack.length, 0)
    }
    function test_other_window_overlay_does_not_block_primary_mode_return() {
        var focus = controller()
        var other = createTemporaryObject(otherHostFixture, test)
        verify(other)
        focus.captureModeFocus()
        firstField.forceActiveFocus()
        other.pageField.forceActiveFocus()
        focus.captureOverlayFocus(other, other.pageField)
        other.promptField.forceActiveFocus()
        host.requestActivate(); tryCompare(host, "active", true)
        focus.restoreModeFocus()
        tryCompare(pageField, "activeFocus", true)
        verify(other.promptField.focus)
    }
    function test_releasing_a_host_discards_only_its_focus_returns() {
        var focus = controller()
        var other = createTemporaryObject(otherHostFixture, test)
        focus.captureOverlayFocus(host, pageField)
        firstField.forceActiveFocus()
        focus.captureOverlayFocus(other, other.pageField)
        focus.restoreOverlayFocus(other)
        focus.discardHost(other)
        compare(focus.focusReturnStack.length, 1)
        verify(focus.focusReturnStack[0].host === host)
        compare(focus.pendingFocusReturns.length, 0)
        focus.restoreOverlayFocus(other)
        compare(focus.focusReturnStack.length, 1)
        host.requestActivate(); tryCompare(host, "active", true)
        focus.restoreOverlayFocus(host)
        tryCompare(pageField, "activeFocus", true)
    }
    function test_nested_prompts_return_to_their_own_drafts() {
        var focus = controller()
        focus.captureOverlayFocus(host, pageField)
        firstField.forceActiveFocus()
        focus.captureOverlayFocus(host, firstField)
        secondField.forceActiveFocus()
        focus.restoreOverlayFocus()
        tryCompare(firstField, "activeFocus", true)
        compare(firstField.text, "First prompt draft")
        focus.restoreOverlayFocus()
        tryCompare(pageField, "activeFocus", true)
    }
    function test_replacement_prompt_discards_an_older_queued_return() {
        var focus = controller()
        focus.captureOverlayFocus(host, pageField)
        firstField.forceActiveFocus()
        focus.restoreOverlayFocus()
        focus.captureOverlayFocus(host, firstField)
        secondField.forceActiveFocus()
        wait(20)
        verify(secondField.activeFocus)
        focus.restoreOverlayFocus()
        tryCompare(firstField, "activeFocus", true)
    }
    function test_destroyed_target_returns_to_live_fallback() {
        var focus = controller()
        var field = createTemporaryObject(transientField, host.contentItem)
        field.forceActiveFocus()
        focus.captureOverlayFocus(host, pageField)
        firstField.forceActiveFocus()
        field.destroy()
        wait(0)
        focus.restoreOverlayFocus()
        tryCompare(pageField, "activeFocus", true)
    }
    function test_mode_return_cannot_steal_a_new_prompt() {
        var focus = controller()
        focus.captureModeFocus()
        focus.restoreModeFocus()
        focus.captureOverlayFocus(host, pageField)
        firstField.forceActiveFocus()
        wait(20)
        verify(firstField.activeFocus)
    }
    function test_released_host_does_not_receive_queued_focus() {
        var focus = controller()
        focus.captureOverlayFocus(host, pageField)
        firstField.forceActiveFocus()
        focus.restoreOverlayFocus()
        host.runtimeReleased = true
        secondField.forceActiveFocus()
        wait(20)
        verify(secondField.activeFocus)
    }
}

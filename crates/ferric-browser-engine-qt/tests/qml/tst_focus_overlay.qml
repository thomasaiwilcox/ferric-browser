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

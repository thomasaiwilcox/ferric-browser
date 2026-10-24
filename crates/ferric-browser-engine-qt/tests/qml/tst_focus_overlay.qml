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
        property bool browserKeyFocusActive: true
        property var modalInputScope: null
        property var selectedPage: pageField
        function activeWebView() { return selectedPage }
        TextField { id: pageField; y: 20; text: "Page draft"; property string nativeUrl: "" }
        TextField { id: firstField; y: 70; text: "First prompt draft" }
        TextField { id: secondField; y: 120; text: "Second prompt draft" }
    }
    Component { id: controllerFixture; FerricFocusOverlayController { browserWindow: host; followTabFocus: true } }
    Component { id: transientField; TextField {} }
    Component {
        id: pageScopeFixture
        FocusScope {
            width: 400; height: 200
            property string nativeUrl: "rb://start"
            property alias documentInput: documentInput
            property alias nativeInput: nativeInput
            TextField { id: documentInput; y: 20; text: "Web document" }
            TextField { id: nativeInput; y: 70; text: "Native control" }
        }
    }
    Component {
        id: otherHostFixture
        ApplicationWindow {
            width: 600; height: 400; visible: true
            property bool runtimeReleased: false
            property var modalInputScope: null
            property var selectedPage: otherPage
            property alias pageField: otherPage
            property alias promptField: otherPrompt
            function activeWebView() { return selectedPage }
            TextField { id: otherPage; y: 20; text: "Other page draft" }
            TextField { id: otherPrompt; y: 70; text: "Other prompt draft" }
        }
    }

    function init() {
        failOnWarning(/invalid context|TypeError|ReferenceError|Binding loop/)
        host.runtimeDisposing = false
        host.runtimeReleased = false
        host.browserKeyFocusActive = true
        host.modalInputScope = null
        host.selectedPage = pageField
        pageField.nativeUrl = ""
        pageField.visible = true
        firstField.visible = true
        secondField.visible = true
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
    function test_tab_change_focuses_the_selected_page_after_layout() {
        var focus = controller()
        firstField.visible = false
        host.selectedPage = firstField
        firstField.visible = true
        tryCompare(firstField, "activeFocus", true)
        host.selectedPage = secondField
        tryCompare(secondField, "activeFocus", true)
    }
    function test_native_surface_change_with_the_same_view_data() {
        return [{tag:"page", target:"page"}, {tag:"browser-input", target:"browser"},
            {tag:"modal", target:"modal"}]
    }
    function test_native_surface_change_with_the_same_view(data) {
        pageField.nativeUrl = "rb://start"
        var focus = controller()
        wait(20)
        firstField.forceActiveFocus()
        if (data.target === "browser") host.browserKeyFocusActive = false
        else if (data.target === "modal") {
            host.modalInputScope = { activeSurface:firstField,
                allowsFocus:function(target) { return target === firstField } }
        }
        pageField.nativeUrl = ""
        if (data.target === "page") tryCompare(pageField, "activeFocus", true)
        else { wait(20); verify(firstField.activeFocus) }
    }
    function test_native_handoff_targets_a_focusable_descendant_inside_the_page_data() {
        return [{tag:"page-change", overlay:false}, {tag:"overlay-return", overlay:true}]
    }
    function test_native_handoff_targets_a_focusable_descendant_inside_the_page(data) {
        var view = createTemporaryObject(pageScopeFixture, host.contentItem)
        host.selectedPage = view
        var focus = controller()
        view.nativeInput.forceActiveFocus()
        tryCompare(view.nativeInput, "activeFocus", true)
        if (data.overlay) {
            focus.captureOverlayFocus(host, view)
            firstField.forceActiveFocus()
        }
        view.nativeInput.visible = false
        view.nativeInput.enabled = false
        view.nativeUrl = ""
        if (data.overlay) {
            wait(20)
            verify(firstField.activeFocus)
            focus.restoreOverlayFocus(host)
        }
        tryCompare(view.documentInput, "activeFocus", true)
    }
    function test_tab_change_cannot_take_focus_from_browser_input() {
        var focus = controller()
        host.browserKeyFocusActive = false
        host.selectedPage = firstField
        secondField.forceActiveFocus()
        wait(20)
        verify(secondField.activeFocus)
        host.browserKeyFocusActive = true
        wait(20)
        verify(secondField.activeFocus) // No stale tab change waits behind the editor.
    }
    function test_deferred_tab_focus_respects_a_new_overlay() {
        var focus = controller()
        host.selectedPage = firstField
        focus.captureOverlayFocus(host, firstField)
        secondField.forceActiveFocus()
        wait(20)
        verify(secondField.activeFocus)
    }
    function test_deferred_tab_focus_respects_another_controllers_modal() {
        var focus = controller()
        host.selectedPage = firstField
        host.modalInputScope = { activeSurface: secondField,
            allowsFocus: function(target) { return target === secondField } }
        secondField.forceActiveFocus()
        wait(20)
        verify(secondField.activeFocus)
    }
    function test_overlay_falls_back_to_the_current_page_in_its_own_window() {
        var focus = controller()
        var other = createTemporaryObject(otherHostFixture, test)
        other.requestActivate(); tryCompare(other, "active", true)
        other.pageField.forceActiveFocus()
        focus.captureOverlayFocus(other, other.pageField)
        other.promptField.forceActiveFocus()
        var replacement = createTemporaryObject(transientField, other.contentItem)
        other.selectedPage = replacement
        other.pageField.visible = false
        focus.restoreOverlayFocus(other)
        tryCompare(replacement, "activeFocus", true)
        verify(!other.pageField.activeFocus)
    }
    function test_current_page_fallback_cannot_steal_another_windows_modal() {
        var focus = controller()
        var other = createTemporaryObject(otherHostFixture, test)
        other.requestActivate(); tryCompare(other, "active", true)
        other.pageField.forceActiveFocus()
        focus.captureOverlayFocus(other, other.pageField)
        other.promptField.forceActiveFocus()
        var replacement = createTemporaryObject(transientField, other.contentItem)
        other.selectedPage = replacement
        other.pageField.visible = false
        other.modalInputScope = { activeSurface: other.promptField,
            allowsFocus: function(target) { return target === other.promptField } }
        focus.restoreOverlayFocus(other)
        wait(20)
        verify(other.promptField.activeFocus)
    }
    function test_released_window_does_not_receive_deferred_tab_or_mode_focus() {
        var focus = controller()
        focus.captureModeFocus()
        focus.restoreModeFocus()
        host.selectedPage = firstField
        host.runtimeReleased = true
        secondField.forceActiveFocus()
        wait(20)
        verify(secondField.activeFocus)
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

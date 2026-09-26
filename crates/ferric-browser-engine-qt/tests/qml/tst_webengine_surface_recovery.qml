import QtQuick
import QtQuick.Controls
import QtTest
import QtWebEngine
import "../../qml/components"

TestCase {
    id: recoveryTest
    name: "WebEngineSurfaceRecovery"
    when: windowShown
    width: 480
    height: 320

    ApplicationWindow {
        id: testWindow
        width: 480
        height: 320
        visible: true
    }

    QtObject {
        id: fakeHost
        property bool active: true
        property int updateRequests: 0
        function requestUpdate() { updateRequests += 1 }
    }

    QtObject {
        id: fakeFrameSource
        signal frameSwapped()
    }

    Component {
        id: viewFixture
        Item {
            property int lifecycleState: WebEngineView.LifecycleState.Active
            property int visibilityChanges: 0
            visible: true
            onVisibleChanged: visibilityChanges += 1
        }
    }

    Component {
        id: controllerFixture
        FerricWebEngineSurfaceRecovery {
            hostWindow: fakeHost
            frameSource: fakeFrameSource
        }
    }

    function createView(properties) {
        return createTemporaryObject(
                    viewFixture, testWindow.contentItem, properties || {})
    }

    function createController(views, enabled) {
        return createTemporaryObject(controllerFixture, testWindow.contentItem, {
            views: views,
            enabled: enabled === undefined ? true : enabled
        })
    }

    function presentInitially(controller) {
        verify(controller)
        fakeFrameSource.frameSwapped()
        compare(controller.recoveryCount, 0)
    }

    function reactivate() {
        fakeHost.active = false
        fakeHost.active = true
    }

    function init() {
        fakeHost.active = true
        fakeHost.updateRequests = 0
    }

    function test_initial_frame_does_not_recover() {
        var view = createView()
        var controller = createController([view])
        presentInitially(controller)
        verify(view.visible)
        compare(fakeHost.updateRequests, 0)
    }

    function test_reactivation_recovers_after_returned_frame() {
        var view = createView()
        var controller = createController([view])
        presentInitially(controller)

        reactivate()
        compare(fakeHost.updateRequests, 1)
        verify(view.visible)

        view.visibilityChanges = 0
        fakeFrameSource.frameSwapped()
        verify(view.visible)
        compare(view.visibilityChanges, 2)
        compare(controller.recoveryCount, 1)
        compare(fakeHost.updateRequests, 2)
    }

    function test_disabled_controller_never_recovers() {
        var view = createView()
        var controller = createController([view], false)
        presentInitially(controller)
        reactivate()
        fakeFrameSource.frameSwapped()
        wait(0)
        verify(view.visible)
        compare(controller.recoveryCount, 0)
        compare(fakeHost.updateRequests, 0)
    }

    function test_ineligible_and_duplicate_views_are_bounded() {
        var activeView = createView()
        var frozenView = createView({
            lifecycleState: WebEngineView.LifecycleState.Frozen
        })
        var hiddenView = createView({visible: false})
        var controller = createController([
            activeView, activeView, frozenView, hiddenView
        ])
        presentInitially(controller)
        reactivate()
        activeView.visibilityChanges = 0
        frozenView.visibilityChanges = 0
        hiddenView.visibilityChanges = 0
        fakeFrameSource.frameSwapped()

        verify(activeView.visible)
        compare(activeView.visibilityChanges, 2)
        verify(frozenView.visible)
        verify(!hiddenView.visible)
        compare(controller.recoveryCount, 1)
        compare(frozenView.visibilityChanges, 0)
        compare(hiddenView.visibilityChanges, 0)
        verify(frozenView.visible)
        verify(!hiddenView.visible)
    }

    function test_current_views_are_resolved_at_frame_time() {
        var oldView = createView()
        var currentView = createView()
        var controller = createController([oldView])
        presentInitially(controller)
        reactivate()
        controller.views = [currentView]
        oldView.visibilityChanges = 0
        currentView.visibilityChanges = 0
        fakeFrameSource.frameSwapped()

        verify(oldView.visible)
        compare(oldView.visibilityChanges, 0)
        verify(currentView.visible)
        compare(currentView.visibilityChanges, 2)
        compare(controller.recoveryCount, 1)
    }

    function test_deactivation_before_the_returned_frame_cancels_recovery() {
        var view = createView()
        var controller = createController([view])
        presentInitially(controller)
        reactivate()
        fakeHost.active = false
        view.visibilityChanges = 0
        fakeFrameSource.frameSwapped()
        verify(view.visible)
        compare(view.visibilityChanges, 0)
        compare(controller.recoveryCount, 0)
    }

    function test_rapid_reactivation_coalesces_before_a_frame() {
        var view = createView()
        var controller = createController([view])
        presentInitially(controller)
        reactivate()
        reactivate()
        compare(fakeHost.updateRequests, 2)

        view.visibilityChanges = 0
        fakeFrameSource.frameSwapped()
        verify(view.visible)
        compare(view.visibilityChanges, 2)
        compare(controller.recoveryCount, 1)
    }
}

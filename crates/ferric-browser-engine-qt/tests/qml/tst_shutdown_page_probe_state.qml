import QtQuick
import QtTest
import "../../qml/scripts/ShutdownPageProbeState.js" as ShutdownPageProbeState

TestCase {
    name: "ShutdownPageProbeState"
    when: windowShown
    width: 320
    height: 240

    readonly property int activeState: 1
    readonly property int frozenState: 2

    function page(url) {
        return {
            lifecycleState: activeState,
            loading: false,
            versioningNavigationEpoch: 3,
            url: url
        }
    }

    function test_unchanged_active_views_remain_current() {
        var first = page("https://example.test/first")
        var second = page("https://example.test/second")
        var views = [first, second]
        var snapshot = ShutdownPageProbeState.snapshotActive(views, activeState)
        verify(snapshot !== null)
        compare(snapshot.length, 2)
        verify(ShutdownPageProbeState.stillCurrent(snapshot, views, activeState))
    }

    function test_same_url_navigation_invalidates_prior_result() {
        var view = page("https://example.test/form")
        var snapshot = ShutdownPageProbeState.snapshotActive([view], activeState)
        view.versioningNavigationEpoch += 1
        verify(!ShutdownPageProbeState.stillCurrent(snapshot, [view], activeState))
    }

    function test_url_loading_and_lifecycle_changes_invalidate() {
        var view = page("https://example.test/form")
        var snapshot = ShutdownPageProbeState.snapshotActive([view], activeState)
        view.url = "https://example.test/other"
        verify(!ShutdownPageProbeState.stillCurrent(snapshot, [view], activeState))
        view.url = "https://example.test/form"
        view.loading = true
        verify(!ShutdownPageProbeState.stillCurrent(snapshot, [view], activeState))
        view.loading = false
        view.lifecycleState = frozenState
        verify(!ShutdownPageProbeState.stillCurrent(snapshot, [view], activeState))
    }

    function test_tab_replacement_reorder_and_addition_invalidate() {
        var first = page("https://example.test/first")
        var second = page("https://example.test/second")
        var snapshot = ShutdownPageProbeState.snapshotActive(
            [first, second], activeState)
        verify(!ShutdownPageProbeState.stillCurrent(
            snapshot, [second, first], activeState))
        verify(!ShutdownPageProbeState.stillCurrent(
            snapshot, [page(first.url), second], activeState))
        verify(!ShutdownPageProbeState.stillCurrent(
            snapshot, [first, second, page("https://example.test/new")], activeState))
    }

    function test_active_loading_page_cannot_start_probe() {
        var view = page("https://example.test/form")
        view.loading = true
        compare(ShutdownPageProbeState.snapshotActive([view], activeState), null)
    }

    function test_loading_prompt_consent_tracks_same_page_and_load_state() {
        var view = page("https://example.test/form")
        view.loading = true
        var snapshot = ShutdownPageProbeState.snapshotActive(
            [view], activeState, true)
        verify(snapshot !== null)
        verify(ShutdownPageProbeState.stillCurrent(
            snapshot, [view], activeState, true))
        view.loading = false
        verify(!ShutdownPageProbeState.stillCurrent(
            snapshot, [view], activeState, true))
    }
}

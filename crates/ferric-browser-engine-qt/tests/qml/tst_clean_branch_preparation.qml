import QtQuick
import QtTest
import "../../qml/scripts/CleanBranchPreparation.js" as CleanBranchPreparation

TestCase {
    name: "CleanBranchPreparation"
    when: windowShown

    function makeParentUi() {
        return {
            aborts: 0,
            publications: 0,
            publishResult: true,
            abort_versioned_clean_branch: function(token) {
                this.aborts += 1
            },
            publish_versioned_clean_branch: function(childUi, token) {
                this.publications += 1
                return this.publishResult
            }
        }
    }

    function makeChild(ui, token) {
        var profile = {}
        var view = {}
        return {
            visible: false,
            destroyed: false,
            windowCleanBranch: true,
            windowBoundBranchToken: token,
            windowBranchParentUi: ui,
            windowBranchProfile: profile,
            windowBranchInitialView: view,
            windowCleanBranchLineagePublished: false,
            windowCleanBranchNavigationAdmitted: false,
            publishCalls: 0,
            publishResult: true,
            windowBranchUi: {
                navigationCalls: 0,
                navigationResult: true,
                navigate_clean_branch_safe_get: function(url) {
                    this.navigationCalls += 1
                    return this.navigationResult
                }
            },
            publishCleanBranchWindow: function() {
                this.publishCalls += 1
                return this.publishResult
            },
            destroy: function() { this.destroyed = true }
        }
    }

    function test_allocation_throw_and_wrong_child_fail_without_publication() {
        var ui = makeParentUi()
        var token = "branch:test"
        compare(CleanBranchPreparation.createHiddenChild({
            createObject: function(owner, properties) { throw new Error("allocation") }
        }, {}, token, ui), null)
        var wrong = makeChild(ui, token)
        wrong.windowBranchParentUi = {}
        compare(CleanBranchPreparation.createHiddenChild({
            createObject: function(owner, properties) { return wrong }
        }, {}, token, ui), null)
        verify(wrong.destroyed)
        compare(ui.publications, 0)
        compare(ui.aborts, 0)
    }

    function test_owner_failure_aborts_unpublished_fork() {
        var ui = makeParentUi()
        var token = "branch:test"
        var prepared = makeChild(ui, token)
        var entry = { parentUi: ui, child: prepared, navigation: "safe-get", url: "https://example.test/" }
        compare(CleanBranchPreparation.publishPreparedChild(entry, token, {
            host: prepared, profile: {}, view: prepared.windowBranchInitialView
        }), "owner-failed")
        verify(prepared.destroyed)
        compare(ui.aborts, 1)
        compare(ui.publications, 0)
        compare(prepared.windowBranchUi.navigationCalls, 0)
    }

    function test_navigation_rejection_destroys_hidden_child() {
        var ui = makeParentUi()
        var token = "branch:test"
        var prepared = makeChild(ui, token)
        prepared.windowBranchUi.navigationResult = false
        var entry = { parentUi: ui, child: prepared, navigation: "safe-get", url: "https://example.test/" }
        compare(CleanBranchPreparation.publishPreparedChild(entry, token, {
            host: prepared,
            profile: prepared.windowBranchProfile,
            view: prepared.windowBranchInitialView
        }), "navigation-failed")
        verify(prepared.destroyed)
        verify(prepared.windowCleanBranchLineagePublished)
        verify(prepared.windowCleanBranchNavigationAdmitted)
        compare(prepared.windowBranchUi.navigationCalls, 1)
        compare(prepared.publishCalls, 0)
        compare(ui.aborts, 0)
    }

    function test_publication_rejection_destroys_child_after_navigation() {
        var ui = makeParentUi()
        var token = "branch:test"
        var prepared = makeChild(ui, token)
        prepared.publishResult = false
        var entry = { parentUi: ui, child: prepared, navigation: "safe-get", url: "https://example.test/" }
        compare(CleanBranchPreparation.publishPreparedChild(entry, token, {
            host: prepared,
            profile: prepared.windowBranchProfile,
            view: prepared.windowBranchInitialView
        }), "publication-failed")
        verify(prepared.destroyed)
        compare(prepared.windowBranchUi.navigationCalls, 1)
        compare(prepared.publishCalls, 1)
    }

    function test_placeholder_opens_without_navigation() {
        var ui = makeParentUi()
        var token = "branch:test"
        var prepared = makeChild(ui, token)
        var entry = { parentUi: ui, child: prepared, navigation: "placeholder", url: "" }
        compare(CleanBranchPreparation.publishPreparedChild(entry, token, {
            host: prepared,
            profile: prepared.windowBranchProfile,
            view: prepared.windowBranchInitialView
        }), "opened")
        verify(!prepared.destroyed)
        compare(prepared.windowBranchUi.navigationCalls, 0)
        compare(prepared.publishCalls, 1)
    }

    function test_window_navigation_starts_selected_get_and_defers_background_once() {
        var schedule = CleanBranchPreparation.windowNavigationSchedule([
            {navigation: "safe-get", url: "https://example.test/first"},
            {navigation: "placeholder"},
            {navigation: "safe-get", url: "https://example.test/selected"}
        ], 2)
        verify(schedule !== null)
        compare(schedule.selectedUrl, "https://example.test/selected")
        compare(String(schedule.deferred[0]), "https://example.test/first")
        compare(String(schedule.deferred[1] || ""), "")
        compare(CleanBranchPreparation.takeDeferredWindowUrl(schedule.deferred, 1), null)
        var taken = CleanBranchPreparation.takeDeferredWindowUrl(schedule.deferred, 0)
        verify(taken !== null)
        compare(taken.url, "https://example.test/first")
        compare(CleanBranchPreparation.takeDeferredWindowUrl(taken.deferred, 0), null)
        compare(CleanBranchPreparation.windowNavigationSchedule(
                    [{navigation: "unknown"}], 0), null)
    }

    function test_window_page_outcomes_follow_stable_ids_not_tab_positions() {
        var ids = ["tabid-1", "tabid-2", "tabid-3"]
        var expected = ["https://example.test/first", "", "https://example.test/third"]
        var initial = ["pending", "pending", "pending"]
        var unstarted = [false, false, false]
        compare(CleanBranchPreparation.startWindowPageNavigation(
                    ids, expected, unstarted, initial, "tabid-1", "about:blank"), null)
        compare(CleanBranchPreparation.startWindowPageNavigation(
                    ids, expected, unstarted, initial, "tabid-2", ""), null)
        var started = CleanBranchPreparation.startWindowPageNavigation(
                    ids, expected, unstarted, initial,
                    "tabid-1", "https://example.test/first")
        compare(started.join(","), "true,false,false")
        var failed = CleanBranchPreparation.recordWindowPageOutcome(
                    ids, started, initial, "tabid-1", "failed")
        compare(failed.join(","), "failed,pending,pending")
        compare(initial.join(","), "pending,pending,pending")
        compare(CleanBranchPreparation.recordWindowPageOutcome(
                    ids, started, failed, "tabid-1", "loaded"), null)
        started = CleanBranchPreparation.startWindowPageNavigation(
                    ids, expected, started, failed,
                    "tabid-3", "https://example.test/third")
        var loaded = CleanBranchPreparation.recordWindowPageOutcome(
                    ids, started, failed, "tabid-3", "loaded")
        compare(loaded.join(","), "failed,pending,loaded")
        compare(CleanBranchPreparation.recordWindowPageOutcome(
                    ids, started, loaded, "tabid-unknown", "failed"), null)
        compare(CleanBranchPreparation.recordWindowPageOutcome(
                    ids, started, loaded, "tabid-2", "unknown"), null)
        compare(CleanBranchPreparation.recordWindowPageOutcome(
                    ["tabid-1"], started, loaded, "tabid-1", "failed"), null)
        compare(CleanBranchPreparation.classifyWindowTerminalOutcome(
                    "loaded", "about:blank"), "failed")
        compare(CleanBranchPreparation.classifyWindowTerminalOutcome(
                    "loaded", "https://example.test/redirect"), "loaded")
        compare(CleanBranchPreparation.classifyWindowTerminalOutcome(
                    "failed", "chrome-error://network"), "failed")
    }

    function test_only_started_pending_original_pages_can_time_out() {
        var ids = ["tabid-1", "tabid-2", "tabid-3"]
        var started = [true, false, true]
        var outcomes = ["pending", "pending", "loaded"]
        var times = [1000, 0, 1000]
        var expired = CleanBranchPreparation.expiredWindowPageIndexes(
                    ids, started, outcomes, times, 16000, 15000)
        compare(expired.join(","), "0")
        compare(CleanBranchPreparation.expiredWindowPageIndexes(
                    ids, started, outcomes, times, 15999, 15000).length, 0)
        compare(CleanBranchPreparation.expiredWindowPageIndexes(
                    ids, started, outcomes, times, 500, 15000).length, 0)
        compare(CleanBranchPreparation.expiredWindowPageIndexes(
                    ids, started, outcomes, [0, 0, 1000], 16000, 15000).length, 0)
        compare(CleanBranchPreparation.expiredWindowPageIndexes(
                    ids, started, outcomes, [1000], 16000, 15000).length, 0)
        compare(CleanBranchPreparation.expiredWindowPageIndexes(
                    ids, started, outcomes, times, 16000, 0).length, 0)
    }
}

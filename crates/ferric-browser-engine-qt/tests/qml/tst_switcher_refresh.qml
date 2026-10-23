import QtQuick
import QtTest
import "../../qml/components"

TestCase {
    name: "SwitcherRefresh"
    when: windowShown

    Component {
        id: fixture
        QtObject {
            id: root
            property bool switcherVisible: true
            property bool switcherLiveRefreshPending: false
            property var switcherUi: null
            property var switcherHost: null
            property var switcherProfile: null
            property string switcherOwnerToken: ""
            property string switcherScope: "tabs"
            property var switcherResultContext: null
            property var switcherResults: []
            property var switcherSourceState: ({})
            property var switcherBatchEntries: []
            property var switcherBatchMerged: []
            property var switcherBatchSources: []
            property var switcherBatchPrevious: []
            property int switcherBatchIndex: 0
            property int switcherBatchGeneration: 0
            property int switcherBatchScheduledGeneration: 0
            property string switcherBatchQuery: ""
            property string switcherBatchScope: "tabs"
            property var entries: []
            property int refreshes: 0
            property int restores: 0
            property var runtime_switcherSurface: QtObject {
                property string query: "first"
                function focusInput() {}
            }
            property var switcherRegistry: QtObject {
                function switcherInvocation() { return root.switcherVisible ? root.entries[0] : null }
                function switcherEntries() { return root.entries }
                function switcherEntryLive(entry) { return !!entry }
            }
            property var presentation: FerricWindowPresentationController { host: root }
            property var switcherRefreshTimer: Timer {
                interval: 35
                onTriggered: root.presentation.processSwitcherRefresh()
            }
            property var switcherBatchTimer: Timer {
                // Model sources arriving over multiple GUI event-loop turns.
                interval: 20
                onTriggered: root.presentation.processSwitcherBatch()
            }
            property var changes: Timer {
                interval: 10
                repeat: true
                onTriggered: root.requestSwitcherLiveRefresh()
            }
            function switcherResultsCurrent() { return presentation.switcherResultsCurrent() }
            function switcherMaxResults() { return 100 }
            function cancelSwitcherBatch() { presentation.cancelSwitcherBatch() }
            function publishedSwitcherBatch() { return presentation.publishedSwitcherBatch() }
            function switcherOwner(row) {
                return entries.find(function(entry) { return entry.ui.window_token === row.owner_token })
            }
            function refreshSwitcher() { ++refreshes; presentation.refreshSwitcher() }
            function requestSwitcherLiveRefresh() { presentation.requestSwitcherLiveRefresh() }
            function closeSwitcher() { presentation.closeSwitcher() }
            function restoreOverlayFocus(host) { ++restores }
            function edit(query) {
                runtime_switcherSurface.query = query
                cancelSwitcherBatch()
                switcherResultContext = null
                switcherResults = []
                switcherRefreshTimer.restart()
            }
        }
    }

    function source(index) {
        var ui = {window_token: "owner-" + index, queries: []}
        ui.switcher_query = function(query, scope) {
            ui.queries.push(query)
            ui.switcher_result_kinds = ["tab"]
            ui.switcher_result_ids = ["tab-" + index]
            ui.switcher_result_generations = ["1"]
            ui.switcher_result_labels = [query + " source " + index]
            ui.switcher_result_secondaries = ["https://example.test/" + index]
            ui.switcher_result_profiles = ["default"]
            ui.switcher_result_workspaces = [""]
            ui.switcher_result_actions = ["focus"]
            ui.switcher_result_ranks = [1]
            ui.switcher_result_recencies = [0]
            return true
        }
        return {ui: ui, host: {}, profile: {}}
    }
    function create(count, delay) {
        var root = createTemporaryObject(fixture, null)
        verify(root)
        var entries = []
        for (var i = 0; i < count; ++i) entries.push(source(i))
        root.entries = entries
        root.switcherUi = entries[0].ui
        root.switcherHost = entries[0].host
        root.switcherProfile = entries[0].profile
        root.switcherOwnerToken = entries[0].ui.window_token
        root.switcherBatchTimer.interval = delay
        return root
    }
    function init() {
        failOnWarning(/TypeError|ReferenceError|Binding loop|Cannot assign|Unable to assign/)
    }
    function sourceDelays() {
        return [{tag: "frame-paced", delay: 20}, {tag: "slow-first-source", delay: 100}]
    }
    function test_live_changes_do_not_postpone_an_edited_query_data() { return sourceDelays() }
    function test_live_changes_do_not_postpone_an_edited_query(data) {
        var root = create(1, data.delay)
        root.changes.start()
        root.edit("edited")
        tryVerify(function() { return root.entries[0].ui.queries.indexOf("edited") >= 0 }, 700)
        verify(root.changes.running)
        compare(root.switcherResults[0].label, "edited source 0")
    }
    function test_every_window_finishes_while_live_changes_continue_data() { return sourceDelays() }
    function test_every_window_finishes_while_live_changes_continue(data) {
        var root = create(12, data.delay)
        root.changes.start()
        root.refreshSwitcher()
        tryVerify(function() { return root.entries[11].ui.queries.length > 0 }, 3000)
        verify(root.changes.running)
        compare(root.switcherResults.length, 12)
        verify(root.entries.every(function(entry) { return entry.ui.queries.length > 0 }))
    }
    function test_query_edits_replace_an_inflight_batch_data() { return sourceDelays() }
    function test_query_edits_replace_an_inflight_batch(data) {
        var root = create(12, data.delay)
        root.changes.start()
        root.refreshSwitcher()
        tryVerify(function() { return root.switcherBatchIndex > 0 }, 300)
        verify(root.switcherBatchIndex < 12)
        root.edit("second")
        compare(root.switcherResults.length, 0)
        tryVerify(function() { return root.entries[11].ui.queries.indexOf("second") >= 0 }, 3000)
        compare(root.entries[11].ui.queries.indexOf("first"), -1)
        verify(root.switcherResults.every(function(row) { return row.label.indexOf("second ") === 0 }))
    }
    function test_close_discards_pending_refresh_and_batch_work_data() { return sourceDelays() }
    function test_close_discards_pending_refresh_and_batch_work(data) {
        var root = create(12, data.delay)
        root.changes.start()
        root.refreshSwitcher()
        tryVerify(function() { return root.switcherBatchIndex > 0 }, 300)
        root.requestSwitcherLiveRefresh()
        root.closeSwitcher()
        var refreshes = root.refreshes
        var queries = root.entries.reduce(function(sum, entry) { return sum + entry.ui.queries.length }, 0)
        wait(120)
        compare(root.refreshes, refreshes)
        compare(root.entries.reduce(function(sum, entry) { return sum + entry.ui.queries.length }, 0), queries)
        verify(!root.switcherRefreshTimer.running)
        verify(!root.switcherBatchTimer.running)
        verify(!root.switcherLiveRefreshPending)
        compare(root.switcherResults.length, 0)
        compare(root.restores, 1)
    }
}

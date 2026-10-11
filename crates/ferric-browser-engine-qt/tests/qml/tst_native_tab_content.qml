import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"

TestCase {
    name: "NativeTabContent"
    when: windowShown
    ApplicationWindow { id: host; width: 480; height: 320; visible: true }
    ListModel { id: backItems; ListElement { url: "https://a.test"; offset: -1 } }
    ListModel { id: forwardItems }
    QtObject {
        id: web
        property int tabIndex: 0
        property var history: ({ backItems: backItems, forwardItems: forwardItems })
        property url url: "https://a.test"
        property string title: "Example"
        property bool loading: false
        property string icon: ""
        function stop() { loading = false }
    }
    QtObject {
        id: ui
        property int active_tab_index: 0
        property int content_history_revision: 0
        property var entries: []
        property var snapshot: null
        property int committedCount: 0
        property string load_state: "complete"
        function content_history_available(index, offset) { return false }
        function content_navigated(index, url, anchor, traversal) {
            entries = entries.concat([{ url: url, anchor: anchor, traversal: traversal }])
        }
        function start_page_icon_path(url, create) { return "" }
        function navigation_committed_for(index, url, title) { committedCount += 1 }
        function navigation_completed_for(index) {}
        function content_history_export(index) { return JSON.stringify(snapshot) }
        function content_history_import(index, value) { snapshot = JSON.parse(value); return true }
    }
    QtObject { id: theme; property color backgroundColor: "#181825" }
    Component {
        id: fixture
        FerricNativeTabContent {
            width: 480; height: 320
            webView: web; browserUi: ui; browserWindow: theme; browserHost: host
        }
    }
    function init() {
        ui.entries = []; ui.snapshot = null; ui.committedCount = 0; web.loading = false; web.url = "https://a.test"
        backItems.clear(); backItems.append({url: "https://a.test", offset: -1})
    }
    function test_document_navigation_cannot_be_recorded_as_same_document() {
        var content = createTemporaryObject(fixture, host.contentItem)
        verify(content)
        content.beginNavigation()
        web.url = "https://b.test"
        content.noteSameDocument()
        wait(0)
        // URL changes can precede Qt's loading flag; wait for the commit callback.
        compare(ui.entries.length, 0)
        content.noteCommit()
        compare(ui.entries.length, 1)
        compare(ui.entries[0].url, "https://b.test")
    }
    function test_same_document_navigation_and_native_traversal_stay_distinct() {
        var content = createTemporaryObject(fixture, host.contentItem)
        verify(content)
        ui.snapshot = {entries: [{url: web.url.toString(), engine_index: 1}], cursor: 0}
        web.url = "https://a.test/#section"
        content.noteSameDocument()
        wait(0)
        compare(ui.snapshot.entries[0].url, web.url.toString())
        content.setContentUrl("rb://blank", true)
        compare(ui.entries.length, 1)
        compare(ui.entries[0].url, "rb://blank")
        verify(ui.entries[0].traversal)
        content.noteCommit()
        compare(ui.entries.length, 1)
    }
    function test_model_sync_and_navigation_action_wait_for_one_commit() {
        var content = createTemporaryObject(fixture, host.contentItem)
        verify(content)
        content.setContentUrl("https://b.test", false)
        // A pending engine action repeats the model's destination before the
        // engine announces that loading started; this must not add an entry.
        content.setContentUrl("https://b.test", false)
        content.noteSameDocument()
        wait(0)
        compare(ui.entries.length, 0)
        content.noteCommit()
        compare(ui.entries.length, 1)
        compare(ui.entries[0].url, "https://b.test")
    }
    function test_rapid_same_document_changes_preserve_each_destination() {
        var content = createTemporaryObject(fixture, host.contentItem)
        verify(content)
        ui.snapshot = {entries: [{url: web.url.toString(), engine_index: 0}], cursor: 0}
        web.url = "https://a.test/#one"
        content.noteSameDocument()
        web.url = "https://a.test/#two"
        backItems.clear(); backItems.append({url: "https://a.test", offset: -2}); backItems.append({url: "https://a.test/#one", offset: -1})
        content.noteSameDocument()
        wait(0)
        compare(ui.snapshot.entries.length, 3)
        compare(ui.snapshot.entries[1].url, "https://a.test/#one")
        compare(ui.snapshot.entries[2].url, "https://a.test/#two")
    }
    function test_engine_model_corrects_an_anchor_captured_before_history_updated() {
        var content = createTemporaryObject(fixture, host.contentItem)
        verify(content)
        web.url = "https://b.test"
        compare(content.engineOffset("https://a.test", -2, true), -1)
        compare(content.engineOffset("https://b.test", 0, false), 0)
        compare(content.engineOffset("https://missing.test", -2, true), null)
    }
    function test_coalesced_url_notifications_recover_the_intermediate_engine_entry() {
        backItems.clear(); backItems.append({url: "https://a.test", offset: -2}); backItems.append({url: "https://a.test/#one", offset: -1})
        web.url = "https://a.test/#two"
        ui.snapshot = { entries: [{url: "https://a.test", engine_index: 0}, {url: web.url.toString(), engine_index: 0}], cursor: 1 }
        var content = createTemporaryObject(fixture, host.contentItem)
        content.synchronizeHistory()
        compare(ui.snapshot.entries.length, 3)
        compare(ui.snapshot.entries[1].url, "https://a.test/#one")
        compare(ui.snapshot.entries[2].engine_index, 2)
        compare(ui.snapshot.cursor, 2)
    }
    function test_native_branch_does_not_reintroduce_hidden_forward_entries() {
        backItems.clear(); backItems.append({url: "https://a.test", offset: -2}); backItems.append({url: "https://b.test", offset: -1})
        web.url = "https://c.test"
        ui.snapshot = { entries: [{url: "rb://start", engine_index: 0}, {url: web.url.toString(), engine_index: 1}], cursor: 1 }
        var content = createTemporaryObject(fixture, host.contentItem)
        content.navigationBaseIndex = 1
        content.synchronizeHistory()
        compare(ui.snapshot.entries.length, 2)
        compare(ui.snapshot.entries[1].engine_index, 2)
    }
    function test_same_url_engine_state_keeps_both_history_positions() {
        backItems.clear(); backItems.append({url: "https://a.test", offset: -2}); backItems.append({url: "https://b.test", offset: -1})
        web.url = "https://b.test"
        ui.snapshot = { entries: [{url: "https://a.test", engine_index: 0}, {url: web.url.toString(), engine_index: 1}], cursor: 1 }
        var content = createTemporaryObject(fixture, host.contentItem)
        content.synchronizeHistory()
        compare(ui.snapshot.entries.length, 3)
        compare(ui.snapshot.entries[1].engine_index, 1)
        compare(ui.snapshot.entries[2].engine_index, 2)
    }
    function test_new_view_cannot_commit_its_placeholder_before_the_navigation_action() {
        var content = createTemporaryObject(fixture, host.contentItem)
        content.setContentUrl(web.url.toString(), false)
        compare(ui.committedCount, 0)
        compare(ui.entries.length, 0)
        content.noteCommit()
        content.setContentUrl(web.url.toString(), false)
        compare(ui.committedCount, 1)
    }
}

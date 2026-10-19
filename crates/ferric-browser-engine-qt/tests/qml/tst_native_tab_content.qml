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
    ListModel { id: provisionalTabs }
    QtObject {
        id: web
        property int tabIndex: 0
        property bool ownerValid: true
        function ownsTab() { return ownerValid && tabIndex >= 0 }
        property var history: ({ backItems: backItems, forwardItems: forwardItems })
        property url url: "https://a.test"
        property string title: "Example"
        property bool loading: false
        property string icon: ""
        property var historyObservation: null
        property bool deferProbe: false
        property var probeCallback: null
        function runJavaScript(source, world, callback) {
            if (deferProbe) probeCallback = callback
            else callback(historyObservation)
        }
        function stop() { loading = false }
    }
    QtObject {
        id: ui
        property int active_tab_index: 0
        property int input_epoch: 7
        property int content_history_revision: 0
        property var entries: []
        property var snapshot: null
        property var begunSameDocumentChanges: []
        property var completedSameDocumentChanges: []
        property var urlChanges: []
        property var documentIdentities: []
        property int committedCount: 0
        property string load_state: "complete"
        function content_history_available(index, offset) { return false }
        function tab_id_for_index(index) { return index === 0 ? "tab-a" : "" }
        function page_document_identity_for(index, tabId, document, epoch) {
            documentIdentities = documentIdentities.concat([{
                index: index, tabId: tabId, document: document, epoch: epoch
            }])
        }
        function begin_same_document_url_change_for(index, tabId, url, document, epoch) {
            begunSameDocumentChanges = begunSameDocumentChanges.concat([{
                index: index, tabId: tabId, url: url, epoch: epoch, document: document
            }])
        }
        function complete_same_document_url_change_for(index, tabId, url, document, epoch) {
            completedSameDocumentChanges = completedSameDocumentChanges.concat([{
                index: index, tabId: tabId, url: url, epoch: epoch, document: document
            }])
        }
        function navigation_url_changed_for(index, url) {
            urlChanges = urlChanges.concat([{index: index, url: url}])
        }
        function content_navigated(index, url, anchor, traversal) {
            entries = entries.concat([{ url: url, anchor: anchor, traversal: traversal }])
        }
        function start_page_icon_path(url, create) { return "" }
        function navigation_committed_for(index, url, title) { committedCount += 1 }
        function navigation_completed_for(index) {}
        function content_history_export(index) { return JSON.stringify(snapshot) }
        function content_history_import(index, value) { snapshot = JSON.parse(value); return true }
        function content_engine_traversed(index, url, anchor) {
            for (var row = 0; row < snapshot.entries.length; ++row) {
                if (snapshot.entries[row].url === url && snapshot.entries[row].engine_index === anchor) {
                    snapshot.cursor = row
                    return true
                }
            }
            return false
        }
    }
    QtObject {
        id: theme
        property color backgroundColor: "#181825"
        property int browserScriptWorld: 1
        function runBrowserScript(view, script, callback, world) { view.runJavaScript(script, world, callback); return true }
    }
    Component {
        id: fixture
        FerricNativeTabContent {
            width: 480; height: 320
            webView: web; browserUi: ui; browserWindow: theme; browserHost: host
        }
    }
    function init() {
        web.tabIndex = 0; web.ownerValid = true; ui.active_tab_index = 0
        provisionalTabs.clear(); provisionalTabs.append({url:"https://a.test",loaded:true})
        ui.entries = []; ui.snapshot = null; ui.begunSameDocumentChanges = []
        ui.completedSameDocumentChanges = []; ui.urlChanges = []; ui.documentIdentities = []
        ui.committedCount = 0; ui.input_epoch = 7; web.loading = false; web.url = "https://a.test"
        backItems.clear(); backItems.append({url: "https://a.test", offset: -1})
        web.historyObservation = null; web.deferProbe = false; web.probeCallback = null
    }
    function test_same_document_url_change_requires_matching_document_identity() {
        var content = createTemporaryObject(fixture, host.contentItem)
        verify(content)
        web.historyObservation = {document: "document-one", sequence: 0, url: web.url.toString()}
        content.synchronizeHistory()
        compare(content.historyDocument, "document-one")

        web.url = "https://a.test/#section"
        web.historyObservation = {document: "document-one", sequence: 0, url: web.url.toString()}
        content.noteSameDocument(web.url.toString())
        wait(0)
        compare(ui.begunSameDocumentChanges.length, 1)
        compare(ui.begunSameDocumentChanges[0].index, 0)
        compare(ui.begunSameDocumentChanges[0].tabId, "tab-a")
        compare(ui.begunSameDocumentChanges[0].url, web.url.toString())
        compare(ui.begunSameDocumentChanges[0].epoch, 7)
        compare(ui.begunSameDocumentChanges[0].document, "document-one")
        compare(ui.completedSameDocumentChanges.length, 1)
        compare(ui.completedSameDocumentChanges[0].document, "document-one")
        compare(ui.urlChanges.length, 0)
    }
    function test_document_replacement_uses_baseline_url_change_path() {
        var content = createTemporaryObject(fixture, host.contentItem)
        verify(content)
        web.historyObservation = {document: "document-one", sequence: 0, url: web.url.toString()}
        content.synchronizeHistory()

        web.url = "https://b.test/"
        web.historyObservation = {document: "document-two", sequence: 0, url: web.url.toString()}
        content.noteSameDocument(web.url.toString())
        wait(0)
        compare(ui.begunSameDocumentChanges.length, 1)
        compare(ui.completedSameDocumentChanges.length, 1)
        compare(ui.completedSameDocumentChanges[0].document, "")
        compare(ui.urlChanges.length, 0)
    }
    function test_delayed_identity_result_is_rejected_after_navigation_start() {
        var content = createTemporaryObject(fixture, host.contentItem)
        verify(content)
        web.historyObservation = {document: "document-one", sequence: 0, url: web.url.toString()}
        content.synchronizeHistory()
        web.deferProbe = true
        web.url = "https://a.test/#section"
        content.noteSameDocument(web.url.toString())
        tryVerify(function() { return !!web.probeCallback }, 1000)
        var callback = web.probeCallback
        verify(callback)

        content.beginNavigation()
        callback({document: "document-one", sequence: 0, url: "https://a.test/#section"})
        compare(ui.begunSameDocumentChanges.length, 1)
        compare(ui.completedSameDocumentChanges.length, 0)
        compare(ui.urlChanges.length, 0)
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
    function test_first_same_url_state_keeps_the_original_entry() {
        backItems.clear(); backItems.append({url:"https://a.test",offset:-1})
        ui.snapshot = {entries:[{url:"https://a.test",engine_index:0}],cursor:0}
        var content = createTemporaryObject(fixture, host.contentItem)
        content.synchronizeHistory()
        compare(ui.snapshot.entries.length,2)
        compare(ui.snapshot.entries[0].engine_index,0)
        compare(ui.snapshot.entries[1].engine_index,1)
        compare(ui.snapshot.cursor,1)
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
    function test_engine_back_waits_for_the_history_model_and_preserves_forward() {
        backItems.clear(); backItems.append({url: "https://a.test", offset: -2}); backItems.append({url: "https://b.test", offset: -1})
        web.url = "https://c.test"
        ui.snapshot = {entries: [{url: "https://a.test", engine_index: 0}, {url: "https://b.test", engine_index: 1}, {url: web.url.toString(), engine_index: 2}], cursor: 2}
        var content = createTemporaryObject(fixture, host.contentItem)
        content.beginNavigation(true)
        web.url = "https://b.test"
        content.noteCommit()
        wait(0)
        compare(ui.entries.length, 0)
        compare(ui.snapshot.cursor, 2)
        verify(content.engineTraversalPending)
        backItems.clear(); backItems.append({url: "https://a.test", offset: -1})
        wait(0)
        compare(ui.snapshot.cursor, 1)
        compare(ui.snapshot.entries.length, 3)
        verify(!content.engineTraversalPending)
        content.beginNavigation(true)
        web.url = "https://c.test"
        backItems.clear(); backItems.append({url: "https://a.test", offset: -2}); backItems.append({url: "https://b.test", offset: -1})
        content.noteCommit()
        wait(0)
        compare(ui.snapshot.cursor, 2)
        compare(ui.snapshot.entries.length, 3)
        compare(ui.entries.length, 0)
    }
    function test_same_document_engine_back_selects_the_previous_same_url_state() {
        ui.snapshot = {entries: [{url: "https://a.test", engine_index: 0}, {url: "https://b.test", engine_index: 1}, {url: "https://b.test", engine_index: 2}], cursor: 2}
        web.url = "https://b.test"
        var content = createTemporaryObject(fixture, host.contentItem)
        content.synchronizeHistory()
        compare(ui.snapshot.cursor, 1)
        compare(ui.snapshot.entries.length, 3)
        compare(ui.entries.length, 0)
    }
    function test_browser_back_corrects_a_stale_anchor_without_duplicating_its_target() {
        ui.snapshot = {entries: [{url: "https://a.test", engine_index: 0}, {url: "https://b.test", engine_index: 2}, {url: "https://c.test", engine_index: 2}], cursor: 1}
        web.url = "https://b.test"
        var content = createTemporaryObject(fixture, host.contentItem)
        content.synchronizeHistory()
        compare(ui.snapshot.cursor, 1)
        compare(ui.snapshot.entries.length, 3)
        compare(ui.snapshot.entries[1].engine_index, 1)
        compare(ui.snapshot.entries[2].url, "https://c.test")
    }
    function test_same_document_forward_preserves_the_remaining_forward_branch() {
        ui.snapshot = {entries: [{url: "https://a.test", engine_index: 0}, {url: "https://b.test", engine_index: 1}, {url: "https://c.test", engine_index: 2}], cursor: 0}
        web.url = "https://b.test"
        web.historyObservation = {document: "document-one", sequence: 1, url: web.url.toString()}
        var content = createTemporaryObject(fixture, host.contentItem)
        content.synchronizeHistory()
        compare(ui.snapshot.cursor, 1)
        compare(ui.snapshot.entries.length, 3)
        compare(ui.snapshot.entries[2].url, "https://c.test")
    }
    function test_same_url_forward_and_new_push_have_different_branch_behavior() {
        backItems.clear(); backItems.append({url: "about:blank", offset: -2}); backItems.append({url: "https://a.test", offset: -1})
        var original = {entries: [{url: "https://a.test", engine_index: 1}, {url: "https://a.test", engine_index: 2}, {url: "rb://start", engine_index: 2}], cursor: 0}
        ui.snapshot = JSON.parse(JSON.stringify(original))
        web.historyObservation = {document: "document-one", sequence: 1, url: web.url.toString()}
        var content = createTemporaryObject(fixture, host.contentItem)
        content.synchronizeHistory()
        compare(ui.snapshot.cursor, 1)
        compare(ui.snapshot.entries.length, 3)
        // No further popstate: this identical URL was pushed as a new entry.
        ui.snapshot = JSON.parse(JSON.stringify(original))
        content.synchronizeHistory()
        compare(ui.snapshot.cursor, 1)
        compare(ui.snapshot.entries.length, 2)
    }
    function test_history_probe_cannot_overwrite_a_new_native_destination() {
        ui.snapshot = {entries: [{url: "https://a.test", engine_index: 0}], cursor: 0}
        web.deferProbe = true
        var content = createTemporaryObject(fixture, host.contentItem)
        content.synchronizeHistory()
        var callback = web.probeCallback
        verify(content.historyProbePending)
        content.setContentUrl("rb://blank", false)
        callback({document: "document-one", sequence: 1, url: web.url.toString()})
        verify(!content.historyProbePending)
        compare(content.nativeUrl, "rb://blank")
        compare(ui.snapshot.cursor, 0)
        compare(ui.snapshot.entries.length, 1)
    }
    function test_unowned_initializer_cannot_update_an_active_tab_data() {
        return [{tag:"web",url:"about:blank",index:-1,owned:true},
                {tag:"native",url:"rb://blank",index:-1,owned:true},
                {tag:"retired-web",url:"about:blank",index:0,owned:false},
                {tag:"retired-native",url:"rb://blank",index:0,owned:false}]
    }
    function test_unowned_initializer_cannot_update_an_active_tab(row) {
        web.tabIndex=row.index
        web.ownerValid=row.owned
        ui.active_tab_index=0
        var content=createTemporaryObject(fixture,host.contentItem,{tabsModel:provisionalTabs})
        verify(content)
        content.setContentUrl(row.url,false)
        compare(provisionalTabs.get(0).url,"https://a.test")
        compare(ui.entries.length,0)
        compare(ui.committedCount,0)
    }

    function test_delayed_history_fact_is_ignored_after_ownership_is_lost_data() {
        return [{tag:"detached",indexLost:true},{tag:"retired",indexLost:false}]
    }
    function test_delayed_history_fact_is_ignored_after_ownership_is_lost(row) {
        var content=createTemporaryObject(fixture,host.contentItem)
        verify(content)
        web.deferProbe=true
        content.synchronizeHistory()
        var callback=web.probeCallback
        verify(typeof callback==="function")
        if (row.indexLost) web.tabIndex=-1; else web.ownerValid=false
        callback({document:"old-document",sequence:0,url:web.url.toString()})
        compare(ui.documentIdentities.length,0)
        compare(ui.completedSameDocumentChanges.length,0)
        compare(ui.entries.length,0)
        web.tabIndex=0;web.ownerValid=true
        content.synchronizeHistory()
        verify(web.probeCallback!==callback,"A valid owner must be able to launch a fresh probe")
        web.probeCallback({document:"new-document",sequence:0,url:web.url.toString()})
        compare(ui.documentIdentities.length,1)
        compare(ui.documentIdentities[0].document,"new-document")
    }

}

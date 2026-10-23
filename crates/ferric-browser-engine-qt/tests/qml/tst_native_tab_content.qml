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
        property var viewUi: ui
        property var viewHost: host
        property var documentRequestOwner: ({})
        property var documentTitleTarget: null
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
        property int completedCount: 0
        property string load_state: "complete"
        property string status_text: ""
        property bool targetCurrent: true
        property var navigationKinds: []
        function navigation_kind_for(index, history) { navigationKinds = navigationKinds.concat([{index:index,history:history}]) }
        function navigation_target_for_index(index) { return index === 0 ? {tabId:"tab-a",generation:"1",documentId:"1"} : null }
        function request_target_current(tabId, generation, documentId) { return targetCurrent && tabId === "tab-a" && generation === "1" && documentId === "1" }
        function content_history_available(index, offset) {
            return !!snapshot && snapshot.cursor + offset >= 0 && snapshot.cursor + offset < snapshot.entries.length
        }
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
        function navigation_completed_for(index) { completedCount += 1; load_state = "complete" }
        function navigation_stopped_for(index) { load_state = "cancelled" }
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
        ui.committedCount = 0; ui.completedCount = 0; ui.load_state = "complete"
        ui.input_epoch = 7; web.loading = false; web.url = "https://a.test"
        ui.status_text="";ui.targetCurrent=true;web.viewUi=ui;web.viewHost=host;web.documentRequestOwner={}
        ui.navigationKinds=[]
        backItems.clear(); backItems.append({url: "https://a.test", offset: -1})
        forwardItems.clear()
        web.historyObservation = null; web.deferProbe = false; web.probeCallback = null
    }
    function test_navigation_kind_follows_the_live_engine_policy_data() {
        return [{tag:"history",history:true},{tag:"fresh",history:false}]
    }
    function test_navigation_kind_follows_the_live_engine_policy(row) {
        var content=createTemporaryObject(fixture,host.contentItem)
        ui.active_tab_index=1
        content.beginNavigation(row.history,false)
        compare(ui.navigationKinds.length,1)
        compare(ui.navigationKinds[0].index,0)
        compare(ui.navigationKinds[0].history,row.history)
        web.ownerValid=false
        content.beginNavigation(!row.history,false)
        compare(ui.navigationKinds.length,1)
    }
    function test_streaming_document_enters_history_without_completing_the_load() {
        ui.snapshot={entries:[{url:"https://a.test",engine_index:0}],cursor:0}
        var content=createTemporaryObject(fixture,host.contentItem)
        content.beginNavigation(false,true)
        content.engineLoadState="loading";ui.load_state="provisional"
        web.url="https://b.test";web.loading=true
        web.historyObservation={document:"streaming-document",sequence:0,url:web.url.toString()}
        content.synchronizeHistory()
        compare(ui.snapshot.entries.length,2)
        compare(ui.snapshot.cursor,1)
        compare(ui.snapshot.entries[1].url,"https://b.test")
        verify(content.canNavigateBack)
        compare(content.engineLoadState,"loading")
        compare(ui.load_state,"provisional")
        compare(ui.committedCount,0);compare(ui.completedCount,0)
        compare(ui.documentIdentities.length,0)
        verify(!content.engineDocumentReady)
        verify(!content.engineDocumentHasFormData)
        verify(content.navigationPending)
    }
    function test_history_command_waits_for_the_document_probe_and_accumulates_count() {
        ui.snapshot={entries:[{url:"https://a.test",engine_index:0}],cursor:0}
        var content=createTemporaryObject(fixture,host.contentItem)
        content.beginNavigation(false,false)
        web.url="https://b.test";web.loading=true;web.deferProbe=true
        var calls=[]
        var perform=function(backwards,count){calls.push({backwards:backwards,count:count,cursor:ui.snapshot.cursor,url:ui.snapshot.entries[ui.snapshot.cursor].url})}
        content.requestHistoryTraversal(true,1,perform)
        content.requestHistoryTraversal(true,2,perform)
        content.requestHistoryTraversal(false,1,perform)
        compare(calls.length,0)
        web.probeCallback({document:"streaming-document",sequence:0,url:web.url.toString()})
        compare(calls.length,1)
        verify(calls[0].backwards);compare(calls[0].count,2)
        compare(calls[0].cursor,1);compare(calls[0].url,"https://b.test")
        compare(content.historyTraversalRequest,null)
    }
    function test_encoded_streaming_history_uses_the_visible_document_data() {
        return [{tag:"committed-response",pendingHeaders:false},
                {tag:"pending-headers",pendingHeaders:true}]
    }
    function test_encoded_streaming_history_uses_the_visible_document(data) {
        var previous="https://a.test/caf%C3%A9?label=owned%20space%2F%CE%BB"
        var next="https://b.test/caf%C3%A9?label=owned%20space%2F%CE%BB"
        web.url=previous
        backItems.setProperty(0,"url",previous)
        ui.snapshot={entries:[{url:web.url.toString(),engine_index:0}],cursor:0}
        var content=createTemporaryObject(fixture,host.contentItem)
        content.beginNavigation(false,false)
        content.engineLoadState="loading";ui.load_state="provisional"
        web.url=next;web.loading=true
        web.historyObservation={document:"owned-document",sequence:0,
            url:data.pendingHeaders ? previous : next}
        var requests=[]
        content.requestHistoryTraversal(true,1,function(backwards,count) {
            requests.push({backwards:backwards,count:count})
        })
        compare(requests.length,1)
        compare(requests[0].backwards,true)
        compare(requests[0].count,1)
        compare(ui.snapshot.entries.length,data.pendingHeaders ? 1 : 2)
        compare(ui.load_state,"provisional")
    }
    function test_pending_headers_keep_the_visible_documents_history_position() {
        ui.snapshot={entries:[{url:"https://first.test",engine_index:0},{url:"https://a.test",engine_index:1}],cursor:1}
        var content=createTemporaryObject(fixture,host.contentItem)
        content.beginNavigation(false,false)
        web.url="https://waiting.test";web.loading=true
        web.historyObservation={document:"previous-document",sequence:0,url:"https://a.test"}
        var called=false
        content.requestHistoryTraversal(true,1,function(backwards,count){called=true;verify(backwards);compare(count,1);compare(ui.snapshot.cursor,1)})
        verify(called)
        compare(ui.snapshot.entries.length,2)
        compare(ui.snapshot.entries[1].url,"https://a.test")
        verify(content.navigationPending)
    }
    function test_unproven_streaming_history_does_not_choose_a_stale_destination_data() {
        return [{tag:"no-probe",value:null},{tag:"foreign-page",value:{document:"other",sequence:0,url:"https://foreign.test"}}]
    }
    function test_unproven_streaming_history_does_not_choose_a_stale_destination(row) {
        ui.snapshot={entries:[{url:"https://a.test",engine_index:0}],cursor:0}
        var content=createTemporaryObject(fixture,host.contentItem)
        content.beginNavigation(false,false)
        web.url="https://b.test";web.loading=true;web.historyObservation=row.value
        var called=false
        content.requestHistoryTraversal(true,1,function(){called=true})
        verify(!called)
        compare(ui.snapshot.entries.length,1)
        compare(ui.status_text,"History is changing; try again")
        compare(content.historyTraversalRequest,null)
    }
    function test_delayed_history_command_expires_with_its_owner_data() {
        return ["target","document","navigation","address","view-ui","view-host","retired","native"].map(function(tag){return {tag:tag}})
    }
    function test_delayed_history_command_expires_with_its_owner(row) {
        ui.snapshot={entries:[{url:"https://a.test",engine_index:0}],cursor:0}
        var content=createTemporaryObject(fixture,host.contentItem)
        web.loading=true;web.deferProbe=true
        var called=false
        content.requestHistoryTraversal(true,1,function(){called=true})
        var callback=web.probeCallback
        verify(typeof callback==="function")
        if(row.tag==="target")ui.targetCurrent=false
        else if(row.tag==="document")web.documentRequestOwner={}
        else if(row.tag==="navigation")content.beginNavigation()
        else if(row.tag==="address")web.url="https://other.test"
        else if(row.tag==="view-ui")web.viewUi=null
        else if(row.tag==="view-host")web.viewHost=null
        else if(row.tag==="retired")web.ownerValid=false
        else content.setContentUrl("rb://blank",false)
        callback({document:"old-document",sequence:0,url:"https://a.test"})
        verify(!called)
    }
    function test_native_history_command_uses_its_logical_history_immediately() {
        var content=createTemporaryObject(fixture,host.contentItem)
        content.nativeUrl="rb://start"
        web.deferProbe=true
        var called=false
        content.requestHistoryTraversal(true,3,function(backwards,count){called=true;verify(backwards);compare(count,3)})
        verify(called)
        compare(web.probeCallback,null)
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
    function test_explicit_fragment_visits_keep_the_existing_document_data() {
        var rows=[]
        for (var base of ["https://a.test/","https://a.test/caf%C3%A9?label=owned%20space%2F%CE%BB"]) {
            for (var post of [false,true]) {
                for (var suffix of ["#one","#two","#","#section%20%CE%BB"]) {
                    rows.push({tag:base+(post ? "post" : "get")+suffix,base:base,suffix:suffix,post:post})
                }
            }
        }
        return rows
    }
    function test_explicit_fragment_visits_keep_the_existing_document(data) {
        web.url=data.base+"#one"
        var content=createTemporaryObject(fixture,host.contentItem)
        content.beginNavigation(false,data.post)
        content.noteCommit(true)
        ui.load_state="provisional"
        content.setContentUrl(data.base+data.suffix,false,false,true)
        compare(web.url.toString(),Qt.resolvedUrl(data.base+data.suffix).toString())
        if(data.suffix!=="#one") {
            compare(ui.completedCount,0,"A changed fragment must wait for its engine history entry")
            backItems.append({url:data.base+"#one",offset:-1})
            tryCompare(ui,"completedCount",1)
        }
        compare(ui.completedCount,1)
        compare(ui.load_state,"complete")
        compare(web.documentTitleTarget,ui.navigation_target_for_index(0))
        verify(!content.navigationPending)
        compare(content.engineDocumentHasFormData,data.post)
        compare(content.navigationHasFormData,data.post)
    }
    function test_fragment_open_preserves_encoded_separators_data() {
        return [{tag:"path",current:"https://a.test/a/b#one",next:"https://a.test/a%2Fb#one"},
                {tag:"query",current:"https://a.test/?q=a/b#one",next:"https://a.test/?q=a%2Fb#one"}]
    }
    function test_fragment_open_preserves_encoded_separators(data) {
        web.url=data.current
        var content=createTemporaryObject(fixture,host.contentItem)
        content.noteCommit(true)
        ui.load_state="provisional"
        content.setContentUrl(data.next,false,false,true)
        compare(ui.completedCount,0)
        compare(content.fragmentVisit,null)
        verify(content.navigationPending)
    }
    function test_fragment_open_does_not_settle_a_failed_or_pending_document_data() {
        return [{tag:"failure"},{tag:"failed-post"},{tag:"pending"}]
    }
    function test_fragment_open_does_not_settle_a_failed_or_pending_document(data) {
        web.url="https://a.test/#one"
        var content=createTemporaryObject(fixture,host.contentItem)
        if(data.tag==="failed-post") {
            content.beginNavigation(false,true)
            content.noteCommit(false)
        } else content.noteCommit(data.tag!=="failure")
        if(data.tag==="pending") web.loading=true
        ui.load_state="provisional"
        content.setContentUrl("https://a.test/#one",false,false,true)
        compare(ui.completedCount,0)
        compare(ui.load_state,"provisional")
        verify(content.navigationPending)
    }
    function test_fragment_visit_does_not_inherit_an_unstarted_form_navigation() {
        web.url="https://a.test/#one"
        var content=createTemporaryObject(fixture,host.contentItem)
        content.noteCommit(true)
        content.beginNavigation(false,true)
        content.setContentUrl("https://a.test/#two",false,false,true)
        backItems.append({url:"https://a.test/#one",offset:-1})
        tryCompare(ui,"completedCount",1)
        verify(!content.engineDocumentHasFormData)
        verify(!content.navigationHasFormData)
        compare(ui.completedCount,1)
    }
    function test_fragment_completion_is_discarded_after_new_navigation_or_owner_loss_data() {
        return [{tag:"navigation"},{tag:"owner"},{tag:"load"}]
    }
    function test_fragment_completion_is_discarded_after_new_navigation_or_owner_loss(data) {
        web.url="https://a.test/#one"
        var content=createTemporaryObject(fixture,host.contentItem)
        content.noteCommit(true)
        content.setContentUrl("https://a.test/#two",false,false,true)
        compare(ui.completedCount,0)
        if(data.tag==="navigation") content.setContentUrl("https://b.test/",false,false,true)
        else if(data.tag==="owner") web.ownerValid=false
        else content.engineLoadState="loading"
        backItems.append({url:"https://a.test/#one",offset:-1})
        wait(0)
        compare(ui.completedCount,0)
    }
    function test_fragment_open_after_a_proven_spa_route_keeps_the_document() {
        var content=createTemporaryObject(fixture,host.contentItem)
        content.noteCommit(true)
        web.historyObservation={document:"same-document",sequence:0,url:web.url.toString()}
        content.synchronizeHistory()
        web.url="https://a.test/new-route#one"
        web.historyObservation={document:"same-document",sequence:1,url:web.url.toString()}
        content.noteSameDocument(web.url.toString())
        tryCompare(content,"engineDocumentUrl",web.url.toString())
        content.setContentUrl("https://a.test/new-route#two",false,false,true)
        compare(ui.completedCount,0)
        backItems.append({url:"https://a.test/new-route#one",offset:-1})
        tryCompare(ui,"completedCount",1)
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
    function test_browser_traversal_keeps_its_visit_while_native_facts_settle_data() {
        return ["model-first", "url-first", "same-url"].map(function(tag) { return {tag:tag} })
    }
    function test_browser_traversal_keeps_its_visit_while_native_facts_settle(row) {
        var destination = "https://a.test"
        var previous = row.tag === "same-url" ? destination : "https://b.test"
        backItems.clear(); backItems.append({url:destination, offset:-1})
        web.url = previous
        ui.snapshot = {entries:[
            {url:destination, engine_index:0, journey_node:"visit-a"},
            {url:previous, engine_index:1, journey_node:"visit-b"}
        ], cursor:0}
        var content = createTemporaryObject(fixture, host.contentItem)
        content.setContentUrl(destination, true, true, false, 0)
        content.beginNavigation(true, false)
        web.loading = true; ui.load_state = "provisional"
        if (row.tag === "model-first") backItems.clear()
        else web.url = destination
        web.historyObservation = {document:"settling-document", sequence:0, url:web.url.toString()}
        content.synchronizeHistory()
        compare(ui.snapshot.cursor, 0)
        compare(ui.snapshot.entries.length, 2)
        compare(ui.snapshot.entries[0].url, destination)
        compare(ui.snapshot.entries[0].journey_node, "visit-a")
        verify(content.traversalPending)
        web.url = destination; backItems.clear()
        web.historyObservation = {document:"destination-document", sequence:0, url:destination}
        content.synchronizeHistory()
        compare(ui.snapshot.cursor, 0)
        compare(ui.snapshot.entries[0].journey_node, "visit-a")
        compare(ui.snapshot.entries[1].journey_node, "visit-b")
        compare(ui.committedCount, 0)
        compare(ui.completedCount, 0)
    }
    function test_browser_back_keeps_its_anchor_when_success_precedes_the_native_model_data() {
        return ["ordinary", "same-url", "stale-anchor", "redirect"].map(function(tag) { return {tag:tag} })
    }
    function test_browser_back_keeps_its_anchor_when_success_precedes_the_native_model(row) {
        var destination = "https://a.test"
        var committed = row.tag === "redirect" ? "https://redirected.test" : destination
        web.url = row.tag === "same-url" ? destination : "https://b.test"
        backItems.clear(); backItems.append({url:destination, offset:-1})
        ui.snapshot = {entries:[
            {url:destination, engine_index:row.tag === "stale-anchor" ? 2 : 0, journey_node:"visit-a"},
            {url:web.url.toString(), engine_index:1, journey_node:"visit-b"}
        ], cursor:0}
        var content = createTemporaryObject(fixture, host.contentItem)
        content.setContentUrl(destination, true, true, false, 0)
        content.beginNavigation(true, false)
        web.url = committed; web.loading = false; ui.load_state = "provisional"
        content.noteCommit(true)
        compare(ui.entries.length, 0, "Success cannot publish the preceding native history index")
        verify(content.traversalPending)
        ui.load_state = "complete"
        web.historyObservation = {document:"destination-document", sequence:0, url:committed}
        content.synchronizeHistory()
        compare(ui.snapshot.cursor, 0)
        compare(ui.snapshot.entries.length, 2)
        verify(content.traversalPending)
        backItems.clear()
        content.synchronizeHistory()
        compare(ui.entries.length, 1)
        compare(ui.entries[0].anchor, 0)
        compare(ui.entries[0].url, committed)
        verify(ui.entries[0].traversal)
        compare(ui.snapshot.entries[0].journey_node, "visit-a")
        compare(ui.snapshot.entries[1].journey_node, "visit-b")
        verify(!content.traversalPending)
    }
    function test_counted_forward_waits_for_the_preceding_backs_native_position_data() {
        return ["forward-two", "combined", "expired-owner"].map(function(tag) { return {tag:tag} })
    }
    function test_counted_forward_waits_for_the_preceding_backs_native_position(row) {
        backItems.clear(); backItems.append({url:"https://a.test", offset:-1})
        forwardItems.clear(); forwardItems.append({url:"https://c.test", offset:1})
        web.url = "https://b.test"
        ui.snapshot = {entries:[
            {url:"https://a.test", engine_index:0},
            {url:"https://b.test", engine_index:1},
            {url:"https://c.test", engine_index:2}
        ], cursor:0}
        var content = createTemporaryObject(fixture, host.contentItem)
        content.setContentUrl("https://a.test", true, true, false, 0)
        content.beginNavigation(true, false)
        web.url = "https://a.test"; web.loading = false; ui.load_state = "provisional"
        content.noteCommit(true); ui.load_state = "complete"
        web.historyObservation = {document:"returned-document", sequence:1, url:"https://a.test"}
        var calls = []
        content.requestHistoryTraversal(false, 2, function(backwards, count) {
            var destination = count === 1 ? "https://b.test" : "https://c.test"
            calls.push({backwards:backwards, count:count, offset:content.engineOffset(destination, count, false)})
        })
        compare(calls.length, 0, "A proven document must still wait for its native history position: " + JSON.stringify(calls))
        if (row.tag === "combined") content.requestHistoryTraversal(true, 1, function() { fail("The pending callback must retain its owner") })
        if (row.tag === "expired-owner") ui.targetCurrent = false
        backItems.clear()
        forwardItems.clear()
        forwardItems.append({url:"https://b.test", offset:1})
        forwardItems.append({url:"https://c.test", offset:2})
        content.synchronizeHistory()
        compare(calls.length, row.tag === "expired-owner" ? 0 : 1)
        if (calls.length > 0) {
            verify(!calls[0].backwards)
            compare(calls[0].count, row.tag === "combined" ? 1 : 2)
            compare(calls[0].offset, calls[0].count)
        }
        compare(content.historyTraversalRequest, null)
        verify(!content.sameDocumentTraversalPending)
        forwardItems.clear()
    }
    function test_stopping_a_browser_traversal_releases_its_expected_native_position() {
        var content = createTemporaryObject(fixture, host.contentItem)
        content.setContentUrl("https://earlier.test", true, true, false, 0)
        content.beginNavigation(true, false)
        content.noteStopped()
        compare(content.requestedHistoryIndex, -1)
        verify(!content.traversalPending)
        verify(!content.navigationPending)
        compare(ui.load_state, "cancelled")
    }
    function test_stay_on_a_completed_page_releases_its_expected_native_position() {
        var content = createTemporaryObject(fixture, host.contentItem)
        content.engineLoadState = "complete"
        content.setContentUrl("https://earlier.test", true, true, false, 0)
        content.beginNavigation(true, false)
        content.noteBeforeUnloadRejected()
        compare(content.requestedHistoryIndex, -1)
        verify(!content.traversalPending)
        compare(ui.load_state, "complete")
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

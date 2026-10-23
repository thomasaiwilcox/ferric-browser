import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"

TestCase {
    id: test
    name: "FormResubmission"
    when: windowShown
    ListModel { id: historyItems }
    ApplicationWindow {
        id: host
        width: 640; height: 480; visible: true
        property var currentView: web
        function activeWebView() { return currentView }
        property real chromeScale: 1
        property color backgroundColor: "#181825"
        property color surfaceColor: "#313244"
        property color panelColor: "#1e1e2e"
        property color primaryTextColor: "#cdd6f4"
        property color secondaryTextColor: "#bac2de"
        property color mutedTextColor: "#a6adc8"
        property color borderColor: "#585b70"
        property color accentColor: "#89b4fa"
        property color warningColor: "#f9e2af"
        property color selectionColor: "#45475a"
        property color selectionTextColor: "#cdd6f4"
    }
    QtObject {
        id: ui
        property int active_tab_index: 0
        property int input_epoch: 0
        property bool targetValid: true
        property string load_state: "provisional"
        property string status_text: "Reload requested"
        property int completed: 0
        property int failed: 0
        property int stopped: 0
        property bool navigation_failure_visible: false
        property int content_history_revision: 0
        function tab_id_for_index(index) { return index === 0 ? "tab" : "other" }
        function request_target_current(tabId, generation, documentId) {
            return targetValid && tabId === "tab" && generation === 3 && documentId === 4
        }
        function navigation_target_for_index(index) {
            return index === 0 && targetValid ? {tabId:"tab",generation:3,documentId:4} : null
        }
        function navigation_kind_for(index, history) {}
        function navigation_completed_for(index) { completed++; if (index === active_tab_index) load_state="complete" }
        function navigation_failed_for(index) { failed++; if (index === active_tab_index) load_state="failed" }
        function navigation_stopped_for(index) { stopped++; if (index === active_tab_index) load_state="cancelled" }
        function clear_navigation_failure() { navigation_failure_visible=false }
        function content_history_available(index, offset) { return false }
        function content_history_export(index) { return "null" }
        function content_navigated(index, url, anchor, traversal) {}
    }
    Item {
        id: web
        parent: host.contentItem
        width: 20; height: 20
        property int tabIndex: 0
        property bool live: true
        function ownsTab() { return live }
        property var viewUi: ui
        property var viewHost: host
        property var documentRequestOwner: ({})
        property url url: "https://example.test/posted#result"
        property string icon: ""
        property var history: ({backItems:historyItems,forwardItems:historyItems})
        property bool loading: false
        property int reloads: 0
        property int bypassReloads: 0
        function reload() { reloads++ }
        function reloadAndBypassCache() { bypassReloads++ }
    }
    Component {
        id: previousControlFixture
        Button { text: "Previous dialog"; width: 120; height: 40 }
    }
    Component {
        id: contentFixture
        FerricNativeTabContent {
            width: host.width; height: host.height
            webView: web; browserUi: ui; browserWindow: host; browserHost: host
            engineLoadState: "complete"
            engineDocumentHasFormData: true
        }
    }
    function init() {
        failOnWarning(/TypeError|ReferenceError|Binding loop|Cannot assign|Unable to assign/)
        host.width=640;host.height=480;host.font.pointSize=10
        host.requestActivate();tryCompare(host,"active",true)
        host.currentView=web;web.live=true;web.viewHost=host;web.viewUi=ui;web.tabIndex=0
        web.documentRequestOwner={};web.url="https://example.test/posted#result"
        web.reloads=0;web.bypassReloads=0;web.loading=false
        ui.active_tab_index=0;ui.input_epoch=0;ui.targetValid=true
        ui.completed=0;ui.failed=0;ui.stopped=0;ui.navigation_failure_visible=false
        ui.load_state="provisional";ui.status_text="Reload requested"
    }
    function request(content, bypass) {
        content.requestReload({kind:"reload",tabId:"tab",generation:3,documentId:4,bypassCache:!!bypass})
        tryCompare(content,"queuedReload",null)
    }
    function prompt(content, bypass) {
        request(content,bypass)
        var surface=findChild(host,"formResubmissionSurface")
        verify(surface)
        tryCompare(surface,"visible",true)
        tryCompare(findChild(surface,"commandDialogAction-cancel"),"activeFocus",true)
        return surface
    }
    function test_form_surface_releases_its_missing_host_data() {
        return [{tag:"unused",open:false},{tag:"open",open:true}]
    }
    function test_form_surface_releases_its_missing_host(data) {
        var content=createTemporaryObject(contentFixture,host.contentItem)
        if (data.open) prompt(content,false)
        content.browserHost=null
        wait(0)
        verify(!findChild(host,"formResubmissionSurface"))
        compare(web.reloads,0)
        compare(web.bypassReloads,0)
    }
    function test_cancel_preserves_the_completed_document_data() {
        return [{tag:"enter",key:Qt.Key_Return},{tag:"escape",key:Qt.Key_Escape},{tag:"n",key:Qt.Key_N}]
    }
    function test_cancel_preserves_the_completed_document(data) {
        var content=createTemporaryObject(contentFixture,host.contentItem)
        var owner=web.documentRequestOwner
        var surface=prompt(content,false)
        compare(ui.load_state,"complete")
        keyClick(data.key)
        tryCompare(surface,"visible",false)
        compare(web.reloads,0);compare(web.bypassReloads,0)
        compare(web.documentRequestOwner,owner)
        compare(ui.status_text,"Reload cancelled")
    }
    function test_cancel_returns_from_an_unavailable_previous_control_data() {
        return [{tag:"hidden"},{tag:"disabled"},{tag:"available"}]
    }
    function test_cancel_returns_from_an_unavailable_previous_control(data) {
        var previous=createTemporaryObject(previousControlFixture,host.contentItem)
        previous.forceActiveFocus()
        verify(previous.activeFocus)
        if (data.tag==="hidden") previous.visible=false
        if (data.tag==="disabled") previous.enabled=false
        var content=createTemporaryObject(contentFixture,host.contentItem)
        var surface=prompt(content,false)
        keyClick(Qt.Key_Escape)
        tryCompare(surface,"visible",false)
        tryCompare(data.tag==="available" ? previous : web,"activeFocus",true)
    }
    function test_confirm_preserves_the_reload_option_and_runs_once_data() {
        return [{tag:"normal",bypass:false},{tag:"bypass",bypass:true}]
    }
    function test_confirm_preserves_the_reload_option_and_runs_once(data) {
        var content=createTemporaryObject(contentFixture,host.contentItem)
        var surface=prompt(content,data.bypass)
        keyClick(Qt.Key_Y)
        tryCompare(surface,"visible",false)
        surface.requestAction("resubmit")
        compare(web.reloads,data.bypass ? 0 : 1)
        compare(web.bypassReloads,data.bypass ? 1 : 0)
    }
    function test_changed_owner_cannot_resubmit_data() {
        return [{tag:"tab"},{tag:"view"},{tag:"document"},{tag:"retired"},
                {tag:"target"},{tag:"address"},{tag:"window"}]
    }
    function test_changed_owner_cannot_resubmit(data) {
        var content=createTemporaryObject(contentFixture,host.contentItem)
        var surface=prompt(content,false)
        if(data.tag==="tab") ui.active_tab_index=1
        else if(data.tag==="view") host.currentView=null
        else if(data.tag==="document") web.documentRequestOwner={}
        else if(data.tag==="retired") web.live=false
        else if(data.tag==="target") {ui.targetValid=false;ui.input_epoch++}
        else if(data.tag==="address") web.url="https://example.test/other"
        else web.viewHost=null
        surface.requestAction("resubmit")
        tryCompare(surface,"visible",false)
        compare(web.reloads,0);compare(web.bypassReloads,0)
        verify(!surface.returnFocus)
    }
    function test_get_reload_does_not_prompt() {
        var content=createTemporaryObject(contentFixture,host.contentItem,{engineDocumentHasFormData:false})
        request(content,false)
        compare(web.reloads,1);compare(ui.completed,0)
        verify(!content.formReloadCreated)
    }
    function test_repeated_reload_keeps_the_pending_forms_resting_state_data() {
        return [{tag:"complete",state:"complete",form:true,nav:false,expected:"complete"},
                {tag:"failed",state:"failed",form:false,nav:true,expected:"failed"}]
    }
    function test_repeated_reload_keeps_the_pending_forms_resting_state(data) {
        var content=createTemporaryObject(contentFixture,host.contentItem,{
            engineLoadState:data.state,engineDocumentHasFormData:data.form,navigationHasFormData:data.nav
        })
        var surface=prompt(content,false)
        ui.load_state="provisional"
        request(content,true)
        compare(ui.load_state,data.expected)
        keyClick(Qt.Key_Escape)
        compare(web.reloads,0);compare(web.bypassReloads,0)
    }
    function navigation(content, form) {
        var event={hasFormData:form,url:web.url,rejected:false,reject:function(){this.rejected=true}}
        var intercepted=content.interceptReloadNavigation(event)
        return {event:event,intercepted:intercepted}
    }
    function streamingContent() {
        web.loading=true
        return createTemporaryObject(contentFixture,host.contentItem,{
            engineLoadState:"loading",engineDocumentHasFormData:false,navigationHasFormData:true
        })
    }
    function test_streaming_form_cancel_keeps_the_response_loading_data() {
        return [{tag:"enter",key:Qt.Key_Return},{tag:"escape",key:Qt.Key_Escape},{tag:"n",key:Qt.Key_N}]
    }
    function test_streaming_form_cancel_keeps_the_response_loading(data) {
        var content=streamingContent()
        request(content,false)
        compare(web.reloads,1)
        var result=navigation(content,true)
        verify(result.intercepted && result.event.rejected)
        var surface=findChild(host,"formResubmissionSurface")
        tryVerify(function(){surface=findChild(host,"formResubmissionSurface");return surface && surface.visible})
        compare(ui.load_state,"provisional");compare(ui.completed,0)
        keyClick(data.key)
        tryCompare(surface,"visible",false)
        verify(web.loading)
        compare(web.reloads,1)
        compare(ui.load_state,"provisional")
        // A second browser Reload can use the verified form entry directly.
        request(content,false)
        tryCompare(surface,"visible",true)
        compare(web.reloads,1)
    }
    function test_streaming_approval_is_consumed_by_one_native_request_data() {
        return [{tag:"normal",bypass:false},{tag:"bypass",bypass:true}]
    }
    function test_streaming_approval_is_consumed_by_one_native_request(data) {
        var content=streamingContent()
        request(content,data.bypass)
        verify(navigation(content,true).event.rejected)
        var surface=null
        tryVerify(function(){surface=findChild(host,"formResubmissionSurface");return surface && surface.visible})
        keyClick(Qt.Key_Y)
        tryCompare(surface,"visible",false)
        var approved=navigation(content,true)
        verify(!approved.intercepted && !approved.event.rejected)
        compare(content.reloadAttempt,null)
        compare(web.reloads,data.bypass ? 0 : 2)
        compare(web.bypassReloads,data.bypass ? 2 : 0)
    }
    function test_pending_form_reload_of_previous_get_is_not_prompted() {
        var content=streamingContent()
        request(content,false)
        var result=navigation(content,false)
        verify(!result.intercepted && !result.event.rejected)
        verify(!content.formReloadCreated)
        compare(content.reloadAttempt,null)
    }
    function test_pending_reload_commands_are_coalesced_before_native_policy() {
        var content=streamingContent()
        request(content,false)
        request(content,true)
        compare(web.reloads,1);compare(web.bypassReloads,0)
        verify(navigation(content,true).event.rejected)
        var surface=null
        tryVerify(function(){surface=findChild(host,"formResubmissionSurface");return surface && surface.visible})
        keyClick(Qt.Key_Y)
        compare(web.reloads,2);compare(web.bypassReloads,0)
    }
    function test_background_streaming_form_waits_for_selection() {
        var content=streamingContent()
        ui.active_tab_index=1
        request(content,false)
        verify(navigation(content,true).event.rejected)
        wait(0)
        verify(!content.formReloadCreated)
        verify(web.loading)
        verify(ui.status_text.indexOf("Select this tab")>=0)
        ui.active_tab_index=0
        var surface=prompt(content,false)
        compare(web.reloads,1)
        keyClick(Qt.Key_Escape)
        compare(web.reloads,1)
    }
    function test_delayed_reload_cannot_borrow_a_new_navigation() {
        var content=streamingContent()
        request(content,false)
        content.beginNavigation(false,false)
        var result=navigation(content,true)
        verify(result.intercepted && result.event.rejected)
        verify(!content.formReloadCreated)
    }
    function test_document_replacement_expires_the_old_streaming_form_decision() {
        var content=streamingContent()
        request(content,false)
        navigation(content,true)
        var surface=null
        tryVerify(function(){surface=findChild(host,"formResubmissionSurface");return surface && surface.visible})
        ui.targetValid=false;ui.input_epoch++
        tryCompare(surface,"visible",false)
        verify(!surface.returnFocus)
        compare(web.reloads,1)
    }
    function test_stream_completion_keeps_the_decision_and_completed_resting_state() {
        var content=streamingContent()
        request(content,false)
        navigation(content,true)
        var surface=null
        tryVerify(function(){surface=findChild(host,"formResubmissionSurface");return surface && surface.visible})
        web.loading=false
        content.noteCommit(true)
        ui.navigation_completed_for(0)
        verify(surface.visible && surface.ownerCurrent)
        ui.load_state="provisional"
        request(content,true)
        compare(ui.load_state,"complete")
        keyClick(Qt.Key_Return)
        tryCompare(surface,"visible",false)
        compare(web.reloads,1)
        compare(ui.load_state,"complete")
    }
    function test_stopped_form_interception_ignores_transient_load_signals() {
        var content=createTemporaryObject(contentFixture,host.contentItem,{
            engineLoadState:"stopped",engineDocumentHasFormData:false,navigationHasFormData:true
        })
        var owner=web.documentRequestOwner
        request(content,false)
        verify(navigation(content,true).event.rejected)
        verify(content.blockedReloadLoading("started"))
        verify(content.blockedReloadLoading("stopped"))
        var surface=null
        tryVerify(function(){surface=findChild(host,"formResubmissionSurface");return surface && surface.visible})
        compare(web.documentRequestOwner,owner)
        compare(ui.load_state,"cancelled")
        keyClick(Qt.Key_Escape)
        compare(web.reloads,1)
    }
    function test_cancelled_beforeunload_releases_the_reload_attempt() {
        var content=streamingContent()
        request(content,false)
        verify(content.reloadAttempt)
        content.cancelReloadAttempt()
        compare(content.reloadAttempt,null)
        request(content,false)
        compare(web.reloads,2)
        compare(ui.load_state,"provisional")
    }
    function test_beforeunload_after_approval_retains_the_verified_form_until_document_change() {
        var content=streamingContent()
        request(content,false)
        verify(navigation(content,true).event.rejected)
        var surface=null
        tryVerify(function(){surface=findChild(host,"formResubmissionSurface");return surface && surface.visible})
        keyClick(Qt.Key_Y)
        verify(!navigation(content,true).event.rejected)
        content.beginNavigation(false,true,true)
        verify(content.reloadOwnerCurrent(content.verifiedFormReload))
        web.loading=false
        content.cancelReloadAttempt()
        compare(content.engineLoadState,"stopped")
        compare(ui.load_state,"cancelled")
        request(content,false)
        tryCompare(surface,"visible",true)
        compare(web.reloads,2)
        keyClick(Qt.Key_Escape)
        tryCompare(surface,"visible",false)
        compare(ui.load_state,"cancelled")
        content.beginNavigation(false,true,false)
        compare(content.verifiedFormReload,null)
    }
    function test_reload_waits_for_the_queued_native_load_result() {
        var content=streamingContent()
        web.loading=false
        content.requestReload({kind:"reload",tabId:"tab",generation:3,documentId:4,bypassCache:false})
        content.noteCommit(false)
        var surface=null
        tryVerify(function(){surface=findChild(host,"formResubmissionSurface");return surface && surface.visible})
        compare(web.reloads,0)
    }
    function test_queued_reload_expires_when_the_document_changes() {
        var content=streamingContent()
        content.requestReload({kind:"reload",tabId:"tab",generation:3,documentId:4,bypassCache:false})
        web.documentRequestOwner={}
        tryCompare(content,"queuedReload",null)
        compare(web.reloads,0)
        verify(!content.formReloadCreated)
    }
    function test_queued_reload_retains_the_first_cache_option() {
        var content=streamingContent()
        content.requestReload({kind:"reload",tabId:"tab",generation:3,documentId:4,bypassCache:false})
        content.requestReload({kind:"reload",tabId:"tab",generation:3,documentId:4,bypassCache:true})
        tryCompare(content,"queuedReload",null)
        compare(web.reloads,1)
        compare(web.bypassReloads,0)
    }
    function emptyFailure() {
        return {errorCode:0,errorDomain:0,errorString:"",isDownload:false,isErrorPage:false,url:web.url}
    }
    function test_stay_preserves_the_response_state_without_an_error_data() {
        var rows=[]
        for(var stage of ["failed","stopped"])
            for(var state of ["loading","stopped","complete","failed"])
                rows.push({tag:stage+"-"+state,stage:stage,state:state,
                    expected:state==="complete" || state==="failed" ? state : "cancelled"})
        return rows
    }
    function test_stay_preserves_the_response_state_without_an_error(data) {
        var content=createTemporaryObject(contentFixture,host.contentItem,{
            engineLoadState:data.state,engineDocumentHasFormData:false,navigationHasFormData:false
        })
        ui.navigation_failure_visible=data.state==="failed"
        content.noteBeforeUnloadRejected()
        var info=emptyFailure()
        if(data.stage==="stopped") {info.errorCode=-3;info.errorDomain=1;info.errorString="Aborted"}
        verify(content.settleBeforeUnloadLoading(data.stage,info))
        compare(ui.load_state,data.expected)
        compare(ui.status_text,"Stayed on page")
        compare(ui.navigation_failure_visible,data.state==="failed")
        compare(web.reloads,0);compare(web.bypassReloads,0)
        compare(content.beforeUnloadCancellation,null)
        verify(!content.settleBeforeUnloadLoading("failed",emptyFailure()))
    }
    function test_stay_does_not_hide_real_or_unowned_failures_data() {
        return [{tag:"dns",code:-105,domain:6},{tag:"network",code:-102,domain:2},
            {tag:"tls",code:-202,domain:3},{tag:"http",code:503,domain:7},
            {tag:"detail",detail:"The request failed"},{tag:"download",download:true},
            {tag:"error-page",errorPage:true},{tag:"missing-code",missingCode:true},
            {tag:"other-address",otherAddress:true},{tag:"still-loading",loading:true},
            {tag:"no-stay",noStay:true},{tag:"automatic-dismissal",automatic:true}]
    }
    function test_stay_does_not_hide_real_or_unowned_failures(data) {
        var content=streamingContent();web.loading=false
        if (data.automatic) content.cancelReloadAttempt()
        else if (!data.noStay) content.noteBeforeUnloadRejected()
        var info=emptyFailure()
        if (data.code!==undefined) info.errorCode=data.code
        if (data.domain!==undefined) info.errorDomain=data.domain
        if (data.detail) info.errorString=data.detail
        if (data.download) info.isDownload=true
        if (data.errorPage) info.isErrorPage=true
        if (data.missingCode) delete info.errorCode
        if (data.otherAddress) info.url="https://example.test/other"
        if (data.loading) web.loading=true
        var stopped=ui.stopped
        verify(!content.settleBeforeUnloadLoading("failed",info))
        compare(ui.stopped,stopped)
        compare(ui.failed,0)
        compare(content.beforeUnloadCancellation,null)
        verify(!content.settleBeforeUnloadLoading("failed",emptyFailure()))
    }
    function test_stay_cancellation_expires_with_its_document_data() {
        return [{tag:"target"},{tag:"document"},{tag:"navigation"},{tag:"view-ui"},
            {tag:"host"},{tag:"address"},{tag:"retired"},{tag:"load-start"}]
    }
    function test_stay_cancellation_expires_with_its_document(data) {
        var content=streamingContent();web.loading=false
        content.noteBeforeUnloadRejected()
        var info=emptyFailure()
        if(data.tag==="target") ui.targetValid=false
        else if(data.tag==="document") web.documentRequestOwner={}
        else if(data.tag==="navigation") content.beginNavigation(false,false,false)
        else if(data.tag==="view-ui") web.viewUi=null
        else if(data.tag==="host") web.viewHost=null
        else if(data.tag==="address") web.url="https://example.test/other"
        else if(data.tag==="retired") web.live=false
        else verify(!content.settleBeforeUnloadLoading("started",{}))
        var stopped=ui.stopped
        verify(!content.settleBeforeUnloadLoading("failed",info))
        compare(ui.stopped,stopped)
        compare(content.beforeUnloadCancellation,null)
    }
    function test_completed_load_after_stay_is_reported_normally() {
        var content=streamingContent();web.loading=false
        content.noteBeforeUnloadRejected()
        verify(!content.settleBeforeUnloadLoading("succeeded",{}))
        content.noteCommit(true);ui.navigation_completed_for(0)
        compare(ui.load_state,"complete")
        verify(!content.settleBeforeUnloadLoading("failed",emptyFailure()))
    }
    function test_stay_accepts_the_engine_address_rollback_for_the_same_document() {
        var content=streamingContent();web.loading=false
        web.url="https://example.test/proposed"
        content.noteBeforeUnloadRejected()
        web.url="https://example.test/posted#result"
        verify(!content.reloadOwnerCurrent(content.beforeUnloadCancellation))
        verify(content.navigationOwnerCurrent(content.beforeUnloadCancellation))
        verify(content.settleBeforeUnloadLoading("failed",emptyFailure()))
        compare(ui.load_state,"cancelled")
        compare(ui.navigation_failure_visible,false)
    }
    function test_failed_form_cancel_preserves_failure_state_and_surface_data() {
        var rows=[]
        for(var visible of [false,true]) {
            for(var key of [Qt.Key_Return,Qt.Key_Escape,Qt.Key_N])
                rows.push({tag:String(visible)+"-"+key,visible:visible,key:key})
        }
        return rows
    }
    function test_failed_form_cancel_preserves_failure_state_and_surface(data) {
        var content=createTemporaryObject(contentFixture,host.contentItem,{
            engineLoadState:"failed",engineDocumentHasFormData:false,navigationHasFormData:true
        })
        ui.navigation_failure_visible=data.visible
        var owner=web.documentRequestOwner
        var surface=prompt(content,false)
        compare(ui.load_state,"failed")
        compare(ui.failed,1);compare(ui.completed,0)
        compare(ui.navigation_failure_visible,data.visible)
        keyClick(data.key)
        tryCompare(surface,"visible",false)
        compare(web.reloads,0);compare(web.bypassReloads,0)
        compare(web.documentRequestOwner,owner)
        compare(ui.load_state,"failed")
        compare(ui.navigation_failure_visible,data.visible)
    }
    function test_failed_form_can_be_explicitly_resubmitted_data() {
        return [{tag:"normal",bypass:false},{tag:"bypass",bypass:true}]
    }
    function test_failed_form_can_be_explicitly_resubmitted(data) {
        var content=createTemporaryObject(contentFixture,host.contentItem,{
            engineLoadState:"failed",engineDocumentHasFormData:false,navigationHasFormData:true
        })
        ui.navigation_failure_visible=true
        var surface=prompt(content,data.bypass)
        keyClick(Qt.Key_Y)
        tryCompare(surface,"visible",false)
        verify(!ui.navigation_failure_visible)
        compare(web.reloads,data.bypass ? 0 : 1)
        compare(web.bypassReloads,data.bypass ? 1 : 0)
    }
    function test_failed_get_does_not_inherit_the_previous_documents_post_method() {
        var content=createTemporaryObject(contentFixture,host.contentItem,{
            engineLoadState:"failed",engineDocumentHasFormData:true,navigationHasFormData:false
        })
        request(content,false)
        compare(web.reloads,1);compare(ui.failed,0)
        verify(!content.formReloadCreated)
    }
    function test_stale_failed_form_does_not_restore_the_old_failure_dialog() {
        var content=createTemporaryObject(contentFixture,host.contentItem,{
            engineLoadState:"failed",navigationHasFormData:true
        })
        ui.navigation_failure_visible=true
        var surface=prompt(content,false)
        ui.navigation_failure_visible=false
        ui.load_state="complete"
        web.documentRequestOwner={}
        ui.status_text="New page"
        surface.requestAction("resubmit")
        tryCompare(surface,"visible",false)
        verify(!ui.navigation_failure_visible)
        compare(ui.status_text,"New page")
        compare(web.reloads,0)
    }
    function test_background_tab_requires_selection_before_resubmitting() {
        var content=createTemporaryObject(contentFixture,host.contentItem)
        ui.active_tab_index=1
        request(content,false)
        compare(web.reloads,0)
        verify(!content.formReloadCreated)
        verify(ui.status_text.indexOf("Select this tab")>=0)
    }
    function test_large_text_actions_remain_reachable_data() {
        return [{tag:"compact",width:640,font:20},{tag:"narrow",width:320,font:26}]
    }
    function test_large_text_actions_remain_reachable(data) {
        host.width=data.width;host.font.pointSize=data.font
        var content=createTemporaryObject(contentFixture,host.contentItem)
        var surface=prompt(content,false)
        keyClick(Qt.Key_Down)
        var confirm=findChild(surface,"commandDialogAction-resubmit")
        tryCompare(confirm,"activeFocus",true)
        var scroll=findChild(surface,"commandDialogActionScroll")
        tryVerify(function(){
            var y=confirm.mapToItem(scroll,0,0).y
            return y>=-1 && y+confirm.height<=scroll.height+1
        })
        keyClick(Qt.Key_Escape)
        compare(web.reloads,0)
    }
}

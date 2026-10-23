import QtQuick
import QtQuick.Window
import QtTest
import QtWebEngine
import io.github.ferricbrowser 1.0
import "../../qml/components"

TestCase {
    id: test
    name: "PageSearch"
    when: windowShown
    visible: true
    width: 1000
    height: 800
    property bool done: false
    property var value: null
    property var selectedView: page
    property string rejectedNavigation: ""
    property bool createdTab: false
    property int nativeMatches: 0
    property int nativeActiveMatch: 0
    QtObject {
        id: ui
        property string mode: "normal"
        property string search_text: ""
        property string status_text: ""
        property bool targetCurrent: true
        property bool activationReady: true
        property var nativeTabs: []
        function execute_ui_action(action, url) {
            if (action !== "browser.tab.open") return false
            nativeTabs=nativeTabs.concat([url])
            return true
        }
        function request_target_current(tab, generation, document) {
            return targetCurrent && tab === "tab" && generation === "1" && document === "doc"
        }
        function search_activation_ready(tab, generation, document) {
            return activationReady && request_target_current(tab,generation,document)
        }
    }
    QtObject {
        id: host
        property bool statusBarVisible: false
        property var browserKeyRouterObject: router
        function executePendingEngineAction() {}
    }
    QtObject {
        id: searchAppearance
        property color panelColor: "#222222"
        property color mutedTextColor: "#888888"
        property color primaryTextColor: "#ffffff"
        property font font: Qt.font({family:"monospace",pointSize:12})
    }
    BrowserKeyRouter { id: router; enabled:true; targetWindow:test.Window.window }
    WebEngineProfile { id: privateProfile; offTheRecord: true }
    WebEngineView {
        id: page
        anchors.fill: parent
        profile: privateProfile
        property bool loaded: false
        onNavigationRequested: function(request) {
            if (request.url.toString().startsWith("https://fixture.test/target-")) {
                test.rejectedNavigation=request.url.toString()
                request.reject()
            }
        }
        onNewWindowRequested: function(request) {
            if (search.acceptNewTabRequest(page,request)) return
            test.createdTab=true
            request.openIn(otherPage)
        }
        onLoadingChanged: function(request) { loaded = request.status === WebEngineView.LoadSucceededStatus }
        onFindTextFinished: function(result) {
            test.nativeMatches=result.numberOfMatches
            test.nativeActiveMatch=result.activeMatch
        }
    }
    WebEngineView { id: otherPage; profile:privateProfile; visible:false }
    // A renderer that never answers must not hold later query edits hostage.
    // It exposes no mainFrame or runJavaScript API: native find needs neither.
    Item {
        id: slowPage
        property var calls: []
        property var callbacks: []
        function findText(query, flags, callback) {
            calls=calls.concat([{query:query,flags:flags||0}])
            if (callback) callbacks=callbacks.concat([callback])
        }
    }
    TextInput {
        id: keyboardInput
        width: parent.width
        height: 32
        z: 40
        visible: false
        onTextChanged: if (visible) {
            ui.search_text=text
            search.execute(test.request(text,true,false),page)
        }
    }
    FerricPageSearch {
        id: search
        appearance: searchAppearance
        browserUi: ui
        hostWindow: host
        activeView: test.selectedView
    }
    function evaluate(source) {
        done=false
        page.runJavaScript(source,WebEngineScript.ApplicationWorld,function(result){value=result;done=true})
        tryVerify(function(){return done},5000)
        return value
    }
    function load(html) {
        page.loaded=false
        page.loadHtml("<!doctype html><meta charset='utf-8'>"+html,"https://fixture.test/")
        tryVerify(function(){return page.loaded},10000)
        page.forceActiveFocus()
    }
    function request(query,newSearch,backward,mode) {
        return {kind:"find",tabId:"tab",generation:"1",documentId:"doc",query:query,
            newSearch:newSearch,backward:!!backward,caseMode:mode||"smart"}
    }
    function find(query,newSearch,backward,mode) {
        search.execute(request(query,newSearch,backward,mode),page)
        tryVerify(function(){return !search.busy && !search.pending.length},5000)
    }
    function init() {
        search.stop()
        selectedView=page
        ui.mode="normal";ui.search_text="";ui.status_text="";ui.targetCurrent=true;ui.activationReady=true
        ui.nativeTabs=[]
        rejectedNavigation="";createdTab=false
        slowPage.calls=[];slowPage.callbacks=[]
        host.statusBarVisible=false
        test.width=1000; test.height=800
        searchAppearance.font=Qt.font({family:"monospace",pointSize:12})
    }
    function cleanup() {
        search.stop()
        keyboardInput.visible=false
        keyboardInput.text=""
        page.settings.javascriptEnabled=true
    }
    function test_native_activation_data() {
        return [
            {tag:"direct",html:"<a href='/target-direct'>needle</a>",destination:"direct"},
            {tag:"wrapped-title",html:"<a href='/target-title'><h3><span>needle title</span></h3></a>",destination:"title"},
            {tag:"no-selection",html:"<style>body{user-select:none}</style><a href='/target-css'><h3><span>needle</span></h3></a>",destination:"css"},
            {tag:"blocked-styles",html:"<meta http-equiv='Content-Security-Policy' content=\"style-src 'none'\"><a href='/target-csp'><span>needle</span></a>",destination:"csp"},
            {tag:"closed-shadow",html:"<div id=host></div><script>host.attachShadow({mode:'closed'}).innerHTML='<a href=/target-shadow>needle</a>'</script>",destination:"shadow"},
            {tag:"open-shadow",html:"<div id=host></div><script>host.attachShadow({mode:'open'}).innerHTML='<a href=/target-open><span>needle</span></a>'</script>",destination:"open"}
        ]
    }
    function test_native_activation(data) {
        load(data.html)
        find("needle",true)
        verify(search.result.found)
        compare(nativeActiveMatch,1)
        search.accept()
        verify(search.follow(false))
        verify(ui.status_text!=="Search result's page is no longer active",ui.status_text)
        tryCompare(test,"rejectedNavigation","https://fixture.test/target-"+data.destination,3000)
        verify(!search.retained)
    }
    function test_foreground_tab_gesture() {
        load("<a href='/target-tab'><h3><span>needle</span></h3></a>")
        find("needle",true)
        verify(search.follow(true))
        tryVerify(function(){return ui.nativeTabs.length===1},3000)
        compare(ui.nativeTabs[0],"https://fixture.test/target-tab")
        verify(!createdTab)
    }
    function test_repeated_activation_without_settling_delays() {
        // This caught the asynchronous selection-handoff race: a native key
        // sent immediately after findText("") could beat StopFinding.
        for(var i=0;i<20;++i) {
            rejectedNavigation=""
            load((i%2 ? "<style>body{user-select:none}</style>" : "")
                +"<a href='/target-cycle-"+i+"'><h3><span>needle</span></h3></a>")
            find("needle",true)
            search.follow(false)
            tryCompare(test,"rejectedNavigation","https://fixture.test/target-cycle-"+i,3000)
        }
    }
    function test_escape_during_selection_handoff_cancels_activation() {
        load("<a href='/target-cancel'>needle</a>")
        find("needle",true)
        search.follow(false)
        verify(search.following)
        search.stop()
        wait(100)
        compare(rejectedNavigation,"")
        verify(!search.retained)
    }
    function test_query_edit_during_selection_handoff_cancels_activation() {
        load("<a href='/target-old'>old needle</a><p>new needle</p>")
        find("old needle",true)
        search.follow(false)
        find("new needle",true)
        wait(100)
        compare(rejectedNavigation,"")
        verify(search.result.found)
        compare(search.query,"new needle")
    }
    function test_button_uses_native_click() {
        load("<button onclick='this.dataset.clicked=Number(this.dataset.clicked||0)+1'><span>needle</span></button>")
        find("needle",true)
        search.follow(false)
        tryVerify(function(){return evaluate("document.querySelector('button').dataset.clicked")==="1"},3000)
    }
    function test_native_tab_gesture_is_scoped_and_consumed_once() {
        load("<p>needle</p>")
        search.nativeTabSource=page
        search.nativeTabTarget=request("needle",true)
        const nativeRequest={userInitiated:true,destination:WebEngineView.NewViewInTab,requestedUrl:"https://fixture.test/target-native"}
        verify(!search.acceptNewTabRequest(otherPage,nativeRequest))
        verify(search.acceptNewTabRequest(page,nativeRequest))
        compare(ui.nativeTabs.length,1)
        verify(!search.acceptNewTabRequest(page,nativeRequest))
        compare(ui.nativeTabs.length,1)
        search.nativeTabSource=page
        search.nativeTabTarget=request("needle",true)
        ui.activationReady=false
        verify(search.acceptNewTabRequest(page,nativeRequest))
        compare(ui.nativeTabs.length,1)
    }
    function test_scripts_disabled() {
        load("<a href='/target-nojs'><h3>needle</h3></a>")
        page.settings.javascriptEnabled=false
        find("needle",true)
        search.follow(false)
        tryCompare(test,"rejectedNavigation","https://fixture.test/target-nojs",3000)
    }
    function test_accept_and_steps_keep_native_order() {
        load("<p>needle plain</p><a href='/target-first'>needle first</a><a href='/target-second'>needle second</a>")
        ui.mode="search"
        find("needle",true)
        verify(!search.accepted)
        compare(nativeMatches,3);compare(nativeActiveMatch,1)
        search.accept();ui.mode="normal"
        compare(nativeActiveMatch,1)
        find("needle",false);compare(nativeActiveMatch,2)
        find("needle",false);compare(nativeActiveMatch,3)
        find("needle",false);compare(nativeActiveMatch,1)
        find("needle",false,true);compare(nativeActiveMatch,3)
        search.follow(false)
        tryCompare(test,"rejectedNavigation","https://fixture.test/target-second",3000)
    }
    function test_feedback_tracks_native_match_position() {
        load("<p>needle one</p><p>needle two</p><p>needle three</p>")
        ui.mode="search"
        find("needle",true)
        compare(search.previewText,"Match 1 of 3 · Enter: keep")
        for (const position of [2,3,1]) {
            find("needle",false)
            compare(nativeActiveMatch,position)
            compare(search.previewText,"Match "+position+" of 3 · Enter: keep")
        }
        find("needle",false,true)
        compare(search.previewText,"Match 3 of 3 · Enter: keep")
        search.accept();ui.mode="normal"
        verify(search.previewText.indexOf("Match 3 of 3 · Enter: activate")===0)
        find("missing",true)
        compare(search.previewText,"No matches")
        search.stop()
        compare(search.previewText,"")
    }
    function test_editing_feedback_is_visible_with_any_status_policy_data() {
        return [{tag:"status-visible",statusVisible:true},{tag:"status-hidden",statusVisible:false}]
    }
    function test_editing_feedback_is_visible_with_any_status_policy(data) {
        load("<p>needle</p>")
        host.statusBarVisible=data.statusVisible
        ui.mode="search"
        find("needle",true)
        var feedback=search.children[0]
        verify(feedback.visible,"The search bar covers the status bar while editing")
        search.accept();ui.mode="normal"
        compare(feedback.visible,!data.statusVisible)
        search.stop()
        verify(!feedback.visible)
    }
    function test_feedback_fits_a_narrow_window_at_large_text_size() {
        test.width=320;test.height=640
        searchAppearance.font=Qt.font({family:"monospace",pointSize:26})
        load("<p>needle one</p><p>needle two</p><p>needle three</p>")
        ui.mode="search"
        find("needle",true)
        verify(waitForPolish(test.Window.window))
        var feedback=findChild(search,"pageSearchFeedback")
        var label=findChild(search,"pageSearchFeedbackText")
        verify(feedback.visible)
        compare(label.font.pointSize,26)
        verify(feedback.width<=test.width-24)
        verify(feedback.y>=0 && feedback.y+feedback.height<=test.height)
        verify(label.lineCount>0 && label.lineCount<=2)
        compare(label.text,"Match 1 of 3 · Enter: keep")
    }
    function test_typing_retains_chrome_focus_data() {
        return [{tag:"normal",css:""},{tag:"unselectable",css:"body{user-select:none}"}]
    }
    function test_typing_retains_chrome_focus(data) {
        load("<style>"+data.css+"</style><a href='/target-title'><h3><span>needle</span></h3></a>")
        ui.mode="search"
        keyboardInput.visible=true;keyboardInput.forceActiveFocus()
        for(const letter of "needle") {
            keyClick(letter.charCodeAt(0)-32)
            tryVerify(function(){return !search.busy},3000)
            verify(keyboardInput.activeFocus)
            compare(ui.mode,"search")
        }
        compare(search.query,"needle");verify(search.result.found)
        compare(evaluate("typeof window.__ferric_pageSearch"),"undefined")
    }
    function test_query_edits_do_not_wait_for_callbacks() {
        selectedView=slowPage
        search.execute(request("n",true),slowPage)
        search.execute(request("ne",true),slowPage)
        search.execute(request("needle",true),slowPage)
        compare(slowPage.calls.map(function(x){return x.query}).join(","),"n,ne,needle")
        slowPage.callbacks[0](0);slowPage.callbacks[1](0)
        verify(search.busy);verify(!search.result.found)
        slowPage.callbacks[2](2)
        verify(!search.busy);verify(search.result.found);compare(search.query,"needle")
    }
    function test_steps_queue_but_query_edit_supersedes() {
        selectedView=slowPage
        search.execute(request("old",true),slowPage)
        search.execute(request("old",false),slowPage)
        search.execute(request("old",false),slowPage)
        compare(slowPage.calls.length,1)
        compare(search.previewText,"Searching…")
        slowPage.callbacks[0](3)
        compare(slowPage.calls.length,2)
        search.execute(request("new",true),slowPage)
        compare(slowPage.calls[2].query,"new")
        slowPage.callbacks[1](3)
        verify(search.busy)
        compare(search.previewText,"Searching…")
        slowPage.callbacks[2](1)
        verify(!search.busy);compare(search.query,"new");compare(search.pending.length,0)
        compare(search.previewText,"1 match · Enter: activate · Shift+Enter: new tab")
    }
    function test_escape_ignores_late_native_reply() {
        selectedView=slowPage
        search.execute(request("needle",true),slowPage)
        search.stop()
        slowPage.callbacks[0](1)
        verify(!search.retained);verify(!search.busy);verify(!search.result.found)
        compare(slowPage.calls[1].query,"")
    }
    function test_changed_target_ignores_reply() {
        selectedView=slowPage
        search.execute(request("needle",true),slowPage)
        ui.targetCurrent=false
        slowPage.callbacks[0](1)
        verify(!search.retained)
    }
    function test_activation_rejects_pending_core_search() {
        load("<a href='/target-pending'>needle</a>")
        find("needle",true)
        ui.activationReady=false
        search.follow(false)
        wait(50);compare(rejectedNavigation,"");verify(search.retained)
    }
    function test_zero_matches_cannot_activate_previous_link() {
        load("<a id=old href='/target-old'>old</a><p>plain</p>")
        evaluate("document.querySelector('#old').focus()")
        find("absent",true)
        verify(!search.result.found)
        search.follow(false)
        wait(50);compare(rejectedNavigation,"")
    }
    function test_background_find_has_no_activation_session() {
        load("<p>needle</p>")
        selectedView=slowPage
        test.nativeMatches=-1
        search.execute(request("needle",true),page)
        tryCompare(test,"nativeMatches",1)
        verify(!search.retained)
    }
    function test_search_mode_and_tab_retire_session() {
        load("<p>needle</p>")
        find("needle",true)
        ui.mode="command";verify(search.retained)
        ui.mode="normal";verify(search.retained)
        ui.mode="search";verify(!search.retained)
        find("needle",true)
        selectedView=slowPage;verify(!search.retained)
    }
    function test_case_and_direction() {
        load("<a href='/target-lower'>needle</a><a href='/target-upper'>Needle</a>")
        find("Needle",true,false,"smart");compare(nativeMatches,1)
        search.stop()
        load("<a href='/target-lower'>needle</a><a href='/target-upper'>Needle</a>")
        find("needle",true,false,"insensitive");compare(nativeMatches,2)
        find("needle",false,true,"insensitive")
        search.follow(false)
        tryCompare(test,"rejectedNavigation","https://fixture.test/target-upper",3000)
    }
}

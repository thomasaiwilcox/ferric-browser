import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"

TestCase {
    id: test
    name: "StartPage"
    when: windowShown
    width: 1000
    height: 760
    ApplicationWindow { id: host; width: 1000; height: 760; visible: true }
    QtObject {
        id: theme
        property color backgroundColor: "#181825"
        property color surfaceColor: "#313244"
        property color primaryTextColor: "#cdd6f4"
        property color secondaryTextColor: "#bac2de"
        property color mutedTextColor: "#a6adc8"
        property color borderColor: "#585b70"
        property color accentColor: "#89b4fa"
        property color errorColor: "#f38ba8"
        property color privateColor: "#cba6f7"
        property real chromeFontPointSize: 10
        function completeBrowserKeyAction(ui, window) {}
    }
    QtObject {
        id: mockView
        property bool viewTransientProfile: false
        property string destination: ""
        function setContentUrl(url, traversal) { destination = url }
    }
    QtObject {
        id: ui
        property string config_json: "{}"
        property string initial_url: "rb://start"
        property string status_text: ""
        property string start_page_view: "guide"
        property string start_page_error: ""
        property bool start_page_busy: false
        property int start_page_revision: 0
        property int start_page_rank_revision: 0
        property int start_page_profile_revision: 0
        property bool start_page_ready: true
        property var start_page_kinds: ["top", "top", "favourite"]
        property var start_page_ids: ["a.test", "b.test", "bookmark-a"]
        property var start_page_urls: ["https://a.test", "https://b.test", "https://pinned.test"]
        property var start_page_titles: ["A", "B", "Pinned"]
        property var start_page_sites: ["a.test", "b.test", "pinned.test"]
        property var start_page_slots: ["1", "2", "8"]
        property var start_page_eligible_urls: ["https://a.test", "https://b.test"]
        property var start_page_eligible_sites: ["a.test", "b.test"]
        property var guide_categories: []
        property var guide_titles: []
        property var guide_keys: []
        property var guide_modes: []
        property var guide_commands: []
        property var guide_summaries: []
        property var guide_details: []
        property var guide_ids: []
        property var guide_explanations: []
        property var guide_arguments: []
        property var guide_examples: []
        property var guide_recoveries: []
        property var guide_counts: []
        property var guide_binding_notes: []
        property bool multipleBindings: false
        property bool twoFeatures: false
        property bool pending: false
        property bool receiptMode: false
        property bool accepted: true
        property bool immediateReceipt: false
        property int receiptSequence: 0
        property string favourite_action_id: ""
        property string favourite_action_state: "idle"
        property string favourite_action_message: ""
        property string favourite_action_detail: ""
        property int navigations: 0
        property var actions: []
        property var favouriteSubmissions: []
        property string engineAction: ""
        function take_engine_request() { return null }
        function take_engine_action() { var result=engineAction;engineAction="";return result }
        function save_favourite(slot,url,title,bookmark) {
            favouriteSubmissions=favouriteSubmissions.concat([{slot:slot,url:url,title:title,bookmark:bookmark}])
            return start_page_action(bookmark.length>0?"bookmark":"pin",slot,bookmark || url,title)
        }
        function refresh_start_page() { start_page_rank_revision++; start_page_revision++; return true }
        function refresh_start_page_metadata() { start_page_revision++ }
        function start_page_action(action, slot, value, title) {
            actions = actions.concat([{ action: action, slot: slot, value: value, title: title }])
            if (receiptMode && ["pin","bookmark","edit","move"].indexOf(action)>=0) {
                favourite_action_id="favourite-"+(++receiptSequence)
                favourite_action_message="Saving favourite…"
                favourite_action_detail=""
                favourite_action_state="pending"
                start_page_busy=true
                if (!accepted || immediateReceipt) {
                    if (accepted) test.publishFavourite({slot:slot,url:value,title:title})
                    favourite_action_message=accepted ? "Favourite saved" : "Couldn't save favourite"
                    favourite_action_detail=accepted ? "" : "Use an http:// or https:// address"
                    start_page_busy=false
                    favourite_action_state=accepted ? "succeeded" : "failed"
                }
            }
            return accepted
        }
        function start_page_icon_path(url, create) { return "" }
        function start_page_key_available() { return !pending }
        function navigate(url) { navigations++; initial_url = url }
        function refresh_guide(search) {
            var visible = search.length === 0 || "open o normal".indexOf(search.toLowerCase()) >= 0
            var count = visible ? multipleBindings || twoFeatures ? 2 : 1 : 0
            guide_categories = Array(count).fill("Opening Pages and Search")
            guide_titles = Array(count).fill("Open a page")
            guide_ids = Array(count).fill("open")
            guide_keys = count === 2 ? ["o", "O"] : count === 1 ? ["o"] : []
            guide_modes = count === 2 ? ["normal", "caret"] : count === 1 ? ["normal"] : []
            guide_commands = count === 2 ? ["open", "open --target tab"] : count === 1 ? ["open"] : []
            guide_summaries = Array(count).fill("Open an address or search query")
            guide_explanations = Array(count).fill("Type a URL. Escape cancels address entry.")
            guide_arguments = Array(count).fill("input: Text (required)")
            guide_examples = Array(count).fill(":open example.org")
            guide_recoveries = Array(count).fill("Escape cancels address entry.")
            guide_counts = Array(count).fill("No numeric count.")
            guide_binding_notes = Array(count).fill("User configuration")
            if (twoFeatures && visible) {
                guide_ids = ["open", "reload"]
                guide_titles = ["Open a page", "Reload a page"]
                guide_keys = ["o", "r"]
                guide_modes = ["normal", "normal"]
                guide_commands = ["open", "reload"]
                guide_explanations = ["Type a URL. Escape cancels address entry.", "Reload fetches this page again."]
            }
            guide_details = Array(count).fill("Type a URL. Escape cancels address entry.")
        }
    }
    Component {
        id: fixture
        FerricStartPage {
            width: host.width; height: host.height
            browserUi: ui; browserWindow: theme; browserHost: host; contentView: mockView
        }
    }
    Component { id: lifecycleFixture; Loader { sourceComponent: fixture } }
    QtObject {
        id: runtimeHost
        property var browserUi: ui
        property var nativePage: null
        function activeWebView() { return {nativeUrl:"rb://start",startPage:nativePage} }
    }
    Component { id: runtimeFixture; FerricRuntimeController { host: runtimeHost } }
    function test_guide_focus_callbacks_do_not_outlive_the_view() {
        failOnWarning(/TypeError|ReferenceError/)
        var loader = createTemporaryObject(lifecycleFixture, host.contentItem)
        verify(loader && loader.item)
        loader.item.guideSurface.focusEntries()
        loader.active = false
        wait(0)
    }
    function init() {
        failOnWarning(/TypeError|ReferenceError|Cannot assign|Unable to assign|Binding loop/)
        host.width = 1000; host.height = 760; ui.multipleBindings = false; ui.twoFeatures = false
        ui.start_page_view = "guide"; ui.start_page_revision = 0; ui.navigations = 0
        ui.actions = []; ui.favouriteSubmissions=[]; ui.pending = false; ui.start_page_ready = true; ui.engineAction="";ui.status_text="";runtimeHost.nativePage=null; ui.start_page_eligible_urls = ["https://a.test", "https://b.test"]; ui.start_page_eligible_sites = ["a.test", "b.test"]
        ui.receiptMode=false;ui.favourite_action_id="";ui.favourite_action_state="idle"
        ui.accepted=true;ui.immediateReceipt=false
        ui.favourite_action_message="";ui.favourite_action_detail=""
        ui.start_page_busy=false;ui.start_page_error="";theme.chromeFontPointSize=10
        ui.start_page_kinds=["top","top","favourite"];ui.start_page_ids=["a.test","b.test","bookmark-a"]
        ui.start_page_urls=["https://a.test","https://b.test","https://pinned.test"]
        ui.start_page_titles=["A","B","Pinned"];ui.start_page_sites=["a.test","b.test","pinned.test"];ui.start_page_slots=["1","2","8"]
    }
    function createPage() {
        var page = createTemporaryObject(fixture, host.contentItem)
        verify(page)
        host.requestActivate()
        tryCompare(host, "active", true)
        return page
    }
    function event(key, modifiers) { return { key: key, modifiers: modifiers || Qt.NoModifier, isAutoRepeat: false, accepted: false } }
    function test_guide_selection_shows_article_without_a_reveal_step() {
        ui.twoFeatures = true
        var page = createPage()
        var guide = page.guideSurface
        compare(page.selectedView, "guide")
        verify(guide.articleOpen)
        var explanation = findChild(guide, "ferricGuideExplanation")
        verify(explanation.visible)
        compare(explanation.text, "Type a URL. Escape cancels address entry.")
        guide.focusEntries()
        verify(guide.handleNavigation(event(Qt.Key_Down)))
        compare(guide.currentRow.identity, "reload")
        verify(guide.articleOpen)
        compare(findChild(guide, "ferricGuideExplanation").text, "Reload fetches this page again.")
        verify(guide.handleNavigation(event(Qt.Key_Return)))
        tryVerify(function() { return findChild(guide, "ferricGuideArticle").activeFocus })
        verify(guide.handleNavigation(event(Qt.Key_Return)))
        verify(guide.articleOpen)
        verify(guide.handleNavigation(event(Qt.Key_Escape)))
        tryVerify(function() { return guide.entriesList.activeFocus })
        verify(guide.articleOpen)
        compare(ui.navigations, 0)
        compare(ui.actions.length, 0)
    }
    function test_narrow_guide_opens_an_article_with_one_click() {
        host.width = 390
        var page = createPage()
        var guide = page.guideSurface
        verify(guide.compact)
        verify(guide.entriesList.visible)
        verify(!guide.articleOpen)
        wait(0)
        var row = findChild(guide, "guideFeatureRowopen")
        mouseClick(row, row.width / 2, row.height / 2)
        tryVerify(function() { return guide.articleOpen && !guide.entriesList.visible })
        verify(findChild(guide, "ferricGuideExplanation").visible)
        wait(0)
        var back = findChild(guide, "ferricGuideBack")
        verify(waitForRendering(back))
        mouseClick(back, back.width / 2, back.height / 2)
        tryVerify(function() { return guide.entriesList.visible && guide.entriesList.activeFocus })
        compare(guide.currentRow.identity, "open")
        verify(!guide.articleOpen)
        compare(ui.navigations, 0)
    }
    function test_number_keys_open_only_populated_grid_slots() {
        var page = createPage()
        page.chooseView("top-sites", false)
        wait(0)
        page.focusContent()
        verify(page.handleNavigation(event(Qt.Key_2)))
        compare(mockView.destination, "https://b.test")
        compare(ui.navigations, 1)
        verify(page.handleNavigation(event(Qt.Key_8)))
        compare(ui.navigations, 1)
        verify(!page.handleNavigation(event(Qt.Key_1, Qt.AltModifier)))
        ui.pending = true
        verify(!page.handleNavigation(event(Qt.Key_1)))
        compare(ui.navigations, 1)
    }
    function test_view_switch_is_explicit_and_saved() {
        var page = createPage()
        page.chooseView("favourites")
        compare(ui.actions[0].action, "view")
        compare(ui.actions[0].value, "favourites")
        page.activate(7)
        compare(mockView.destination, "https://pinned.test")
    }
    function test_background_snapshot_cannot_move_numbered_targets() {
        var page = createPage()
        page.chooseView("top-sites", false)
        var first = page.topTiles[0].url
        ui.start_page_urls = ["https://changed.test", "https://b.test", "https://pinned.test"]
        ui.start_page_revision++
        compare(page.topTiles[0].url, first)
        ui.start_page_eligible_sites = ["b.test"]
        ui.start_page_revision++
        compare(page.topTiles[0], null)
        compare(page.topTiles[1].url, "https://b.test")
        ui.start_page_urls = ["https://a.test", "https://b.test", "https://pinned.test"]
    }
    function test_open_favourite_editor_is_retained_by_the_close_check() {
        var page = createPage()
        compare(page.closeNeedsConfirmation, false)
        page.editSlot(0, "pin", null)
        tryCompare(page, "closeNeedsConfirmation", true)
        keyClick(Qt.Key_Escape)
        tryCompare(page, "closeNeedsConfirmation", false)
        compare(ui.actions.length, 0)
    }
    function favouriteEditor(page) {
        page.chooseView("favourites",false)
        page.editSlot(0,"pin",null)
        var editor=findChild(page,"ferricFavouriteEditor");verify(editor)
        tryVerify(function(){return host.activeFocusItem && host.activeFocusItem.Accessible.name==="Favourite URL"})
        return editor
    }
    function publishFavourite(draft) {
        ui.start_page_kinds=["favourite"];ui.start_page_ids=["saved-favourite"]
        ui.start_page_urls=[draft.url];ui.start_page_titles=[draft.title]
        ui.start_page_sites=["owned.test"];ui.start_page_slots=[String(draft.slot)]
        ui.start_page_revision++
    }
    function beginFavouriteSave() {
        ui.receiptMode=true
        var page=createPage(), editor=favouriteEditor(page)
        var url=findChild(page,"favouriteUrlInput"), name=findChild(page,"favouriteNameInput")
        url.text="https://owned.test/keyboard"
        keyClick(Qt.Key_Return);tryCompare(host,"activeFocusItem",name)
        name.text="An unfinished favourite"
        keyClick(Qt.Key_Return)
        return {page:page,editor:editor,url:url,name:name}
    }
    function finishFavourite(state) {
        ui.favourite_action_message=state==="failed"?"Couldn't save favourite":"Favourite saved"
        ui.favourite_action_detail=state==="failed"?"<b>Owned disk failure</b>":""
        ui.start_page_busy=false;ui.favourite_action_state=state
    }
    function test_favourite_url_return_moves_to_its_name_without_saving() {
        var page=createPage(), editor=favouriteEditor(page)
        host.activeFocusItem.text="https://owned.test/keyboard"
        keyClick(Qt.Key_Return)
        compare(ui.actions.length,0,"The URL field must not submit an unnamed favourite")
        verify(editor.visible)
        compare(host.activeFocusItem.Accessible.name,"Favourite label")
    }
    function test_keyboard_bookmark_choice_submits_its_identity_and_edited_name() {
        ui.start_page_kinds=["bookmark","bookmark"];ui.start_page_ids=["old","chosen"]
        ui.start_page_urls=["https://owned.test/","https://owned.test/"]
        ui.start_page_titles=["Older bookmark","Chosen bookmark"]
        ui.start_page_sites=["owned.test","owned.test"];ui.start_page_slots=["0","0"]
        var page=createPage(), editor=favouriteEditor(page)
        var picker=findChild(page,"favouriteBookmarkPicker")
        keyClick(Qt.Key_Backtab,Qt.ShiftModifier);tryCompare(host,"activeFocusItem",picker)
        keyClick(Qt.Key_Down);keyClick(Qt.Key_Down)
        compare(picker.currentIndex,2);compare(page.editBookmarkId,"chosen")
        compare(findChild(page,"favouriteUrlInput").text,"https://owned.test/")
        compare(findChild(page,"favouriteNameInput").text,"Chosen bookmark")
        keyClick(Qt.Key_Tab);keyClick(Qt.Key_Return)
        var name=findChild(page,"favouriteNameInput")
        tryCompare(host,"activeFocusItem",name)
        name.text="The name I entered";keyClick(Qt.Key_Return)
        compare(ui.actions.length,1)
        compare(ui.actions[0].action,"bookmark");compare(ui.actions[0].value,"chosen")
        compare(ui.actions[0].title,"The name I entered")
    }
    function test_a_late_content_focus_request_preserves_the_open_favourite_form() {
        var page=createPage(), editor=favouriteEditor(page)
        var url=host.activeFocusItem
        page.focusContent()
        compare(host.activeFocusItem,url)
        keyClick(Qt.Key_Return)
        var name=host.activeFocusItem
        page.focusContent()
        compare(host.activeFocusItem,name)
    }
    function test_an_incoming_capture_preserves_an_open_unsaved_form() {
        var page=createPage(), editor=favouriteEditor(page)
        var url=findChild(page,"favouriteUrlInput"),name=findChild(page,"favouriteNameInput")
        url.text="https://first.test/";name.text="My unfinished name"
        name.forceActiveFocus();name.select(3,8)
        page.addCaptured("https://second.test/","Another request","second-id")
        compare(url.text,"https://first.test/")
        compare(name.text,"My unfinished name")
        compare(page.editBookmarkId,"")
        compare(host.activeFocusItem,name);compare(name.selectionStart,3);compare(name.selectionEnd,8)
        compare(ui.actions.length,0)
    }
    function test_a_library_capture_cannot_replace_a_pending_draft_identity() {
        var saved=beginFavouriteSave()
        saved.page.editBookmarkId="first-id"
        runtimeHost.nativePage=saved.page
        var runtime=createTemporaryObject(runtimeFixture,test)
        verify(runtime)
        ui.engineAction="start-page-bookmark\tsecond-id\thttps://second.test/\tSecond bookmark"
        runtime.executePendingEngineAction()
        compare(saved.page.editBookmarkId,"first-id")
        compare(saved.url.text,"https://owned.test/keyboard")
        compare(saved.name.text,"An unfinished favourite")
        compare(ui.actions.length,1)
    }
    function test_metadata_reordering_keeps_the_chosen_bookmark_and_local_text() {
        ui.start_page_kinds=["bookmark","bookmark"];ui.start_page_ids=["first","chosen"]
        ui.start_page_urls=["https://first.test/","https://chosen.test/"]
        ui.start_page_titles=["First bookmark","Chosen bookmark"]
        ui.start_page_sites=["first.test","chosen.test"];ui.start_page_slots=["0","0"]
        var page=createPage(),editor=favouriteEditor(page)
        var picker=findChild(page,"favouriteBookmarkPicker"), name=findChild(page,"favouriteNameInput")
        picker.currentIndex=2;picker.activated(2)
        name.text="My unfinished name";name.forceActiveFocus();name.select(3,8)
        ui.start_page_ids=["chosen","first"];ui.start_page_urls=["https://chosen.test/","https://first.test/"]
        ui.start_page_titles=["Changed elsewhere","First bookmark"];ui.start_page_revision++
        tryCompare(picker,"currentIndex",1)
        compare(page.editBookmarkId,"chosen")
        compare(name.text,"My unfinished name")
        compare(host.activeFocusItem,name);compare(name.selectionStart,3);compare(name.selectionEnd,8)
    }
    function test_queued_captures_are_reviewed_in_order_after_cancelling_the_current_form() {
        var page=createPage(),editor=favouriteEditor(page)
        var url=findChild(page,"favouriteUrlInput"),name=findChild(page,"favouriteNameInput")
        url.text="https://first.test/";name.text="Unfinished first"
        page.addCaptured("https://second.test/","Second");page.addCaptured("https://third.test/","Third")
        compare(page.queuedFavouriteCaptures.length,2)
        verify(!page.reviewNextFavourite());compare(url.text,"https://first.test/")
        keyClick(Qt.Key_Escape);tryCompare(editor,"visible",false)
        var review=findChild(page,"reviewQueuedFavourite")
        tryCompare(host,"activeFocusItem",review)
        verify(page.closeNeedsConfirmation)
        keyClick(Qt.Key_Return);tryCompare(editor,"visible",true)
        compare(url.text,"https://second.test/");compare(name.text,"Second")
        compare(page.queuedFavouriteCaptures.length,1);compare(ui.actions.length,0)
        keyClick(Qt.Key_Escape);tryCompare(host,"activeFocusItem",review)
        keyClick(Qt.Key_Return);tryCompare(editor,"visible",true)
        compare(url.text,"https://third.test/");compare(name.text,"Third")
        compare(page.queuedFavouriteCaptures.length,0);compare(ui.actions.length,0)
        keyClick(Qt.Key_Escape);tryCompare(page,"closeNeedsConfirmation",false)
    }
    function test_a_queued_request_waits_for_the_current_save_and_any_newer_edit_data() {
        return [{tag:"saved",state:"succeeded",newer:false},{tag:"failed",state:"failed",newer:false},{tag:"newer",state:"succeeded",newer:true}]
    }
    function test_a_queued_request_waits_for_the_current_save_and_any_newer_edit(data) {
        var saved=beginFavouriteSave()
        if(data.newer)saved.name.text="A newer first name"
        saved.page.addCaptured("https://next.test/","Next")
        keyClick(Qt.Key_Escape);tryCompare(saved.editor,"visible",false)
        var review=findChild(saved.page,"reviewQueuedFavourite")
        verify(!review.enabled);verify(!saved.page.reviewNextFavourite())
        if(data.state==="succeeded")publishFavourite(saved.page.favouriteRequest.draft)
        finishFavourite(data.state)
        var retained=data.state==="failed" || data.newer
        compare(review.enabled,!retained);compare(saved.page.queuedFavouriteCaptures.length,1)
        if(retained) {
            var resume=findChild(saved.page,"resumeFavouriteEdit")
            resume.forceActiveFocus();keyClick(Qt.Key_Return);tryCompare(saved.editor,"visible",true)
            compare(saved.name.text,data.newer?"A newer first name":"An unfinished favourite")
            keyClick(Qt.Key_Escape)
        }
        tryCompare(review,"enabled",true)
        tryCompare(host,"activeFocusItem",review)
        keyClick(Qt.Key_Return);tryCompare(saved.editor,"visible",true)
        compare(saved.url.text,"https://next.test/");compare(saved.name.text,"Next")
        compare(saved.page.queuedFavouriteCaptures.length,0)
        compare(ui.actions.length,1)
    }
    function test_waiting_requests_are_bounded_without_replacing_accepted_requests() {
        var page=createPage(),editor=favouriteEditor(page)
        var name=findChild(page,"favouriteNameInput");name.text="My unfinished edit"
        for(var i=0;i<16;++i)verify(page.addCaptured("https://queued.test/"+i,"Queued "+i))
        verify(!page.addCaptured("https://overflow.test/","Overflow"))
        compare(page.queuedFavouriteCaptures.length,16)
        compare(page.queuedFavouriteCaptures[0].title,"Queued 0")
        compare(page.queuedFavouriteCaptures[15].title,"Queued 15")
        verify(page.favouriteQueueError.length>0)
        compare(ui.status_text,page.favouriteQueueError)
        verify(page.addCaptured("https://queued.test/0","Queued 0"))
        compare(page.queuedFavouriteCaptures.length,16)
        page.dismissNextFavourite();compare(page.queuedFavouriteCaptures[0].title,"Queued 1")
        verify(page.addCaptured("https://overflow.test/","Overflow"))
        compare(page.queuedFavouriteCaptures.length,16);compare(name.text,"My unfinished edit")
    }
    function test_profile_changes_clear_waiting_captures_and_bookmark_identity() {
        var page=createPage(),editor=favouriteEditor(page)
        page.addCaptured("https://old-profile.test/","Old profile","old-id")
        verify(page.closeNeedsConfirmation)
        ui.start_page_profile_revision++
        tryCompare(editor,"visible",false);compare(page.queuedFavouriteCaptures.length,0)
        compare(page.editBookmarkId,"");verify(!page.closeNeedsConfirmation)
    }
    function selectedBookmarkDraft() {
        ui.start_page_kinds=["bookmark"];ui.start_page_ids=["chosen"]
        ui.start_page_urls=["https://chosen.test/old"];ui.start_page_titles=["Chosen"]
        ui.start_page_sites=["chosen.test"];ui.start_page_slots=["0"]
        var page=createPage(),editor=favouriteEditor(page)
        var picker=findChild(page,"favouriteBookmarkPicker")
        picker.currentIndex=1;picker.activated(1)
        var name=findChild(page,"favouriteNameInput");name.text="My edited name";name.forceActiveFocus()
        return {page:page,editor:editor,picker:picker,name:name,url:findChild(page,"favouriteUrlInput")}
    }
    function test_changed_bookmark_addresses_require_an_explicit_choice_data() {
        return [{tag:"updated",updated:true},{tag:"entered",updated:false}]
    }
    function test_changed_bookmark_addresses_require_an_explicit_choice(data) {
        var selected=selectedBookmarkDraft()
        ui.start_page_urls=["https://chosen.test/new"];ui.start_page_revision++
        tryCompare(selected.page,"favouriteBookmarkStale",true)
        compare(selected.url.text,"https://chosen.test/old");compare(selected.name.text,"My edited name")
        verify(!findChild(selected.page,"saveFavouriteEdit").enabled)
        keyClick(Qt.Key_Return);compare(ui.favouriteSubmissions.length,0)
        var choice=findChild(selected.page,data.updated?"useUpdatedFavouriteAddress":"useEnteredFavouriteAddress")
        if(!data.updated)choice.forceActiveFocus()
        tryCompare(host,"activeFocusItem",choice)
        keyClick(Qt.Key_Return);tryCompare(host,"activeFocusItem",selected.url)
        compare(selected.url.text,data.updated?"https://chosen.test/new":"https://chosen.test/old")
        compare(selected.name.text,"My edited name")
        compare(selected.page.editBookmarkId,data.updated?"chosen":"")
        verify(!selected.page.favouriteBookmarkStale)
        keyClick(Qt.Key_Return);keyClick(Qt.Key_Return)
        compare(ui.favouriteSubmissions.length,1)
        compare(ui.favouriteSubmissions[0].url,data.updated?"https://chosen.test/new":"https://chosen.test/old")
        compare(ui.favouriteSubmissions[0].bookmark,data.updated?"chosen":"")
    }
    function test_removed_bookmarks_keep_the_draft_until_the_entered_address_is_chosen() {
        var selected=selectedBookmarkDraft()
        ui.start_page_kinds=[];ui.start_page_ids=[];ui.start_page_urls=[];ui.start_page_titles=[];ui.start_page_slots=[]
        ui.start_page_revision++
        tryCompare(selected.page,"favouriteBookmarkStale",true)
        tryVerify(function(){return selected.picker.displayText==="Unavailable bookmark"})
        compare(selected.page.editBookmarkId,"chosen");compare(selected.name.text,"My edited name")
        keyClick(Qt.Key_Return)
        var entered=findChild(selected.page,"useEnteredFavouriteAddress")
        tryCompare(host,"activeFocusItem",entered);keyClick(Qt.Key_Return)
        tryCompare(host,"activeFocusItem",selected.url)
        compare(selected.url.text,"https://chosen.test/old");compare(selected.name.text,"My edited name")
        compare(selected.page.editBookmarkId,"");tryCompare(selected.picker,"currentIndex",0)
    }
    function test_changed_address_choices_and_inspection_fit_narrow_windows_data() {
        return test_favourite_editor_reveals_its_focused_field_data()
    }
    function test_changed_address_choices_and_inspection_fit_narrow_windows(data) {
        host.width=data.width;host.height=600;theme.chromeFontPointSize=data.font
        var selected=selectedBookmarkDraft()
        ui.start_page_urls=["https://chosen.test/new-address"];ui.start_page_revision++
        keyClick(Qt.Key_Return)
        var choice=findChild(selected.page,"useUpdatedFavouriteAddress")
        var updated=findChild(selected.page,"updatedFavouriteAddress")
        var caption=findChild(selected.page,"updatedFavouriteAddressLabel")
        tryCompare(host,"activeFocusItem",choice)
        compare(updated.text,"https://chosen.test/new-address");verify(updated.readOnly)
        compare(updated.cursorPosition,0)
        tryVerify(function() {
            var button=choice.mapToItem(selected.editor.contentItem,0,0)
            var address=updated.mapToItem(selected.editor.contentItem,0,0)
            var label=caption.mapToItem(selected.editor.contentItem,0,0)
            return button.y>=-1 && button.y+choice.height<=selected.editor.contentItem.height+1
                && address.y>=-1 && address.y+updated.height<=selected.editor.contentItem.height+1
                && label.y>=-1 && label.y+caption.height<=selected.editor.contentItem.height+1
        })
        if(testArtifactDirectory.length>0)grabImage(host.contentItem).save(testArtifactDirectory+"/changed-address-"+data.tag+".png")
    }
    function test_waiting_request_controls_fit_narrow_windows_data() {
        return test_favourite_editor_reveals_its_focused_field_data()
    }
    function test_view_control_names_fit_their_buttons_data() {
        return [{tag:"normal",width:1000,font:10},{tag:"390-20",width:390,font:20},{tag:"320-26",width:320,font:26}]
    }
    function test_view_control_names_fit_their_buttons(data) {
        host.width=data.width;theme.chromeFontPointSize=data.font
        var page=createPage()
        verify(waitForPolish(host))
        for(var view of ["top-sites","favourites","guide"]) {
            var button=findChild(page,"startViewControl"+view)
            verify(button.contentItem.implicitWidth<=button.availableWidth+1,view+" must show its complete name")
            var p=button.mapToItem(button.parent,0,0)
            verify(p.x>=-1 && p.y>=-1 && p.x+button.width<=button.parent.width+1 && p.y+button.height<=button.parent.height+1)
        }
        if(testArtifactDirectory.length>0)grabImage(host.contentItem).save(testArtifactDirectory+"/start-views-"+data.tag+".png")
    }
    function test_waiting_request_controls_fit_narrow_windows(data) {
        host.width=data.width;host.height=600;theme.chromeFontPointSize=data.font
        var page=createPage(),editor=favouriteEditor(page)
        page.addCaptured("https://queued.test/long/address","A long waiting favourite name")
        keyClick(Qt.Key_Escape);tryCompare(editor,"visible",false)
        var review=findChild(page,"reviewQueuedFavourite")
        tryCompare(host,"activeFocusItem",review)
        var actions=findChild(page,"queuedFavouriteActions")
        for(var button of [review,findChild(page,"dismissQueuedFavourite")]) {
            var p=button.mapToItem(actions,0,0)
            verify(p.x>=0 && p.x+button.width<=actions.width+1)
        }
        var viewport=findChild(page,"startSitesScroll").contentItem
        waitForPolish(host)
        tryVerify(function() {
            var p=review.mapToItem(viewport,0,0)
            return review.activeFocus && p.y>=-1 && p.y+review.height<=viewport.height+1
        })
        if(testArtifactDirectory.length>0)grabImage(host.contentItem).save(testArtifactDirectory+"/waiting-favourites-"+data.tag+".png")
    }
    function test_enter_on_an_empty_favourite_tile_keeps_focus_in_the_editor() {
        var page=createPage()
        page.chooseView("favourites",false)
        page.focusContent()
        tryCompare(host,"activeFocusItem",findChild(page,"startSiteTile1"))
        keyClick(Qt.Key_Return)
        var editor=findChild(page,"ferricFavouriteEditor")
        tryCompare(editor,"visible",true)
        tryCompare(host,"activeFocusItem",findChild(page,"favouriteUrlInput"))
        compare(ui.actions.length,0)
    }
    function test_background_favourite_failure_keeps_the_edit_for_retry() {
        ui.receiptMode=true
        var page=createPage(), editor=favouriteEditor(page), url=host.activeFocusItem
        url.text="https://owned.test/keyboard"
        keyClick(Qt.Key_Tab)
        var name=host.activeFocusItem
        compare(name.Accessible.name,"Favourite label")
        name.text="An unfinished favourite"
        keyClick(Qt.Key_Return)
        verify(editor.visible,"An accepted save must wait for its storage result")
        ui.start_page_busy=false
        ui.favourite_action_message="Couldn't save favourite"
        ui.favourite_action_detail="<b>Owned disk failure</b>"
        ui.favourite_action_state="failed"
        verify(editor.visible);compare(url.text,"https://owned.test/keyboard")
        compare(name.text,"An unfinished favourite")
        compare(host.activeFocusItem,name)
        var feedback=findChild(page,"favouriteEditorFeedback")
        verify(feedback.failed);compare(feedback.detail,"<b>Owned disk failure</b>")
    }
    function test_earlier_metadata_read_does_not_complete_a_pending_favourite() {
        var saved=beginFavouriteSave()
        ui.start_page_busy=false;ui.start_page_revision++
        verify(saved.editor.visible);verify(saved.page.favouritePending)
        verify(!findChild(saved.page,"saveFavouriteEdit").enabled)
        keyClick(Qt.Key_Return);compare(ui.actions.length,1)
        publishFavourite(saved.page.favouriteRequest.draft);finishFavourite("succeeded")
        tryCompare(saved.editor,"visible",false)
        tryCompare(host,"activeFocusItem",findChild(saved.page,"startSiteTile1"))
    }
    function test_retry_retains_the_draft_and_uses_its_own_completion() {
        var saved=beginFavouriteSave(), previous=ui.favourite_action_id
        finishFavourite("failed")
        verify(saved.editor.visible);keyClick(Qt.Key_Return)
        verify(ui.favourite_action_id!==previous);verify(saved.page.favouritePending)
        compare(saved.url.text,"https://owned.test/keyboard");compare(saved.name.text,"An unfinished favourite")
        publishFavourite(saved.page.favouriteRequest.draft);finishFavourite("succeeded")
        tryCompare(saved.editor,"visible",false)
    }
    function test_favourite_failure_details_open_with_enter_and_remain_plain_text() {
        var saved=beginFavouriteSave();finishFavourite("failed")
        var feedback=findChild(saved.page,"favouriteEditorFeedback")
        var button=findChild(feedback,"managerFeedbackDetailsButton")
        verify(waitForPolish(host))
        tryVerify(function(){return button.width>0 && button.height>0})
        verify(button.checkable)
        var activations=0
        verify(!button.checked,"Failure details initially start collapsed")
        button.clicked.connect(function(){activations++})
        button.forceActiveFocus();tryCompare(host,"activeFocusItem",button)
        keyClick(Qt.Key_Return)
        verify(saved.editor.visible,"Enter on Show details must not accept or close the favourite editor")
        compare(activations,1,"Enter must activate the focused details control exactly once")
        compare(button.checked,true)
        var details=findChild(feedback,"managerFeedbackDetails")
        verify(details.visible);compare(details.text,"<b>Owned disk failure</b>")
        compare(details.textFormat,TextEdit.PlainText);verify(saved.editor.visible)
    }
    function test_a_newer_favourite_edit_survives_an_earlier_save() {
        var saved=beginFavouriteSave()
        saved.name.text="A newer name"
        publishFavourite(saved.page.favouriteRequest.draft);finishFavourite("succeeded")
        verify(saved.editor.visible);compare(saved.name.text,"A newer name")
        verify(findChild(saved.page,"favouriteNewerEditNotice").visible)
        compare(host.activeFocusItem,saved.name)
        compare(saved.page.editingAction,"edit","A later name change must edit the bookmark already saved in this slot")
        keyClick(Qt.Key_Return)
        compare(ui.actions[ui.actions.length-1].action,"edit")
        compare(ui.actions[ui.actions.length-1].title,"A newer name")
    }
    function test_save_completion_preserves_a_quick_tab_to_tile_options() {
        var saved=beginFavouriteSave()
        publishFavourite(saved.page.favouriteRequest.draft);finishFavourite("succeeded")
        var options=findChild(saved.page,"startSiteOptions1")
        verify(options && options.visible)
        options.forceActiveFocus()
        wait(30)
        compare(host.activeFocusItem,options)
    }
    function test_closing_during_a_save_preserves_a_failed_or_newer_draft_data() {
        return [{tag:"failed",state:"failed",newer:false},{tag:"saved",state:"succeeded",newer:false},{tag:"newer",state:"succeeded",newer:true}]
    }
    function test_closing_during_a_save_preserves_a_failed_or_newer_draft(data) {
        var saved=beginFavouriteSave()
        if(data.newer)saved.name.text="A newer name"
        keyClick(Qt.Key_Escape);tryCompare(saved.editor,"visible",false)
        verify(saved.page.closeNeedsConfirmation)
        if(data.state==="succeeded")publishFavourite(saved.page.favouriteRequest.draft)
        finishFavourite(data.state)
        var retained=data.state==="failed" || data.newer
        compare(saved.page.retainedFavouriteDraft,retained)
        compare(saved.page.closeNeedsConfirmation,retained)
        var resume=findChild(saved.page,"resumeFavouriteEdit")
        compare(resume.visible,retained)
        if(retained) {
            var activated=false
            resume.clicked.connect(function(){activated=true})
            resume.forceActiveFocus();tryCompare(host,"activeFocusItem",resume)
            keyClick(Qt.Key_Return);verify(activated,"Resume edit must receive keyboard activation")
            tryCompare(saved.editor,"visible",true)
            compare(saved.name.text,data.newer?"A newer name":"An unfinished favourite")
        }
    }
    function test_another_tabs_save_preserves_the_closed_favourite_draft_data() {
        return [{tag:"failed",state:"failed",newer:false},
            {tag:"newer",state:"succeeded",newer:true},
            {tag:"saved",state:"succeeded",newer:false}]
    }
    function test_another_tabs_save_preserves_the_closed_favourite_draft(data) {
        var saved=beginFavouriteSave()
        if(data.newer)saved.name.text="A newer name"
        keyClick(Qt.Key_Escape);tryCompare(saved.editor,"visible",false)
        if(data.state==="succeeded")publishFavourite(saved.page.favouriteRequest.draft)
        finishFavourite(data.state)
        var retained=data.state==="failed" || data.newer
        compare(saved.page.retainedFavouriteDraft,retained)
        var message=ui.favourite_action_message, detail=ui.favourite_action_detail
        saved.page.visible=false

        var other=createPage()
        verify(other.addCaptured("https://other.test/","Other tab"))
        var otherEditor=findChild(other,"ferricFavouriteEditor")
        tryCompare(otherEditor,"visible",true)
        keyClick(Qt.Key_Return)
        tryCompare(host,"activeFocusItem",findChild(other,"favouriteNameInput"))
        keyClick(Qt.Key_Return)
        verify(other.favouritePending)
        verify(!saved.page.favouriteReceiptMatches)
        verify(!saved.page.favouritePending,"Another tab's pending save must not belong to this draft")
        compare(saved.page.retainedFavouriteDraft,retained)
        compare(saved.page.closeNeedsConfirmation,retained)
        publishFavourite(other.favouriteRequest.draft);finishFavourite("succeeded")
        tryCompare(otherEditor,"visible",false)
        other.visible=false
        saved.page.visible=true

        var resume=findChild(saved.page,"resumeFavouriteEdit")
        compare(resume.visible,retained)
        compare(saved.page.closeNeedsConfirmation,retained)
        if(!retained)return
        var feedback=findChild(saved.page,"favouritePageFeedback")
        verify(feedback.visible)
        compare(feedback.message,message);compare(feedback.detail,detail)
        compare(feedback.failed,data.state==="failed")
        verify(!feedback.pending)
        saved.page.addCaptured("https://waiting.test/","Waiting request")
        compare(saved.page.queuedFavouriteCaptures.length,1)
        compare(saved.url.text,"https://owned.test/keyboard")
        compare(saved.name.text,data.newer?"A newer name":"An unfinished favourite")
        resume.forceActiveFocus();keyClick(Qt.Key_Return)
        tryCompare(saved.editor,"visible",true)
        compare(saved.page.editingAction,data.newer?"edit":"pin")
        var editorFeedback=findChild(saved.page,"favouriteEditorFeedback")
        compare(editorFeedback.message,message);compare(editorFeedback.detail,detail)
        compare(findChild(saved.page,"favouriteNewerEditNotice").visible,data.newer)
        saved.name.forceActiveFocus();keyClick(Qt.Key_Return)
        verify(saved.page.favouritePending)
        verify(!other.favouritePending)
        publishFavourite(saved.page.favouriteRequest.draft);finishFavourite("succeeded")
        tryCompare(saved.editor,"visible",false)
        verify(!saved.page.retainedFavouriteDraft)
        compare(saved.page.queuedFavouriteCaptures.length,1)
    }
    function test_synchronous_favourite_completion_is_observed_after_return() {
        ui.immediateReceipt=true
        var saved=beginFavouriteSave()
        tryCompare(saved.editor,"visible",false)
        compare(ui.favourite_action_state,"succeeded")
        tryCompare(host,"activeFocusItem",findChild(saved.page,"startSiteTile1"))
    }
    function test_closed_move_returns_to_its_destination_without_stealing_control_focus_data() {
        return [{tag:"grid",toolbar:false},{tag:"view-control",toolbar:true}]
    }
    function test_closed_move_returns_to_its_destination_without_stealing_control_focus(data) {
        ui.receiptMode=true
        var page=createPage()
        page.chooseView("favourites",false)
        page.editSlot(0,"move",{url:"https://owned.test/keyboard",title:"Owned favourite"})
        var editor=findChild(page,"ferricFavouriteEditor")
        tryCompare(editor,"visible",true)
        var destination=host.activeFocusItem
        tryVerify(function(){return destination && destination.Accessible.name==="Destination slot 1"})
        keyClick(Qt.Key_Tab);keyClick(Qt.Key_Return)
        compare(host.activeFocusItem.Accessible.name,"Destination slot 2")
        var save=findChild(page,"saveFavouriteEdit")
        save.forceActiveFocus();keyClick(Qt.Key_Return)
        compare(ui.favourite_action_state,"pending")
        keyClick(Qt.Key_Escape);tryCompare(editor,"visible",false)
        var control=findChild(page,"startViewControlfavourites")
        if(data.toolbar)control.forceActiveFocus()
        else page.focusContent()
        finishFavourite("succeeded")
        tryCompare(host,"activeFocusItem",data.toolbar?control:findChild(page,"startSiteTile2"))
    }
    function test_immediate_rejection_keeps_the_name_focus_and_selection() {
        ui.receiptMode=true;ui.accepted=false
        var page=createPage(), editor=favouriteEditor(page)
        host.activeFocusItem.text="not-a-url";keyClick(Qt.Key_Return)
        var name=host.activeFocusItem;name.text="An unfinished favourite";name.select(2,5)
        keyClick(Qt.Key_Return)
        verify(editor.visible);compare(host.activeFocusItem,name)
        compare(name.selectionStart,2);compare(name.selectionEnd,5)
        compare(ui.favourite_action_state,"failed")
    }
    function test_profile_replacement_clears_the_favourite_editor_and_ignores_old_results() {
        var saved=beginFavouriteSave()
        ui.start_page_profile_revision++
        verify(!saved.editor.visible);verify(!saved.page.closeNeedsConfirmation)
        compare(saved.url.text,"");compare(saved.name.text,"")
        finishFavourite("failed")
        verify(!findChild(saved.page,"favouritePageFeedback").visible)
    }
    function test_favourite_editor_reveals_its_focused_field_data() {
        return [{tag:"390-20",width:390,font:20},{tag:"320-26",width:320,font:26}]
    }
    function test_favourite_editor_reveals_its_focused_field(data) {
        host.width=data.width;host.height=600;theme.chromeFontPointSize=data.font
        var page=createPage(), editor=favouriteEditor(page), url=host.activeFocusItem
        verify(waitForPolish(host))
        tryVerify(function() {
            var p=url.mapToItem(editor.contentItem,0,0)
            return p.y>=0 && p.y+url.height<=editor.contentItem.height+1
        },5000,"The focused URL must be readable inside the editor viewport")
        if(testArtifactDirectory.length>0)grabImage(host.contentItem).save(testArtifactDirectory+"/favourite-editor-"+data.tag+".png")
        keyClick(Qt.Key_Return)
        var name=findChild(page,"favouriteNameInput"),hint=findChild(page,"favouriteNameScopeHint")
        tryCompare(host,"activeFocusItem",name)
        tryVerify(function() {
            var n=name.mapToItem(editor.contentItem,0,0), h=hint.mapToItem(editor.contentItem,0,0)
            return n.y>=0 && n.y+name.height<=editor.contentItem.height+1
                && h.y>=0 && h.y+hint.height<=editor.contentItem.height+1
        },5000,"The focused name and its shared-bookmark explanation must both be readable")
        if(testArtifactDirectory.length>0)grabImage(host.contentItem).save(testArtifactDirectory+"/favourite-name-"+data.tag+".png")
    }
    function buttonWithText(item,text) {
        if(typeof item.clicked==="function" && item.text===text) return item
        for(var child of item.children || []) {var match=buttonWithText(child,text);if(match)return match}
        return null
    }
    function test_favourite_actions_fit_a_narrow_large_font_dialog_data() {
        return test_favourite_editor_reveals_its_focused_field_data()
    }
    function test_favourite_actions_fit_a_narrow_large_font_dialog(data) {
        host.width=data.width;host.height=600;theme.chromeFontPointSize=data.font
        var page=createPage(), editor=favouriteEditor(page)
        verify(waitForPolish(host))
        for(var label of ["Cancel","Save favourite"]) {
            var button=buttonWithText(editor.footer,label);verify(button)
            var p=button.mapToItem(editor.footer,0,0)
            verify(p.x>=0 && p.x+button.width<=editor.footer.width+1 && p.y>=0 && p.y+button.height<=editor.footer.height+1,label+" must fit the dialog footer")
        }
    }
    function test_guide_filter_keeps_a_useful_empty_state() {
        var page = createPage()
        page.guideSurface.searchText = "nothing-matches"
        compare(page.guideSurface.rows.length, 0)
        page.guideSurface.searchText = "o"
        compare(page.guideSurface.rows.length, 1)
    }
    function test_guide_groups_shortcuts_without_losing_invocations() {
        ui.multipleBindings = true
        var page = createPage()
        var guide = page.guideSurface
        compare(guide.rows.filter(function(row) { return !row.heading }).length, 1)
        compare(guide.currentRow.bindings.length, 2)
        compare(guide.currentRow.bindings[0].keys, "o")
        compare(guide.currentRow.bindings[1].keys, "O")
        compare(guide.currentRow.bindings[1].command, "open --target tab")
        compare(guide.currentRow.bindings[1].mode, "caret")
        compare(guide.currentRow.recovery, "Escape cancels address entry.")
    }
    function test_guide_changed_query_selects_first_result_but_refresh_keeps_selection() {
        ui.twoFeatures = true
        var guide = createPage().guideSurface
        guide.searchInput.text = "open"
        guide.focusEntries()
        keyClick(Qt.Key_Down)
        compare(guide.currentRow.identity, "reload")
        guide.refresh()
        compare(guide.currentRow.identity, "reload")
        guide.searchInput.text = "o"
        compare(guide.currentRow.identity, "open")
        verify(guide.rows.every(function(row) { return !row.heading }))
        guide.searchInput.text = ""
        verify(guide.rows.some(function(row) { return row.heading }))
    }
    function test_guide_hints_follow_the_focused_keyboard_recipient() {
        var guide = createPage().guideSurface
        guide.focusSearch()
        tryCompare(guide.searchInput, "activeFocus", true)
        verify(guide.navigationHelp.indexOf("Type to search") >= 0)
        verify(guide.navigationHelp.indexOf("j/k") < 0)
        keyClick(Qt.Key_O)
        verify(guide.navigationHelp.indexOf("Esc clear search") >= 0)
        keyClick(Qt.Key_Down)
        tryCompare(guide.entriesList, "activeFocus", true)
        verify(guide.navigationHelp.indexOf("j/k browse") >= 0)
        keyClick(Qt.Key_Return)
        tryCompare(findChild(guide, "ferricGuideArticle"), "activeFocus", true)
        verify(guide.navigationHelp.indexOf("j/k scroll") >= 0)
        var command = findChild(guide, "ferricGuideInvocation")
        command.forceActiveFocus()
        verify(guide.navigationHelp.indexOf("Ctrl+C copy") >= 0)
        keyClick(Qt.Key_Escape)
        verify(guide.navigationHelp.indexOf("j/k scroll") >= 0)
    }
    function test_guide_reader_is_bounded_and_narrow_back_restores_focus() {
        host.width = 1800
        var page = createPage()
        var guide = page.guideSurface
        guide.focusReader()
        wait(0)
        verify(guide.readingWidth <= 620)
        verify(!guide.compact)
        host.width = 390
        wait(0)
        verify(guide.compact)
        verify(!guide.entriesList.visible)
        var reader = findChild(page, "ferricGuideArticle")
        reader.forceActiveFocus()
        verify(guide.handleNavigation(event(Qt.Key_Escape)))
        tryVerify(function() { return guide.entriesList.visible && guide.entriesList.activeFocus })
        verify(!guide.articleOpen)
        compare(ui.navigations, 0)
    }
    function test_guide_slash_returns_to_search_with_query_selected_data() {
        return [{tag:"wide-list",width:1000,reader:false},
                {tag:"wide-reader",width:1000,reader:true},
                {tag:"compact-list",width:390,reader:false},
                {tag:"compact-reader",width:390,reader:true}]
    }
    function test_guide_slash_returns_to_search_with_query_selected(data) {
        host.width=data.width
        var guide=createPage().guideSurface
        guide.searchInput.text="open"
        guide.focusEntries()
        if(data.reader) guide.focusReader()
        wait(0)
        var help=guide.navigationHelp
        keyClick(Qt.Key_Slash)
        tryCompare(guide.searchInput,"activeFocus",true)
        verify(help.indexOf("/ search")>=0)
        verify(guide.searchInput.visible)
        verify(!guide.readerActive)
        compare(guide.searchInput.selectedText,"open")
        keyClick(Qt.Key_O)
        compare(guide.searchInput.text,"o")
        keyClick(Qt.Key_Slash)
        compare(guide.searchInput.text,"o/")
        compare(ui.navigations,0)
    }
    function test_guide_reader_scroll_and_copy_keep_keyboard_actions_local() {
        host.height = 520
        var page = createPage()
        var guide = page.guideSurface
        guide.focusEntries()
        guide.focusReader()
        wait(0)
        var scroll = findChild(page, "ferricGuideReaderScroll")
        verify(scroll.contentItem.contentHeight > scroll.contentItem.height)
        verify(guide.handleNavigation(event(Qt.Key_PageDown)))
        verify(scroll.contentItem.contentY > 0)
        var selected = guide.currentRow.key
        verify(guide.handleNavigation(event(Qt.Key_J)))
        compare(guide.currentRow.key, selected)
        verify(guide.handleNavigation(event(Qt.Key_Space, Qt.ShiftModifier)))
        verify(guide.handleNavigation(event(Qt.Key_PageUp)))
        compare(scroll.contentItem.contentY, 0)
        var command = findChild(page, "ferricGuideInvocation")
        command.forceActiveFocus()
        verify(page.inputActive)
        verify(!guide.handleNavigation(event(Qt.Key_J)))
        keyClick(Qt.Key_Escape)
        tryVerify(function() { return !page.inputActive })
        verify(guide.articleOpen)
        compare(ui.navigations, 0)
    }
    function test_only_saved_sites_have_numeric_shortcut_badges() {
        var page = createPage()
        page.chooseView("favourites", false)
        wait(0)
        var saved = findChild(page, "favouriteShortcut8")
        var empty = findChild(page, "favouriteShortcut1")
        verify(saved.visible)
        compare(saved.text, "8")
        verify(!empty.visible)
        compare(findChild(page, "siteFallbackGlyph8").kind, "globe")
        compare(findChild(page, "siteFallbackGlyph1").kind, "plus")
        page.focusContent()
        verify(page.handleNavigation(event(Qt.Key_1)))
        compare(ui.navigations, 0)
    }
    function test_guide_disclosure_uses_the_focused_control() {
        var page = createPage()
        var guide = page.guideSurface
        guide.focusReader()
        wait(0)
        var disclosure = findChild(page, "ferricGuideAdvanced")
        wait(0)
        disclosure.forceActiveFocus()
        wait(0)
        verify(disclosure.activeFocus)
        verify(page.inputActive)
        verify(!guide.handleNavigation(event(Qt.Key_Space)))
        keyClick(Qt.Key_Space)
        tryCompare(disclosure, "checked", true)
        verify(guide.articleOpen)
        keyClick(Qt.Key_Escape)
        tryVerify(function() { return guide.entriesList.activeFocus })
        verify(guide.articleOpen)
        compare(ui.navigations, 0)
    }
    function test_view_buttons_keep_keyboard_activation_local() {
        var page = createPage()
        wait(0)
        var topSites = findChild(page, "startViewControltop-sites")
        topSites.forceActiveFocus()
        wait(0)
        verify(topSites.activeFocus)
        verify(page.inputActive)
        verify(!page.handleNavigation(event(Qt.Key_1)))
        keyClick(Qt.Key_Space)
        tryCompare(page, "selectedView", "top-sites")
        tryVerify(function() { return !page.inputActive })
        compare(ui.navigations, 0)
    }
    function test_start_buttons_accept_fresh_enter_once_data() {
        return [{tag:"return",key:Qt.Key_Return},{tag:"enter",key:Qt.Key_Enter}]
    }
    Component {
        id: wrappingWithNavigation
        Item {
            width: 300; height: 80
            property int escapedReturns: 0
            property int clicks: 0
            property alias button: button
            Keys.onPressed: function(event) {
                if (event.key===Qt.Key_Return || event.key===Qt.Key_Enter) {
                    escapedReturns++;event.accepted=true
                }
            }
            FerricWrappingButton {
                id: button
                text: "Previous"
                checkable: true
                onClicked: parent.clicks++
                Keys.onPressed: function(event) { event.accepted=false }
            }
        }
    }
    function test_wrapping_button_return_cannot_escape_a_navigation_handler_data() {
        return [{tag:"return",key:Qt.Key_Return},{tag:"enter",key:Qt.Key_Enter}]
    }
    function test_wrapping_button_return_cannot_escape_a_navigation_handler(data) {
        var surface=createTemporaryObject(wrappingWithNavigation,host.contentItem)
        verify(surface)
        waitForPolish(host)
        surface.button.forceActiveFocus()
        keyClick(data.key)
        compare(surface.clicks,1)
        compare(surface.escapedReturns,0)
        compare(surface.button.checked,true)
    }
    function test_start_buttons_accept_fresh_enter_once(data) {
        var page=createPage(), button=findChild(page,"startViewControltop-sites")
        button.forceActiveFocus();tryCompare(host,"activeFocusItem",button)
        keyClick(data.key)
        tryCompare(page,"selectedView","top-sites")
        compare(ui.actions.length,1);verify(button.checked)
        compare(ui.navigations,0)
    }
    function test_text_entry_does_not_activate_numbered_targets() {
        var page = createPage()
        page.chooseView("guide", false)
        page.guideSurface.searchInput.forceActiveFocus()
        verify(page.inputActive)
        verify(!page.handleNavigation(event(Qt.Key_1)))
        compare(ui.navigations, 0)
    }
    function test_refresh_waits_for_ranked_result() {
        var page = createPage()
        page.chooseView("top-sites", false)
        page.refreshTopRequested = true
        page.requestedRankRevision = ui.start_page_rank_revision
        ui.start_page_urls = ["https://changed.test", "https://b.test", "https://pinned.test"]
        ui.start_page_revision++
        compare(page.topTiles[0].url, "https://a.test")
        verify(page.refreshTopRequested)
        ui.start_page_rank_revision++
        ui.start_page_revision++
        compare(page.topTiles[0].url, "https://changed.test")
        verify(!page.refreshTopRequested)
        ui.start_page_urls = ["https://a.test", "https://b.test", "https://pinned.test"]
    }
    function test_profile_switch_clears_tiles_and_waits_for_preference() {
        var page = createPage()
        page.chooseView("favourites", false)
        ui.start_page_ready = false
        ui.start_page_profile_revision++
        compare(page.selectedView, "guide")
        verify(page.favouriteTiles.every(function(tile) { return tile === null }))
        ui.start_page_view = "top-sites"
        ui.start_page_ready = true
        ui.start_page_revision++
        compare(page.selectedView, "top-sites")
        ui.start_page_view = "guide"
    }
    function test_deleted_url_clears_slot_even_when_site_still_exists() {
        var page = createPage()
        page.chooseView("top-sites", false)
        ui.start_page_eligible_urls = ["https://a.test/different", "https://b.test"]
        ui.start_page_revision++
        compare(page.topTiles[0], null)
        compare(page.topTiles[1].url, "https://b.test")
    }
    function test_popup_controls_keep_numbers_out_of_grid() {
        var page = createPage()
        page.chooseView("top-sites", false)
        page.focusContent()
        page.menuActive = true
        verify(page.inputActive)
        verify(!page.handleNavigation(event(Qt.Key_1)))
        compare(ui.navigations, 0)
    }

}

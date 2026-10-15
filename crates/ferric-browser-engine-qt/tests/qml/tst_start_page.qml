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
        property int navigations: 0
        property var actions: []
        function refresh_start_page() { start_page_rank_revision++; start_page_revision++; return true }
        function refresh_start_page_metadata() { start_page_revision++ }
        function start_page_action(action, slot, value, title) { actions = actions.concat([{ action: action, slot: slot, value: value }]); return true }
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
    function test_guide_focus_callbacks_do_not_outlive_the_view() {
        failOnWarning(/TypeError|ReferenceError/)
        var loader = createTemporaryObject(lifecycleFixture, host.contentItem)
        verify(loader && loader.item)
        loader.item.guideSurface.focusEntries()
        loader.active = false
        wait(0)
    }
    function init() {
        host.width = 1000; host.height = 760; ui.multipleBindings = false; ui.twoFeatures = false
        ui.start_page_view = "guide"; ui.start_page_revision = 0; ui.navigations = 0
        ui.actions = []; ui.pending = false; ui.start_page_ready = true; ui.start_page_eligible_urls = ["https://a.test", "https://b.test"]; ui.start_page_eligible_sites = ["a.test", "b.test"]
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
    function test_guide_filter_keeps_a_useful_empty_state() {
        var page = createPage()
        page.guideSurface.searchText = "nothing-matches"
        compare(page.guideSurface.rows.length, 0)
        page.guideSurface.searchText = "o"
        compare(page.guideSurface.rows.length, 2)
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

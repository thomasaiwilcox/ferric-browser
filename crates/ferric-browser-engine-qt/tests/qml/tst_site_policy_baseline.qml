import QtQuick
import QtTest
import QtWebEngine
import "../../qml/scripts/BrowserScripts.js" as BrowserScripts

// P00: qualify the local test subject and ordinary production discovery before
// introducing policy. These assertions do not qualify enhanced addressing.
TestCase {
    id: tests
    name: "SitePolicyBaseline"
    when: windowShown
    width: 1000
    height: 800
    property bool done: false
    property var result
    WebEngineView {
        id: page
        anchors.fill: parent
        property bool loaded: false
        onLoadingChanged: function(request) {
            loaded = request.status === WebEngineView.LoadSucceededStatus
        }
    }
    function evaluate(source, world) {
        done = false
        page.runJavaScript(source, world, function(value) {
            result = value
            done = true
        })
        tryVerify(function() { return done }, 5000)
        return result
    }
    function main(source) { return evaluate(source, WebEngineScript.MainWorld) }
    function collect() {
        return evaluate(BrowserScripts.hintCollector("links"), WebEngineScript.ApplicationWorld)
    }
    function init() {
        page.loaded = false
        page.url = Qt.resolvedUrl("../fixtures/site-policy/interaction.html")
        tryVerify(function() { return page.loaded }, 10000)
        compare(main("typeof window.policyFixture"), "object")
    }
    function cleanup() {
        evaluate(BrowserScripts.hintStopTracking(), WebEngineScript.ApplicationWorld)
    }
    function test_navigation_scope_duplicate_and_nonactivation() {
        var candidates
        tryVerify(function() {
            candidates = collect().candidates
            return candidates.filter(function(c) { return c.text === "Mac" }).length === 2
        }, 5000)
        compare(candidates.filter(function(c) { return c.text === "Watch" }).length, 1)
        main("policyFixture.duplicateMac(); true")
        tryVerify(function() {
            return collect().candidates.filter(function(c) { return c.text === "Mac" }).length === 3
        }, 5000)
        main("policyFixture.removeWatch(); true")
        compare(collect().candidates.filter(function(c) { return c.text === "Watch" }).length, 0)
        compare(main("policyFixture.snapshot().activations"), 0)
    }
    function test_distinct_cards_same_url_and_tall_primary() {
        var candidates
        tryVerify(function() {
            candidates = collect().candidates
            return candidates.some(function(c) { return c.text === "Tall card primary" })
        }, 5000)
        var first = candidates.filter(function(c) { return c.text === "Video one" })[0]
        var second = candidates.filter(function(c) { return c.text === "Distinct card, same URL" })[0]
        verify(!!first && !!second)
        verify(first.element_id !== second.element_id)
        compare(main("policyFixture.snapshot().activations"), 0)
    }
    function test_recycled_card_invalidates_retained_target_without_activation() {
        var original
        tryVerify(function() {
            original = collect().candidates.filter(function(c) { return c.text === "Video one" })[0]
            return !!original
        }, 5000)
        main("window.retainedPolicyCard = document.querySelector('.fixture-video-card'); policyFixture.recycle(); true")
        compare(main("document.querySelector('.fixture-video-card') === window.retainedPolicyCard"), true)
        compare(main("window.retainedPolicyCard.dataset.videoId"), "replacement")
        var stale = evaluate(BrowserScripts.hintFresh(original), WebEngineScript.ApplicationWorld)
        compare(stale.visible, false)
        var replacement
        tryVerify(function() {
            replacement = collect().candidates.filter(function(c) { return c.text === "Video one" })[0]
            return !!replacement && replacement.href.endsWith("#replacement")
        }, 5000)
        verify(original.href !== replacement.href)
        compare(main("policyFixture.snapshot().activations"), 0)
    }
    function test_delayed_target_requires_a_fresh_collection() {
        page.loaded = false
        page.url = Qt.resolvedUrl("../fixtures/site-policy/roots.html")
        tryVerify(function() { return page.loaded }, 10000)
        var initial = evaluate(BrowserScripts.hintCollector("buttons"), WebEngineScript.ApplicationWorld)
        compare(initial.candidates.filter(function(c) { return c.text === "Delayed primary" }).length, 0)
        main("rootFixture.addDelayed(); true")
        // A retained result stays a snapshot; a later collection discovers the
        // newly authored fixture target. No implicit activation is performed.
        compare(initial.candidates.filter(function(c) { return c.text === "Delayed primary" }).length, 0)
        tryVerify(function() {
            var current = evaluate(BrowserScripts.hintCollector("buttons"), WebEngineScript.ApplicationWorld)
            return current.candidates.filter(function(c) { return c.text === "Delayed primary" }).length === 1
        }, 5000)
    }
    function test_shadow_focus_trace_and_ordinary_discovery() {
        main("document.querySelector('#fixture-grid').scrollIntoView(); true")
        tryVerify(function() {
            return collect().candidates.some(function(c) { return c.text === "Grid video 1" })
        }, 5000)
        main("document.querySelector('#shadow-card-0').shadowRoot.querySelector('a').focus(); true")
        // Fixture-only synthetic observation checks trace structure, not native
        // delivery or trusted-event behavior. Native qualification is separate.
        main("document.querySelector('#shadow-card-0').shadowRoot.querySelector('a').dispatchEvent(new KeyboardEvent('keydown',{key:'p',code:'KeyP',bubbles:true,composed:true})); true")
        var observed = main("policyFixture.snapshot().events.slice(-1)[0]")
        compare(observed.focusPath[0].id, "shadow-card-0")
        compare(observed.focusPath[1].id, "shadow-primary-0")
        compare(observed.targetPath[0].id, "shadow-primary-0")
        compare(observed.trusted, false)
        compare(main("policyFixture.snapshot().activations"), 0)
    }
    function test_distinct_grid_and_sidebar_collections() {
        page.loaded = false
        page.url = Qt.resolvedUrl("../fixtures/site-policy/sidebar.html")
        tryVerify(function() { return page.loaded }, 10000)
        var candidates
        tryVerify(function() {
            candidates = collect().candidates
            return candidates.length === 4
        }, 5000)
        var grid = candidates.filter(function(c) { return c.text === "Grid primary one" })[0]
        var sidebar = candidates.filter(function(c) { return c.text === "Sidebar primary one" })[0]
        verify(!!grid && !!sidebar)
        compare(grid.href, sidebar.href)
        verify(grid.element_id !== sidebar.element_id)
        compare(main("document.querySelector('#sidebar').getBoundingClientRect().left > document.querySelector('#grid').getBoundingClientRect().right"), true)
    }
    function test_frame_chain_and_foreign_origin_negative_boundary() {
        page.loaded = false
        page.url = Qt.resolvedUrl("../fixtures/site-policy/frames.html")
        tryVerify(function() { return page.loaded }, 10000)
        tryVerify(function() { return main("frameFixture.ready") }, 5000)
        compare(main("frameFixture.origin"), "null")
        compare(main("document.querySelector('#foreign').contentDocument === null"), true)
        var candidates
        tryVerify(function() {
            candidates = collect().candidates
            return candidates.some(function(c) { return c.text === "Deep frame primary" })
        }, 5000)
        // Ordinary discovery cannot cross the opaque origin. The fixture's
        // message handshake supplies a subject for future broker tests, not a
        // claim that production has implemented or authorized that broker.
        compare(candidates.filter(function(c) { return c.text === "Foreign primary" }).length, 0)
        main("document.querySelector('#foreign').contentWindow.postMessage('fixture-focus', '*'); true")
        tryVerify(function() { return main("frameFixture.focused") }, 5000)
        compare(main("document.activeElement.id"), "foreign")
        compare(main("frameFixture.activations"), 0)
    }
    function test_root_subjects_slot_frame_and_occlusion() {
        page.loaded = false
        page.url = Qt.resolvedUrl("../fixtures/site-policy/roots.html")
        tryVerify(function() { return page.loaded }, 10000)
        tryVerify(function() {
            return main("!!document.querySelector('#same-origin').contentDocument.querySelector('#frame-primary')")
        }, 5000)
        var candidates
        tryVerify(function() {
            candidates = collect().candidates
            return candidates.some(function(c) { return c.text === "Slotted primary" })
                && candidates.some(function(c) { return c.text === "Frame primary" })
                && candidates.some(function(c) { return c.text === "Frame shadow primary" })
        }, 5000)
        compare(candidates.filter(function(c) { return c.text === "Slotted primary" }).length, 1)
        compare(candidates.filter(function(c) { return c.text === "Covered primary" }).length, 0)
        compare(main("document.querySelector('#catalog').shadowRoot.querySelector('slot').assignedElements().length"), 1)
        main("rootFixture.addAssignedDuplicate(); true")
        compare(main("document.querySelector('#catalog').shadowRoot.querySelector('slot').assignedElements().length"), 2)
        tryVerify(function() {
            return collect().candidates.filter(function(c) { return c.text === "Slotted primary" }).length === 2
        }, 5000)
        main("rootFixture.reassignSlot(); true")
        compare(main("document.querySelector('#catalog').shadowRoot.querySelector('slot').assignedElements().length"), 1)
        tryVerify(function() {
            return collect().candidates.filter(function(c) { return c.text === "Slotted primary" }).length === 1
        }, 5000)
    }
}

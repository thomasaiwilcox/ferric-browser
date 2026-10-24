import QtQuick
import QtQuick.Layouts
import QtTest
import "../../qml/components"

TestCase {
    id: test
    name: "TabViews"
    when: windowShown
    visible: true
    width: 900; height: 700
    Component {
        id: uiFixture
        QtObject {
            property var ids: ["tabid-1", "tabid-2"]
            readonly property int tab_count: ids.length
            property int active_tab_index: 0
            property string pending: ""
            property string status_text: ""
            property var pinned: ({})
            property var muted: ({})
            property var descriptors: ({})
            function tab_pinned_at(index) { return !!pinned[ids[index]] }
            function tab_muted_at(index) { return !!muted[ids[index]] }
            function tab_id_for_index(index) { return ids[index] || "" }
            function tab_transfer_payload(id) { return JSON.stringify(descriptors[id] || {url:"rb://blank"}) }
            function take_session_restore_values() { var value=pending;pending="";return value }
        }
    }
    Component {
        id: testViewFactory
        Item {
            property string stableTabId: ""
            property int tabIndex: -1
            property var viewUi
            property var viewHost
            property var viewTabs
            property var viewModel
            property var viewProfile
            property var viewInterceptor
            property bool viewTransientProfile: false
            property bool viewTransferred: false
            property bool viewRetired: false
            property bool initialPinned: false
            property bool initialMuted: false
            property real initialZoom: -1
            Component.onCompleted: {
                initialPinned = viewModel.pinned
                initialMuted = viewModel.muted
                initialZoom = viewModel.zoom
            }
        }
    }
    Component {
        id: harness
        Item {
            id: state
            required property var owner
            property bool ready: true
            property alias controller: controller
            property alias tabs: tabs
            ListModel { id: tabs }
            StackLayout {
                id: viewport
                anchors.fill: parent
                currentIndex: {
                    var selected = controller.tabViews[state.owner.active_tab_index]
                    return selected ? selected.StackLayout.index : -1
                }
            }
            FerricTabViewController {
                id: controller
                ready: state.ready
                browserHost: test
                browserUi: state.owner
                tabs: state.tabs
                viewParent: viewport
                viewFactory: testViewFactory
                profile: test
                interceptor: test
                transientProfile: false
            }
        }
    }
    function setup(owner) {
        var view=createTemporaryObject(harness,test,{owner:owner})
        verify(view)
        view.controller.syncTabModel()
        return view
    }
    function init() {
        failOnWarning(/TypeError|ReferenceError|Cannot assign|Unable to assign|Binding loop/)
    }
    function test_each_logical_tab_gets_a_view_and_owner() {
        var owner=createTemporaryObject(uiFixture,test)
        var state=setup(owner)
        compare(state.tabs.count,2);compare(state.controller.tabViews.length,2)
        for(var i=0;i<2;i++) {
            var view=state.controller.tabViews[i]
            compare(view.stableTabId,owner.ids[i]);compare(view.tabIndex,i)
            compare(view.viewUi,owner);compare(view.viewHost,test)
            compare(view.viewTabs,state.tabs)
        }
    }
    function test_recovered_settings_are_present_before_view_creation() {
        var owner = createTemporaryObject(uiFixture, test, {
            ids: ["recovered-tab"],
            pinned: ({"recovered-tab":true}),
            muted: ({"recovered-tab":true}),
            descriptors: ({"recovered-tab": {
                url:"https://recovered.test/", pinned:true, muted:true, zoom_hundredths:175
            }})
        })
        var state = setup(owner)
        var view = state.controller.tabViews[0]
        compare(view.initialPinned, true)
        compare(view.initialMuted, true)
        compare(view.initialZoom, 1.75)
        compare(state.tabs.get(0).zoom, 1.75)
    }
    function test_bootstrap_does_not_materialize_provisional_tabs_or_consume_restore() {
        var owner=createTemporaryObject(uiFixture,test,{ids:["provisional-tab"]})
        var state=createTemporaryObject(harness,test,{owner:owner,ready:false})
        verify(state)
        owner.pending="replace\nhttps://restored.test/\tfalse\tfalse\t1\t0\t0"
        for (var i=0;i<3;i++) state.controller.syncTabModel()
        compare(state.tabs.count,0)
        compare(state.controller.tabViews.length,0)
        verify(owner.pending.length>0)
        owner.ids=["configured-tab"]
        state.controller.syncTabModel()
        compare(state.controller.tabViews.length,0)
        state.ready=true
        tryCompare(state.controller,"tabIdentitySignature","configured-tab")
        compare(state.controller.tabViews.length,1)
        compare(state.controller.tabViews[0].stableTabId,"configured-tab")
        compare(state.controller.tabViews[0].viewUi,owner)
        compare(state.tabs.get(0).url,"https://restored.test/")
        compare(owner.pending,"")
    }
    function test_restore_rows_follow_ids_when_pinning_reorders_the_plan() {
        var owner = createTemporaryObject(uiFixture, test, {ids:["startup"]})
        var state = setup(owner)
        owner.ids = ["pinned", "selected"]
        owner.active_tab_index = 1
        owner.pinned = ({pinned:true})
        owner.muted = ({selected:true})
        owner.pending = "replace\nhttps://selected.test/\tfalse\ttrue\t1.5\t0\t0\tselected"
            + "\nhttps://pinned.test/\ttrue\tfalse\t1.25\t0\t0\tpinned"
        state.controller.syncTabModel()
        compare(state.tabs.get(0).tabId, "pinned")
        compare(state.tabs.get(0).url, "https://pinned.test/")
        compare(state.tabs.get(0).zoom, 1.25)
        compare(state.tabs.get(0).loaded, false)
        compare(state.tabs.get(1).tabId, "selected")
        compare(state.tabs.get(1).url, "https://selected.test/")
        compare(state.tabs.get(1).zoom, 1.5)
        compare(state.tabs.get(1).muted, true)
        compare(state.tabs.get(1).loaded, true)
    }
    function test_append_pin_grouping_retains_neighbors_and_lazy_pages() {
        var owner = createTemporaryObject(uiFixture, test, {
            ids:["existing-pin", "existing-normal"], pinned:({"existing-pin":true})
        })
        var state = setup(owner)
        var first = state.controller.tabViews[0], second = state.controller.tabViews[1]
        owner.ids = ["existing-pin", "new-pin", "existing-normal", "new-selected", "new-hidden"]
        owner.active_tab_index = 3
        owner.pinned = ({"existing-pin":true, "new-pin":true})
        owner.muted = ({"new-selected":true})
        owner.pending = "append\nhttps://selected.test/\tfalse\ttrue\t1.5\t0\t0\tnew-selected"
            + "\nhttps://pinned.test/\ttrue\tfalse\t1.25\t0\t0\tnew-pin"
            + "\nhttps://hidden.test/\tfalse\tfalse\t1.75\t0\t0\tnew-hidden"
        state.controller.syncTabModel()
        compare(state.controller.tabViews[0], first)
        compare(state.controller.tabViews[2], second)
        compare(state.tabs.get(1).tabId, "new-pin")
        compare(state.tabs.get(1).url, "https://pinned.test/")
        compare(state.tabs.get(1).zoom, 1.25)
        compare(state.tabs.get(1).loaded, false)
        compare(state.tabs.get(3).url, "https://selected.test/")
        compare(state.tabs.get(3).loaded, true)
        compare(state.tabs.get(3).muted, true)
        compare(state.tabs.get(4).url, "https://hidden.test/")
        compare(state.tabs.get(4).zoom, 1.75)
        compare(state.tabs.get(4).loaded, false)
    }
    function test_reordering_retains_live_view_identity() {
        var owner=createTemporaryObject(uiFixture,test)
        var state=setup(owner)
        var first=state.controller.tabViews[0],second=state.controller.tabViews[1]
        owner.ids=["tabid-2","tabid-1"]
        state.controller.syncTabModel()
        compare(state.controller.tabViews[0],second);compare(state.controller.tabViews[1],first)
        tryCompare(second,"visible",true);compare(first.visible,false)
        compare(first.tabIndex,1);compare(first.viewModel.tabId,"tabid-1")
        owner.ids=["tabid-1"]
        state.controller.syncTabModel()
        compare(state.controller.tabViews.length,1);compare(state.controller.tabViews[0],first)
        compare(first.tabIndex,0)
    }
    function test_pin_and_mute_updates_keep_the_same_view() {
        var owner=createTemporaryObject(uiFixture,test)
        var state=setup(owner),view=state.controller.tabViews[0]
        owner.pinned=({"tabid-1":true});owner.muted=({"tabid-1":true})
        state.controller.syncTabModel()
        compare(state.tabs.get(0).pinned,true);compare(state.tabs.get(0).muted,true)
        compare(state.controller.tabViews[0],view)
    }
    function test_restore_creates_views_without_loading_hidden_pages() {
        var owner=createTemporaryObject(uiFixture,test)
        var state=setup(owner)
        owner.ids=["tabid-3","tabid-4","tabid-5"]
        owner.active_tab_index=1
        owner.pinned=({"tabid-3":true});owner.muted=({"tabid-4":true})
        owner.pending="replace\nhttps://one.test/\ttrue\tfalse\t1.25\t10\t20\nhttps://two.test/\tfalse\ttrue\t1.5\t30\t40\nhttps://three.test/\tfalse\tfalse\t1\t0\t0"
        state.controller.syncTabModel()
        compare(state.controller.tabViews.length,3)
        compare(state.tabs.get(0).loaded,false);compare(state.tabs.get(1).loaded,true)
        compare(state.tabs.get(2).loaded,false)
        compare(state.tabs.get(0).pinned,true);compare(state.tabs.get(1).muted,true)
        compare(state.tabs.get(1).zoom,1.5);compare(state.tabs.get(1).scrollY,40)
        compare(state.controller.tabViews[1].stableTabId,"tabid-4")
        var hidden=state.controller.tabViews[2]
        owner.active_tab_index=2;state.controller.syncTabModel()
        compare(state.tabs.get(2).loaded,true);compare(state.controller.tabViews[2],hidden)
    }
}

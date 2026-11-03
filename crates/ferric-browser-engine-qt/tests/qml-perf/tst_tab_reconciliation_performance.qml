import QtQuick
import QtQuick.Layouts
import QtTest
import Ferric.Testing 1.0
import "../../qml/components"

TestCase {
    id: test
    name: "TabReconciliationPerformance"
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
            property var promptAssociation: null
            function refreshPromptViewIdentity() {
                promptAssociation={ui:viewUi,index:tabIndex,id:stableTabId}
            }
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
    function test_samples_data() {
        var rows = []
        for (var count of [20, 100, 500]) {
            rows.push({tag:String(count)+"-aligned",count:count,reorder:false})
            rows.push({tag:String(count)+"-reversed",count:count,reorder:true})
        }
        return rows
    }
    function test_samples(data) {
        var ids = []
        for (var index=0; index<data.count; ++index) ids.push("tabid-"+String(index))
        var owner=createTemporaryObject(uiFixture,test,{ids:ids})
        var state=setup(owner)
        for (var warm=0; warm<10; ++warm) {
            if (data.reorder) owner.ids=owner.ids.slice().reverse()
            state.controller.syncTabModel()
        }
        var samples=[]
        for (var sample=0; sample<100; ++sample) {
            if (data.reorder) owner.ids=owner.ids.slice().reverse()
            var started=PerformanceClock.nowNsecs()
            state.controller.syncTabModel()
            samples.push((PerformanceClock.nowNsecs()-started)/1000000)
            compare(state.tabs.count,data.count)
            for (var row=0; row<data.count; ++row) {
                compare(state.tabs.get(row).tabId,owner.ids[row])
                compare(state.controller.tabViews[row].stableTabId,owner.ids[row])
            }
        }
        console.info("FERRIC_TAB_RECONCILIATION_JSON "+JSON.stringify({
            count:data.count,scenario:data.tag,warmups:10,samples_ms:samples,
            boundary:"production controller and ListModel with Item views; excludes WebEngine rendering"
        }))
    }
}

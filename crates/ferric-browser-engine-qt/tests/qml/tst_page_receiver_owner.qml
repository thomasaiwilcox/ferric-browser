import QtQuick
import QtTest
import "../../qml/components" as Components

TestCase {
    id: test
    name: "PageReceiverOwner"
    property var selectedView: firstView
    property bool registered: true
    property int nativeProbes: 0
    property int observations: 0
    property var lastProbeView: null
    property var entry: null
    QtObject {
        id: ui
        property int input_epoch: 13
        property int active_tab_index: 0
        function page_key_routing_active() { return true }
        function navigation_target_for_index(index) { return {tabId:"tab-"+index,generation:"1",documentId:"1"} }
        function page_key_focus_observed_for(index,url,path,epoch,facts) { ++test.observations }
    }
    QtObject { id: otherUi }
    QtObject { id: frame; property bool isValid: true }
    QtObject { id: firstView; property string nativeUrl: ""; property var mainFrame: frame; property url url: "https://fixture.invalid/watch" }
    QtObject { id: secondView; property string nativeUrl: ""; property var mainFrame: frame; property url url: "https://fixture.invalid/watch" }
    QtObject {
        id: router
        function probePageReceiver(view,script,epoch) { ++test.nativeProbes;test.lastProbeView=view }
    }
    QtObject {
        id: host
        property bool runtimeReleased: false
        property bool runtimeDisposing: false
        property var browserUi: ui
        property var browserKeyRouterObject: router
        function activeWebView() { return test.selectedView }
        function browserWindowEntryForUi(value) { return test.registered && value===ui ? test.entry : null }
        function selectionProviderFor(view) { return null }
        function probeFocusFrame() {}
        function focusProbeScript() { return "true" }
    }
    QtObject { id: wrongHost }
    Components.FerricPageServices { id: services; host: host }
    function init() {
        selectedView=firstView;registered=true;host.runtimeReleased=false;host.runtimeDisposing=false
        nativeProbes=0;observations=0;lastProbeView=null;ui.active_tab_index=0
        entry={ui:ui,host:host,view:firstView,profileName:"fixture"}
    }
    function test_switch_resolves_current_page() {
        compare(services.diagnosticsOwner(ui,null).view,firstView)
        selectedView=secondView;ui.active_tab_index=1
        const owner=services.diagnosticsOwner(ui,host)
        compare(owner.view,secondView)
        compare(owner.profileName,"fixture")
        compare(entry.view,firstView,"registration is a snapshot, not active page authority")
        selectedView=firstView
        compare(services.diagnosticsOwner(ui,null).view,firstView)
    }
    function test_second_tab_receives_native_probe() {
        selectedView=secondView;ui.active_tab_index=1
        services.probeActiveFocus(ui,1,secondView)
        compare(nativeProbes,1)
        compare(lastProbeView,secondView)
    }
    function test_late_old_tab_receipt_rejected() {
        selectedView=secondView;ui.active_tab_index=1
        services.acceptPageReceiverObservation(ui,firstView,13,"0","{}")
        compare(observations,0)
        services.acceptPageReceiverObservation(ui,secondView,12,"0","{}")
        compare(observations,0)
        services.acceptPageReceiverObservation(ui,secondView,13,"0","{}")
        compare(observations,1)
    }
    function test_owner_guards() {
        compare(services.diagnosticsOwner(ui,wrongHost),null)
        compare(services.diagnosticsOwner(otherUi,host),null)
        host.runtimeReleased=true
        compare(services.diagnosticsOwner(ui,host),null)
        host.runtimeReleased=false;host.runtimeDisposing=true
        compare(services.diagnosticsOwner(ui,host),null)
    }
    function test_primary_fallback() {
        registered=false;selectedView=secondView
        compare(services.diagnosticsOwner(ui,null).view,secondView)
        compare(services.diagnosticsOwner(otherUi,null),null)
    }
}

import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components"

TestCase {
    id: test
    name: "DevToolsWorkspace"
    when: windowShown
    property int destroyedInspectors: 0
    Component {
        id: hostFixture
        ApplicationWindow {
            id: host
            width: 800; height: 600; visible: true
            property bool runtimeReleased: false
            property bool runtimeDisposing: false
            property var currentView: page
            property alias pageItem: page
            property alias consoleItem: inspectorControl
            function activeWebView() { return currentView }
            Item {
                id: page
                property var viewHost: host
                property var viewUi: null
                property bool viewRetired: false
                property var profile: "owner-profile"
                visible: host.currentView === page
                function ownsTab() { return !viewRetired }
                anchors.fill: parent
            }
            Item { id: inspectorControl; anchors.fill: parent }
        }
    }
    Component {
        id: ownerFixture
        QtObject {
            property string core_window_id: "owner-one"
            property bool profile_bootstrap_pending: false
            property string status_text: ""
        }
    }
    Component {
        id: detachedFixture
        ApplicationWindow {
            required property var browserWindow
            required property var ownerWindow
            required property var inspectView
            required property var inspectionProfile
            property alias inspectorView: inspectorControl
            width: 500; height: 400
            Item { id: inspectorControl; anchors.fill: parent }
            Component.onDestruction: test.destroyedInspectors++
        }
    }
    Component { id: workspaceFixture; FerricDevToolsWorkspace {} }
    Component {
        id: peerFixture
        Item {
            property var viewHost
            property var viewUi
            property bool viewRetired: false
            function ownsTab() { return !viewRetired }
        }
    }
    function init() {
        failOnWarning(/TypeError|ReferenceError|Cannot assign|Unable to assign|Binding loop|invalid context/)
    }
    function setup() {
        var host=createTemporaryObject(hostFixture,test)
        var owner=createTemporaryObject(ownerFixture,test)
        verify(host);verify(owner)
        host.pageItem.viewUi=owner
        var workspace=createTemporaryObject(workspaceFixture,host.contentItem,{
            browserWindow:host,hostWindow:host,browserUi:owner,activeView:host.pageItem,
            windowFactory:detachedFixture,attachedInspector:host.consoleItem})
        verify(workspace)
        host.requestActivate();tryCompare(host,"active",true)
        host.pageItem.forceActiveFocus()
        return {host:host,owner:owner,workspace:workspace}
    }
    function test_attached_console_owns_only_its_input_and_page() {
        var s=setup()
        verify(s.workspace.toggle(false))
        tryCompare(s.host.consoleItem,"activeFocus",true)
        verify(s.workspace.inputActive)
        compare(s.workspace.inspectorFor(s.host.pageItem),s.host.consoleItem)
        var peer=createTemporaryObject(peerFixture,s.host.contentItem,{viewHost:s.host,viewUi:s.owner})
        compare(s.workspace.inspectorFor(peer),null)
        s.host.pageItem.forceActiveFocus()
        tryCompare(s.workspace,"inputActive",false)
        s.workspace.close()
        tryCompare(s.host.pageItem,"activeFocus",true)
        verify(!s.workspace.visible)
    }
    function test_detached_windows_keep_their_own_host_and_close_handle() {
        var a=setup(),b=setup()
        verify(a.workspace.toggle(true));verify(b.workspace.toggle(true))
        compare(a.workspace.detachedWindow.ownerWindow,a.host)
        compare(b.workspace.detachedWindow.ownerWindow,b.host)
        compare(a.workspace.detachedWindow.inspectView,a.host.pageItem)
        compare(a.workspace.detachedWindow.inspectionProfile,a.host.pageItem.profile)
        var previous=test.destroyedInspectors
        a.workspace.detachedWindow.close()
        tryCompare(a.workspace,"detachedWindow",null)
        verify(!a.workspace.visible)
        verify(b.workspace.detachedWindow.visible)
        tryCompare(test,"destroyedInspectors",previous+1)
        b.workspace.close()
    }
    function test_other_tabs_do_not_retarget_an_inspector() {
        var s=setup()
        verify(s.workspace.toggle(false))
        var peer=createTemporaryObject(peerFixture,s.host.contentItem,{viewHost:s.host,viewUi:s.owner})
        s.host.currentView=peer;s.workspace.activeView=peer
        verify(!s.workspace.attachedVisible)
        compare(s.workspace.inspectorFor(peer),null)
        compare(s.workspace.inspectorFor(s.host.pageItem),s.host.consoleItem)
        s.workspace.close()
        tryCompare(peer,"activeFocus",true)
    }
    function test_invalidated_owners_release_inspectors_data() {
        return [{tag:"profile",kind:"profile"},{tag:"bootstrap",kind:"bootstrap"},
                {tag:"tab-close",kind:"tab"},{tag:"window-close",kind:"window"},
                {tag:"transfer",kind:"transfer"}]
    }
    function test_invalidated_owners_release_inspectors(data) {
        var s=setup()
        verify(s.workspace.toggle(true))
        if(data.kind==="profile") s.owner.core_window_id="replacement"
        else if(data.kind==="bootstrap") s.owner.profile_bootstrap_pending=true
        else if(data.kind==="tab") s.host.pageItem.viewRetired=true
        else if(data.kind==="window") s.host.runtimeReleased=true
        else s.host.pageItem.viewHost=null
        compare(s.workspace.inspectorFor(s.host.pageItem),null)
        tryCompare(s.workspace,"visible",false)
        compare(s.workspace.detachedWindow,null)
    }
    function test_starting_window_reports_waiting_without_creating_an_inspector() {
        var s=setup()
        s.owner.profile_bootstrap_pending=true
        verify(!s.workspace.toggle(true))
        verify(!s.workspace.visible)
        compare(s.workspace.detachedWindow,null)
        verify(s.owner.status_text.indexOf("still starting")>=0)
    }
    function test_unavailable_target_does_not_create_an_inspector() {
        var s=setup()
        s.host.pageItem.viewRetired=true
        verify(!s.workspace.toggle(true))
        verify(!s.workspace.visible)
        compare(s.workspace.detachedWindow,null)
        verify(s.owner.status_text.indexOf("Select a tab")>=0)
    }
}

import QtQuick
import QtTest
import "../../qml/components"

TestCase {
    id: test
    name: "SwitcherOwners"
    when: windowShown
    Component { id: profileFactory; QtObject { property bool offTheRecord: false } }
    Component { id: uiFactory; QtObject { property string window_token: ""; property bool profile_bootstrap_pending: false } }
    Component { id: hostFactory; QtObject { property bool runtimeReleased: false; property bool runtimeDisposing: false } }
    Component {
        id: rootFactory
        QtObject {
            property var browserWindowRegistry: []
            property var popupWindowRegistry: []
            property var switcherUi: null
            property var switcherHost: null
            property var switcherProfile: null
            property string switcherOwnerToken: ""
            function liveWindowEntries() { return browserWindowRegistry.concat(popupWindowRegistry) }
        }
    }
    Component { id: controllerFactory; FerricWindowRegistryController {} }
    property var root
    property var controller
    property var primary
    property var regularProfile
    function item(factory, properties) { var value=createTemporaryObject(factory,test,properties || {});verify(value);return value }
    function entry(token, profile, isPrivate, isEphemeral, invocation) {
        return {ui:item(uiFactory,{window_token:token}),host:item(hostFactory),profile:profile,
            privateProfile:!!isPrivate,ephemeralProfile:!!isEphemeral,
            ephemeralInvocationToken:invocation || ""}
    }
    function invoke(owner) {
        root.switcherUi=owner.ui;root.switcherHost=owner.host;root.switcherProfile=owner.profile;
        root.switcherOwnerToken=owner.ui.window_token
    }
    function init() {
        failOnWarning(/TypeError|ReferenceError|Binding loop|Cannot assign|Unable to assign/)
        root=item(rootFactory)
        regularProfile=item(profileFactory)
        primary=entry("primary",regularProfile)
        root.browserWindowRegistry=[primary]
        invoke(primary)
        controller=item(controllerFactory,{browserWindow:root,browserUi:primary.ui})
    }
    function test_regular_query_excludes_actual_and_declared_transient_profiles() {
        var ordinary=entry("ordinary",item(profileFactory))
        var privateWindow=entry("private",item(profileFactory,{offTheRecord:true}),true)
        var temporaryWindow=entry("temporary",item(profileFactory,{offTheRecord:true}),false,true,"temporary-session")
        var incompleteMetadata=entry("off-record",item(profileFactory,{offTheRecord:true}))
        root.browserWindowRegistry=[primary,ordinary,privateWindow,temporaryWindow,incompleteMetadata]
        compare(controller.switcherEntries().map(function(row){return row.ui.window_token}),["primary","ordinary"])
        compare(controller.switcherOwner({owner_token:"private"}),null)
        compare(controller.switcherOwner({owner_token:"temporary"}),null)
    }
    function test_private_query_uses_shared_profile_identity() {
        var privateProfile=item(profileFactory,{offTheRecord:true})
        var owner=entry("private-owner",privateProfile,true)
        var shared=entry("shared-private",privateProfile,true)
        var independent=entry("other-private",item(profileFactory,{offTheRecord:true}),true)
        root.browserWindowRegistry=[primary,owner,shared,independent]
        invoke(owner)
        compare(controller.switcherEntries().map(function(row){return row.ui.window_token}),["private-owner","shared-private"])
        compare(controller.switcherOwner({owner_token:"primary"}),null)
        compare(controller.switcherOwner({owner_token:"other-private"}),null)
        compare(controller.switcherOwner({owner_token:"shared-private"}).ui,shared.ui)
    }
    function test_ephemeral_query_keeps_its_invocation_token() {
        var profile=item(profileFactory,{offTheRecord:true})
        var owner=entry("owner",profile,false,true,"invocation-a")
        var shared=entry("shared",profile,false,true,"invocation-a")
        var other=entry("other-invocation",profile,false,true,"invocation-b")
        var mismatched=entry("private-mode",profile,true,false)
        root.browserWindowRegistry=[primary,owner,shared,other,mismatched]
        invoke(owner)
        compare(controller.switcherEntries().map(function(row){return row.ui.window_token}),["owner","shared"])
    }
    function test_missing_or_retired_captured_owner_does_not_fall_back() {
        var child=entry("child",regularProfile)
        root.browserWindowRegistry=[primary,child]
        compare(controller.switcherOwner({owner_token:"child"}).ui,child.ui)
        child.host.runtimeReleased=true
        compare(controller.switcherOwner({owner_token:"child"}),null)
        compare(controller.switcherOwner({owner_token:"unknown"}),null)
        compare(controller.switcherOwner({owner_token:""}),null)
        compare(controller.switcherOwner({}),null)
        compare(controller.switcherOwner(null),null)
    }
    function test_invocation_is_invalid_after_profile_or_identity_change() {
        primary.ui.window_token="new-owner"
        compare(controller.switcherInvocation(),null)
        compare(controller.switcherEntries().length,0)
        primary.ui.window_token="primary"
        primary.profile=item(profileFactory)
        compare(controller.switcherInvocation(),null)
        compare(controller.switcherEntries().length,0)
    }
    function test_opening_or_disposing_windows_cannot_contribute() {
        var opening=entry("opening",regularProfile)
        opening.ui.profile_bootstrap_pending=true
        var disposing=entry("disposing",regularProfile)
        disposing.host.runtimeDisposing=true
        root.browserWindowRegistry=[primary,opening,disposing]
        compare(controller.switcherEntries().map(function(row){return row.ui.window_token}),["primary"])
        primary.host.runtimeDisposing=true
        compare(controller.switcherInvocation(),null)
    }
    function test_popup_owners_use_the_same_privacy_boundary() {
        var ordinaryPopup=entry("regular-popup",regularProfile)
        var privatePopup=entry("private-popup",item(profileFactory,{offTheRecord:true}),true)
        root.popupWindowRegistry=[ordinaryPopup,privatePopup]
        compare(controller.switcherEntries().map(function(row){return row.ui.window_token}),["primary","regular-popup"])
        compare(controller.switcherOwner({owner_token:"regular-popup"}).ui,ordinaryPopup.ui)
        compare(controller.switcherOwner({owner_token:"private-popup"}),null)
    }
}

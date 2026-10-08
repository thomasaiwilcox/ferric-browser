import QtQuick
import QtTest
import "../../qml/scripts/CleanBranchPopupOwnership.js" as CleanBranchPopupOwnership

TestCase {
    name: "CleanBranchPopupOwnership"
    when: windowShown

    function makeFixture() {
        var profile = {offTheRecord: true}
        var view = {profile: profile}
        var ui = {}
        var interceptor = {}
        var host = {
            windowBoundBranchToken: "branch:test",
            windowCleanBranchPublished: true,
            windowBranchUi: ui,
            windowBranchInitialView: view,
            windowRequestInterceptor: interceptor
        }
        var owner = {host: host, profile: profile, view: view}
        var root = {
            cleanBranchOwner: function(token) {
                return token === host.windowBoundBranchToken ? owner : null
            }
        }
        var popup = {
            popupBranchToken: host.windowBoundBranchToken,
            popupOpenerHost: host,
            popupProfile: profile,
            popupPermissionUi: ui,
            popupRequestInterceptor: interceptor,
            popupPrivateProfile: true,
            popupEphemeralProfile: true
        }
        return {profile: profile, view: view, ui: ui,
                interceptor: interceptor, host: host, owner: owner,
                root: root, popup: popup}
    }

    function test_exact_branch_popup_is_admitted() {
        var f = makeFixture()
        verify(CleanBranchPopupOwnership.sourceReady(
                   f.root, f.host, f.ui, f.profile, f.interceptor, f.view))
        verify(CleanBranchPopupOwnership.popupReady(f.root, f.popup, f.view))
        f.host.windowShutdownApproved = true
        verify(!CleanBranchPopupOwnership.sourceReady(
                    f.root, f.host, f.ui, f.profile, f.interceptor, f.view))
    }

    function test_parent_or_other_profile_cannot_receive_popup() {
        var f = makeFixture()
        verify(!CleanBranchPopupOwnership.sourceReady(
                    f.root, f.host, f.ui, {}, f.interceptor, f.view))
        f.popup.popupProfile = {}
        verify(!CleanBranchPopupOwnership.popupReady(f.root, f.popup, f.view))
        f.popup.popupProfile = f.profile
        f.popup.popupPermissionUi = {}
        verify(!CleanBranchPopupOwnership.popupReady(f.root, f.popup, f.view))
    }

    function test_registered_group_view_keeps_exact_branch_owner() {
        var f = makeFixture()
        var second = {
            profile: f.profile,
            viewHost: f.host,
            viewUi: f.ui,
            viewInterceptor: f.interceptor
        }
        f.host.windowBranchGroupViews = [second]
        verify(CleanBranchPopupOwnership.sourceReady(
                   f.root, f.host, f.ui, f.profile, f.interceptor, second))
        second.viewInterceptor = {}
        verify(!CleanBranchPopupOwnership.sourceReady(
                    f.root, f.host, f.ui, f.profile, f.interceptor, second))
        second.viewInterceptor = f.interceptor
        second.profile = {}
        verify(!CleanBranchPopupOwnership.sourceReady(
                    f.root, f.host, f.ui, f.profile, f.interceptor, second))
    }

    function test_tab_capable_group_view_uses_exact_native_creation_host() {
        var f = makeFixture()
        var nativeHost = {}
        f.host.versioningViewHostObject = nativeHost
        var second = {
            profile: f.profile,
            versioningOwnerHost: nativeHost,
            viewUi: f.ui,
            viewInterceptor: f.interceptor
        }
        f.host.windowBranchGroupViews = [second]
        verify(CleanBranchPopupOwnership.sourceReady(
                   f.root, f.host, f.ui, f.profile, f.interceptor, second))
        second.versioningOwnerHost = {}
        verify(!CleanBranchPopupOwnership.sourceReady(
                    f.root, f.host, f.ui, f.profile, f.interceptor, second))
    }

    function test_unregistered_or_stale_group_view_is_rejected() {
        var f = makeFixture()
        var second = {
            profile: f.profile,
            viewHost: f.host,
            viewUi: f.ui,
            viewInterceptor: f.interceptor
        }
        verify(!CleanBranchPopupOwnership.sourceReady(
                    f.root, f.host, f.ui, f.profile, f.interceptor, second))
        f.host.windowBranchGroupViews = [second]
        f.owner.view = {profile: f.profile}
        verify(!CleanBranchPopupOwnership.sourceReady(
                    f.root, f.host, f.ui, f.profile, f.interceptor, second))
        f.owner.view = f.view
        f.host.windowCleanBranchPublished = false
        verify(!CleanBranchPopupOwnership.sourceReady(
                    f.root, f.host, f.ui, f.profile, f.interceptor, second))
    }

    function test_closed_or_rebound_owner_fails_before_navigation() {
        var f = makeFixture()
        f.host.windowCleanBranchPublished = false
        verify(!CleanBranchPopupOwnership.sourceReady(
                    f.root, f.host, f.ui, f.profile, f.interceptor, f.view))
        verify(!CleanBranchPopupOwnership.popupReady(f.root, f.popup, f.view))
        f.host.windowCleanBranchPublished = true
        f.owner.view = {profile: f.profile}
        verify(!CleanBranchPopupOwnership.sourceReady(
                    f.root, f.host, f.ui, f.profile, f.interceptor, f.view))
        verify(!CleanBranchPopupOwnership.popupReady(f.root, f.popup, f.view))
    }

    function test_transient_permission_and_interceptor_required() {
        var f = makeFixture()
        f.popup.popupPrivateProfile = false
        verify(!CleanBranchPopupOwnership.popupReady(f.root, f.popup, f.view))
        f.popup.popupPrivateProfile = true
        f.popup.popupEphemeralProfile = false
        verify(!CleanBranchPopupOwnership.popupReady(f.root, f.popup, f.view))
        f.popup.popupEphemeralProfile = true
        f.popup.popupRequestInterceptor = {}
        verify(!CleanBranchPopupOwnership.popupReady(f.root, f.popup, f.view))
        f.popup.popupRequestInterceptor = f.interceptor
        f.profile.offTheRecord = false
        verify(!CleanBranchPopupOwnership.sourceReady(
                    f.root, f.host, f.ui, f.profile, f.interceptor, f.view))
        verify(!CleanBranchPopupOwnership.popupReady(f.root, f.popup, f.view))
    }
}

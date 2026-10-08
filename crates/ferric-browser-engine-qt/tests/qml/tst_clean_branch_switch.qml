import QtQuick
import QtTest
import "../../qml/scripts/CleanBranchSwitch.js" as CleanBranchSwitch

TestCase {
    name: "CleanBranchSwitch"
    when: windowShown

    readonly property string branchId: "branch:12345678-1234-4234-8234-123456789abc"

    function makeFixture() {
        var parentUi = {}
        var parentProfile = {}
        var parentHost = {}
        var parentEntry = {
            ui: parentUi,
            coreWindowId: "windowid-6",
            host: parentHost,
            profile: parentProfile,
            view: {profile: parentProfile}
        }
        var childProfile = {}
        var childView = {profile: childProfile}
        var childUi = {}
        var childHost = {
            windowBranchToken: branchId,
            windowCleanBranchPublished: true,
            windowCleanBranchLineagePublished: true,
            windowBranchProfile: childProfile,
            windowBranchInitialView: childView,
            windowBranchUi: childUi
        }
        return {
            parentUi: parentUi,
            parentHost: parentHost,
            parentEntry: parentEntry,
            childHost: childHost,
            childUi: childUi,
            owner: {host: childHost, profile: childProfile, view: childView},
            activated: null,
            validated: 0,
            allowed: true
        }
    }

    function activate(fixture, target, entries, owner) {
        return CleanBranchSwitch.activate(
                    fixture.parentUi, target, entries,
                    function(token) { return owner },
                    function(ui, id) {
                        fixture.validated += 1
                        return fixture.allowed && ui === fixture.childUi
                                && id === branchId
                    },
                    function(host, ui) {
                        fixture.activated = {host: host, ui: ui}
                        return true
                    })
    }

    function test_parent_focus_uses_exact_registered_owner() {
        var f = makeFixture()
        verify(activate(f, "parent\twindowid-6", [f.parentEntry], null))
        compare(f.activated.host, f.parentHost)
        compare(f.activated.ui, f.parentUi)
        compare(f.validated, 0)
    }

    function test_parent_missing_or_ambiguous_host_never_falls_back() {
        var f = makeFixture()
        verify(!activate(f, "parent\twindowid-6", [], null))
        verify(!activate(f, "parent\twindowid-6",
                         [f.parentEntry, f.parentEntry], null))
        compare(f.activated, null)
    }

    function test_child_focus_rechecks_native_and_qml_owner() {
        var f = makeFixture()
        verify(activate(f, "child\t" + branchId, [f.parentEntry], f.owner))
        compare(f.activated.host, f.childHost)
        compare(f.activated.ui, f.childUi)
        compare(f.validated, 1)
    }

    function test_stale_or_rebound_child_never_falls_back() {
        var f = makeFixture()
        verify(!activate(f, "child\t" + branchId, [f.parentEntry], null))
        f.owner.view = {profile: f.owner.profile}
        verify(!activate(f, "child\t" + branchId, [f.parentEntry], f.owner))
        f.owner.view = f.childHost.windowBranchInitialView
        f.allowed = false
        verify(!activate(f, "child\t" + branchId, [f.parentEntry], f.owner))
        compare(f.activated, null)
    }

    function test_malformed_target_is_never_activated() {
        var f = makeFixture()
        verify(!activate(f, "child\tbranch:not-an-id", [f.parentEntry], f.owner))
        verify(!activate(f, "child\t" + branchId + "\textra", [f.parentEntry], f.owner))
        verify(!activate(f, "parent\t", [f.parentEntry], f.owner))
        compare(f.activated, null)
        compare(f.validated, 0)
    }
}

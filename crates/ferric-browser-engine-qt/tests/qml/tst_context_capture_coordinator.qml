import QtQuick
import QtTest
import "../../qml/scripts/ContextCaptureCoordinator.js" as Coordinator

TestCase {
    name: "ContextCaptureCoordinator"
    when: windowShown

    function fixture() {
        var calls = []
        var source = {
            context_name: "research", core_window_id: "windowid-1",
            window_token: "source-token", status_text: "",
            active_context_capture_ref: function(name) { return "context-uuid" },
            active_context_capture_members: function(name) {
                return ["windowid-1", "windowid-2"]
            },
            stage_context_window_membership: function(name) {
                calls.push("member-source")
                return "member-source-handle"
            },
            prepare_staged_context_save: function(name, handles) {
                calls.push("prepare-save:" + handles.join(","))
                return "save-plan"
            },
            confirm_staged_context_window_membership: function(handle) {
                calls.push("confirm-source:" + handle)
                return true
            },
            finish_staged_context_save: function(plan) {
                calls.push("finish-save:" + plan)
                return true
            },
            cancel_staged_context_save: function(plan) {
                calls.push("cancel-save:" + plan)
                return true
            },
            take_context_commit_message: function() { return "Checkpoint" },
            stage_context_window_capture: function(name, ref, ordinal) {
                calls.push("stage-source:" + ordinal)
                return "handle-source"
            },
            prepare_staged_context_commit: function(handles, message) {
                calls.push("prepare:" + handles.join(",") + ":" + message)
                return "plan"
            },
            bind_staged_context_window_capture: function(handle) {
                calls.push("bind-source:" + handle)
                return true
            },
            finish_staged_context_commit: function(plan) {
                calls.push("finish:" + plan)
                return ["operation", "commit"]
            },
            cancel_staged_context_commit: function(plan) {
                calls.push("cancel:" + plan)
                return true
            }
        }
        var peer = {
            core_window_id: "windowid-2", window_token: "peer-token",
            context_name: "research",
            stage_context_window_membership: function(name) {
                calls.push("member-peer")
                return "member-peer-handle"
            },
            confirm_staged_context_window_membership: function(handle) {
                calls.push("confirm-peer:" + handle)
                return true
            },
            stage_context_window_capture: function(name, ref, ordinal) {
                calls.push("stage-peer:" + ordinal)
                return "handle-peer"
            },
            bind_staged_context_window_capture: function(handle) {
                calls.push("bind-peer:" + handle)
                return true
            }
        }
        var sourceHost = { scheduleRuntimeWork: function(delay) {
            calls.push("schedule-source")
        } }
        var peerHost = { scheduleRuntimeWork: function(delay) {
            calls.push("schedule-peer")
        } }
        var sourceEntry = {
            ui: source, host: sourceHost, coreWindowId: "windowid-1",
            windowToken: "source-token", profileName: "default",
            privateProfile: false, ephemeralProfile: false
        }
        var peerEntry = {
            ui: peer, host: peerHost, coreWindowId: "windowid-2",
            windowToken: "peer-token", profileName: "default",
            privateProfile: false, ephemeralProfile: false
        }
        return { calls: calls, source: source, peer: peer,
                 sourceEntry: sourceEntry, peerEntry: peerEntry }
    }

    function test_exact_two_owner_handoff_finishes_after_both_bindings() {
        var f = fixture()
        verify(Coordinator.commitSavedContext(f.source, f.sourceEntry,
                                              [f.sourceEntry, f.peerEntry]))
        compare(f.calls.join("|"),
                "stage-source:0|stage-peer:1|prepare:handle-source,handle-peer:Checkpoint"
                + "|bind-source:handle-source|schedule-source"
                + "|bind-peer:handle-peer|schedule-peer|finish:plan|schedule-source")
    }

    function test_save_membership_uses_both_exact_live_owners() {
        var f = fixture()
        verify(Coordinator.saveContextMembership(f.source, f.sourceEntry,
                                                 [f.sourceEntry, f.peerEntry]))
        compare(f.calls.join("|"),
                "member-source|member-peer"
                + "|prepare-save:member-source-handle,member-peer-handle"
                + "|confirm-source:member-source-handle"
                + "|confirm-peer:member-peer-handle"
                + "|finish-save:save-plan|schedule-source")
    }

    function test_save_refuses_duplicate_owner_or_failed_peer_confirmation() {
        var f = fixture()
        verify(!Coordinator.saveContextMembership(
                   f.source, f.sourceEntry,
                   [f.sourceEntry, f.peerEntry, f.peerEntry]))
        compare(f.source.status_text, "Context member owner is ambiguous or stale")
        compare(f.calls.length, 0)

        f = fixture()
        f.peer.confirm_staged_context_window_membership = function(handle) {
            f.calls.push("confirm-peer-refused")
            return false
        }
        verify(!Coordinator.saveContextMembership(
                   f.source, f.sourceEntry, [f.sourceEntry, f.peerEntry]))
        verify(f.calls.indexOf("cancel-save:save-plan") >= 0)
        verify(f.calls.indexOf("finish-save:save-plan") < 0)
    }

    function test_ambiguous_or_stale_owner_cannot_stage_or_publish() {
        var f = fixture()
        verify(!Coordinator.commitSavedContext(
                   f.source, f.sourceEntry,
                   [f.sourceEntry, f.peerEntry, f.peerEntry]))
        compare(f.source.status_text, "Context member owner is ambiguous")
        verify(f.calls.indexOf("finish:plan") < 0)

        f = fixture()
        f.peerEntry.windowToken = "stale-token"
        verify(!Coordinator.commitSavedContext(
                   f.source, f.sourceEntry, [f.sourceEntry, f.peerEntry]))
        compare(f.source.status_text, "Context member window is unavailable")
        verify(f.calls.indexOf("finish:plan") < 0)
    }

    function test_peer_bind_refusal_cancels_plan_before_publication() {
        var f = fixture()
        f.peer.bind_staged_context_window_capture = function(handle) {
            f.calls.push("bind-peer-refused")
            return false
        }
        verify(!Coordinator.commitSavedContext(
                   f.source, f.sourceEntry, [f.sourceEntry, f.peerEntry]))
        compare(f.source.status_text, "Context member changed during capture")
        verify(f.calls.indexOf("cancel:plan") >= 0)
        verify(f.calls.indexOf("finish:plan") < 0)
    }
}

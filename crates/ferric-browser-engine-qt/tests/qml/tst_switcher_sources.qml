import QtQuick
import QtTest
import "../../qml/scripts/SwitcherSources.js" as Sources

TestCase {
    name: "SwitcherSources"
    function test_another_window_is_not_a_completed_empty_search() {
        var state = Sources.aggregate([], true)
        verify(state.pending)
        verify(!state.failed)
        compare(state.notice, "Searching other browser windows…")
    }
    function test_read_failures_and_other_pending_sources_remain_distinct() {
        var state = Sources.aggregate([
            {pending: false, failed: true, retryable: true, notice: "failed",
                detail: "Literal <b>storage detail</b>"},
            {pending: true, failed: false, retryable: false, notice: "Loading history…", detail: ""}
        ], false)
        verify(state.pending && state.failed && state.retryable)
        compare(state.notice, "Some items couldn't be loaded. Other items are still loading.")
        compare(state.detail, "Literal <b>storage detail</b>")
        verify(state.notice.indexOf("<b>") < 0)
    }
    function test_optional_private_sources_do_not_become_io_failures() {
        var notice = "Saved sessions aren't available in this private session."
        var state = Sources.aggregate([{pending: false, failed: false, retryable: false,
            notice: notice, detail: ""}], false)
        verify(!state.pending && !state.failed && !state.retryable)
        compare(state.notice, notice)
        compare(state.detail, "")
    }
    function test_same_profile_repeats_do_not_repeat_status_copy() {
        var source = {pending: true, failed: false, retryable: false,
            notice: "Loading history…", detail: ""}
        var state = Sources.aggregate([source, source, source], false)
        compare(state.notice, "Loading history…")
    }
    function test_query_failure_keeps_primary_copy_readable_and_details_available() {
        var state = Sources.fromUi({status_text: "Internal query detail <b>literal</b>"}, false)
        verify(state.failed && state.retryable && !state.pending)
        compare(state.notice, "Some items couldn't be searched. Refresh to try again.")
        compare(state.detail, "Internal query detail <b>literal</b>")
    }
    function test_unavailable_source_keeps_its_actual_next_action() {
        var notice = "Saved items aren't available in this window. Reopen the profile to try again."
        var state = Sources.aggregate([{pending:false,failed:true,retryable:false,
            notice:notice,detail:"owned diagnostic"}],false)
        verify(state.failed && !state.retryable && !state.pending)
        compare(state.notice,notice)
        verify(state.notice.indexOf("Refresh") < 0)
    }
    function test_native_query_failure_preserves_safe_message_and_separate_detail() {
        var message = "This private session changed. Open a new search and try again."
        var state = Sources.fromUi({switcher_sources_notice:message,
            switcher_sources_detail:"Exact private scope diagnostic <b>literal</b>",
            switcher_sources_retryable:false,status_text:message},false)
        verify(state.failed && !state.retryable && !state.pending)
        compare(state.notice,message)
        compare(state.detail,"Exact private scope diagnostic <b>literal</b>")
    }

}

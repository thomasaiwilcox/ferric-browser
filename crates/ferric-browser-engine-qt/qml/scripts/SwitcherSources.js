.pragma library

function fromUi(ui, accepted) {
    if (!accepted) return {
        pending: false, failed: true,
        retryable: ui.switcher_sources_retryable === undefined ? true : ui.switcher_sources_retryable === true,
        notice: String(ui.switcher_sources_notice || "Some items couldn't be searched. Refresh to try again."),
        detail: String(ui.switcher_sources_detail || ui.status_text || "The search did not complete.")
    }
    return {
        pending: ui.switcher_sources_pending === true,
        failed: ui.switcher_sources_failed === true,
        retryable: ui.switcher_sources_retryable === true,
        notice: String(ui.switcher_sources_notice || ""),
        detail: String(ui.switcher_sources_detail || "")
    }
}

function aggregate(sources, queryingMore) {
    var result = {pending: queryingMore, failed: false, retryable: false, notice: "", detail: ""}
    var notices = []
    var details = []
    for (var i = 0; i < sources.length; ++i) {
        var source = sources[i]
        result.pending = result.pending || source.pending
        result.failed = result.failed || source.failed
        result.retryable = result.retryable || source.retryable
        if (source.notice && notices.indexOf(source.notice) < 0) notices.push(source.notice)
        if (source.detail && details.indexOf(source.detail) < 0) details.push(source.detail)
    }
    if (result.failed && result.pending)
        result.notice = "Some items couldn't be loaded. Other items are still loading."
    else if (result.failed && !result.retryable)
        result.notice = notices.length > 0 ? notices.join("\n")
            : "The search couldn't be completed. Close it and try again."
    else if (result.failed)
        result.notice = "Some items couldn't be loaded. Refresh to try again."
    else if (notices.length > 0)
        result.notice = notices.join("\n")
    else if (queryingMore)
        result.notice = "Searching other browser windows…"
    result.detail = details.join("\n\n")
    return result
}

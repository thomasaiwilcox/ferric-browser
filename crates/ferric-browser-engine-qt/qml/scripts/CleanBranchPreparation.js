// The hidden-child handoff is kept small and side-effect explicit so QML
// fixtures can exercise allocation and navigation failures with the same
// decisions as the production window. Native owner checks remain in Rust.

function createHiddenChild(factory, properties, token, parentUi) {
    if (!factory || typeof factory.createObject !== "function")
        return null
    var child = null
    try {
        child = factory.createObject(null, properties)
    } catch (error) {
        return null
    }
    if (!child || !child.windowCleanBranch || child.visible
            || child.windowBoundBranchToken !== token
            || child.windowBranchParentUi !== parentUi) {
        if (child)
            child.destroy()
        return null
    }
    return child
}

function publishPreparedChild(entry, token, owner) {
    var child = entry.child
    var ui = entry.parentUi
    if (!owner || owner.host !== child
            || owner.profile !== child.windowBranchProfile
            || owner.view !== child.windowBranchInitialView
            || !ui.publish_versioned_clean_branch(child.windowBranchUi, token)) {
        child.destroy()
        ui.abort_versioned_clean_branch(token)
        return "owner-failed"
    }
    child.windowCleanBranchLineagePublished = true
    child.windowCleanBranchNavigationAdmitted = true
    if (entry.navigation === "safe-get"
            && !child.windowBranchUi.navigate_clean_branch_safe_get(entry.url)) {
        child.destroy()
        return "navigation-failed"
    }
    if (!child.publishCleanBranchWindow()) {
        child.destroy()
        return "publication-failed"
    }
    return "opened"
}

// Navigation policy is decided after the complete group lineage publishes.
// Only the selected proved GET starts immediately; background proved GETs are
// one-use and wait for their tab to become active. Placeholders stay blank.
function windowNavigationSchedule(pages, selectedIndex) {
    if (!Array.isArray(pages) || pages.length === 0
            || !Number.isInteger(selectedIndex)
            || selectedIndex < 0 || selectedIndex >= pages.length)
        return null
    var deferred = []
    var selectedUrl = ""
    for (var index = 0; index < pages.length; ++index) {
        var page = pages[index]
        if (!page || (page.navigation !== "safe-get"
                      && page.navigation !== "placeholder"))
            return null
        if (page.navigation !== "safe-get")
            continue
        if (index === selectedIndex)
            selectedUrl = page.url
        else
            deferred[index] = page.url
    }
    return { selectedUrl: selectedUrl, deferred: deferred }
}

function takeDeferredWindowUrl(deferred, activeIndex) {
    if (!Array.isArray(deferred) || !Number.isInteger(activeIndex)
            || activeIndex < 0 || activeIndex >= deferred.length)
        return null
    var next = deferred.slice()
    var url = String(next[activeIndex] || "")
    if (url.length === 0)
        return null
    next[activeIndex] = ""
    return { url: url, deferred: next }
}

// Only the planned safe GET may start an original page outcome. The bootstrap
// about:blank load, unrelated later navigation and error-document loads are
// not evidence that the planned replay succeeded.
function startWindowPageNavigation(pageIds, expectedUrls, started, outcomes, tabId, url) {
    if (!Array.isArray(pageIds) || pageIds.length === 0
            || pageIds.length > 128 || !Array.isArray(expectedUrls)
            || !Array.isArray(started) || !Array.isArray(outcomes)
            || expectedUrls.length !== pageIds.length
            || started.length !== pageIds.length
            || outcomes.length !== pageIds.length)
        return null
    var index = pageIds.indexOf(String(tabId || ""))
    if (index < 0 || outcomes[index] !== "pending"
            || expectedUrls[index] !== String(url || "")
            || !/^https?:\/\//.test(expectedUrls[index]))
        return null
    var next = started.slice()
    next[index] = true
    return next
}

function classifyWindowTerminalOutcome(outcome, url) {
    if (outcome === "failed")
        return "failed"
    if (outcome !== "loaded")
        return null
    // Qt can finish an internal blank/error document after a failed replay.
    // Only an HTTP(S) terminal document can count as a loaded child page.
    return /^https?:\/\//.test(String(url || "")) ? "loaded" : "failed"
}

// Outcomes belong to original stable IDs, not current tab indices, and the
// first terminal result wins. A subsequent error-page success cannot erase a
// failed native replay.
function recordWindowPageOutcome(pageIds, started, outcomes, tabId, outcome) {
    if (!Array.isArray(pageIds) || pageIds.length === 0
            || pageIds.length > 128 || !Array.isArray(started)
            || !Array.isArray(outcomes)
            || started.length !== pageIds.length
            || outcomes.length !== pageIds.length
            || (outcome !== "loaded" && outcome !== "failed"))
        return null
    var index = pageIds.indexOf(String(tabId || ""))
    if (index < 0 || started[index] !== true
            || outcomes[index] !== "pending")
        return null
    var next = outcomes.slice()
    next[index] = outcome
    return next
}

// A started replay that never receives a terminal Qt callback must not stay
// pending forever. The caller still rechecks the exact live view before it
// records a timeout as failed.
function expiredWindowPageIndexes(pageIds, started, outcomes, startedAt,
                                  now, deadlineMs) {
    if (!Array.isArray(pageIds) || pageIds.length === 0
            || pageIds.length > 128 || !Array.isArray(started)
            || !Array.isArray(outcomes) || !Array.isArray(startedAt)
            || started.length !== pageIds.length
            || outcomes.length !== pageIds.length
            || startedAt.length !== pageIds.length
            || !Number.isFinite(now) || !Number.isFinite(deadlineMs)
            || deadlineMs <= 0)
        return []
    var expired = []
    for (var index = 0; index < pageIds.length; ++index) {
        var began = Number(startedAt[index])
        if (started[index] === true && outcomes[index] === "pending"
                && Number.isFinite(began) && began > 0
                && now >= began && now - began >= deadlineMs)
            expired.push(index)
    }
    return expired
}

.pragma library

// Pure chrome-presentation helpers. This resource deliberately has no bridge,
// storage, or page access; it only projects already-validated chrome facts.
var VERSION = "3"

function byteSize(bytes) {
    var value = Number(bytes)
    if (!isFinite(value) || value < 0) return "Size unknown"
    var units = ["B", "KiB", "MiB", "GiB", "TiB"]
    var unit = 0
    while (value >= 1024 && unit < units.length - 1) {
        value /= 1024
        unit++
    }
    return value.toFixed(unit === 0 || value >= 100 ? 0 : 1) + " " + units[unit]
}

function fileName(path) {
    var parts = String(path || "").split(/[\\/]/)
    return parts[parts.length - 1] || "Download"
}

function downloadStateLabel(state) {
    var labels = { "offered": "Waiting to start", "in-progress": "Downloading",
        "paused": "Paused", "completed": "Complete", "cancelled": "Cancelled",
        "interrupted": "Interrupted" }
    return typeof labels[state] === "string" ? labels[state] : "Status unavailable"
}

function downloadProgress(bytes, total, speed, state) {
    var text = byteSize(bytes)
    if (isFinite(Number(total)) && Number(total) >= 0)
        text += " of " + byteSize(total)
    if (state === "in-progress" && isFinite(Number(speed)) && Number(speed) > 0)
        text += " · " + byteSize(speed) + "/s"
    return text
}

function switcherKindLabel(kind) {
    var labels = { "tab": "Tab", "window": "Window", "context": "Context",
        "command": "Command", "history": "History", "bookmark": "Bookmark",
        "quickmark": "Quickmark", "session": "Session", "download": "Download",
        "closed": "Recently closed", "action": "Action" }
    return typeof labels[kind] === "string" ? labels[kind] : "Browser item"
}

function switcherActionLabel(action) {
    var labels = { "focus": "Switch to", "open": "Open", "enter": "Enter",
        "load-preview": "Preview session", "load": "Restore session",
        "show": "Show in folder", "help": "View help", "execute": "Run",
        "reopen": "Reopen", "delete": "Delete", "close": "Close" }
    return typeof labels[action] === "string" ? labels[action] : "Action"
}

function settingValueLabel(key, value) {
    var choices = {
        "ui.statusbar": { "in-mode": "Outside Normal mode", "always": "Always", "never": "Never" },
        "ui.reduced_motion": { "system": "Follow system", "on": "Reduce motion", "off": "Allow motion" },
        "input.entry_mode": { "normal": "Normal", "insert": "Insert", "passthrough": "Pass-through" },
        "hints.proxy_targets": { "off": "Off", "validated": "Custom editors and labels" },
        "content.autoplay": { "engine-default": "Browser default", "require-gesture": "After interaction" },
        "downloads.collision": { "ask": "Ask what to do", "rename": "Keep both files" },
        "desktop.portals": { "auto": "Automatic", "required": "Required" },
        "logging.level": { "error": "Errors", "warn": "Warnings", "info": "Information", "debug": "Debug" }
    }
    var labels = choices[key]
    return labels && typeof labels[value] === "string" ? labels[value] : String(value)
}

function settingApplyLabel(value) {
    var labels = { "live": "Applies immediately", "navigation": "Applies on the next navigation",
        "next-navigation": "Applies on the next navigation", "restart": "Requires a restart",
        "profile-restart": "Requires a profile restart", "reload": "Applies after reloading configuration",
        "startup": "Applies when Ferric starts" }
    return typeof labels[value] === "string" ? labels[value] : String(value || "")
}

function navigationFailureMessage(kind) {
    var messages = {
        "dns": "The site's address couldn't be found. Check the address and your internet connection.",
        "network": "The connection couldn't be completed. Check your connection, then open the page again.",
        "tls": "A secure connection couldn't be established. Check the site's address and your device's date and time.",
        "http": "The server couldn't provide this page. Check the address or try again later.",
        "download": "This address was handled as a download. Check Downloads for its status."
    }
    return typeof messages[kind] === "string" ? messages[kind]
            : "The page couldn't be opened. Check the address or try again later."
}

function siteScopeKey(data) {
    data = data || {}
    var capture = data.capture || {}
    return JSON.stringify([data.private === true, data.origin || "", capture.tab_id || "",
        capture.document_id || "", capture.profile_id || "", data.context || ""])
}

function siteFactTitle(id) {
    var titles = {"network.blocking": "Content blocking", "permissions": "Site permissions",
        "engine.webengine": "Web engine", "capture": "Screen sharing",
        "page.userscripts": "Page scripts", "compatibility.workarounds": "Compatibility adjustments"}
    return typeof titles[id] === "string" ? titles[id] : "Site information"
}

function listCount(value) {
    // Nested QML model roles can expose QVariantList wrappers rather than a
    // JavaScript Array. Both provide a finite, non-negative length.
    return value && typeof value === "object" && typeof value.length === "number"
        && isFinite(value.length) && value.length >= 0 ? value.length : -1
}

function siteFactSummary(fact, privateSession, hasOrigin) {
    var value = fact.value || {}
    if (fact.id === "engine.webengine")
        return value.status === "available" ? "Web engine available" : "Web engine status unavailable"
    if (privateSession && fact.id !== "network.blocking")
        return "Site details are withheld in private browsing"
    if (fact.id === "network.blocking") {
        var state = value.enabled === true ? "Content blocking is on" : value.enabled === false
                ? "Content blocking is off" : "Blocking status unavailable"
        if (privateSession) return state + " · site activity is withheld"
        if (!hasOrigin) return state + " · no web site selected"
        var count = value.active_site_blocked_requests
        return state + (typeof count === "number" && isFinite(count) && count >= 0
                ? " · " + count + (count === 1 ? " request blocked for this site" : " requests blocked for this site")
                : " · site activity unavailable")
    }
    if (!hasOrigin && fact.id !== "capture") return "Available for HTTP and HTTPS pages"
    if (fact.id === "permissions") {
        if (value.ready !== true) return "Saved permission rules are loading"
        var rules = listCount(value.durable_rules)
        if (rules < 0) return "Saved permission rules unavailable"
        return rules === 0 ? "No saved rules for this site · requests are handled when they occur"
                : rules + (rules === 1 ? " saved rule for this site" : " saved rules for this site")
    }
    if (fact.id === "capture") {
        if (value.status === "inactive") return "No sharing is recorded for this tab"
        if (value.status === "active") return "Sharing is recorded for this tab"
        return "Sharing status unavailable"
    }
    if (fact.id === "page.userscripts") {
        if (value.status === "not-configured") return "No page scripts configured"
        if (value.status !== "available") return "Page script information unavailable"
        return value.matching_active_site === true
                ? "Page scripts match this site · this does not confirm they ran"
                : "No enabled page scripts match this site"
    }
    if (fact.id === "compatibility.workarounds") {
        if (value.status !== "available" || listCount(value.active_ids) < 0)
            return "Compatibility information unavailable"
        return value.active_ids.length === 0 ? "No compatibility adjustments apply"
                : value.active_ids.length + (value.active_ids.length === 1 ? " adjustment applies" : " adjustments apply")
    }
    return "Details available for inspection"
}

function siteExperimentLabel(kind) {
    var labels = {"blocking-bypass": "Temporarily bypass content blocking",
        "compiled-defaults": "Try default site settings", "userscripts-off": "Try without page scripts",
        "fresh-view": "Open a fresh tab in this profile"}
    return typeof labels[kind] === "string" ? labels[kind] : "Temporary site test"
}

function interactionDetails(status) {
    status = status || {}
    var focus = status.focus || {}, policy = status.shortcut_policy || {}
    var prompt = status.native_prompt || {}, errors = status.recent_action_errors || {}
    var categories = errors.categories || []
    return "Interaction status · " + (status.mode || "mode unknown")
        + " · focus " + (focus.category || "unknown")
        + " · shortcut policy " + (policy.value || "unknown") + " (" + (policy.source || "source unknown") + ")"
        + (policy.policy_site_rule ? " · policy rule " + policy.policy_site_rule : "")
        + (policy.page_keys_site_rule ? " · page-key rule " + policy.page_keys_site_rule : "")
        + " · page keys " + (policy.page_key_count || 0)
        + " · native prompt " + (prompt.active ? prompt.category : "inactive")
        + " · prompt outcome " + (prompt.last_outcome || "unknown")
        + " · scroll policy " + ((status.scroll_policy && status.scroll_policy.status) || "unknown")
        + " · recent action errors " + (categories.length === 0 ? "no recent action errors"
            : categories.map(function(entry) { return entry.category + " ×" + entry.count }).join(", "))
}

function colorChannels(value) {
    if (typeof value === "string"
            && /^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(value)) {
        return {
            r: parseInt(value.slice(1, 3), 16) / 255,
            g: parseInt(value.slice(3, 5), 16) / 255,
            b: parseInt(value.slice(5, 7), 16) / 255,
            a: value.length === 9 ? parseInt(value.slice(7, 9), 16) / 255 : 1.0
        }
    }
    return value
}

function luminance(color) {
    function linear(channel) {
        return channel <= 0.03928 ? channel / 12.92
                                   : Math.pow((channel + 0.055) / 1.055, 2.4)
    }
    return 0.2126 * linear(color.r) + 0.7152 * linear(color.g)
            + 0.0722 * linear(color.b)
}

function contrastRatio(foreground, background) {
    foreground = colorChannels(foreground)
    background = colorChannels(background)
    var alpha = typeof foreground.a === "number" ? foreground.a : 1.0
    var compositedForeground = {
        r: foreground.r * alpha + background.r * (1.0 - alpha),
        g: foreground.g * alpha + background.g * (1.0 - alpha),
        b: foreground.b * alpha + background.b * (1.0 - alpha)
    }
    var foregroundLuminance = luminance(compositedForeground)
    var backgroundLuminance = luminance(background)
    var lighter = Math.max(foregroundLuminance, backgroundLuminance)
    var darker = Math.min(foregroundLuminance, backgroundLuminance)
    return (lighter + 0.05) / (darker + 0.05)
}

function contrastText(background) {
    return contrastRatio("#000000", background) >= contrastRatio("#ffffff", background)
            ? "#000000" : "#ffffff"
}

function readableTextColor(candidate, background) {
    return contrastRatio(candidate, background) >= 4.5
            ? candidate : contrastText(background)
}

function statusBarVisible(policy, mode) {
    if (mode === "command" || mode === "search") {
        return false
    }
    if (policy === "always") {
        return true
    }
    // "command" was Ferric's original name for this policy. Accept it at
    // the presentation boundary as well as normalizing it in Rust so an old
    // live configuration cannot produce a surprising hidden bar.
    return (policy === "in-mode" || policy === "command") && mode !== "normal"
}

function passThroughMode(mode) {
    return mode === "pass-through" || mode === "pass-through-once"
}

function inputModeLabel(mode) {
    if (mode === "pass-through-once") return "PASS NEXT KEY"
    return mode.replace(/-/g, " ").toUpperCase()
}

function inputModeHint(mode) {
    return passThroughMode(mode) ? "Ctrl+Esc: Normal" : ""
}

function inputModeDescription(mode) {
    if (mode === "pass-through-once")
        return "Next key goes to the webpage, then returns to Normal. Ctrl+Escape cancels."
    if (mode === "pass-through")
        return "Keys, including Escape, go to the webpage. Ctrl+Escape returns to Normal."
    return ""
}

function contrastReport(colors) {
    var checks = [
        { name: "primary-on-background", ratio: contrastRatio(
            colors.primaryText, colors.background) },
        { name: "secondary-on-surface", ratio: contrastRatio(
            colors.secondaryText, colors.surface) },
        { name: "muted-on-panel", ratio: contrastRatio(
            colors.mutedText, colors.panel) },
        { name: "accent-on-background", ratio: contrastRatio(
            colors.accent, colors.background) }
    ]
    var failing = []
    for (var index = 0; index < checks.length; index++) {
        if (checks[index].ratio < 4.5) {
            failing.push(checks[index].name)
        }
    }
    return { status: failing.length === 0 ? "pass" : "warning", failing: failing }
}

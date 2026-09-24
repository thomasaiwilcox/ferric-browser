.pragma library

// Typed spellcheck feature values are projected here for chrome presentation.
// This helper has no authority to install dictionaries or change configuration.
var VERSION = "1"

function languageKey(language) {
    return String(language || "").trim().replace(/-/g, "_").toLowerCase()
}

function languageIsValid(language) {
    var grandfathered = [
        "art-lojban", "cel-gaulish", "en-gb-oed", "i-ami", "i-bnn",
        "i-default", "i-enochian", "i-hak", "i-klingon", "i-lux",
        "i-mingo", "i-navajo", "i-pwn", "i-tao", "i-tay", "i-tsu",
        "no-bok", "no-nyn", "sgn-be-fr", "sgn-be-nl", "sgn-ch-de",
        "zh-guoyu", "zh-hakka", "zh-min", "zh-min-nan", "zh-xiang"
    ]
    var normalized = String(language || "").toLowerCase()
    return grandfathered.indexOf(normalized) >= 0
            || /^(?:[A-Za-z]{2,8}(?:-[A-Za-z0-9]{1,8})*|x(?:-[A-Za-z0-9]{1,8})+)$/.test(language)
}

function dictionaryStatus(configured, installed, systemLocale) {
    configured = configured && configured.length ? configured : ["system"]
    installed = installed || []
    var requested = []
    for (var index = 0; index < configured.length && index < 16; ++index) {
        var language = String(configured[index] || "").trim()
        if (language === "system") language = systemLocale
        if (languageIsValid(language) && requested.indexOf(language) < 0) requested.push(language)
    }
    if (requested.length === 0) requested = [systemLocale]
    var installedKeys = []
    for (var installedIndex = 0; installedIndex < installed.length; ++installedIndex) {
        var installedKey = languageKey(installed[installedIndex])
        if (installedKey.length > 0 && installedKeys.indexOf(installedKey) < 0) installedKeys.push(installedKey)
    }
    var active = [], missing = []
    for (var requestedIndex = 0; requestedIndex < requested.length; ++requestedIndex) {
        var requestedLanguage = requested[requestedIndex]
        var requestedKey = languageKey(requestedLanguage)
        var found = installedKeys.indexOf(requestedKey) >= 0
                || installedKeys.indexOf(requestedKey.split("_")[0]) >= 0
        ;(found ? active : missing).push(requestedLanguage)
    }
    return { requested: requested, active: active, missing: missing, installed: installed }
}

function enabled(featureEnabled, status) {
    return featureEnabled !== false && status.active.length > 0
}

function statusText(status) {
    if (status.missing.length === 0) {
        return status.installed.length > 0 ? "Spellcheck dictionaries: " + status.active.join(", ")
                                           : "Spellcheck dictionary inventory unavailable; no download was attempted"
    }
    return "Missing spellcheck dictionaries: " + status.missing.join(", ")
            + " (no download was attempted)"
}

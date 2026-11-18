.pragma library

// Normal-mode printable-key normalization. Qt's logical key identifies the
// keyboard-layout result, while some WebEngine/Wayland paths provide empty or
// unexpectedly lower-case text for shifted ASCII letters.
function printableKey(text, key, shifted, firstLetterKey, lastLetterKey) {
    if (shifted && key >= firstLetterKey && key <= lastLetterKey) {
        return String.fromCharCode(key)
    }
    text = text || ""
    if (text.length === 0 || text.length > 4) {
        return ""
    }
    for (var index = 0; index < text.length; ++index) {
        if (text.charCodeAt(index) < 0x20 || text.charCodeAt(index) === 0x7f) {
            return ""
        }
    }
    return text
}

// Qt event adapter for Core's LogicalKeyToken vocabulary. Authored syntax and
// alias/conflict validation stay in Rust; this boundary describes one event.
function logicalNormalKeyText(event) {
    if (!event || (event.modifiers & Qt.GroupSwitchModifier)) return ""
    var control = !!(event.modifiers & Qt.ControlModifier)
    var shift = !!(event.modifiers & Qt.ShiftModifier)
    var alt = !!(event.modifiers & Qt.AltModifier)
    var meta = !!(event.modifiers & Qt.MetaModifier)
    var command = control || alt || meta
    var key = event.key
    var token = ""
    if (key === Qt.Key_Return || key === Qt.Key_Enter) token = "Enter"
    else if (key === Qt.Key_Space) token = "Space"
    else if (key === Qt.Key_Tab || key === Qt.Key_Backtab) {
        token = shift || key === Qt.Key_Backtab ? "Backtab" : "Tab"
        shift = false
    } else if (key === Qt.Key_Backspace) token = "Backspace"
    else if (key === Qt.Key_Delete) token = "Delete"
    else if (key === Qt.Key_Insert) token = "Insert"
    else if (key === Qt.Key_Escape) token = "Escape"
    else if (key === Qt.Key_Left) token = "Left"
    else if (key === Qt.Key_Right) token = "Right"
    else if (key === Qt.Key_Up) token = "Up"
    else if (key === Qt.Key_Down) token = "Down"
    else if (key === Qt.Key_Home) token = "Home"
    else if (key === Qt.Key_End) token = "End"
    else if (key === Qt.Key_PageUp) token = "PgUp"
    else if (key === Qt.Key_PageDown) token = "PgDown"
    else if (key === Qt.Key_Back) token = "Back"
    else if (key === Qt.Key_Forward) token = "Forward"
    else if (key >= Qt.Key_F1 && key <= Qt.Key_F12) token = "F" + (key - Qt.Key_F1 + 1)
    else if (control && !alt && !meta && (key === Qt.Key_6 || key === Qt.Key_AsciiCircum)) {
        // Existing documented Ctrl+^ chord, including its physical Shift form.
        token = "^"
        shift = false
    } else if (command && key >= Qt.Key_A && key <= Qt.Key_Z) {
        // CapsLock changes text, not the explicit Shift bit on a command chord.
        token = String.fromCharCode(key).toLowerCase()
    } else {
        token = printableKey(event.text, key, false, Qt.Key_A, Qt.Key_Z)
        if (!token && !command && key >= Qt.Key_A && key <= Qt.Key_Z)
            token = String.fromCharCode(key + (shift ? 0 : 32))
        if (!token && key >= Qt.Key_0 && key <= Qt.Key_9 && !shift)
            token = String.fromCharCode(key)
        if (!token && key === Qt.Key_Plus) token = "+"
        if (!token) return ""
        // Literal case/punctuation is the layout-produced character. It does
        // not carry a second physical Shift modifier in the logical grammar.
        shift = false
        if (token === "+") token = "Plus"
    }
    return (control ? "Ctrl+" : "") + (alt ? "Alt+" : "")
            + (meta ? "Meta+" : "") + (shift ? "Shift+" : "") + token
}

// Canonical vocabulary does not itself grant browser ownership. Keep the
// existing Normal fallback set; newly supported chords/named keys require an
// explicit binding or Mixed admission before their owner can change.
function normalBrowserFallback(token) {
    if (!token) return false
    if (token.indexOf("+") < 0 && (token.length === 1 || /[^\x20-\x7e]/.test(token))) return true
    return ["Plus", "Space", "Shift+Space", "Enter", "Shift+Enter", "F5", "F11", "Back", "Forward",
        "Alt+1", "Alt+2", "Alt+3", "Alt+4", "Alt+5", "Alt+6", "Alt+7", "Alt+8", "Alt+9", "Alt+m",
        "Ctrl+Alt+p", "Ctrl+Space", "Ctrl+Shift+Space", "Ctrl+^",
        "Ctrl+b", "Ctrl+c", "Ctrl+d", "Ctrl+f", "Ctrl+n", "Ctrl+p", "Ctrl+q", "Ctrl+s",
        "Ctrl+t", "Ctrl+u", "Ctrl+v", "Ctrl+w", "Ctrl+x", "Ctrl+F5", "Ctrl+Tab",
        "Ctrl+PgUp", "Ctrl+PgDown", "Ctrl+Shift+n", "Ctrl+Shift+t", "Ctrl+Shift+v", "Ctrl+Shift+w"
    ].indexOf(token) >= 0
}

// Grid owns its chord set. Alt+Enter is an explicit, qualification-gated
// double-click request; other Alt/Meta combinations remain unavailable.
function logicalGridKeyText(event) {
    if (!event) {
        return ""
    }
    var control = !!(event.modifiers & Qt.ControlModifier)
    var shift = !!(event.modifiers & Qt.ShiftModifier)
    var alt = !!(event.modifiers & Qt.AltModifier)
    var meta = !!(event.modifiers & Qt.MetaModifier)
    if (meta) {
        return ""
    }
    if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
        if (alt && !control && !shift) {
            return "Alt+Enter"
        }
        if (alt) {
            return ""
        }
        if (control && !shift) {
            return "Ctrl+Enter"
        }
        if (shift && !control) {
            return "Shift+Enter"
        }
        return !control && !shift ? "Enter" : ""
    }
    if (alt || control || shift) {
        if (!alt && !control && shift
                && event.key === Qt.Key_Slash && event.text === "?") {
            return "?"
        }
        return ""
    }
    if (event.key >= Qt.Key_0 && event.key <= Qt.Key_9) {
        return String.fromCharCode(event.key)
    }
    if (event.key === Qt.Key_Space) {
        return "Space"
    }
    if (event.key === Qt.Key_Backspace) {
        return "Backspace"
    }
    if (event.key === Qt.Key_Escape) {
        return "Escape"
    }
    return event.text === "?" ? "?" : ""
}

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

// Convert physical Qt keys and modifiers into the binding resolver's key names.
function logicalNormalKeyText(event) {
    if (!event) {
        return ""
    }
    var control = !!(event.modifiers & Qt.ControlModifier)
    var shift = !!(event.modifiers & Qt.ShiftModifier)
    var alt = !!(event.modifiers & Qt.AltModifier)
    var meta = !!(event.modifiers & Qt.MetaModifier)
    if (alt && !control && !shift && !meta) {
        if (event.key >= Qt.Key_1 && event.key <= Qt.Key_9) {
            return "Alt+" + String.fromCharCode(event.key)
        }
        if (event.key === Qt.Key_M) {
            return "Alt+m"
        }
        return ""
    }
    if (control && alt && !shift && !meta && event.key === Qt.Key_P) {
        return "Ctrl+Alt+p"
    }
    if (control && !alt && !meta) {
        if (event.key === Qt.Key_Space) {
            return shift ? "Ctrl+Shift+Space" : "Ctrl+Space"
        }
        if (event.key === Qt.Key_6 || event.key === Qt.Key_AsciiCircum) {
            return "Ctrl+^"
        }
        if (shift) {
            if (event.key === Qt.Key_N) {
                return "Ctrl+Shift+n"
            }
            if (event.key === Qt.Key_T) {
                return "Ctrl+Shift+t"
            }
            if (event.key === Qt.Key_V) {
                return "Ctrl+Shift+v"
            }
            if (event.key === Qt.Key_W) {
                return "Ctrl+Shift+w"
            }
            return ""
        }
        if (event.key === Qt.Key_B) {
            return "Ctrl+b"
        }
        if (event.key === Qt.Key_D) {
            return "Ctrl+d"
        }
        if (event.key === Qt.Key_F) {
            return "Ctrl+f"
        }
        if (event.key === Qt.Key_F5) {
            return "Ctrl+F5"
        }
        if (event.key === Qt.Key_N) {
            return "Ctrl+n"
        }
        if (event.key === Qt.Key_U) {
            return "Ctrl+u"
        }
        if (event.key === Qt.Key_P) {
            return "Ctrl+p"
        }
        if (event.key === Qt.Key_Q) {
            return "Ctrl+q"
        }
        if (event.key === Qt.Key_S) {
            return "Ctrl+s"
        }
        if (event.key === Qt.Key_Tab) {
            return "Ctrl+Tab"
        }
        if (event.key === Qt.Key_T) {
            return "Ctrl+t"
        }
        if (event.key === Qt.Key_V) {
            return "Ctrl+v"
        }
        if (event.key === Qt.Key_W) {
            return "Ctrl+w"
        }
        if (event.key === Qt.Key_PageDown) {
            return "Ctrl+PgDown"
        }
        if (event.key === Qt.Key_PageUp) {
            return "Ctrl+PgUp"
        }
        return ""
    }
    if (!control && !alt && !meta) {
        if (event.key === Qt.Key_F5) {
            return "F5"
        }
        if (event.key === Qt.Key_F11) {
            return "F11"
        }
        if (event.key === Qt.Key_Back) {
            return "Back"
        }
        if (event.key === Qt.Key_Forward) {
            return "Forward"
        }
    }
    if (alt || meta) {
        return ""
    }
    return printableKey(
                event.text, event.key, shift, Qt.Key_A, Qt.Key_Z)
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

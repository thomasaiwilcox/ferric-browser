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

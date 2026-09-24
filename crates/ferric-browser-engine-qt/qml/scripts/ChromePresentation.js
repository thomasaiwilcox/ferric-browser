.pragma library

// Pure chrome-presentation helpers. This resource deliberately has no bridge,
// storage, or page access; it only projects already-validated theme colors.
var VERSION = "1"

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

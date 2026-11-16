.pragma library

// Explicit, memory-only samples omit node handles, page text and field values.
function sample(facts) {
    if (!facts || facts.version !== 1) throw new Error("Capture a fresh target first.")
    return {location:facts.location || "", tag:facts.tag, root:JSON.stringify(facts.root),
        root_complete:facts.root_complete === true,
        selectors:(facts.selectors || []).slice(0,6).map(function(value) {
            return {selector:value.selector, count:value.count, complete:value.complete === true}
        })}
}

function compareSamples(kept, facts) {
    if (!kept.length) return "No samples kept. Keep only samples you want to compare; they remain in this builder session."
    var current = facts ? sample(facts) : null
    return kept.map(function(previous,index) {
        var lines = ["Sample " + (index+1) + ": " + previous.location + " · " + previous.tag]
        if (!current) return lines.join("\n")
        var sameRoot = previous.root === current.root && previous.root_complete && current.root_complete
        lines.push(sameRoot ? "Root descriptor agrees; this does not prove stability." : "Root changed or is incomplete; compare paths and preview explicitly.")
        previous.selectors.forEach(function(value) {
            var next = current.selectors.find(function(other) { return other.selector === value.selector })
            var before = value.complete ? value.count : "incomplete"
            var after = sameRoot && next && next.complete ? next.count : "not evaluated in the same complete root"
            lines.push(value.selector + ": kept " + before + "; current " + after)
        })
        return lines.join("\n")
    }).join("\n\n")
}

// A finite, explicit array for the ordinary collection editor. Prefix-free
// fixed-width codes avoid the 1/10 collision of unpadded numeric grids.
function codes(kind, start, rows, columns, alphabet) {
    if (!Number.isInteger(start) || start < 1 || !Number.isInteger(rows) || !Number.isInteger(columns)
            || rows < 1 || columns < 1 || rows * columns > 64)
        throw new Error("Choose a positive start and a grid containing 1–64 cells.")
    var count = rows * columns, last = start + count - 1
    if (!Number.isSafeInteger(last) || last > 99999999) throw new Error("The last rank must fit in eight digits.")
    var symbols = kind === "numeric" ? "0123456789" : alphabet
    if (typeof symbols !== "string" || symbols.length < 2 || symbols.length > 32)
        throw new Error("Choose 2–32 different printable ASCII letters or symbols.")
    var seen = ({})
    for (var i = 0; i < symbols.length; ++i) {
        var code = symbols.charCodeAt(i)
        if (code < 0x21 || code > 0x7e || seen[symbols[i]]) throw new Error("Alphabet characters must be unique printable ASCII.")
        seen[symbols[i]] = true
    }
    var maximum = kind === "numeric" ? last : last - 1, width = 1
    while (maximum >= Math.pow(symbols.length, width)) ++width
    if (width > 8) throw new Error("The generated codes would exceed eight characters.")
    var result = []
    for (var rank = start; rank <= last; ++rank) {
        var value = kind === "numeric" ? rank : rank - 1, text = ""
        for (var digit = 0; digit < width; ++digit) {
            text = symbols[value % symbols.length] + text
            value = Math.floor(value / symbols.length)
        }
        result.push(text)
    }
    return result
}

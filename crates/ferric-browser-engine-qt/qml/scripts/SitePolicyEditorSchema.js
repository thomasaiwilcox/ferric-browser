.pragma library

// Closed form vocabulary for Interaction. Values are transported as typed
// draft proposals; these descriptors never execute a command or page script.
var families = ["all", "actions", "focus", "links", "inputs", "buttons", "images", "media", "scrollables", "regions", "surfaces"]
var contexts = ["passthrough-once", "passthrough", "normal-page-key", "hint-page-key"]
var enums = {
    family: families, context: contexts,
    autoFollow: ["always", "unique-match", "full-match", "never"],
    order: ["dom", "top-to-bottom", "row-major-ltr", "row-major-rtl"],
    textCase: ["sensitive", "ascii-insensitive"],
    rootStart: ["top-document", "focused-frame"],
    stepKind: ["open-shadow", "same-origin-frame", "assigned-slot"],
    receiverScope: ["top-document", "focused-frame", "top-or-focused-frame"]
}
function field(key, label, type, help, required, item, maximum, when) {
    return { key: key, label: label, type: type, help: help || "", required: required === true,
        item: item || "text", maximum: maximum || 64, when: when || "" }
}
var types = {
    proposal: [
        field("pattern", "HTTP(S) pattern", "text", "A whole-rule change also retargets its preserved ordinary settings.", true),
        field("priority", "Priority", "integer", "Higher priority is applied later. Equal priorities retain source order.", true),
        field("interaction", "Interaction section", "interaction", "Clear this section to remove only Interaction. Ordinary settings are preserved.")
    ],
    interaction: [
        field("enabled", "Enabled", "bool", "An explicit false disables the composed section. An absent field leaves lower values available."),
        field("labels", "Labels", "labels", "Enhanced label settings; ordinary settings retain their separate source boundaries."),
        field("anchors", "Semantic anchors", "list", "A present list replaces the lower list. An empty list explicitly clears it.", false, "anchor", 64),
        field("collections", "Collections", "list", "Collection order reserves stable ranks, including unavailable holes.", false, "collection", 16),
        field("hint_page_keys", "Selected Hint page keys", "list", "Canonical source tokens admitted in Hint mode. Remap destinations are not exclusions.", false, "text", 32),
        field("page_receiver", "Page receiver", "receiver", "Focused frames require exact permitted HTTP(S) origins and current noneditable focus."),
        field("remaps", "Page-key mappings", "list", "Mappings select one terminal destination; they never recurse or execute while editing.", false, "remap", 32)
    ],
    labels: [
        field("alphabet", "Alphabet", "text", "Unique printable ASCII characters; live capacity is computed by the shared codebook."),
        field("min_chars", "Minimum label length", "integer", "Total label length, 1–8 characters."),
        field("fallback_prefix", "Fallback prefix", "text", "Ordinary labels use this prefix; special codes retain their exact spelling."),
        field("excluded_chars", "Excluded characters", "list", "Each entry is one character. A present empty list clears configured exclusions.", false, "text", 32),
        field("auto_follow", "Auto-follow", "autoFollow", "Enhanced hints require full-match or never; validation reports incompatible inherited values.")
    ],
    anchor: [
        field("id", "Anchor ID", "text", "Stable lowercase identity within this section.", true),
        field("code", "Reserved code", "text", "1–8 printable ASCII characters. Missing targets retain their reservation.", true),
        field("families", "Invocation families", "list", "Choose literal invocation families; all does not replace the other families.", true, "family", 11),
        field("root", "Explicit root", "root", "Start in the top document or an authorized focused frame, then traverse explicit boundaries."),
        field("scope_selector", "Scope selector", "text", "Constrained CSS selector identifying one scope; absence uses the selected root."),
        field("target_selector", "Target selector", "text", "Constrained CSS selector within the declared scope."),
        field("predicates", "Target predicates", "predicates", "Every supplied predicate must match complete evidence.")
    ],
    collection: [
        field("id", "Collection ID", "text", "Stable lowercase identity within this section.", true),
        field("families", "Invocation families", "list", "The literal invocations which declare these slots.", true, "family", 11),
        field("codes", "Reserved rank codes", "list", "One code per rank, in order. Missing ranks remain holes.", true, "text", 64),
        field("item_selector", "Item selector", "text", "Constrained CSS selecting items inside the container.", true),
        field("order", "Item order", "order", "DOM order or explicit geometric row/rank ordering.", true),
        field("root", "Explicit root", "root", "Root used to locate the collection container."),
        field("container_selector", "Container selector", "text", "Use exactly one container selector or container self."),
        field("container_self", "Container is root", "bool", "Explicit self selection instead of a container selector."),
        field("primary_root_steps", "Per-item root path", "list", "Boundary traversal relative to each matched item.", false, "step", 8),
        field("primary_selector", "Primary target selector", "text", "Select one primary target per item, or use primary self."),
        field("primary_self", "Primary is item", "bool", "Explicit self target instead of a primary selector."),
        field("item_attributes", "Item attribute tests", "list", "All supplied tests apply to each collection item.", false, "attribute", 4),
        field("primary_predicates", "Primary target predicates", "predicates", "Applied to each primary target after root traversal."),
        field("identity_attributes", "Stable identity attributes", "list", "Up to four bounded attributes used to retain original item identity.", false, "text", 4),
        field("row_tolerance_px", "Row tolerance (px)", "number", "Tolerance for explicit geometric row ordering.")
    ],
    root: [
        field("start", "Root start", "rootStart", "Focused-frame start requires an exact receiver origin.", true),
        field("receiver_origin", "Focused receiver origin", "text", "Exact canonical HTTP(S) origin; omit for top-document start."),
        field("steps", "Root boundary path", "list", "At most eight steps across the complete path.", true, "step", 8)
    ],
    step: [
        field("kind", "Boundary kind", "stepKind", "Only open shadow roots, same-origin frames and assigned slots are supported.", true),
        field("host_selector", "Host selector", "text", "Select one host, or explicitly use self."),
        field("self", "Host is current node", "bool", "Exactly one host selector or true self is required."),
        field("assigned_selector", "Assigned-node selector", "text", "Assigned-slot steps select one flattened assigned element.")
    ],
    receiver: [
        field("scope", "Receiver scope", "receiverScope", "Changing to top-document removes the frame-origin list.", true),
        field("origins", "Permitted frame origins", "list", "1–8 exact canonical HTTP(S) origins. No wildcard or opaque-origin authority.", true, "text", 8, "frame-receiver")
    ],
    remap: [
        field("id", "Mapping ID", "text", "Stable lowercase mapping identity.", true),
        field("from", "Source key", "text", "One canonical logical token, for example p or Ctrl+Left.", true),
        field("to", "Terminal destination", "text", "One qualified native destination; editing never sends it.", true),
        field("contexts", "Mapping contexts", "list", "Explicit contexts only. Normal mode still requires Mixed page-key admission.", true, "context", 4)
    ],
    predicates: [
        field("hint_name", "Hint name", "name", "NFC and Unicode whitespace normalization; select equals or contains."),
        field("href", "Link destination", "href", "Exact origin and/or existing absolute path-glob grammar."),
        field("role", "Role", "text", "One bounded lowercase role-attribute token."),
        field("attributes", "Attribute tests", "list", "Each test selects exactly one of equals, prefix or present.", false, "attribute", 4),
        field("width_px", "Width (px)", "range", "Inclusive width range."),
        field("height_px", "Height (px)", "range", "Inclusive height range."),
        field("viewport_x", "Viewport X", "range", "Normalized viewport fraction, 0–1."),
        field("viewport_y", "Viewport Y", "range", "Normalized viewport fraction, 0–1.")
    ],
    name: [
        field("equals", "Equals", "text", "Choose either equals or contains."),
        field("contains", "Contains", "text", "Choose either contains or equals."),
        field("case", "Case matching", "textCase", "Sensitive by default; optional ASCII-only case folding.")
    ],
    href: [field("origin", "Exact origin", "text", "Canonical HTTP(S) origin."), field("path", "Path glob", "text", "Absolute path glob without query or fragment.")],
    attribute: [
        field("name", "Attribute name", "text", "A permitted bounded attribute name.", true),
        field("equals", "Equals", "text", "Exact bounded value; choose one matcher."),
        field("prefix", "Prefix", "text", "Bounded value prefix; choose one matcher."),
        field("present", "Presence", "bool", "True requires presence; false requires absence. Choose one matcher.")
    ],
    range: [field("min", "Minimum", "number", "Finite inclusive lower bound.", true), field("max", "Maximum", "number", "Finite inclusive upper bound, at least the minimum.", true)]
}
var defaults = {
    proposal: {pattern:"https://example.test/*",priority:0,interaction:{}}, interaction:{}, labels:{},
    anchor:{id:"",code:"",families:["links"],target_selector:"a"},
    collection:{id:"",families:["links"],codes:["aa","ab"],item_selector:"a",order:"dom"},
    root:{start:"top-document",steps:[]}, step:{kind:"open-shadow",host_selector:"x-host"},
    receiver:{scope:"top-document"}, remap:{id:"",from:"p",to:"k",contexts:["hint-page-key"]},
    predicates:{}, name:{equals:""}, href:{path:"/*"}, attribute:{name:"aria-label",equals:""}, range:{min:0,max:1}
}
function clone(value) { return JSON.parse(JSON.stringify(value)) }
function fields(kind) { return types[kind] || [] }
function choices(kind) { return enums[kind] || [] }
function defaultValue(kind) {
    if (kind === "list") return []
    if (kind === "bool") return false
    if (kind === "number" || kind === "integer") return 0
    if (enums[kind]) return enums[kind][0]
    if (defaults[kind]) return clone(defaults[kind])
    return ""
}
function present(object, key) { return object && object[key] !== undefined && object[key] !== null }
function applicable(spec, object) { return spec.when !== "frame-receiver" || object.scope !== "top-document" }
function choose(object, kind, key, value) {
    object[key] = value
    if (kind === "receiver" && key === "scope") {
        if (value === "top-document") delete object.origins
        else if (!Array.isArray(object.origins)) object.origins = []
    }
}

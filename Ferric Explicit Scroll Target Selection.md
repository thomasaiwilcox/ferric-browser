# Ferric Explicit Scroll Target Selection

**Status:** Final implementation specification

**Baseline:** `main` at `14409d659a6d` (PR #4 merged, 2026-09-26)

**Target release:** First post-keyboard/spatial-navigation increment

**Primary owner:** `ferric-browser-engine-qt`, with command and IPC metadata in
`ferric-browser-core` and `ferric-browser-ipc`

## 1. Decision

Ferric will let the user explicitly choose which visible scrollable DOM region
receives browser scrolling commands.

The feature will:

- reuse the existing `scrollables` Hint family;
- add one first-class command, `scroll-target`, with four actions;
- keep target state inside the active document's isolated application world;
- preserve the existing focus-aware automatic policy when no explicit policy is
  active;
- distinguish automatic targeting from explicitly targeting the document;
- validate an element target immediately before every scroll operation;
- never move DOM focus, persist a selector, or expose an element handle through
  Rust or IPC.

The public command surface is final:

```text
scroll-target select
scroll-target auto
scroll-target document
scroll-target status
```

The default Normal-mode bindings are final:

```text
;s    scroll-target select
;S    scroll-target auto
```

There is no default binding for `scroll-target document` in this increment.

## 2. User-visible outcome

Given a page with a document, a sidebar, and a message timeline:

1. The user presses `;s`.
2. Ferric enters the existing Hint mode with only live `scrollables`
   candidates.
3. The user selects the timeline's label.
4. Ferric returns to Normal mode and reports:

   ```text
   Scroll target: selected region
   ```

5. `h`, `j`, `k`, `l`, `Ctrl+d`, `Ctrl+u`, `Ctrl+f`, `Ctrl+b`, `gg`, and `G`
   now operate on that exact timeline element.
6. `;S` clears the selection and reports:

   ```text
   Scroll target: automatic
   ```

The feature changes no scrolling behavior until the user selects `document` or
an element.

## 3. Why this feature exists

Modern pages often contain several independently scrollable surfaces: sidebars,
message histories, code panes, terminals, tables, feeds, modal bodies, and the
document itself. Focus-based routing is useful but can be ambiguous in a
keyboard-oriented browser.

The required user invariant is:

> When an explicit target is active, every Ferric scrolling command owns that
> same target until the target is cleared or becomes invalid.

Focus and scroll ownership are deliberately independent. For example, a text
editor can retain DOM focus while `j` and `k` scroll its surrounding message
timeline.

## 4. Existing baseline

The baseline already provides most of the mechanism:

- `crates/ferric-browser-core/src/command.rs` registers `scroll`,
  `scroll-page`, and `scroll-to`.
- `crates/ferric-browser-core/src/input.rs` binds the standard scrolling keys.
- `crates/ferric-browser-engine-qt/qml/scripts/BrowserScripts.js::scroll()`
  first uses `window.__ferric_browserScrollTarget` when connected, otherwise it
  searches the focused element's ancestors and falls back to the document.
- the Hint parser, collector, completion code, and IPC schema accept the
  `scrollables` family;
- selecting a `HintKind::Scrollable` returns a `scroll-target` presentation
  action;
- `BrowserScripts.js::hintSetScrollTarget()` stores the selected element;
- the primary QML presentation already invokes that setter after fresh Hint
  validation.

This specification turns that partial behavior into a complete, typed, and
testable contract. It does not introduce a new keyboard mode.

## 5. Scope

### 5.1 Included

- explicit selection of a visible scrollable element;
- explicit pinning to the top-level document scroller;
- clearing back to focus-aware automatic targeting;
- status reporting;
- all existing incremental, page, half-page, and edge scrolling commands;
- vertical-only, horizontal-only, and two-axis regions;
- same-origin frame and open-shadow-root elements already discoverable by the
  Hint collector;
- stale, hidden, disconnected, adopted, or no-longer-scrollable target
  invalidation;
- command parsing, completion, generated command/binding documentation, CLI
  encoding, and JSON-RPC compatibility command encoding;
- primary and secondary-view dispatch for non-Hint actions, following the
  repository's existing reduced-chrome policy for Hint availability.

### 5.2 Excluded

- a `Mode::Scroll` keyboard mode;
- Grid-based scroll-container selection;
- cross-origin-frame DOM traversal;
- closed shadow-root discovery;
- selectors, XPath, coordinates, or element IDs supplied by the user;
- restoration after reload, navigation, browser restart, or session restore;
- a permanent chrome indicator;
- target names derived from arbitrary page text;
- typed `ActionRegistry` entries specifically for changing the scroll-target
  policy;
- changes to native pointer delivery;
- fixing unrelated limitations of Hint presentation in reduced-chrome
  secondary windows.

The existing typed actions for `scroll`, `scroll-page`, and `scroll-to` continue
to work and automatically observe the selected page-side policy. A separate
typed action for `scroll-target` can be proposed later if a concrete action
consumer needs one.

## 6. Semantic model

Each live document has exactly one of three policies.

### 6.1 `auto`

This is the default and the representation used when no state object exists.

For each operation Ferric uses the current behavior:

1. walk from `document.activeElement` toward the document;
2. choose the nearest ancestor scrollable in the requested axis;
3. otherwise use `document.scrollingElement` (falling back to
   `document.documentElement`).

Automatic resolution remains per operation. Changing focus can therefore
change the automatic target.

### 6.2 `document`

Ferric always uses the current top-level `document.scrollingElement`, regardless
of focus inside a nested scroller.

The document element itself must not be stored as the durable identity for this
policy. Resolve the root on every operation so the policy remains correct if
the document changes which element is its scrolling element.

### 6.3 `element`

Ferric stores weak references to the exact DOM element selected through Hint
mode and its exact `ownerDocument` object. The connected DOM/frame tree remains
the strong owner of a valid target; the selection must not prolong the lifetime
of a detached element or obsolete child document. Ferric never stores or later
resolves a selector, coordinates, a tag/role description, or the Hint numeric
ID as target identity.

The element may move or resize. It remains selected only while the same object
is connected, belongs to its captured document, remains in the current
same-origin frame tree, remains meaningfully visible, and remains scrollable on
at least one axis.

### 6.4 Policy and requested-axis validity are different

An element target is valid if it remains scrollable on at least one axis. A
particular command is supported only if the target is scrollable in that
command's axis.

Therefore:

- selecting a vertical-only pane and pressing `h` is a no-op;
- the pane remains selected;
- Ferric does not scroll the document horizontally;
- reaching the pane's top or bottom also does not clear it;
- if the pane loses vertical scrolling but still has horizontal scrolling,
  vertical commands are no-ops and the target remains selected;
- if it loses scrolling on every axis, it is invalidated.

`scroll-to top` and `scroll-to bottom` are vertical operations.

### 6.5 Invalid element behavior

If an element policy is invalid immediately before an operation:

1. atomically replace the policy with `auto`;
2. resolve the automatic target for the same operation;
3. execute that operation once against the automatic target, if one exists.

Do not wait for the next key press, and do not find a replacement by old
coordinates, Hint ID, DOM position, tag, role, or selector.

Invalidation during `scroll-target status` also changes the policy to `auto`
before reporting it.

## 7. Exact command contract

### 7.1 Registry metadata

Add one `CommandDefinition` to `CommandRegistry::default_v1()`:

| Field | Value |
| --- | --- |
| name | `scroll-target` |
| aliases | none |
| modes | `Normal`, `Command` |
| count | `NotSupported` |
| sensitive | `false` |
| scope | `Tab` |
| effect | `Mutating` |
| completion | `None` plus the explicit enum completion listed below |
| required argument | `action: Enum` |
| examples | all four public forms |

Use this description:

```text
Select, clear, inspect, or pin the active page's keyboard scroll target.
```

`Mutating` is intentionally conservative for the combined command because
three actions can change ephemeral page state and `status` may invalidate stale
state. Do not split the public surface merely to give `status` a different
metadata effect.

Update the registry-size assertion in `command.rs`; do not weaken or remove the
assertion.

### 7.2 Parser

Add a Qt-independent parser in
`crates/ferric-browser-engine-qt/src/command_options.rs`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ScrollTargetAction {
    Select,
    Auto,
    Document,
    Status,
}

pub(super) fn scroll_target(arguments: &[String])
    -> Result<ScrollTargetAction, String>;
```

It must accept exactly one argument and return this error for every missing,
extra, or unknown form:

```text
scroll-target requires exactly one of select, auto, document, or status
```

Also add `is_scroll_target(name: &str) -> bool` beside the existing
`is_scroll()` helper and re-export both helpers through `lib.rs` using the
existing naming convention.

### 7.3 Actions

#### `scroll-target select`

- Validate the command through the shared registry.
- Invoke `set_hint_options("scrollables", false, "current", None, false, 1)`.
- Do not enqueue a page-script engine action at this point.
- Enter the existing Hint mode and return immediately.
- No count is accepted or applied.
- If the current surface does not support Hint chrome under existing policy,
  reject it with that surface's existing full-chrome/Hints-unavailable error;
  do not leave its `BrowserUi` stranded in Hint mode.

Make that capability check explicit rather than inferring it from a window
name. Add a non-durable `hint_chrome_available: bool` presentation-capability
property to `BrowserUi`, default it to `true`, and set it to `false` on the
current reduced-chrome `secondaryUi`. Check it before `set_hint_options()`.
This covers both command-line and binding execution, including `;s`; the
existing `secondary_command_supported()` guard alone is insufficient because
Normal-mode bindings do not enter through that interactive-command guard.

The existing `hint scrollables` command remains valid and produces the same
element-selection result. `scroll-target select` is the discoverable,
purpose-specific entry point.

#### `scroll-target auto`

- Enqueue `scroll-target\tauto` for the owning active WebEngine view.
- The page script replaces any state with the canonical automatic state.
- On callback, report `Scroll target: automatic`.
- Repeating it is idempotent.

#### `scroll-target document`

- Enqueue `scroll-target\tdocument`.
- The page script records the `document` policy without changing DOM focus.
- On callback, report `Scroll target: document`.
- Repeating it is idempotent.

#### `scroll-target status`

- Enqueue `scroll-target\tstatus`.
- The page script validates an element policy exactly as a scroll would, but
  performs no scrolling.
- Report exactly one of:

  ```text
  Scroll target: automatic
  Scroll target: document
  Scroll target: selected region
  ```

- Do not include `innerText`, a selector, a URL, or other page-derived content
  in v0.1.

If no live view can execute a non-Hint action, report:

```text
Scroll target unavailable: no active page
```

### 7.4 Repeat and macro behavior

`scroll-target` is not repeatable and must not be added to
`macro_policy::is_repeatable_command`. Consequently `.` continues to repeat
the last actual scroll rather than the preceding target-policy change, and the
existing macro recorder does not record the policy command.

## 8. Default bindings and completion

Add to the Normal-mode binding registry next to the existing semicolon Hint
family:

```rust
binding(Mode::Normal, &[";", "s"], "scroll-target select")
binding(Mode::Normal, &[";", "S"], "scroll-target auto")
```

Do not implement the binding as `hint scrollables`; generated binding help must
show the first-class command.

In `completion.rs::enum_candidates`, return these values for
`scroll-target`:

```text
select auto document status
```

Existing user binding overrides continue to work through the normal binding
configuration layer. No new configuration setting is required.

## 9. Page-side state contract

### 9.1 Representation

Replace the loose element variable with a versioned state object in the same
isolated application world used by the existing browser-owned scripts.

The conceptual shape is:

```javascript
window.__ferric_browserScrollTargetState = {
    version: 1,
    policy: "auto" | "document" | "element",
    elementRef: WeakRef<Element> | null,
    ownerDocumentRef: WeakRef<Document> | null
}
```

Canonical forms:

```javascript
{ version: 1, policy: "auto", elementRef: null, ownerDocumentRef: null }
{ version: 1, policy: "document", elementRef: null, ownerDocumentRef: null }
{
    version: 1,
    policy: "element",
    elementRef: new WeakRef(el),
    ownerDocumentRef: new WeakRef(el.ownerDocument)
}
```

Absence of the property means `auto`. Any malformed or unknown-version object
must be normalized to `auto`; browser scripts must not throw because the object
was altered or partially initialized.

`WeakRef` is required for element assignment. If the page engine does not
provide it, leave or restore the canonical `auto` state and return
`{ok:false,error:"weak-reference-unavailable"}`. Do not silently fall back to
strong references. `auto` and `document` remain usable without `WeakRef`.

Remove all reads and writes of the old
`window.__ferric_browserScrollTarget` property. Do not keep two authorities.

### 9.2 Generated-script helpers

`BrowserScripts.js` is a QML library which returns JavaScript source strings;
its helper functions do not themselves run in the page. Keep that boundary.

Add source-building helpers so `scroll()`, `scrollTargetPolicy()`,
`scrollTargetStatus()`, and `hintSetScrollTarget()` embed one consistent set of
page-world predicates. Do not allow the four generated scripts to drift into
different definitions of scrollability or validity.

At minimum the emitted runtime needs helpers equivalent to:

```javascript
state()
setPolicy(policy, element)
axisScrollable(element, axis)
isReachableFromTopDocument(element, capturedDocument)
isMeaningfullyVisible(element)
validateElementState(state)
automaticTarget(axis)
effectiveTarget(axis)
```

Names may differ, but the behavior below is normative.

### 9.3 Axis scrollability

For an ordinary element, axis `x` is scrollable only when:

- computed `overflow-x` is `auto`, `scroll`, or `overlay`; and
- `scrollWidth > clientWidth`.

Axis `y` is equivalent using `overflow-y`, `scrollHeight`, and `clientHeight`.

Do not concatenate all overflow properties and use one regular expression: an
`overflow-x: auto` element must not thereby be treated as vertically
scrollable. `hidden`, `clip`, and `visible` are not selectable scroll axes in
this feature even if script can alter their offsets.

Use the same axis predicate in:

- the `scrollables` Hint collector;
- `hintFresh()` kind revalidation;
- `hintSetScrollTarget()`;
- pre-scroll target validation.

This consistency change is part of the feature, not optional cleanup.

The top-level document root is special: the `document` and automatic fallback
policies may address it regardless of its computed overflow value. Whether a
requested operation moves it is still determined by its actual dimensions.

### 9.4 Reachability and identity

At the start of validation, require `typeof WeakRef === "function"` and
brand-check both fields by calling the current realm's intrinsic
`WeakRef.prototype.deref` with each field as its receiver. Call the intrinsic
exactly once per field and retain the two results in local variables for the
rest of that operation; do not call a replaceable `state.elementRef.deref`
property. This makes the objects stable for the current JavaScript job without
retaining them between operations. A brand-check exception or missing
dereference result invalidates the element policy.

An element state then passes identity/reachability validation only if all of
these are true:

1. the dereferenced element is an element object and `element.isConnected` is
   true;
2. `element.ownerDocument === capturedDocument`;
3. the captured document still has a `defaultView`;
4. walking from that view through each `frameElement` reaches the current
   top-level `window`;
5. every traversed frame element is connected and still owns the traversed
   child window;
6. the walk does not cross an inaccessible/cross-origin boundary;
7. the element is scrollable on at least one axis.

Catch DOM/security exceptions and treat them as invalid. Adoption into another
document invalidates the target even if the element becomes connected there.
Do not use the top window's `element instanceof Element` as the only type check:
that is false for valid elements from another same-origin frame realm. Use a
realm-neutral node-type check or the element's owning window constructor.

Do not require the original frame path or Hint ID after selection. Dereferenced
DOM object identity plus the dereferenced captured `ownerDocument` is the
target identity. A same-document frame element moving in the outer DOM does
not by itself select a replacement or invalidate a still-reachable target.

### 9.5 Meaningful visibility

Use the same practical visibility model as fresh Hint validation:

- reject `display: none`;
- reject `visibility: hidden` or `collapse`;
- reject zero opacity;
- reject an inert element or inert composed ancestor;
- reject a non-positive client rectangle;
- clip through scroll/hidden/clip ancestors, through every containing frame,
  and against the top viewport;
- require a positive final intersection;
- require composed hit testing to reach the element or one of its composed
  descendants at at least one of the existing bounded sample points.

`pointer-events: none` remains non-hit-testable and therefore invalid. Use the
existing shadow-aware parent/containment helpers rather than plain
`parentElement`/`Element.contains` where they would mishandle open shadow roots.
Apply the suppressing style/inert checks across the composed ancestor and frame
chain, not just to the selected element. While walking frames, verify
`frameElement.contentWindow === childView` before proceeding to the parent
view.

Selection already performs `hintFresh()` validation, but
`hintSetScrollTarget()` must repeat the identity, visibility, and scrollability
checks because its asynchronous callback introduces a time-of-check/time-of-use
gap.

### 9.6 Script return values

Return small structured objects, not overloaded booleans.

Policy setters and status return:

```javascript
{ ok: true, policy: "auto" }
{ ok: true, policy: "document" }
{ ok: true, policy: "element" }
```

Failed Hint assignment returns:

```javascript
{ ok: false, error: "stale-target" }
{ ok: false, error: "weak-reference-unavailable" }
```

`scroll()` returns one of:

```javascript
{ ok: true, policy: "auto" | "document" | "element" }
{ ok: true, policy: "element", no_op: "unsupported-axis" }
{ ok: false, error: "no-scroll-root" }
```

An invalid element which falls back successfully reports the resulting
`policy: "auto"`. These objects are bounded and contain no page text.

Every generated IIFE must catch unexpected DOM/security exceptions at its
outer boundary. A setter/status failure returns `{ok:false,error:"script-error"}`;
a scroll failure returns the same shape and performs no second, unvalidated
operation. No exception may escape into QML as an unstructured result.

Routine scroll commands do not replace the status bar based on this result;
the result exists for diagnostics and live tests. Policy-changing commands do
use their callback result for status reporting.

### 9.7 Scrolling geometry

When the target is the document root, use the top-level viewport dimensions.
For an element target, use that element's `clientWidth` and `clientHeight`.

Do not use `Math.max(window.innerHeight, target.clientHeight)` for a nested
target; that makes page scrolling inside a small pane use the outer window's
height.

Retain the current bounded motion constants:

- incremental scroll: 15% of the target viewport, minimum 40 CSS pixels;
- full page: 90% of the target viewport;
- half page: 50% of the target viewport;
- count: clamped to `1..=9_999` before source generation.

`scroll-to top` and `scroll-to bottom` alter only `scrollTop`. Preserve
`scrollLeft`; vertical edge commands must not unexpectedly jump a horizontally
scrolled pane to its left or right edge.

Use the existing `scrollBy`/`scrollTo` calls. Do not dispatch synthetic keyboard
or wheel events.

## 10. Hint integration

The state machine remains:

```text
Normal
  |
  | ;s / scroll-target select
  v
Hint(scrollables)
  |
  | freshly validated label selection
  v
Normal + element policy
```

Required changes:

1. Preserve the existing `HintKind::Scrollable` branch in
   `browser_ui_navigation.rs`.
2. Continue returning the exact `element_id` and `frame_path` from the fresh
   candidate.
3. Change the QML wrapper and page script call to pass both values:

   ```text
   hintSetScrollTarget(elementId, framePath)
   ```

4. Require the retained Hint record to match both fields before reading its
   element.
5. On `{ok: true, policy: "element"}`, clear Hint presentation state, remain in
   Normal mode, and report `Scroll target: selected region`.
6. On failure, close the Hint session safely and report:

   ```text
   Scroll target selection failed: region is no longer available
   ```

7. Never call `focus()`, `blur()`, or synthesize an input event as part of
   selection.

The page-side target state holds weak references independently of the Hint
element map. Once the final activation callback has finished reading its
record, closing a Hint session must clear the map's strong element records and
remove or clear the `window.__ferric_browserHintElements` alias. Do not clear
the map between activations in an active rapid-Hint session. Releasing a closed
session's map does not clear a valid selection: a connected element and its
document remain strongly owned by the live DOM/frame tree, while the target
state retains only `WeakRef`s.

## 11. Rust and IPC plumbing

### 11.1 Interactive executor

Add `execute_scroll_target_command()` beside `execute_scroll_command()` in
`browser_ui_browsing_commands.rs`.

Its responsibilities are limited to:

- registry validation;
- exact action parsing through `command_options`;
- entering Hint mode for `select`;
- queuing a bounded engine-action string for the other three actions;
- returning a bounded JSON acknowledgement.

Use these acknowledgement shapes:

```json
{"status":"accepted","action":"select","mode":"hint"}
{"status":"accepted","action":"auto","pending":true}
{"status":"accepted","action":"document","pending":true}
{"status":"accepted","action":"status","pending":true}
```

Do not add policy fields to `BrowserUiRust`, `ApplicationState`, tab snapshots,
session snapshots, or storage. The `hint_chrome_available` boolean described
above is a QML-supplied presentation capability only; it is not scroll-target
state and must not be serialized.

Add a `scroll-target` branch to both:

- `browser_ui_command_dispatch.rs` for interactive/binding execution;
- `browser_ui_ipc.rs` for compatibility command execution.

For IPC, reject context routing with:

```text
context routing is not valid for scroll targeting
```

Then validate the ordinary tab/window route exactly as page scrolling does.

### 11.2 Shared IPC codec

The compatibility command envelope is:

```json
{
  "command": "scroll-target",
  "arguments": { "action": "select|auto|document|status" }
}
```

Update all of the following authorities; none may accept a broader language
than the others:

- `crates/ferric-browser-ipc/src/command_schema.rs` — accepted field is exactly
  `action`;
- `crates/ferric-browser-ipc/src/command_codec_navigation.rs` — encode exactly
  one valid positional action beside `scroll`, `scroll-page`, and `scroll-to`;
- `crates/ferric-browser-ipc/src/command_argument_validation.rs` — require a string in
  the four-value enum;
- `crates/ferric-browser-ipc/src/command_argument_decoder.rs` — decode it to the one
  positional argument;
- `crates/ferric-browser-engine-qt/src/ipc_schema.rs` — retain the same closed enum at
  the Qt compatibility boundary if local semantic validation remains
  duplicated there.

Reject missing `action`, `null`, non-string values, unknown enum values, and
unknown fields before page state is touched.

No arbitrary script, element identity, frame path, selector, or descriptor is
accepted over IPC.

As with existing page-scroll commands, `command.execute` is asynchronous: the
IPC response is an accepted acknowledgement with the operation ID and command
context added by `handle_ipc_request`; the page-script callback supplies the
visible browser status later. This increment does not add a synchronous DOM
query or a new IPC query method. Tests must not claim the acknowledgement
contains the eventual policy.

### 11.3 CLI

No command-specific change should be needed in the CLI after the shared codec
is updated. Add a CLI/IPC round-trip test so a future codec reordering cannot
silently fall through to the no-argument generic encoder.

## 12. QML dispatch

### 12.1 Script wrappers

Expose these wrappers through the existing runtime layers:

```text
scrollTargetPolicyScript(policy)
scrollTargetStatusScript()
hintSetScrollTargetScript(elementId, framePath)
```

Keep script assembly in `BrowserScripts.js`; QML must not construct JavaScript
source.

Increment `BrowserScripts.VERSION` from `9` to `10` and update the versioned
resource test.

### 12.2 Shared action application

Add one QML helper on the root runtime, conceptually:

```text
applyScrollTargetAction(ui, view, action)
```

It must:

1. reject a missing `view` with the exact unavailable message;
2. choose the policy or status script from the validated action;
3. run it through `runBrowserScript()` in `browserScriptWorld`;
4. validate the returned object and enum;
5. write the exact status text to the supplied `ui`, not always the primary
   `browserUi`.

Only `auto`, `document`, and `status` are valid pending actions. For setters,
the returned policy must equal the requested policy. For status, map `element`
to `selected region`. Any unknown action, false return from
`runBrowserScript()`, absent/malformed callback value, `{ok:false,...}`, or
unexpected policy must produce this bounded status without exposing page or
exception text:

```text
Scroll target command failed
```

Both pending-action consumers must call it:

- `FerricBrowserRuntimeSurface.qml` with `browserUi` and the primary active
  view;
- `FerricBrowserWindow.qml` with `secondaryUi` and
  `secondaryWindow.activeView`.

This avoids duplicating result parsing and ensures `auto`, `document`, and
`status` affect the view which owns the command.

`select` continues to follow the existing Hint-presentation availability of
the owning surface. This feature must not silently run the primary window's
Hint overlay against a secondary window's view. Set
`secondaryUi.hint_chrome_available: false`; the primary `BrowserUi` uses the
default `true`. A future secondary-Hint project can flip the capability only
after that window owns a complete collector, overlay, refresh, and activation
path.

### 12.3 Ordinary scroll dispatch

Both existing scroll action consumers continue to call
`BrowserScripts.scroll()`. No Rust-side target parameter is added: the called
document resolves its own policy.

Continue scheduling the normal document scroll-position capture after a scroll
request. Session capture remains document-only and must not serialize the
selected element's offset or identity.

## 13. Lifecycle

Element and document policies are ephemeral document-world state.

They must be cleared by construction when any of these creates a new document
world:

- normal top-level navigation;
- reload or cache-bypassing reload;
- renderer/document replacement;
- tab destruction or discard;
- browser restart;
- session restoration.

No Rust cleanup action is required merely to mirror the state. The new document
starts in `auto` because its isolated-world global has no state object.

Same-document fragment changes and `history.pushState()` do not create a new
document and therefore do not automatically clear the policy. Ordinary SPA DOM
updates may retain an element target, but the next operation still applies full
identity, reachability, visibility, and scrollability validation.

An element may survive movement, resizing, content updates, or reparenting
inside the same captured document. Adoption into another document is invalid.

## 14. Security and privacy constraints

This remains a browser-owned one-shot script feature in Qt WebEngine's isolated
application world.

The implementation must not:

- add a WebChannel bridge or expose QML/native objects to page JavaScript;
- cross origin boundaries to discover or operate on frame DOM;
- accept page-provided script through this command;
- expose DOM references through Rust, QML properties, JSON-RPC, or storage;
- persist selectors, Hint IDs, frame paths, element descriptions, or offsets;
- change DOM focus to influence scrolling;
- send synthetic key, wheel, mouse, or touch events;
- include page text in status, logs, command history, or IPC responses;
- retain an old element and later reinterpret it as a replacement.

The state property name is not a security boundary. Isolation, closed command
arguments, exact object identity, and revalidation are the boundaries.

## 15. File-by-file implementation plan

### 15.1 `crates/ferric-browser-core/src/command.rs`

- register `scroll-target` beside the scrolling commands;
- set its required `action: Enum` metadata and four examples;
- update the registry count assertion;
- add registry metadata tests.

### 15.2 `crates/ferric-browser-core/src/input.rs`

- add `;s` and `;S` Normal bindings;
- assert both exact key chains and commands;
- assert they resolve without conflicting with the existing semicolon family.

### 15.3 `crates/ferric-browser-core/src/completion.rs`

- add the four ordered enum completions;
- test empty and partially typed action completion.

### 15.4 `crates/ferric-browser-ipc/src/`

- update `command_schema.rs`;
- update `command_codec_navigation.rs`, beside the existing scroll encoders;
- update `command_argument_validation.rs`;
- update `command_argument_decoder.rs`;
- add success and rejection tests for all four actions.

### 15.5 `crates/ferric-browser/src/tests.rs`

- add a CLI-to-IPC round-trip test for all four actions;
- assert that each positional action reaches the typed `action` envelope field
  and decodes back to the same parsed command;
- assert that missing, extra, and invalid positional actions are rejected
  rather than falling through to the generic no-argument encoder.

### 15.6 `crates/ferric-browser-engine-qt/src/command_options.rs`

- add `ScrollTargetAction`;
- add its exact parser and command-name predicate;
- unit-test every valid action and invalid arity/value.

### 15.7 `crates/ferric-browser-engine-qt/src/lib.rs`

- re-export the new option parser/predicate using the existing private import
  pattern;
- add the `hint_chrome_available` presentation-capability property and backing
  boolean, initialized to `true`; do not add public API beyond the QML property.

### 15.8 `crates/ferric-browser-engine-qt/src/browser_ui_browsing_commands.rs`

- add `execute_scroll_target_command()`;
- keep it page-state-agnostic and queue only the bounded action string;
- do not add durable or core model state.

### 15.9 `crates/ferric-browser-engine-qt/src/browser_ui_command_dispatch.rs`

- route the command through the new executor before the core fallback;
- preserve normal registry validation and command-chain queuing.

### 15.10 `crates/ferric-browser-engine-qt/src/browser_ui_ipc.rs`

- add the routed IPC execution branch;
- reject context routing;
- return the specified acknowledgement.

### 15.11 `crates/ferric-browser-engine-qt/src/ipc_schema.rs`

- close and validate the local compatibility envelope consistently with the
  shared IPC crate.

### 15.12 `crates/ferric-browser-engine-qt/qml/scripts/BrowserScripts.js`

- bump `VERSION` to `10`;
- replace the old loose target variable with the versioned policy state;
- retain selected elements and captured documents only through `WeakRef`, and
  clear strong Hint element records when their session closes;
- centralize emitted state/validity predicates;
- add policy/status script builders;
- strengthen `scroll()` and return structured results;
- make scrollability axis-specific in collector and fresh validation;
- change `hintSetScrollTarget()` to accept and verify frame path;
- use target-local geometry and preserve horizontal offset for vertical edge
  commands.

### 15.13 `FerricBrowserRuntimeServices.qml`

- add script wrappers which delegate to `BrowserScripts.js`;
- add the shared result-validating `applyScrollTargetAction()` helper.

### 15.14 `FerricBrowserRuntimePresentation.qml`

- pass frame path during Hint assignment;
- consume the structured assignment result;
- emit the final success/failure text.

### 15.15 `FerricBrowserRuntimeSurface.qml`

- consume `scroll-target\tACTION` pending actions for the primary view through
  the shared helper.

### 15.16 `FerricBrowserWindow.qml`

- consume non-Hint policy/status actions for the secondary active view through
  the same helper;
- set `secondaryUi.hint_chrome_available: false` and preserve the existing
  full-chrome restriction for `select`.

### 15.17 Tests and evidence

- extend core, IPC, Qt option, command-protocol, and presentation-boundary
  tests;
- add the fixture and evidence updates in Section 17;
- regenerate `docs/user/commands.md` and `docs/user/bindings.md` rather than
  editing them by hand.

No changes are expected in storage, configuration schema, reducer/model state,
session serialization, or native pointer adapters.

## 16. Automated test requirements

### 16.1 Core registry and bindings

Test that:

- `scroll-target` resolves in Normal and Command modes;
- it has one required enum argument, Tab scope, Mutating effect, and no count;
- all four examples are present;
- `;s` maps exactly to `scroll-target select`;
- `;S` maps exactly to `scroll-target auto`;
- existing semicolon bindings still resolve;
- completion returns `select`, `auto`, `document`, `status` and filters prefixes.

### 16.2 Pure option parser

Test:

- each action maps to the correct enum;
- zero arguments fail;
- two arguments fail;
- case variants such as `Auto` fail;
- unknown actions fail;
- every failure uses the specified bounded error.

Also test `scroll-target select` through the full executor with
`hint_chrome_available` true and false. The false case must leave the mode
unchanged and queue no engine action.

### 16.3 IPC

For each action, test parsed-command to envelope encoding and envelope back to
the same parsed command. Also reject:

- `{}`;
- `{"action": null}`;
- `{"action": 1}`;
- `{"action": "element"}`;
- an extra field;
- two CLI positional actions;
- an action containing control characters.

In `crates/ferric-browser/src/tests.rs`, exercise the real CLI conversion path
for all four actions and the invalid positional cases. This is a separate gate
from the shared IPC crate's codec unit tests.

Add Qt command-protocol coverage proving interactive and IPC entry points queue
the same bounded action, and that context routing is rejected.

### 16.4 QML/resource boundaries

Extend `presentation_boundaries.rs` to assert:

- `BrowserScripts.VERSION` is `10`;
- all three new/changed wrappers delegate to `BrowserScripts.js`;
- both primary and secondary pending-action consumers recognize the bounded
  `scroll-target\t` prefix;
- the secondary `BrowserUi` explicitly disables Hint chrome capability;
- QML does not assemble page JavaScript;
- the old `window.__ferric_browserScrollTarget` authority is absent;
- the versioned state name and structured result fields exist;
- element state uses `WeakRef`/`deref()` rather than strong element or document
  fields, and closed Hint sessions clear their strong element records;
- Hint assignment passes frame path;
- page scripts contain no `.focus()` call in the scroll-target functions.

Static string tests supplement but do not replace live DOM qualification.

### 16.5 Regression tests

Retain all existing tests for:

- `h/j/k/l`;
- half/full-page scrolling;
- `gg/G`;
- typed scroll command bounds;
- primary and secondary scroll payload dispatch;
- Hint stale-candidate validation;
- session document scroll-position capture.

With no policy state, these commands must behave exactly as before except for
the intentional corrections to axis classification, nested-target viewport
geometry, and horizontal-offset preservation documented above.

## 17. Live fixture and qualification

Add `docs/testing/fixtures/scroll-targets.html`. Keep it local and deterministic.
It must contain:

1. a vertically scrollable document;
2. a vertical-only pane with an `input` child;
3. a horizontal-only pane;
4. a two-axis pane;
5. nested inner and outer scrollable panes;
6. two independent side-by-side vertical panes;
7. a same-origin `srcdoc` frame containing a scrollable pane;
8. an open shadow root containing a scrollable pane;
9. controls which remove a selected pane, set `display:none`, remove overflow,
   remove only one axis, reparent/adopt where feasible, and navigate or replace
   the same-origin frame document;
10. visible readouts of every region's `scrollLeft`, `scrollTop`, and the
    focused element's stable ID.

The fixture must not depend on timers, remote resources, or network content.

Perform and record this matrix in a new evidence file,
`docs/testing/explicit-scroll-target.md`:

| Case | Required result |
| --- | --- |
| No selection, focused child in vertical pane, `j` | focused pane moves |
| Invoke the actual `;s` binding, select other vertical pane, then press `j` | Hint mode opens; only selected pane moves |
| Invoke the actual `;S` binding after an element or document selection | policy becomes automatic; focus-aware scrolling resumes |
| Same selection, `h` | no region moves; target remains selected |
| Same selection at bottom, `j` | no fallback; target remains selected |
| Select horizontal pane, `l` | only selected pane moves |
| Select two-axis pane, mix `j/l/Ctrl+d/G` | every command owns that pane |
| Selected pane with nonzero `scrollLeft`, `gg`/`G` | vertical edge changes; `scrollLeft` is preserved |
| Focus input A, select pane B | input A remains `document.activeElement` |
| Remove selected pane, `j` | policy becomes auto; same command uses automatic target |
| Hide selected pane, `j` | same invalidation/fallback behavior |
| Remove all overflow, `j` | same invalidation/fallback behavior |
| Remove vertical but retain horizontal overflow, `j` | no-op; element remains selected |
| Then press `l` | selected element moves horizontally |
| `scroll-target document` with focus in pane | only document scrolls |
| `scroll-target auto` | focus-aware behavior resumes |
| `scroll-target status` in every state | exact status text |
| Select same-origin frame/shadow target | exact selected element moves |
| Select the frame's pane, then navigate or replace that frame document and press `j` | old target invalidates; same command resolves once through automatic policy |
| After each remove, hide, overflow-loss, adoption, or frame-document invalidation, run `scroll-target status` | exact status is `Scroll target: automatic` |
| Reload after selection | status reports automatic |
| Navigate after selection | new document reports automatic |
| Switch tabs | each document's ephemeral policy is independent |
| Restore a session | restored documents start automatic |

Explicitly note the existing exclusion for cross-origin frame contents.

## 18. Documentation updates

Implementation is incomplete until all documentation authorities agree.

1. Update `docs/DEVELOPMENT_SPEC.md`:
   - add the command to the V1 catalog;
   - change the scrolling rows from the vague “focused container” wording to
     the `element/document/auto` contract;
   - extend HINT-004 with explicit selection and focus independence.
2. Update `docs/requirements.csv` HINT-004 evidence and, if the project uses a
   separate scrolling requirement by implementation time, add the final
   acceptance/evidence row there rather than inventing an untracked ID in code.
3. Update `docs/testing/m0-91-page-scroll.md` and
   `docs/testing/m1-06-hints.md` with the implemented behavior and evidence.
4. Add `docs/testing/explicit-scroll-target.md` and the fixture.
5. Run `cargo xtask docs` to regenerate `docs/user/commands.md` and
   `docs/user/bindings.md`.

Do not manually edit generated user registry tables.

## 19. Recommended implementation order

The following order keeps every intermediate change reviewable:

1. Add the core command metadata, enum completion, bindings, and unit tests.
2. Add shared IPC encode/validate/decode support and round-trip tests.
3. Add the pure Qt-side action parser and executor routing, initially queuing
   bounded actions only.
4. Refactor `BrowserScripts.js` to the versioned state and shared predicates;
   implement policy/status scripts and the strengthened `scroll()` contract.
5. Update Hint assignment to use the new state and frame-path check.
6. Add shared QML action application and wire primary/secondary consumers.
7. Add/adjust static boundary and command-protocol tests.
8. Add the live fixture, perform the qualification matrix, and record evidence.
9. Update specification/requirements evidence and regenerate user docs.
10. Run the complete verification gate.

Do not start by adding Rust tab state. The policy belongs to the document and
requires no persistence migration.

## 20. Acceptance invariants

**SCROLLTARGET-001** — With no page-side policy, Ferric retains focus-aware
automatic scrolling.

**SCROLLTARGET-002** — Selecting or changing a scroll target never changes DOM
focus.

**SCROLLTARGET-003** — All existing scroll commands resolve through the same
policy.

**SCROLLTARGET-004** — An element target is the exact selected DOM object and
cannot migrate to a replacement.

**SCROLLTARGET-005** — A disconnected, adopted, unreachable, hidden, or
entirely non-scrollable element is cleared to automatic, and the triggering
operation is retried exactly once through automatic resolution.

**SCROLLTARGET-006** — An unsupported axis or reached boundary is a no-op and
does not clear or bypass a valid explicit target.

**SCROLLTARGET-007** — `document` never redirects because focus is inside a
nested container.

**SCROLLTARGET-008** — `document` and `auto` remain observably distinct.

**SCROLLTARGET-009** — A new document, reload, restart, discard, or session
restore starts in `auto`; same-document navigation may retain a still-valid
target.

**SCROLLTARGET-010** — Selection reuses Hint mode and introduces no persistent
keyboard mode.

**SCROLLTARGET-011** — Selection performs fresh identity, frame, visibility,
and axis-aware scrollability validation after the user chooses a label.

**SCROLLTARGET-012** — No DOM identity, selector, frame path, descriptor, or
offset enters Rust durable state, session state, IPC output, logs, or command
history.

**SCROLLTARGET-013** — Cross-origin isolation is unchanged.

**SCROLLTARGET-014** — Nested element page/half-page motion uses the element's
viewport rather than the outer browser viewport.

**SCROLLTARGET-015** — Vertical edge commands preserve horizontal position.

**SCROLLTARGET-016** — `auto`, `document`, and `status` operate on the owning
primary or secondary active view; `select` follows existing Hint chrome
availability and never targets a different window's view.

**SCROLLTARGET-017** — The old loose target variable is removed so there is one
page-side authority.

**SCROLLTARGET-018** — Element policy and closed Hint sessions do not strongly
retain selected DOM elements or captured documents; each operation dereferences
the exact weakly held objects and validates them before use.

## 21. Definition of done

The feature is done only when:

- every invariant above is implemented;
- all automated tests in Section 16 pass;
- the Section 17 live matrix has recorded evidence, including focus identity
  and dynamic invalidation;
- generated docs are current;
- no persistence or configuration migration was introduced;
- the working tree contains no hand-edited generated registry table;
- the following gates pass from the repository root:

```text
cargo fmt --all -- --check
cargo test -p ferric-browser-core -p ferric-browser-ipc -p ferric-browser-engine-qt --locked --offline
cargo test -p ferric-browser --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo xtask docs
cargo xtask check --locked --offline
cargo build -p ferric-browser --locked
cargo xtask test adapter
```

If the environment has a working Wayland compositor, also run the repository's
native Wayland smoke and record whether the interactive fixture matrix was run
there. Lack of a compositor may defer native qualification, but it must be
stated explicitly rather than reported as a pass.

## 22. Repository references

The implementation should be read against these current authorities:

- `crates/ferric-browser-core/src/command.rs`
- `crates/ferric-browser-core/src/input.rs`
- `crates/ferric-browser-core/src/completion.rs`
- `crates/ferric-browser/src/tests.rs`
- `crates/ferric-browser-ipc/src/command_schema.rs`
- `crates/ferric-browser-ipc/src/command_codec_navigation.rs`
- `crates/ferric-browser-ipc/src/command_argument_validation.rs`
- `crates/ferric-browser-ipc/src/command_argument_decoder.rs`
- `crates/ferric-browser-engine-qt/src/command_options.rs`
- `crates/ferric-browser-engine-qt/src/browser_ui_browsing_commands.rs`
- `crates/ferric-browser-engine-qt/src/browser_ui_command_dispatch.rs`
- `crates/ferric-browser-engine-qt/src/browser_ui_ipc.rs`
- `crates/ferric-browser-engine-qt/src/browser_ui_navigation.rs`
- `crates/ferric-browser-engine-qt/qml/scripts/BrowserScripts.js`
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimeServices.qml`
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimePresentation.qml`
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimeSurface.qml`
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserWindow.qml`
- `crates/ferric-browser-engine-qt/src/tests/command_protocol.rs`
- `crates/ferric-browser-engine-qt/src/tests/presentation_boundaries.rs`
- `docs/DEVELOPMENT_SPEC.md`
- `docs/requirements.csv`
- `docs/testing/m0-91-page-scroll.md`
- `docs/testing/m1-06-hints.md`

This specification is deliberately self-contained. An implementation agent
should not need to invent product semantics, persistence rules, command
language, page-state shape, invalidation behavior, QML routing, IPC payloads,
test cases, or completion criteria.

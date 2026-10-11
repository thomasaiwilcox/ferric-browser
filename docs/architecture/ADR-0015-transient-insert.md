# ADR-0015: Transient and locked insert sessions

Normal mode is home. `Mode::Insert` is transient; `Mode::InsertLocked` is
explicit persistent editing. The default bindings are immediate `i` and `I`,
respectively. Both reuse existing insert focus and event routing. The status
bar displays `INSERT` and `INSERT LOCKED`. No input is automatically chosen.

The native application event filter observes actual Return/Enter keys and Qt
modifiers. An unmodified transient Enter (including keypad Enter) records a
physical gesture and its input epoch, allowing the original press through.
At the final non-repeat release, the router synchronously dispatches the
original release through Qt with a recursion guard, then signals mode exit.
Both halves follow Qt's native delivery path before mode changes.
Completion follows the owning session's epoch even if Enter moves focus or
disables its keyboard router; a newer mode or tab invalidates that ownership.
The release still goes to its native Qt receiver when focus moves elsewhere.
A fresh physical press
supersedes a lost release of the same key before modifiers or composition are
classified. This prevents a later modified Enter or IME confirmation from
inheriting an old ordinary Enter's exit policy. A fresh native press also
clears stale consumed-key ownership. No synthetic Escape, DOM blur, replacement
keyboard event, timer, or additional input queue is used.
Chromium handles the delivered events asynchronously; this boundary guarantees
Qt delivery ordering, not completion of page JavaScript or network navigation.
Shift, Control, Alt and Meta Enter keep editing; Locked Insert keeps editing
for all Enter combinations. Existing Escape handling consumes the gesture.

Input-method preedit events identify active composition. Enter during active
preedit does not schedule transient exit. Commit clears that state. Entry
projects the insert variant before its input epoch so preedit already active
in the focused receiver survives transient entry; exit and invalid editing
session boundaries clear the native tracking. Platform IMEs that consume Enter
before Qt sees it naturally leave the mode unchanged.
An IME which clears preedit before forwarding its confirmation Enter cannot
be distinguished from an ordinary Enter with the Qt event information alone;
platform IME qualification remains necessary.

The existing browser-owned focus observer records editable element identities
using a WeakMap, including focused descendants of open shadow roots. Focus-out
publishes after a microtask, allowing the new focus target to settle. The
existing focus probe also refreshes identity and editability. Focus and input
notifications request an isolated-world probe through Qt console signals;
notification contents never select a mode. One narrowly scoped MutationObserver
watches direct child-list changes on the focused editable element's ancestors
(up to 64), disconnecting on blur. It catches field or ancestor removal without
subtree/attribute observation, whole-page scanning or continuous polling. A
transient session ends on blur or replacement of an observed editable context.
Locked Insert ignores these focus transitions. Both reset on active tab changes,
active page navigation, active renderer termination and view destruction.
Mode and lifecycle transitions increment an input epoch; focus query callbacks
capture it before dispatch.
The adapter also checks tab index, page URL, frame path and observation sequence.
Late focus results and old Enter releases cannot overwrite a newer mode.
Completion and Escape cancel the existing bounded focus-probe retries, while
epochs invalidate queries already in flight. Existing suppression prevents a
completed Enter interaction from re-entering Insert just because its field
remains focused.

Focus detection inherits existing asynchronous probe latency. Closed shadow
roots, ancestry beyond the 64-node observation bound and editor-specific focus
proxies cannot reliably expose element identity. Removal or replacement of an
embedded frame without a focus notification can also escape element-level
tracking; the observer does not instrument the frame's embedding document.
No speculative editor or website heuristics are added. Lifecycle reset and
ordinary Enter remain reliable fallback boundaries. Locked Insert is
recommended for document editors and repeated paragraphs.

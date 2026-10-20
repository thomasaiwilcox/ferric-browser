# Changelog

## Unreleased

- Navigating the active document now ends Hint mode even when navigation
  retires an activation callback. Leaving Hint mode clears pending collection
  tokens; background tab loads preserve the active hint session. Hint visibility
  respects descendant CSS overrides of inherited pointer-events and visibility.
  Programmatically focusable custom controls declaring type=button are included
  as button actions, covering controls such as Mumsnet's React widget.

- Keyboard Back/Forward, reload and other captured engine bindings now reach
  the typed dispatcher instead of queuing legacy action names. SPA history
  imports refresh navigation availability, and a same-URL push at the first
  history entry retains both states. Normal-mode mouse Back/Forward uses one
  browser traversal per gesture. Both window status bars offer Copy URL even
  with a single tab; the Normal-mode `yy` shortcut remains available. Bare
  `hint` bindings now use the same action-first default as the command bar.

- Default hints favour actions and editable fields, omitting focus-only wrappers.
  `hint actions` is the new default; `hint all` retains comprehensive targets and
  `hint focus` explicitly focuses keyboard regions. Both new views inherit
  `hints.selectors.all`. Existing custom bindings are preserved. Shared rendering
  checks exclude hidden disclosure contents, including slotted controls, during
  collection and activation. Hint descriptions distinguish Edit from Focus.

- Hints prefer native expand/collapse headers over redundant focus-only containers,
  fixing collapsed Reddit threads entering Insert without opening. Checkbox, radio,
  submit, reset and image-button inputs now activate through their native behaviour.
  Button hints include these controls; caption names exclude editable field values,
  and activation rejects inputs whose type changed after collection.

- Embedded consent dialogs can expose semantic hints through a verified focused
  frame/document lease. Unfocused frames offer Hint inside frame; collection and
  activation reject stale documents, redirected focus, covered targets and
  unsupported geometry. Origin isolation and explicit fallback actions remain.

- Visible article titles covered by a transparent card link now retain their own
  URL hints, including nested titles on scrolled feeds. Badges avoid neighboring
  title fragments, recheck cached placements after reflow, and use short edge
  connectors. Native pointer and semantic click delivery retain exact-target checks.

- Hint discovery now excludes plain slotted containers and reuses completed
  traversal work. Labels and partial input survive page updates without passive
  activation or label reuse. Anchored badges, target outlines, accessible names,
  action previews and selectable overlap stacks make controls easier to identify.
  Focus-only widgets focus correctly; supported frame geometry includes scaling,
  borders and padding. New controls beyond the current label capacity require
  restarting hints, preserving the labels already on screen.

- Pass-through now forwards ordinary Escape to the webpage; use Ctrl+Escape
  to return to Normal. Ctrl+v retains persistent Pass-through, while
  Ctrl+Shift+v and `; v` forward one complete key gesture and return to Normal.
  Both window types show the mode and recovery hint. Page/tab boundaries
  reset both variants; window deactivation cancels one-key forwarding.

- `i` now enters Transient Insert: ordinary Enter reaches the page before
  returning to Normal. `I` enters Locked Insert for extended editing.
  Shift+Enter and other modified Enter combinations keep editing; Escape
  exits either variant. Focus and page/tab lifecycle changes invalidate stale
  editing callbacks. Fresh physical presses cannot inherit an earlier lost
  Enter release, including modified Enter and IME confirmation.
  Enter completion also survives focus moving to browser chrome or another
  window, without overwriting a newer editing session.
- CLI `mode-enter insert-locked`, binding queries without `--mode`, and tab
  queries without `--window` now
  produce valid IPC payloads. Window queries report the same canonical mode
  names as the keyboard and command references.
  Locked Insert is also accepted by configuration/runtime binding validation,
  typed binding commands and the existing binding-help projection.

The current development line contains the typed command/action registry,
bounded local IPC, durable session and profile storage, context routing,
diagnostics, and the system-Qt Arch packaging path. See the development
specification and testing records for qualification status and known limits.

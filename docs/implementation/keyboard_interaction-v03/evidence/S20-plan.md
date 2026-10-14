# S20 experiment plan

## Gate question

Can the existing Ferric public-Qt pointer boundary support one bounded in-view
left-button press/move/release transaction, with both endpoints fixed before
press, while retaining the original owner and clearing held-button state on
Escape, window deactivation, a native prompt, target destruction, renderer
loss, or watchdog expiry? Can cleanup avoid sending an event into a replacement
view, and can the fixture distinguish pointer-driven widgets from HTML drag and
drop?

No production drag UI or adapter operation will be added during this research
slice. The only experiment is a disposable native-insert WebEngine fixture that
uses the public `QQuickWindow` event path already used by
`FerricPagePointerAdapter`, plus test-only bookkeeping that observes the
adapter's existing invalidation and target lifecycle signals.

## Probe plan

1. Add a bounded `drag-probe.html` fixture containing an ordinary
   pointer-driven control, an explicit Pointer Events capture control, a range
   slider, a canvas-like pointer surface, and a native HTML draggable/drop
   source and destination. Log event type, target, coordinates, buttons,
   `isTrusted`, capture state, drop count, and any committed local fixture state.
2. Choose start and end points before press. Send one public-Qt left-button
   sequence through the owning Quick window with no modifiers, no cross-window
   movement, 12 straight-line steps over 200 ms, and a 1,000 ms operation
   watchdog. Never use a page-script click or a desktop-wide input service.
3. Observe press/move/release, pointer capture and release, range slider
   changes, canvas-like movement, and native HTML drag/drop independently.
   Do not generalize one widget's result to the other categories.
4. Exercise cancellation after press for Escape, deactivation, native-prompt
   input blocking, target-item destruction, renderer/view loss, and watchdog
   expiry. A live original receiver may receive one cleanup release; a destroyed
   receiver clears bookkeeping without sending to a replacement. Record whether
   release already committed a local drop and do not claim rollback.
5. Run the existing native Wayland harness separately. A nested startup smoke
   is not pointer-input evidence. If required native input is unavailable,
   retain an environment-blocked decision rather than treating missing input
   as a technical no-go.

## Evidence mapping

- T047/native-qt-offscreen: event path, capture, cancellation, receiver-loss,
  watchdog and category results.
- T049/native-qt-offscreen: release bookkeeping, replacement isolation, and
  whether an accepted local drop has an irreversible fixture side effect.
- T047 and T049/native-wayland: report only actual input coverage; keep this
  separate from Qt offscreen evidence.

The result may support only specific pointer-driven widget categories, or may
be a concrete no-go if the public boundary cannot maintain ownership and
cleanup. Even a positive experiment does not authorize a product UI. If native
Wayland evidence is unavailable, S21 cannot be treated as a qualified go.

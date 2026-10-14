# ADR-0018 — Native editing and source-qualified selection

Status: accepted (2026-10-14). Qualification is recorded in
[the clipboard report](../testing/clipboard-robustness-2026-10-14.md).

Normal Ctrl+V previously entered Pass-through instead of pasting. Ctrl+Shift+V
could paste and enter Pass Next Key in the same gesture. Context-menu Copy
extracted the main document selection, returning stale parent text for frames
and stripping rich MIME formats.

The owner approved standard native clipboard shortcuts. Persistent Pass-through
moves to ;V; Pass Next Key remains ;v. Explicit configured bindings are retained
and diagnosed through binding-explain. ShortcutOverride and held gestures use
the same synchronous ownership policy; no fabricated key replay is added.

Native edits use typed ClipboardEdit operations and public Qt WebEngine actions.
Native clipboard formats remain engine-owned. Popup edits resolve the claimed
popup target through the existing core dispatcher; opener indices do not
authorize popup operations. Selection yank/search/external
reads retain their bounded plain-text contracts. Selection sources use exact
native frame leases, independent from hint sessions, and isolated-world numeric
selection metadata; text is acquired only on explicit requests. Browser-owned
focus handoffs preserve the captured source, while document/frame/selection
changes reject stale requests without parent fallback or a second transport.

Native edit acknowledgements mean requested, not proven completed. Plain-text
copy success follows acceptance by the Qt clipboard adapter. Clipboard API
permissions, regular/primary separation and paste-open bounds are unchanged.

Plain native chrome fields provide Ctrl+Shift+V through a focused Qt shortcut
that calls the native field paste method, without reading clipboard data in QML.

Production regressions are required through cargo xtask test
application-clipboard and CI. Offscreen tests do not qualify real Wayland data
exchange or physical keyboard input; those results must be recorded separately.

# Clipboard navigation failure evidence

Date: 2026-09-16  
Status: in-progress

## Clipboard-to-navigation boundary

Normal-mode `p` reads the desktop clipboard only in response to the key event
and navigates the current tab. `P` creates a foreground tab only after the
clipboard text has passed validation, then navigates that tab. The
`paste-open` command provides the same current-tab operation through the
command and typed IPC surfaces.

The clipboard boundary rejects empty text, text larger than 64 KiB, and any
control character. It trims surrounding whitespace, resolves the remaining
text through the normal navigation resolver, and reports a bounded failure
status without dispatching a navigation effect. For the new-tab target, all
validation occurs before tab creation, so clipboard read, validation, and URL
resolution failures cannot leave an empty or garbage tab behind.

The read is not polled or persisted. A missing Qt GUI application or clipboard
object is represented as an empty read and follows the same no-navigation
failure path.

## Verification

```text
cargo test -p ferric-browser-engine-qt clipboard_navigation_input_is_bounded_and_rejects_failures --locked --offline
cargo fmt --all -- --check
cargo xtask check
```

Automated coverage verifies whitespace normalization, empty/control-character
rejection, the 64 KiB boundary, and the explicit navigation path wiring.
Interactive Wayland clipboard and compositor failure injection remain to be
qualified.

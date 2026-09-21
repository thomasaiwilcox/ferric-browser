# M0-49 Ephemeral hint target

Date: 2026-09-16  
Status: in-progress  
Requirement: EPROFILE-002

## Implemented slice

`hint --target ephemeral links` is accepted as a typed, non-rapid hint action.
After the browser revalidates the selected link against the live document, Rust
publishes an `ephemeral-window` engine action containing only the sanitized URL.
QML consumes that action by creating a fresh secondary native window with a
unique ephemeral name/label and `windowEphemeralProfile: true`.

The new window has its own `BrowserUi` and off-the-record `WebEngineProfile`,
does not open a Rust profile store, and navigates only the selected safe URL.
`--rapid` is refused for this target so one hint activation cannot silently
create an unbounded set of transient windows.

## Verification

CLI and Qt static regression tests cover the typed target and the native-window
action. The full project test gate and Wayland startup smoke remain required;
invocation-token reuse of an already-live ephemeral profile remains later.

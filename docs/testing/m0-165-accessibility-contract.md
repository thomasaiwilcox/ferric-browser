# M0-165 accessibility contract evidence

Date: 2026-09-19  
Status: in-progress

The project check now validates the browser-owned QML accessibility contract
before compiling tests. It requires named and described controls, dialog,
editable-text, list, menu-item, tab-list, tab, and status-bar roles, selected
tab state, visible-overlay focus boundaries, and Escape handling. Empty
accessible names are rejected.

This protects the native chrome metadata from accidental deletion during QML
edits. It is intentionally a structural guard rather than a claim that static
text proves AT-SPI behavior: screen-reader announcements, focus order, native
key/IME handling, and portal focus restoration still require a native desktop
qualification run.

## Validation

- `cargo xtask test accessibility`
- cargo xtask check --locked --offline
- cargo xtask test adapter
- workspace strict Clippy and formatting checks

The dedicated xtask entry makes the structural accessibility guard independently
repeatable. It does not replace native AT-SPI, screen-reader, focus-order, or
portal-dialog qualification.

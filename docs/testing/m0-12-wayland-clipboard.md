# Wayland clipboard evidence

Date: 2026-09-20  
Status: in-progress

## Clipboard capability boundary

Browser-owned copy operations now use the Qt clipboard object for the regular
clipboard. CLI `yank --primary` and typed IPC `yank` with `primary: true` may
request the primary-selection channel explicitly; URL, title, and selection
sources are available through the same typed path. Qt reports whether that
channel is supported both at the write boundary and in live browser
diagnostics; when it is unavailable, Rust exposes the concrete status
`Primary selection is unavailable` and does not silently write to the regular
clipboard.

Selection copy preserves the requested target until the asynchronous page
selection result returns. URL, clean-URL, hint, and page-selection requests
also carry their sensitivity and target-channel metadata to the QML surface,
so sensitive content remains masked in the notice while the requested channel
is preserved.

Clipboard reads used by `p`/`P` remain user-gesture initiated and use the
regular clipboard. They do not consume website clipboard permission requests,
poll the clipboard, or persist its contents.

The explicit `paste-open [--target current|tab] [--primary]` command reads the
selected clipboard channel only when requested. An unavailable or empty primary
selection produces a bounded status message and does not start navigation;
ordinary `paste-open` continues to read the regular clipboard in the current
tab by default.

## Verification

```text
cargo test -p ferric-browser-engine-qt --locked --offline
cargo fmt --all -- --check
cargo xtask check
cargo build -p ferric-browser --locked --offline
```

Automated coverage includes typed primary-selection command encoding, the
clipboard navigation validation boundary, and diagnostics tests that preserve
distinct available/unavailable primary-selection facts. Live native Wayland
coverage for primary-selection transfers, IME, drag/drop, smooth scrolling, and
keyboard layouts remains required.

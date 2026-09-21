# M0-29 accessibility and international input evidence

Date: 2026-09-19  
Status: in-progress

## Task card

- Requirement: A11Y-004.
- Files: `crates/ferric-browser-engine-qt/qml/Main.qml`,
  `crates/ferric-browser-engine-qt/src/lib.rs`, and `crates/ferric-browser-core/src`.
- Observable result: normal-mode browser commands consume a bounded logical
  key string only when Ctrl, Alt, and Meta are absent. Shift-produced logical
  bindings remain available, while modified combinations—including AltGr's
  Ctrl+Alt form—are not mistaken for browser commands. Empty/control-bearing
  text is not passed to the binding resolver. Native editable fields retain
  Qt's Unicode input path for search, paths, titles, clipboard, and IME text.

## Verification

```text
cargo fmt --all -- --check
cargo test -p ferric-browser-engine-qt -p ferric-browser-core --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo xtask check
QT_QPA_PLATFORM=wayland timeout 10s target/debug/ferric-browser --temp-basedir
```

The static QML test covers the logical-key boundary and editable-text role;
core tests cover Unicode-aware command and completion data. The Wayland smoke
is expected to end with timeout status 124 after remaining alive with no QML
startup error. On 2026-09-19, `cargo xtask test accessibility` passed its
browser-owned QML accessibility contract check; this remains structural
evidence, not screen-reader traversal evidence.

## Remaining matrix qualification

Native evidence is still required for a US layout, a non-US layout with AltGr
and dead keys, at least one CJK IME, layout switching during Ctrl/Shift input,
rich page editors after hint activation, bidirectional chrome text, clipboard
round trips, pointer/drag input, autorepeat, and screen-reader traversal. No
global key grab or compositor-level key injection is used.

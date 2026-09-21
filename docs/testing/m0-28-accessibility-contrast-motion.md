# M0-28 accessibility contrast and motion evidence

Date: 2026-09-16  
Status: in-progress

## Task card

- Requirement: A11Y-003.
- Files: `crates/ferric-browser-config/src/lib.rs`,
  `crates/ferric-browser-engine-qt/src/diagnostics.rs`,
  `crates/ferric-browser-engine-qt/src/lib.rs`, and
  `crates/ferric-browser-engine-qt/qml/Main.qml`.
- Observable result: semantic theme colors are assessed as rendered pairs
  against the WCAG AA 4.5:1 normal-text threshold. Builtin colors pass; an
  imported theme remains usable but exposes failing pair names and a warning
  through diagnostics and the native diagnostics surface. Security surfaces
  remain opaque and state is described textually as well as visually.

## Motion and scaling

The current chrome defines no optional animation transitions. The validated
`ui.reduced_motion` setting uses a conservative effective policy: `on` keeps
optional motion disabled, `off` permits future optional motion, and `system`
uses a bounded worker-backed `gsettings` probe for
`org.gnome.desktop.interface enable-animations`. Unknown or unavailable
preferences keep optional motion disabled. Chrome dimensions continue to
derive from `FontMetrics`, and font sizes remain bounded by validated config.
System font scaling uses a second bounded worker-backed `gsettings` probe for
`org.gnome.desktop.interface text-scaling-factor`; accepted values from 0.5 to
3.0 scale the validated chrome font size, while unavailable values fall back to
1.0.

## Verification

```text
cargo fmt --all -- --check
cargo test -p ferric-browser-config -p ferric-browser-engine-qt --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo xtask check
QT_QPA_PLATFORM=wayland timeout 10s target/debug/ferric-browser --temp-basedir
```

The focused unit tests cover the builtin pass case, an imperfect user theme,
alpha compositing, diagnostics exposure, effective post-normalization QML
contrast checks, bounded reduced-motion and font-scale parsers, and the
reduced-motion QML gate. The
Wayland smoke is expected to end with timeout status 124 after remaining alive;
the empty log indicates no QML startup error. Rendered-pixel contrast review,
AT-SPI/screen-reader qualification and final rendered-pixel capture review
remain manual follow-up.

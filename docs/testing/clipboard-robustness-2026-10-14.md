# Clipboard robustness qualification — 2026-10-14

Base: `948c06d851922e5d282a80d544a86bd8a51ac424`; branch: `fix/robust-clipboard`.
Implementation decision: [ADR-0018](../architecture/ADR-0018-native-clipboard-ownership.md).

## Defects and resulting behavior

- Normal Ctrl+V previously entered Pass-through. Clipboard keys now reach native
  editors by default; ;V enters persistent Pass-through and ;v passes one key.
- Context-menu Copy previously used the parent document selection for child
  frames and discarded HTML. Native editing preserves engine-supported formats;
  explicit plain-text operations use a verified native frame and selection.
- Configured modified shortcuts were split into printable characters. Runtime
  bindings and binding explanations now share bounded gesture tokenization.
- Qt plain text fields omit Ctrl+Shift+V. The shared native field supplies this
  equivalent paste gesture with focus, visibility, enabled and read-only guards.
- Popup menu edits resolve the claimed popup target, rather than the opener's
  active tab; source validation includes the popup document and native frame.
- Browser-owned repeats cannot fall through into paste after a pending count,
  keychain or configuration changes. Macro-register entry retains ownership.

Selection leases survive browser-owned focus handoffs and reject changed,
ambiguous, detached or expired sources. Password and unsupported opaque sources
fail without copying a remembered parent selection. Native editing and bounded
plain-text extraction remain distinct operations. Sensitive copy notices clear
retained values when hidden; native actions report requested, not applied.

## Reproduction and automated coverage

Use disposable profiles; the installed browser and normal profiles are not test
fixtures. Local investigation artifacts under `target/clipboard-investigation/`
are ignored build outputs, not release inputs.

- `cargo xtask test application-clipboard`: production Main.qml, real Qt input,
  65 native editing combinations, actual context menus in same/cross-origin
  frames, rich MIME, command chrome, secondary/popup windows, selection
  handoff/cancellation, Unicode,
  protected fields and 200 repeated source cancellation/recovery cycles.
- `cargo xtask test clipboard-wayland`: private nested Weston, independent
  wl-copy/wl-paste clients and compositor-delivered input. Every injection checks
  that the private Weston PID owns the parent compositor's focused window.
- `cargo xtask check --locked --offline`: repository checks, generated documents,
  QML lint, WebEngine/QML tests and workspace tests. Run with
  `QTWEBENGINE_CHROMIUM_FLAGS='--disable-gpu --js-flags=--expose-gc'`.
- Strict workspace Clippy, formatting, adapter smoke and embedded resource parity
  are required alongside the clipboard production suite.

## Final local results

Production source commit: `3eacaff`; regression/CI commit: `fc9b26b`.
The release build and tests use Qt 6.11.2 on this Linux/Wayland machine.

| Check | Result |
| --- | --- |
| Release browser and Qt adapter build (`--locked --offline`) | Passed |
| Production `application_clipboard` CTest suite | 81 passed, 0 failed, 0 skipped |
| Embedded resource parity and shared native-frame regressions | 29 passed, 0 failed |
| Private Wayland transport | Four cross-process exchanges passed with compositor-delivered input |
| Repository gate and workspace tests | Passed; QML suite: 196 passed |
| Strict workspace Clippy (`--all-targets -- -D warnings`) | Passed |
| Formatting and `git diff --check` | Passed |
| Debug Qt/QML adapter startup smoke | Passed (bounded 10-second lifetime) |

The four transport checks cover native page Copy to an independent reader,
browser URL yank to an independent reader, an independent writer to native page
Paste, and an independent writer to command-bar Paste. The focused production
suite is now required in GitHub Actions; that remote run has not been performed
as part of this local qualification.

Release artifact: `target/release/ferric-browser`; SHA-256:
`6287882233154af417d710063bae721201bc839c374068206c7da9577bc9453f`.
The artifact was built for review and testing; the installed browser and normal
profiles were not replaced or modified.

## Evidence limits

Qt-generated keyboard events are insufficient to qualify repeated Wayland
selection writes: they do not supply fresh compositor input serials. The native
runner therefore uses verified compositor-delivered input and never substitutes
an offscreen pass for a missing seat or transport.

The native runner requires Weston, wl-copy/wl-paste and wtype on a Wayland
session. Parent-window ownership verification currently supports Hyprland and
Sway. It refuses input when ownership cannot be verified or the private Weston
window loses focus; other parent compositors remain unqualified by this runner.

Physical keyboard/layout/IME testing, primary selection across supported desktop
compositors, and arbitrary live editors remain separate qualification. A Qt
clipboard write acknowledgement means adapter acceptance, not proof that every
external clipboard client received the data. Existing Qt/QML engine-destruction
warnings observed in the baseline are not clipboard success evidence.

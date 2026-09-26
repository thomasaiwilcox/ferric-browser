# M0-22 internal browser surfaces evidence

Status: implemented and regression-tested, updated 2026-09-25.

## Scope

Existing management views are explicitly browser-owned Qt Quick surfaces. Site
Ledger/Doctor, history/bookmarks/quickmarks library views, the universal
switcher, downloads, profiles, named sessions, session/profile previews, and
clean-link previews now share the command-first `FerricModalSurface` contract.
They expose a visible command name and stable accessible dialog name, block page
input, use window-scoped Escape even if WebEngine retained focus, and restore
the captured browser target. Simple decisions additionally use
`FerricCommandDialog`: a safe initial row, `j`/`k` and arrow navigation, Enter,
direct action keys, and the same mouse-selectable rows used by shutdown. Nested
previews restore only the focus layers they opened. Their content comes from
native Rust/IPC models and sanitized payloads; no privileged WebChannel is used
to render arbitrary page content.

The blank initial page remains `about:blank`, so it issues no network request.
Dedicated searchable help, settings, diagnostics, permissions, WebAuthn,
certificate, page-dialog, screen-sharing, renderer-recovery, and external-URI
surfaces use the same contract. Native portal pickers and non-modal hints,
capture state, and transient notices remain intentionally specialized.

## Verification

```text
cargo xtask check
cargo test -p ferric-browser-engine-qt --lib
QT_QPA_PLATFORM=wayland timeout 10s target/debug/ferric-browser --temp-basedir
```

The runtime smoke is expected to end with timeout status 124 after staying
alive for the full interval. Qt Quick interaction tests cover window-scoped
Escape, `j`/`k`, Enter, direct action keys, safe default selection, and mouse
activation. Full AT-SPI screen-reader traversal remains manual qualification.

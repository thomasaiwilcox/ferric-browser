# M0-22 internal browser surfaces evidence

Status: in progress, implementation slice recorded 2026-09-16.

## Scope

Existing management views are explicitly browser-owned Qt Quick surfaces. Site
Ledger/Doctor, history/bookmarks/quickmarks library views, the universal
switcher, downloads, profiles, named sessions, session/profile previews, and
clean-link previews now expose dialog roles and stable accessible names. They
take focus while visible, close through Escape or a named action, and restore
the captured browser target. Nested previews restore only the focus layers they
opened. Their content comes from native Rust/IPC models and sanitized payloads;
no privileged WebChannel is used to render arbitrary page content.

The blank initial page remains `about:blank`, so it issues no network request.
Dedicated searchable help, settings, and diagnostics managers are also
browser-owned surfaces. The later journey graph remains a separate surface
still under implementation.

## Verification

```text
cargo xtask check
QT_QPA_PLATFORM=wayland timeout 10s target/debug/ferric-browser --temp-basedir
```

The runtime smoke is expected to end with timeout status 124 after staying
alive for the full interval. Interactive keyboard and assistive-technology
traversal of each management surface remains manual follow-up work.

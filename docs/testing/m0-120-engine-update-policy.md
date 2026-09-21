# Engine version tracking and update authority

Date: 2026-09-17  
Status: in-progress

## Task card

- Requirement: UPDATE-001.
- Files: `packaging/engine-version-policy.toml` and
  `crates/ferric-browser-engine-qt/src/diagnostics.rs`.
- Observable result: diagnostics compare the runtime Qt version with the
  versioned qualified set and distinguish `qualified`, `unqualified`,
  `blocked`, and `unavailable` states.

The browser does not infer Chromium security-patch status from a Qt or
Chromium base version. The running QtWebEngine library now exposes both
authoritative values through its public runtime APIs, while the release
record still requires a maintainer-led advisory review before drawing a
security conclusion. The browser never silently runs pacman/yay, changes
repositories, downgrades Qt, or updates itself; the system package manager
remains the update authority.

At GUI startup, the same policy state is surfaced in the browser status bar:
blocked builds are marked as requiring a system-package-manager update and
parseable but unqualified builds are clearly labeled for verification. A
qualified build adds no warning; no user-agent spoofing is used to change the
qualification result.

## Verification

```text
cargo test -p ferric-browser-engine-qt diagnostics --locked --offline
cargo xtask check --locked
ferric-browser diagnostics --format json
```

# M0-35 context menus and spellcheck

Date: 2026-09-20  
Status: in-progress

## Task card

- Requirement: FILE-006.
- Files: `crates/ferric-browser-engine-qt/qml/Main.qml` and
  `crates/ferric-browser-engine-qt/src/lib.rs`.
- Observable result: Qt WebEngine context-menu requests are accepted by the
  browser and represented by one native, accessible menu surface. The menu
  exposes supported link, media, selection, editing, spelling, and inspect
  actions through the engine's public `triggerWebAction` and
  `replaceMisspelledWord` APIs.

Menu state is tied to the originating view and is cleared on navigation,
close, and view destruction. Link/media URLs used for copy are sanitized to
remove userinfo, fragments, and obvious secret query keys. Download actions
use the existing browser download boundary and do not write directly to a
page-selected path. Untrusted labels and spelling suggestions are bounded and
control-character sanitized.

Normal and private profiles receive the validated spellcheck enabled flag and
language list. The `system` language token resolves to the current Qt locale.
The configuration boundary now accepts at most 16 unique, structurally checked
BCP 47 tags (including script, region, extension, numeric-variant,
private-use, and registered grandfathered forms such as `i-klingon`, plus the
exact lowercase `system` token), rejecting malformed separators, underscores,
control characters, and duplicate tags before values reach Qt. The QML
dictionary filter accepts the same bounded multi-subtag shape and grandfathered
set, so valid configured tags do not disappear before reaching the WebEngine
profile.
Rust now performs a bounded, path-redacted `hunspell -D` inventory probe,
filters configured languages when an inventory is available, and surfaces
missing or unavailable dictionary status in Settings. The browser never
downloads dictionaries as a side effect of configuration.

## Verification

```text
cargo fmt --all -- --check
cargo test -p ferric-browser-engine-qt --locked
cargo xtask check
cargo build -p ferric-browser --locked
QT_QPA_PLATFORM=wayland timeout 10s target/debug/ferric-browser --temp-basedir
```

The smoke is expected to end with timeout status 124 after remaining alive
with an empty startup log. Spell replacement with installed Hunspell
dictionaries, portal-native positioning, link/media fixture actions, and
desktop inspect qualification remain manual work.

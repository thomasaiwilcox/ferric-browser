# S15 implementation outline and evidence map

## Initial state

- Slice starts from `700a8092831369d1f5bdbe6781321988a5a2ae09` on
  `implementation/keyboard-interaction-v03`; S14 is recorded as
  `implemented_unqualified`.
- The worktree already contains unrelated untracked `aqtinstall.log` and
  `tools/`; they are excluded from this slice.
- Existing toolchain is Rust 1.98.0 and Qt 6.11.2. No toolchain or dependency
  changes are planned.
- S14 provides the weak page-scope handoff and HintWithin machinery. S15 adds a
  separate GridWithin handoff for eligible region and surface candidates.

## Intended changes

- Separate the captured WebEngineView viewport, interaction root and current
  cell in `SpatialSession`, retaining the legacy full-view Grid geometry.
- Acquire a bounded weak scope lease from the exact selected candidate; bind
  it to the local one-use palette permit, owner, target, configuration,
  selector policy, source session, family, mode and parent lease.
- Project scope geometry through same-origin nested frames and borders, clip to
  the page viewport, include measured CSS/browser zoom, and reject transforms
  that cannot be mapped safely. Cross-origin iframe internals remain opaque.
- Map top-document CSS coordinates once into WebEngineView-local Qt logical
  coordinates. Leave DPR and window placement to the existing native adapter.
- Revalidate the scope identity and geometry before dispatch, retain ordinary
  repaint behavior, and clear the lease on cancellation, owner/revision drift,
  surface replacement or invalid geometry.
- Keep the overlay passive and express it against the full viewport while
  drawing the interaction and current rectangles at their scoped origins.

## Required test-to-path map

| Test | Planned production evidence |
| --- | --- |
| T010 | Rust local permit and operation checks plus the GridWithin admission path; report whether a composed native palette-to-page journey exists. |
| T035 | WebEngine fixture proves only the outer cross-origin iframe surface is available for raw Grid; native Wayland scope remains separately qualified. |
| T036 | Rust exact-receipt/owner/session/configuration checks and WebEngine weak-lease transfer and teardown tests. |
| T038 | Core rectangle conversion and WebEngine nested-frame/border/zoom geometry tests; native adapter and Wayland levels are reported only if exercised. |
| T039 | WebEngine scroll/reflow and transform invalidation plus scoped Rust reset/hover behavior; report native key journey availability and preserve ordinary repaint. |
| T045 | Full engine/QML/native-insert gate and repository check preserve legacy full-view Grid and interaction behavior. |
| T055 | Engine unit and generated resource/JSON boundary checks keep script contracts and documentation aligned. |

## Validation and qualification

Run `cargo fmt --all -- --check`, the locked/offline engine suite, full
`cargo xtask check`, the available native Wayland command, and pack/evidence
validation. Keep the exact commands, outcomes and log paths in `evidence/S15.json`.
The architecture JSON-contract allowlist is outside the S15 anchor list; record
its exact path and necessity as a C23 scope amendment. Do not claim native
Wayland or a composed BrowserUi palette journey unless those exact flows pass.

## Observed result

The implementation adds scoped GridWithin admission through the existing local
palette flow, binds the one-use permit to the candidate receipt and live page
owner, and keeps full-view Grid geometry unchanged. Scoped reset, hover, and
history use the captured interaction root. The QML overlay remains passive and
uses the full WebEngine viewport as its coordinate basis.

`crates/ferric-browser-engine-qt/src/browser_ui_scope_handoff.rs` is the small
new module allowed by S15. It holds the tightly related scope receipt, handoff,
and capability helpers extracted from the palette module to keep both
implementation modules within the repository's 1,000-line limit.

The WebEngine fixture covers a nonzero scoped rectangle through nested
same-origin frames, borders, browser and CSS zoom, rejection of unsupported
transforms, and the parent-visible outer surface of a cross-origin frame. Its
scroll revision test changes the exact scope's scroll position and explicitly
dispatches a scroll event: Qt's offscreen WebEngine does not emit the event for
a JavaScript `scrollTop` assignment by itself. The production listener remains
attached to the scope and participating documents/windows.

The final engine run passed 433 Rust tests, 138 QML tests, and native-insert
CTest (1/1). `cargo fmt --all -- --check` and `git diff --check` passed. The
repository-wide check passed all workspace Rust tests, QML 138/138,
release-manifest and license checks, formatting, and generated resource and
documentation checks. The nested Wayland browser startup smoke passed, but its
editable-page input qualification could not run because Weston does not expose
the virtual-keyboard protocol required by `wtype`. Neither native suite
composes the local palette through a final scoped page dispatch, so the S15
native journey levels remain unqualified.

The exact C23 scope amendments are recorded in `S15.json`. They cover
`browser_ui_hint_selection.rs` and `browser_ui_navigation.rs` for local
GridWithin permit routing and exact handoff validation;
`PageInteractionScripts.js` and `BrowserScripts.js` for candidate-bound scope
measurement, lifecycle revisions, and script versioning; both Rust `lib.rs`
files for exporting/ registering the scoped API; and
`docs/architecture/qml-json-contracts.md` for the bounded opaque geometry JSON
strings at the script boundary. Each is required by the existing scoped
palette-to-page call chain and does not change the slice's authority model.

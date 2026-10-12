# S14 implementation outline and evidence map

## Initial state

- Slice starts from `1421b54c19b7ba9db526bd13827ac4dc6196efb4` on
  `implementation/keyboard-interaction-v03`; S13 is recorded as
  `implemented_unqualified`.
- The worktree already contains unrelated untracked `aqtinstall.log` and
  `tools/`; they are excluded from this slice.
- Existing toolchain: Rust/Cargo 1.98.0 and Qt 6.11.2. No toolchain or
  dependency changes are planned.
- The S13 capability table already defines `HintWithin` for Region and
  ScrollableRegion, while the UI deliberately disables it pending this slice.
  SearchWithin and GridWithin remain disabled for their later slices.

## Intended changes

- Add one scoped weak lease to the existing ApplicationWorld Hint state. It
  retains only WeakRefs to the exact eligible region and its owner document,
  plus bounded identity/revision facts. The source candidate map remains live
  until that lease is verified; then the source map and its observers are
  released before scoped collection starts.
- Bind the handoff to the active window, Hint session, exact candidate,
  document, configuration revision, selector-policy revision, and one-use
  local palette permit. A missing WeakRef implementation fails closed.
- Add descendant-only Hint collection rooted at the leased region. Exclude
  the region itself; traverse its open shadow roots and same-origin frames;
  reject stale, replaced, empty, or cross-origin semantic scopes without
  falling back to a full-page collection.
- Revalidate lease identity and candidate freshness during collection and
  final selection. Cancel, navigation, configuration/document change, and
  replacement clear the lease. Preserve focus and scroll; route the explicit
  scroll-target action through the existing setter.
- Enable only HintWithin for Region and ScrollableRegion in the existing
  Rust-described palette. SearchWithin and GridWithin remain unavailable.
- Update the generated command guide and `docs/TESTING.md` with the new local
  action and its weak-reference, boundary, and teardown evidence.

## Required test-to-path map

| Test | Planned production evidence |
| --- | --- |
| T010 | Rust local-permit and UI action tests plus native Qt offscreen coverage show that a local palette can request the scoped handoff once; IPC, CLI, aliases, macros, repeat, userscript, and generic executor routes cannot mint or replay it. |
| T020 | ApplicationWorld WebEngine tests hold the source map until exact lease acquisition succeeds, then verify source cleanup, weak-only retention, stale callback rejection, and single teardown of observers/listeners. |
| T035 | WebEngine tests prove exterior-only cross-origin visibility and rejection of semantic within for inaccessible children. Physical native Wayland evidence is recorded only if an existing physical journey is available. |
| T036 | Rust session/owner tests and WebEngine handoff tests cover one session per page/window, exact source-record retention until destination validation, and cleanup on cancellation or replacement. |
| T037 | WebEngine fixtures cover descendant-only collection, root exclusion, empty-scope failure without fallback, no focus/scroll changes, and the existing Set scroll target behavior. |
| T044 | Repeated create/cancel tests exercise 1000 handoffs and bounded registry/listener state. Forced-GC evidence is claimed only if the existing Qt WebEngine test runtime exposes a supported deterministic GC hook. |

## Validation and qualification

Use the repository's existing locked/offline `cargo xtask check` gates and
engine test command, including the registered native CTest suite when
available. Run focused QML geometry, webapp interaction, and native-insert
tests before the full gates. Validate evidence and pack integrity with
`python3 tools/validate_pack.py --integrity --evidence` from the pack directory.

This slice does not implement SearchWithin or GridWithin and does not change
the default Hint family. No physical Wayland journey is assumed; absent such a
journey, T035 remains explicitly unqualified at that level. Do not claim forced
GC if the engine cannot expose it. Commit implementation and its evidence as
separate commits on the existing branch; do not push or merge.

## Observed result

- The locked offline engine suite passed: 432 Rust tests, 133 WebEngine QML
  tests, and native-insert CTest 1/1. The repository-wide `cargo xtask check`
  also passed, including QML lint, generated-document checks, and workspace
  tests.
- WebEngine tests passed for transfer ordering, cross-origin and closed-root
  rejection, descendant-only collection, empty/replaced scope failure, and
  preservation of focus, scroll, and the existing scroll-target setter.
- The forced-GC stress test completed 1,000 scope lifecycle cycles in 1,087 ms in the engine run and 1,111 ms in the repository check,
  with zero retained element records, observers, or window listeners; the live
  weak target remained usable afterward.
- The T036 Rust admission test passes exact receipt, owner, source session,
  parent lease, selector revision, family, and mode checks. T010 native
  palette-to-page acceptance and the physical Wayland portion of T035 were not
  run; these gaps keep S14 `implemented_unqualified`.

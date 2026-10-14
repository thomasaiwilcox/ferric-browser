# S16 implementation outline and evidence map

## Initial state

- Slice starts from `f732ba30c4b13d010dc042a41f252477512d0188` on
  `implementation/keyboard-interaction-v03`; S15 is recorded as
  `implemented_unqualified`.
- Unrelated root `aqtinstall.log` and `tools/` are present and will remain
  untouched.
- Existing Rust 1.98.0, Qt 6.11.2 and locked dependencies remain in use.
- `HintInteraction` already implements bounded text-mode ranking using the
  shared switcher tokenizer/ranker. S16 adds a direct local Target Search view
  over that model and the existing Hint registry.

## Intended changes

- Add `target-search [links|controls|regions|surfaces|all]` with no query
  argument or new default key binding; reject remote/CLI invocation.
- Present visible candidates in a native palette, keep query and selection
  ephemeral, require Enter to confirm, and reuse the existing action palette
  for regions/surfaces.
- Reuse `HintInteraction` and `switcher_rank`; preserve the captured candidate
  identity and action meaning across refresh, clearing selection when that
  exact record disappears or changes meaning.
- Keep the page collector visible-only and bounded, coalesce live refresh,
  deduplicate only exact candidate identities, and never scroll the page or
  inherit a virtualised replacement's selection.
- Keep SearchWithin descendant-only and local-permit-bound. Maintain the
  existing action, transport, focus, and scroll authorities.
- Record any source paths outside the S16 write boundary as exact C23
  amendments in `evidence/S16.json`.

## Required test-to-path map

| Test | Planned production evidence |
| --- | --- |
| T010 | Local palette/command provenance, generic executor and IPC/CLI negative paths; report whether a composed native journey exists. |
| T020 | WebEngine lease handoff retains source candidates until validated destination scope acquisition and tears down exactly once. |
| T037 | Descendant-only SearchWithin, no empty-scope fallback, unchanged focus/scroll, and existing scroll-target semantics. |
| T040 | Rust Target Search reuses HintInteraction ranking; QML palette explicitly confirms and does not auto-activate. |
| T041 | Virtualised removal/action-meaning changes clear selection by identity; no row-index retargeting or offscreen promise. |
| T042 | Query/labels remain transient; labels avoid field values and credential-bearing URLs; no query/history persistence. |
| T045 | Full engine, QML and native insert regression. |
| T055 | Command registry, codecs/help, generated docs and resources remain aligned. |

## Validation and qualification

Run the relevant focused Rust and WebEngine fixture suites, full engine suite,
`cargo xtask check`, native smoke where useful, formatting, diff checks and pack
evidence validation. Keep exact commands and logs in `evidence/S16.json`. Do
not claim a live native page action until the exact palette-to-page journey is
exercised.

## Observed result

Implementation was committed as `ec425d4ed3fe38ef0fee92729754ded1c493c8ad`
from baseline `f732ba30c4b13d010dc042a41f252477512d0188`. Target Search now
opens a local native palette, uses the existing Hint text matcher, collects
visible candidates on demand, requires explicit confirmation, and rejects
expanded IPC commands. Region SearchWithin uses the existing weak scope lease
and descendant collector; selection is reconciled by DOM identity and action
meaning, not row position.

The final `cargo xtask check` passed the complete workspace suites (including
438 engine tests), all 140 QML/WebEngine tests, artifact-manifest and license
checks. The dedicated engine task passed 437 engine tests, all 140 QML tests,
and native-insert CTest 1/1. `cargo xtask docs` regenerated command,
configuration, binding, and action references cleanly. Logs and the exact C23
scope amendments are recorded in `evidence/S16.json`.

The slice remains `implemented_unqualified`: no composed native BrowserUi
Target Search palette-to-page journey is available, so T010's
`native-qt-offscreen` evidence is not claimed.

# S04 implementation plan

## Threat model and trust boundary

The protected action is one native left-pointer sequence directed at one
retained Hint candidate in the active WebEngine view. The page is untrusted and
can replace, move, disable, hide, cover, or adopt that object after discovery.
Other untrusted inputs include command text, aliases, IPC/CLI, macros, repeat,
history replay, userscripts, and QML callback payloads. The browser window,
active tab/document generation, effective Hint configuration revision, native
surface stamp, and one-shot local permit form the owner identity.

Authorization begins only at the direct local commandbar or a physical
keyboard binding whose resolved command itself is `hint --target pointer
FAMILY`. Generic execution, aliases expanding into Hint, IPC/CLI, macro,
repeat, history, and action-executor paths must reject Pointer before changing
mode or starting script work. A label click only selects/highlights. A fresh
Enter press and its matching release precede preflight. Preflight revalidates
the retained element and its original owner document, tests exact identity and
hit eligibility, and returns only bounded geometry. Host state checks owner,
configuration, visible surface, expiry, and the one-shot permit before handing
one request to the existing `FerricPagePointerAdapter`.

The browser page and native adapter do not share an atomic transaction. A page
can race after the last DOM check, Qt delivery can reach a different render
target after a change, and `isTrusted` or application-level success is not
guaranteed. An uncertain/rejected/receiver-gone result is terminal: never retry,
replay, or fall back to `element.click()` or URL navigation.

## Write boundary and scope amendments

In-scope anchors: `hint_policy.rs`, `browser_ui_navigation.rs`,
`browser_ui_hint_selection.rs`, `BrowserScripts.js`,
`FerricBrowserRuntimePresentation.qml`, `FerricBrowserRuntimeChrome.qml`,
`FerricBrowserRuntimeSurface.qml`, `FerricBrowserRuntimeBase.qml`,
`FerricBrowserWindow.qml`, the existing native-insert fixture, and
`docs/TESTING.md`.

Scope amendments required by the inspected call chain:

- `browser_ui_input.rs` and `browser_ui_command_dispatch.rs`: distinguish the
  two direct local entry points from the generic command executor and reject
  alias-expanded or replayed Pointer commands before effects.
- `browser_ui_ipc.rs`: reject the new target at the remote boundary, including
  commands expanded from aliases.
- `lib.rs`: declare bounded native-action properties and callback invokables;
  these are needed for the QML-to-Rust owner/lease handoff.
- `browser_page_pointer_adapter.h/.cpp` and
  `tests/native-insert/CMakeLists.txt`: expose a dispatch watchdog outcome that
  distinguishes uncertain delivery and compile the existing adapter into the
  native fixture; no second adapter is introduced.
- Existing Rust/QML/native fixture tests and generated user help/resources:
  verify local-only grammar, key-release ordering, preflight rejection,
  pointer-only activation, and documentation parity.
- `docs/architecture/qml-json-contracts.md` and its boundary test: record the
  bounded preflight result and one-shot native request as opaque transport
  contracts, not presentation or policy state.
- `crates/ferric-browser-core/src/guide.rs` and generated
  `docs/user/commands.md`: publish the local-only Pointer syntax and explicit
  Enter-release behavior through the authored Guide source.
- `crates/ferric-browser-engine-qt/src/browser_ui_navigation.rs`: move the
  existing generic Hint command handler out of the 1,000-line dispatch module
  so its new pre-effect Pointer-chain guard remains within the module budget.
- `docs/implementation/keyboard_interaction-v03/tools/test_validate_pack.py`
  and `SHA256SUMS.txt`: make the existing “complete without evidence” negative
  test explicitly clear S00's inherited evidence path, then refresh its
  integrity entry so the validator self-test checks the intended failure.
- A small `browser_ui_hint_pointer.rs` module may be added if the state machine
  does not fit the existing selection module's line budget. It would own only
  local request lifecycle/terminal handling; this avoids expanding the public
  serialized Hint schema.

## Implementation invariants

- `INVARIANT:` local Pointer authority is minted only for direct UI or physical
  keyboard entry and binds window/session/tab/generation/document/configuration.
- `INVARIANT:` Enter release must be observed before preflight begins; while a
  native request is pending, Escape cancels and other Hint input is consumed.
- `INVARIANT:` a preflight is bounded to 250 ms, exact-retained-target checked,
  and dispatched at most once through the existing adapter.
- `INVARIANT:` a terminal or uncertain outcome clears request state/maps once;
  no DOM click, URL action, retry, or event replay follows it.
- `INVARIANT:` CSS-to-Qt mapping uses measured viewport/view dimensions and
  qualification tests; device-pixel ratio is never applied as a second scale.

## Evidence still required

T010–T014, T020, T045, and T055 will be recorded against their required levels.
Offscreen QTest event injection is not physical Wayland evidence. If the
Wayland test cannot be run with physical input, it remains `not_run` and the
capability remains `implemented_unqualified`. Residual JS/native targeting and
engine activation restrictions will be reported without a trust or success
claim.

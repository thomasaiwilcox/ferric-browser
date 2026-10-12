# S10 implementation outline and evidence map

## Lifecycle policy

Native Qt dialogs and visible Qt menus use short-lived, per-`BrowserUi` leases
started and completed by their real QML lifecycle hooks. A lease binds the
window, tab, WebEngine view identity, document identity, saved mode, expected
post-suspension input epoch, configuration revision, and shortcut-policy
revision. Nested prompts are LIFO and capped at depth eight. Out-of-order,
stale, owner-changed, or manually changed state leaves the browser in Normal
mode and never restores a transient Hint, Grid, Caret, or Pass Next Key state.

Pass Through can return when the exact owner, document, epoch, and revisions
still match. Insert and Insert Locked additionally require a fresh post-close
focus probe of the exact previously editable frame/context. This probe is
completed from WebEngine JavaScript callbacks; no timer estimates dialog
completion. While a prompt surface remains visible, the QML key route leaves
original Qt input untouched before Mixed policy or browser shortcuts run.

The small `browser_ui_native_prompts.rs` module holds the BrowserUi lease
coordination extracted from `browser_ui_tab_state.rs`; it is specific to this
slice and keeps both modules below the repository's 1,000-line implementation
budget. `native_prompt_lease.rs` holds the bounded pure lease state machine.

The read-only `interaction.status` projection bounds focus observations to 16
numeric frame paths and reports shortcut policy provenance, native prompt
category/depth, and scroll policy as unknown when its page-local state is not
retained. It does not return URLs, titles, page labels, DOM identifiers, or raw
key mappings.

## Per-case capability result

| Case | Result | Evidence |
| --- | --- | --- |
| Lease nesting and overflow | LIFO nesting completes safely; depth above eight clears the stack. | `native_prompt_lease` unit tests |
| Owner, epoch, configuration, and policy changes | Restoration is rejected; manual mode changes clear the lease. | `native_prompt_lease` unit tests and BrowserUi control-flow review |
| Persistent and editing restore candidates | Only Pass Through, Insert, and Insert Locked qualify; editing requires the exact editable frame/context. | `native_prompt_lease` unit tests and QML focus probe |
| Keyboard chooser acceptance/cancellation | Qt Quick FileDialog accepts through keyboard focus/activation and cancels with Escape. | `test_native_prompt_file_chooser_accept_and_cancel_by_keyboard` |
| Native key ownership during a prompt | Original keydown/up pass through without Mixed routing. | `nativePromptInputBypassesMixedRouting` and production guard boundary test |
| Whole interaction regression | Existing hint, focus, recovery, menu, and WebEngine fixtures pass. | `cargo xtask test engine` |
| Wayland prompt journey | Not run; no physical keyboard/native Wayland chooser journey was performed. | Recorded in `S10.json` |

The offscreen fixtures exercise the Qt Quick chooser and the production key
router guard boundary, while the pure lease tests cover the ownership decision
rules. They do not compose a live BrowserUi, WebEngine tab, and native platform
dialog to prove full application restoration across tab/document/configuration
changes. S10 therefore remains `implemented_unqualified`.

## Validation

Run the focused lease unit tests, `cargo xtask test engine`, the full
`cargo xtask check --locked --offline` gate, strict workspace Clippy, formatting,
and the pack validator. Keep the initial fixture failure beside the final
passing run so the evidence shows that the keyboard chooser case was corrected
before qualification.

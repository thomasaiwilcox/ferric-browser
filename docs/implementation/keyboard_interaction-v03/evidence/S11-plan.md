# S11 implementation outline and evidence map

## Prepared selector configuration

The config crate now represents the seven existing hint families as typed
selector entries with `extend` and `replace` modes. Per-entry validation bounds
the number and UTF-8 size of selectors, rejects empty/control/duplicate values,
and applies selector overrides in stable order. The family settings are
registered through the existing runtime setting codecs, and config, profile,
site-rule, runtime, and CLI-layer candidates share the same validation and
merge path.

The Qt engine prepares candidates without publishing them, then sends their
selector strings to a small browser-owned `data:` document. The WebEngine
`document.querySelector` parser is the syntax authority. A request carries a
generation, selector-config revision, and bounded timeout; only a current
successful callback can commit the prepared candidate. A rejection, timeout,
unavailable validation surface, or stale callback leaves an existing active
configuration in place. During startup, the published config stays at built-in
safe defaults until the layered candidate is accepted; rejected selector
configuration reports an error and does not partially activate its other
settings. Runtime override persistence is deferred until syntax validation and
configuration commit succeed.

The validation harness is owned by QML and wired to both primary and secondary
browser windows. CLI config checking uses structure-only wording when selector
syntax requires an engine; GUI checking reports full validation only after the
WebEngine callback accepts every selector. No regex parser or current-page DOM
is used.

## Per-case evidence result

| Case | Result | Evidence |
| --- | --- | --- |
| Selector shape, limits, family keys, and overlay order | Unit tests cover accepted/rejected structure and stable extend/replace behavior. | Config crate tests in `xtask-check-final.log` |
| CSS syntax | Valid CSS succeeds and malformed CSS throws in the installed WebEngine. | `tst_selector_validation.qml` in `engine-tests.log` and `xtask-check-final.log` |
| Callback decision | Unit tests cover acceptance, parser rejection, generation mismatch, and config-revision mismatch. | Engine Rust tests in `xtask-check-final.log` |
| Candidate activation and persistence | Candidate settings are queued while selectors are checked; persistence happens only after successful acceptance/commit. | Implementation review; parser and helper tests |
| Complete BrowserUi lifecycle | Not run: no live fixture drives startup, unavailable engine, timeout, stale callback, and disk-write failure through a composed BrowserUi. | Recorded as a qualification limit in `S11.json` |
| Documentation and registry parity | Generated user docs and the full generated-contract gate pass. | `cargo xtask check --locked --offline` |

The selector policy is prepared and validated here; discovery and activation
consumption remain in S12. The existing QML fixture proves WebEngine syntax
acceptance and rejection but does not prove the composed BrowserUi commit
lifecycle, so S11 remains `implemented_unqualified`.

## Validation

The final gate set is strict workspace formatting and Clippy, full
`cargo xtask check --locked --offline`, and `cargo xtask test engine` for the
WebEngine fixture and native CTest target. The first broad check identified that
the configuration command module crossed the 1,000-line limit; the command was
moved into `browser_ui_runtime_settings.rs`, after which the final architecture
check passed. The first strict Clippy attempt found two small config-crate
findings; both were corrected before the final run. Initial failures are kept
beside their final passing evidence.

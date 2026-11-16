# Site Policy qualification — 15 November 2026

The crash-focused implementation pass is complete and independently reviewed.
It mitigates a confirmed Qt 6.11.2 QML JIT corruption path and fixes shared modal
Tab traversal that interfered with Library, list and Downloads navigation.
The original live crash has not been uniquely reproduced; see the
[workaround's evidence and AOT tradeoff](qt6112-qml-jit-safety.md).

The final pre-PR quality pass reviewed the uncommitted Rust, native routing and
editor changes with three independent reviewers. It found and fixed one editor
defect: adding, removing or reordering list entries could discard an unfinished
numeric edit and clear its blocking error. The recursive editor now uses the
existing error map to disable those actions until correction; explicit clearing
remains available. The regression failed before the fix and passed afterward.
The UI reviewer approved the fix, and no further actionable defects were found
in the reviewed scope.

The owner explicitly stopped further pursuit of the failed latency and
5,000-target stress gates. Historical samples remain failed or unqualified.
No performance pass is claimed for the interpreter build. Full completion of
the original platform/package plan remains open; the delivery-scope question
about the wider X11/layout matrix is awaiting the owner's answer.

## Final artifact and checks

The production build uses the qualified Qt 6.11.2 SDK, matching private headers
and owned QtWayland. The release binary is
`builds/site-policy-validation-source/target/qt611/release/ferric-browser`
relative to the parent Ferric workspace. SHA-256:
`ed87dd50ff362cb4f4a5784fadfe628ca1d95a4b7fe31efd9fd397aa3379f187`.
The linked engine archive is
`b119513d9d71a235f77c5303823765bf1c0c0ddc4f60232689c2e067cd6f9b68`.
The native test binary used for the final run is
`37927470241a40962176d61da1b9d600db312cc3cd5a9e3ca954bff7459f76da`.

Evidence paths below are under `target/site-policy-p05-revalidation/`.

| Final pre-PR check | Result | Evidence under `p17-final-quality-1/` |
| --- | --- | --- |
| Core, config, IPC, application and storage release Rust suites | 1,106 passed, 0 failed/ignored | `rust-tests.log`, `rust-results.json` |
| Full QML suite after the nested-input fix | 1,319 passed, 0 failed/skipped | `qml.log`, `qml.xml` |
| Production release rebuild and native editor/page-test/resource-parity checks | Build passed; 18 passed, 0 failed/skipped | `build-checks.json`, `native.log`, `native-results.xml` |
| Exact invalid-input regression before/after the fix | Failed before; passed after | `nested-list-red.log`, `nested-list-green.log` |

The QML suite exercises current source with the existing interpreter-enabled
harness. The subsequent production rebuild embeds that exact source, and the
native run verifies resource parity and actual editor/page-test behavior.
Source and validation-copy hashes were exact at build completion. Formatting,
diff whitespace and the schema-v2 KDE-free policy checks also passed.

Earlier crash-focused evidence remains retained with its original artifact scope:

| Check | Result | Evidence |
| --- | --- | --- |
| Production release and native build; capture, preview and embedded resource parity | Build passed; 9 checks passed, 0 failed/skipped | `p16-final-crash-fixes-1/build-record.json`, `native.log` |
| Full QML suite, including legacy modal navigation and Site Policies | 1,318 passed, 0 failed/skipped | `p16-final-qualification-1/qml.log`, `qml.xml` |
| Actual native builder keyboard and page-test workflows | 17 passed, 0 failed/skipped | `p16-final-qualification-1/native-editor.log`, `native-editor.xml` |
| Owned Wayland routing, recovery, receiver, prompt, destruction and clipboard checks | 15 passed, 0 failed/skipped | `p16-final-owned-wayland-1/record.json`, `results.xml` |

The Wayland run uses constructed Qt source events on an isolated owned compositor
with the GB layout; it does not qualify physical hardware input. The recovery
fixture explicitly reacquires its mapped PID/title surface after deliberate
focus loss. Production focus behavior and held-source assertions are unchanged.
Startup tests begin with the interpreter override unset and JIT threshold zero,
then assert that the production factory sets the fallback before UI creation.

The normal QML suite uses the existing test-only WebEngine `--expose-gc` flag.
The earlier full run's 40 failures remain in `p16-interpreter-full-qml-1`:
39 exposed the shared Tab regression, and one required the normal GC test flag.
Targeted old/new-harness comparison and the final complete run verify the fix.
No failing receipt was removed or relabelled as a pass.

The earlier `p16-jit-native-regressions-1` passed 105 native checks before the
final modal change. Earlier P14 correctness/security and P15 bounded-lifetime
evidence retains its recorded source scope; it is not a new all-platform run.
Format, schema-v2 KDE-free policy, Arch metadata/desktop validation and release
artifact/license validation passed. Metadata validation is not an installed
Arch-package test.

Independent UI and native reviewers approved the fallback, modal opt-in,
fixture focus correction and evidence limits. No unresolved defect was found
in the final delta. Source hashes were exact at build and rechecked at handover;
subsequent documentation changes are recorded separately.

## Remaining qualification

- The complete original X11/Wayland/layout/physical-input matrix is unqualified.
  Prior US/GB/DE subsets retain their narrower recorded scope.
- An installed Arch package, installed file-dialog flows and the complete
  installed accessibility matrix have not been qualified in this finish pass.
- Live YouTube sidebar and Hint-context media checks remain open. Other live
  examples were exercised on earlier binaries; no current-build live replay is
  claimed. See [live verification](../../examples/site-policies/LIVE_VERIFICATION.md).
- The original comprehensive acceptance inventory still has unmapped rows;
  missing evidence is not a pass.

Use [the user guide](../user/site-policies.md) and the opt-in examples for the
implemented workflow. The full original P16/P17 exit gates remain unchecked.

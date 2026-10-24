# Open destination routing

Status: complete and locally qualified. The broader browser usability goal remains active.

The public CLI accepted `open --target tab` but navigated the selected page. On an
owned running browser, the previous qualified release replaced `/source` with
`/new-tab`, retaining the same Core tab ID and one-tab count. The new external
process fixture reproduces this failure in `target/open-targets-cli-baseline/`.
The native fixture also fails both the ordinary tab and confirmed clean-link tab
cases against that release: two failures and two setup/cleanup passes in 36.77 s
(`target/open-targets-native-baseline.log`).

`current` now stays distinct from `tab` through CLI parsing, command encoding,
interactive parsing, typed route decoding and command-context reporting. The
unflagged `open` default remains `current`, as documented in the development
specification. Explicit foreground and background destinations use the existing
Core `tab-open` path, including its selection and journey behavior.

Clean-link foreground opens capture the destination window and create their new
tab after confirmation. Canceling the preview keeps the original tab count and
page state. Current-tab clean opens retain their captured document target. A
context-route prompt retains the requested foreground or background destination
when accepted or dismissed. Profile/context affinity is checked before execution,
including deferred new-tab confirmation.

The native fixture covers primary and secondary windows across 12 scenarios each:
current, foreground, background, clean foreground confirmation, clean cancellation,
clean current, unchanged clean URL, accepted context foreground/background,
dismissed context foreground/background, and explicit profile/context selection.
It observes actual renderer URLs, stable tab/view ownership, selection and an
unfinished JavaScript page value in the original document. Foreground and
background cases also export the committed journey graph and require an edge
from the original page to the new page.

The separate public CLI fixture launches and reaps its own browser process group,
uses disposable XDG and profile directories plus a loopback HTTP server, and
checks startup, foreground, explicit current, default current, background,
configured profile/context routing, additional URL inputs with and without an explicit
destination, and textual command forwarding. Later inputs use routed `open`
requests so profile/context selectors survive every request.
It records the executable hash and all observed tab rows. CI includes the new
native offscreen, native Wayland and public CLI gates; hosted CI has not been run.

Frozen implementation and fixture hashes and final local results are recorded
under `target/open-targets-`.

The first release build passed in 13 min 44 s, but its engine unit run found
three legacy routing/source assertions. The journey capture assertion exposed
behavior that the shared new-tab executor needed to retain, so source-node
capture now happens before dispatch and is applied to the resulting navigation.
The other assertions now follow the shared route and distinguish `current` from
`tab`. Strict Clippy also found an implicit string clone; the owned value is now
moved into its JSON envelope. Logs of this revision remain under
`target/open-targets-pre-journey-`; they do not qualify the final implementation.

The first full native run passed 23 checks and failed three secondary context
cases because the fixture created its context in `default` while its secondary
host uses `page-search`. The explicit request was correctly rejected as a
cross-profile route; accepted automatic routing correctly opened the other
profile in a window. The fixture now creates and selects the context in its
actual host profile. This correction changes no production code. The diagnostic
journey preview also now handles immediate memory-only results and asynchronous
durable readiness through the public API.

The public-process fixture uses a valid authored `contexts.toml` beside an
explicit user configuration, outside its initially empty data directory. A
separate diagnostic waits until the live context catalog contains a newly
created context and then finds that CLI `open --context` still rejects it with
`E_CONFIG` during cold-start configuration selection, before existing-owner
forwarding. That independently reproduced behavior remains a usability follow-up
(`target/open-targets-runtime-created-context/`); it is outside the destination
correction qualified here.

## Final verification

The final release build passed in 7 min 59 s. Its exact executable artifacts
passed all 1,070 Rust unit checks: Core 170, IPC 36, storage 110, runtime 9,
application 123, Qt adapter 510 and CLI 112. One optional Qt integration test
remains ignored. Strict seven-package, all-target release Clippy passed with
`-D warnings` in 457.97 s. Formatting and whitespace checks pass.

| Check | Result |
| --- | --- |
| Open destinations, offscreen | 26 passed, 110.23 s |
| Open destinations, owned Wayland compositor | 26 passed, 79.72 s |
| Public CLI journey | 10 recorded steps, 7.75 s |
| Resource parity, window routing, existing clean-link and new-window behavior | 30 passed, 87.93 s |
| Last-tab Undo and incoming-page preservation smoke | 4 passed, 4.74 s |

The native total is 86 checks. After the two native destination gates passed,
only the separate public CLI fixture changed; its user-config setup was corrected
and the remaining checks resumed. The prior and final manifests differ only in
that Python fixture. Rust and native results were retained against identical
implementation and C++ fixture sources. No QML source changed in this pass.

The final audit verifies all 688 frozen source/fixture/CI files, checks the native
CMake cache against the canonical release archive, hashes the production CLI,
archive, native runner and Rust test executables, and matches the CLI fixture's
executable hash. All three new gates occur once in the actual 33-entry CI browser
surface selection; hosted CI was not run. The owned CLI process is reaped.

Authoritative evidence is `target/open-targets-final-facts.json`,
`target/open-targets-artifact-hashes.json`, `target/open-targets-source-hashes.json`
and their linked logs. This qualification resolves explicit destination routing;
the runtime-created-context selector issue described above remains open.

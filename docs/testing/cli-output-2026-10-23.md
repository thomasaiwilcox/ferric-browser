# CLI output and closed pipes

Status: complete and locally qualified. The broader browser usability goal remains active.

A confirmed earlier public CLI probe, `ferric-browser --help | head -n 45`,
aborted in `std::io::stdio::__print` with `Broken pipe` (SIGABRT, PID 1737119).
The system-managed core was left untouched. This pass captured the previous
qualified release's normal help and executable hash without deliberately
repeating the abort (`target/cli-output-baseline.json`).

All production CLI stdout writes now use one fallible output boundary, including
help, version, config checks, desktop status, diagnostics, query/action results,
and forwarded-command acknowledgements. Each write includes its flush. A typed
local output `BrokenPipe` returns quietly from `main` with status zero. Browser
connection errors and remote failures retain their existing failure paths.
Other stdout failures report `E_IO` with status one. Output error guidance tells
the caller to check command results before retrying; already accepted browser
work is retained.

Stderr diagnostics, structured logging and the development platform warning use
best-effort writes, so an unavailable stderr cannot turn the original failure
into an abort. Configuration validation returns its summary separately from
writing it, keeping an output failure distinct from invalid configuration.
The static help header is one text block, followed by the existing generated
registry catalog. Its complete output is byte-for-byte identical to the baseline.

## Verification

The final release all-target CLI build passed in 59.81 s. All **116 CLI unit
checks** passed, including buffered flush failure, local output versus connection
error classification, sanitized public I/O errors and line formatting. Strict
seven-package, all-target release Clippy passed with `-D warnings`; Cargo reported
20.28 s. Formatting and whitespace checks pass.

The independent public-process fixture passes **32 cases** in 9.21 s including
runner overhead. Its kernel pipe has its reader closed before spawning the CLI,
so the closed-stdout cases force `EPIPE`. `/dev/full` supplies genuine write
failures. The ordinary reader journey consumes the first help line and closes.

| Cases | Count | Expected result |
| --- | --- | --- |
| Normal help, version, config checks, diagnostics and desktop status | 6 | Valid output, status 0 |
| Closed stdout, including live queries, diagnostics, command, open, focus and activate | 12 | Quiet status 0 |
| Help reader closes after the first line | 1 | Quiet status 0 |
| Full output device, including live query and accepted command receipts | 8 | `E_IO`, status 1 |
| Closed stderr for text/JSON failures and successful structured logging | 3 | Original statuses retained |
| Invalid configuration and missing instance with closed stdout | 2 | Status 2 / 3 retained |

The live cases launch an isolated offscreen browser in disposable XDG and data
directories. After the final command's stdout fails, query observes exactly four
tabs: the original and the three requested additions. The command was accepted
once, and the original tab remains present. The fixture kills and reaps only its
owned browser process group. Desktop-status output uses an owned `xdg-settings`
stub; the desktop's settings are not changed.

The existing public CLI destination journey also passes all **10 steps** in
7.58 s including runner overhead. Both fixtures report the exact final production
CLI hash. The new `application_cli_output` CTest gate is present once among the
34 entries selected by the actual CI browser-surface filter; hosted CI was not
run.

The audit verifies all **690 frozen source/fixture/CI files** and the final CLI
and unit-test hashes. It also confirms that the Qt engine archive and native
runner are byte-identical to the prior destination-routing qualification. No
engine or QML source changed, so those native suites were not repeated.

Two intermediate Clippy findings concerned help-function length and unnecessary
raw-string delimiters. They were corrected before the final build and complete
process rerun; intermediate evidence remains under `target/cli-output-pre-*`.

Authoritative local evidence is `target/cli-output-final-facts.json`,
`target/cli-output-artifact-hashes.json`, `target/cli-output-source-hashes.json`
and their linked logs. The separate runtime-created-context CLI lookup issue
remains in the usability follow-up list.

# KDE-free Wayland qualification v2

`kde_free.py` is the explicit successor to the historical everyday-v1 native
qualification schemas. Historical KF6 preflight, actual-link ordering, ABI and
runtime assertions remain required for those original schemas. They cannot
qualify the owned bridge; no synthetic KF6 success fields are accepted here.

Run `python3 tools/qualification/kde_free.py policy` for the active dependency
policy. CI and `cargo xtask check` run this check. It inspects dependency
declarations and native includes, preserving historical receipts and upstream
attribution.

Collect evidence in an ignored build directory. `snapshot` records all current
build/product inputs, including untracked source, and the starting revision.
`record --output PATH.json -- COMMAND ...` executes each real build/test/install
command, preserving its log, exit status, elapsed time and source manifests
before/after. Source changes require fresh receipts. `--artifact PATH` also
records the exact tested binary. For native suites, use CTest's `--output-junit`
and pass `--junit REPORT.xml` to record the newly produced report with zero
failures and skips and nonzero discovery.

`sdk --prefix SDK --generated BUILD_DIR` records Qt 6.11.2, the exact private
native-window header, pinned protocol provenance, scanner version and nonempty
generated build inputs. `runtime --pid PID --artifact INSTALLED_BINARY --phase
file-dialog` (and `window-mode`) inspects the actual installed process's maps and
the shared-library closure. Run with non-KDE portal backends. Do not treat a
standalone fixture as the installed product or static inspection as runtime
exercise.

Run the complete public-page-host inventory with its `qualify.py --build
HOST_BUILD --compositor BUILD/portal_compositor --report REPORT.xml`. It runs
logical component tests offscreen, the captured-command test with the owned
Wayland compositor's disposable clipboard, and compositor-delivered input on
the inherited live Hyprland session. The last scope sets the existing test's
required `FERRIC_TEST_HYPRLAND_RUNTIME_DIR`. Separate raw reports and a combined
report preserve every discovered test and its actual result. Offscreen success
is not evidence of native input or tagging. `run_owned.py` also supports selected
Wayland fixture cases; the small owned compositor does not implement every
window-manager activation policy and cannot replace the live Hyprland test.

The top-level JSON has schema `ferric.kde-free-wayland.v2`, `source` (the complete
snapshot), `sdk`, `installed_artifact`, `runs`, `runtime` and
`performance`, and `baseline` (a reference or null). Evidence references have
`path` and `sha256`. The exact required
run inventory is `REQUIRED_RUNS` in the verifier. Every source manifest must
match. Every new native, workspace, installed-product and performance gate must
pass. Functional/native receipts must
cover every scenario in the migration acceptance table, including actual Qt
recreation, non-source participant freshness and untagged fallback. Passing a
portable window test is not evidence of tagging.

The starting revision can have existing failures; retain their nonzero exit
statuses and original logs. Only the public-page-host comparison can retain a
baseline failure. Its control uses `ferric.kde-removal-baseline.v1`, the frozen
`dc9bf4f` revision, a complete run under the same qualification harness, exact
Qt library hashes, original assertion-source references and baseline executable
references. The verifier checks assertion sources against both Git and the
candidate, compares complete test inventories, requires unchanged Qt test-row
counts and zero skips, and rejects any new failing assertion or changed failure
message/location. Its final result explicitly lists preserved failures. A
baseline comparison never turns a failed command into a successful receipt and
cannot exempt the new bridge, lifetime, native freshness or runtime gates.

Performance evidence contains `candidate_source`, `pairs: 5`, `warmups: 10`,
`samples: 100` and `workloads` for startup at one/twenty windows, creation,
recreation and idle at one/twenty windows. Each workload retains at least ten
hashed raw independent process receipts, all ten warmups and 100 samples,
derived baseline/candidate summaries (latency in ms), and five baseline-only
idle repetitions for both CPU and GUI event-dispatcher wakeups. Run
`tools/performance/kde_free_pair.py` with the same test-only performance plugin
for both frozen products. It follows the existing alternating paired harness
conventions and measures actual product windows. Startup samples launch new
processes; recreation alternates real role and surface transitions. Compilation
must be quiescent. The verifier rejects latency increases
exceeding both 5% and 1 ms and idle increases above the measured baseline range;
unresolved variability is not qualification.

Finally, `verify RECEIPT.json` rejects incomplete, stale, failed or changed
evidence. This tool does not manufacture qualification receipts or substitute
for examining the recorded functional assertions, generated inputs, installed
artifact, actual backend and raw performance results.

On an Arch qualification host, `without_kde.py --inventory PATH --writable
BUILD_DIR -- COMMAND ...` runs a fresh build in an unprivileged mount namespace
with installed KDE packages' executable, library, plugin, header and discovery
files unavailable. `--hide-prefix` also masks historical local prefixes.
Networking is disabled, and the root filesystem is read-only outside declared
build/cache paths and `/tmp`. This preserves the host desktop; the inventory
records the exact masked package versions/files. It works when no KDE packages
are installed as well. This is namespace isolation, not package uninstallation.

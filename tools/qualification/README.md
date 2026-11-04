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

The public-page-host fixtures require an active native window. Run their CTest
inventory through `tests/public-page-host/run_owned.py` (under the engine crate),
passing `--compositor BUILD/portal_compositor -- ctest --test-dir HOST_BUILD ...`.
Wrap that command in `dbus-run-session
--config-file=crates/ferric-browser-engine-qt/tests/application/notification-test-bus.conf
-- env FERRIC_PORTAL_TEST_BUS=isolated ...`. This reuses the owned Qt compositor
and its automatic seat focus with isolated runtime/config/cache/data directories;
it does not rely on the user's desktop granting focus to a new test process.

The top-level JSON has schema `ferric.kde-free-wayland.v2`, `source` (the complete
snapshot), `sdk`, `installed_artifact`, `runs`, `runtime` and
`performance`. Evidence references have `path` and `sha256`. The exact required
run inventory is `REQUIRED_RUNS` in the verifier. Every source manifest must
match, and every command must have succeeded. Functional/native receipts must
cover every scenario in the migration acceptance table, including actual Qt
recreation, non-source participant freshness and untagged fallback. Passing a
portable window test is not evidence of tagging.

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

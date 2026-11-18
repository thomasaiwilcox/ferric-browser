# P00 baseline tools

These sources reproduce the baseline at
`f2c5d6deebd97eb861084503f25613a6a819949d`. The overlay manifests enumerate
every test change permitted in that detached checkout. The measurement driver
checks the complete source inventory and records the Cargo producer, native
link command and executable hashes.

The original v7–v11 patches and the broker diagnostic patch are retained with
their recorded hashes. Their materialization paths describe the original
local preparation. The drivers read the patches and manifests from this
directory. Generated logs, binaries, caches and SDK installations belong under
the artifact directory.

## Environment

Use the qualified Qt 6.11.2 SDK and matching Wayland/private headers described
in the repository development specification. Set `FERRIC_QT_PREFIX` and
`FERRIC_QTWAYLAND_PREFIX` when those installations use different paths from
`tools/performance/with_qt611.sh`. The drivers use two build jobs and Cargo
`--locked --offline`, so the Cargo dependency cache must be populated.

`FERRIC_P00_ARTIFACT_DIR` selects the worktree, build and evidence directory.
Its default is `target/site-policy-baseline` beneath this repository. A ccache
binary at `ARTIFACT_DIR/build-tools/ccache` is used when present. Otherwise
the drivers invoke `cc` and `c++` directly.

## Original-source checks

Choose a fresh artifact directory for a new reproduction. Existing result
names are protected against replacement. Run these commands from the
repository root before applying a measurement overlay.

```sh
export FERRIC_P00_ARTIFACT_DIR="$PWD/target/p00-reproduction"
mkdir -p "$FERRIC_P00_ARTIFACT_DIR"
git worktree add --detach "$FERRIC_P00_ARTIFACT_DIR/p00-pinned-source" \
  f2c5d6deebd97eb861084503f25613a6a819949d
mkdir -p "$FERRIC_P00_ARTIFACT_DIR/p00-pinned-source/target"
ln -s "$FERRIC_P00_ARTIFACT_DIR/p00-pinned-target/release" \
  "$FERRIC_P00_ARTIFACT_DIR/p00-pinned-source/target/release"
python3 tools/performance/p00/p00-pinned-run.py build-tools
python3 tools/performance/p00/p00-pinned-run.py rust-perf-debug
python3 tools/performance/p00/p00-pinned-run.py rust-perf-release
```

The original runner also accepts `clippy`, `check`, `workspace`, `engine`,
`application`, `application-wayland`, `wayland-input`, `wayland`, `package`,
`qml-perf-dpr1` and `qml-perf-dpr2`. The `check` or `engine` stage builds the
QML executable through the original repository's QML test task. Run one of
those stages before either QML performance stage.

The QML suite requires its historical comparator scripts. Export their exact
Git blobs before running either QML performance stage.

```sh
python3 - <<'PY'
import hashlib, json, os, subprocess
from pathlib import Path
source = Path('tools/performance/p00')
base = Path(os.environ['FERRIC_P00_ARTIFACT_DIR'])
spec = json.loads((source / 'p00-pinned-qml-comparator-inputs.json').read_text())
directory = base / 'p00-pinned-qml-comparator-inputs'
directory.mkdir()
for name, facts in spec['files'].items():
    content = subprocess.check_output(['git', 'show', facts['git_blob']])
    if hashlib.sha256(content).hexdigest() != facts['sha256']:
        raise RuntimeError(name + ' differs from the comparator manifest')
    (directory / name).write_bytes(content)
spec['artifact_directory'] = str(directory.resolve())
(base / 'p00-pinned-qml-comparator-inputs.json').write_text(json.dumps(spec, indent=2) + '\n')
PY
```

## Browser measurements

After original-source checks finish, apply the selected measurement overlay
to the detached checkout. The current harness is v11.

```sh
git -C "$FERRIC_P00_ARTIFACT_DIR/p00-pinned-source" apply \
  "$PWD/tools/performance/p00/p00-measurement-overlay-v11.patch"
python3 tools/performance/p00/p00-build-measurement.py plain --overlay v11
python3 tools/performance/p00/p00-run-plain-matrix.py \
  --build "$FERRIC_P00_ARTIFACT_DIR/p00-v11-build-plain-1/build-record.json" \
  --output-dir "$FERRIC_P00_ARTIFACT_DIR/p00-v11-ready-rooted-baselines-1" \
  ready:1 ready:2 roots:1 roots:2 root-stages:1 root-stages:2
```

The matrix runs one workload at a time. `--smoke` selects one warm-up and
two measured samples. Full timing uses ten warm-ups and 100 samples, with
1,000 samples for the Normal command journey. Additional workload names are
`command`, `hints`, `refresh`, `refresh-visible`, `activation` and `lifetime`.
Append `:1` or `:2` to select DPR. Full lifetime uses 1,000 ordinary and
1,000 foreign-frame Hint cycles plus 100 shared and 100 private window cycles.

V11 invokes QtTest with `-maxwarnings 0` so high-volume sample records remain
available beyond the default message cap. A suppression sentinel is a harness
error even when the child exits successfully. The stopped v10 rooted attempt
is retained separately; completed earlier workloads with intact logs remain
valid evidence.

Each workload retains its source manifest, runtime metadata, complete log,
result rows and sample events. Workload failures preserve partial results and
allow independent workloads to continue. Source, producer or harness failures
stop the matrix. The summary tools recompute nearest-rank p95 and max and
verify each run's artifact binding. Ready, cold, activation and lifecycle
summaries accept `--matrix-dir` for a different result directory.
The rooted summary also hashes the sample-event file and checks operation
order, warm-ups, fixture facts and exact event-to-row sample accounting.
Validated collection samples remain available when a later fresh or cleanup
step fails; the failed operation contributes no successful sample.

```sh
python3 tools/performance/p00/p00-summarize-ready.py 1 \
  --matrix-dir "$FERRIC_P00_ARTIFACT_DIR/p00-v11-ready-rooted-baselines-1"
python3 tools/performance/p00/p00-summarize-rooted.py \
  "$FERRIC_P00_ARTIFACT_DIR/p00-v11-ready-rooted-baselines-1" roots 1
```

`--reuse-producer BUILD_RECORD` can reuse an existing plain build's engine
and browser when the complete production source and artifact hashes match.
The native harness is relinked and receives a new receipt. Diagnostic mode
requires the separate broker patch and is used for smoke traces. Reverse the
entire diagnostic patch before building plain timing inputs.

The main checkout's application runner supports candidate measurements by
default. Pass `--require-baseline` there when the original production-source
guard is required. The detached P00 runner always applies that guard.

## Blank-page idle observation

The `build-tools` stage above creates the release `xtask` and records its
original-source hash. Run idle separately from other measurements and builds:

```sh
python3 tools/performance/p00/p00-pinned-idle.py before-lifetime \
  --build "$FERRIC_P00_ARTIFACT_DIR/p00-v11-build-plain-1/build-record.json" \
  --tools-record "$FERRIC_P00_ARTIFACT_DIR/p00-pinned-build-tools.json"
```

This records two independent 120-second blank-page observations and rendered
Quit. `--tools-record` can select another passing original-source command
receipt containing the same release `xtask` hash. `after-lifetime` requires a
matching prior observation; it measures a fresh process and does not prove
reclamation within the earlier long-running browser.

```sh
python3 tools/performance/p00/p00-pinned-idle.py after-lifetime \
  --build "$FERRIC_P00_ARTIFACT_DIR/p00-v11-build-plain-1/build-record.json" \
  --tools-record "$FERRIC_P00_ARTIFACT_DIR/p00-pinned-build-tools.json"
```

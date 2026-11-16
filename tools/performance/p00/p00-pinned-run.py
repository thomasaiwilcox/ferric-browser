#!/usr/bin/env python3
"""Record one original-source P00 check; never edit the pinned checkout."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

SOURCE_DIR = Path(__file__).resolve().parent
BASE = Path(os.environ.get('FERRIC_P00_ARTIFACT_DIR', str(SOURCE_DIR.parents[2] / 'target/site-policy-baseline'))).resolve()
ROOT = BASE / "p00-pinned-source"
PIN = "f2c5d6deebd97eb861084503f25613a6a819949d"
ENV = os.environ.copy()
ENV.update(
    CARGO_NET_OFFLINE="true",
    CARGO_BUILD_JOBS="2",
    CARGO_TARGET_DIR=str(BASE / "p00-pinned-target"),
    CCACHE_DIR=str(BASE / "compiler-cache"),
    CCACHE_CONFIGPATH=str(SOURCE_DIR / "ccache.conf"),
    CCACHE_REMOTE_STORAGE="",
    CC=(str(BASE / "build-tools/ccache") + " cc") if (BASE / "build-tools/ccache").is_file() else "cc",
    CXX=(str(BASE / "build-tools/ccache") + " c++") if (BASE / "build-tools/ccache").is_file() else "c++",
    FERRIC_QTWAYLAND_PREFIX=os.environ.get("FERRIC_QTWAYLAND_PREFIX", str(BASE / "qtwayland-sdk")),
)
WRAPPER = ROOT / "tools/performance/with_qt611.sh"
COMMANDS = {
    "build-tools": ["cargo", "build", "--release", "-p", "xtask", "--offline", "--locked"],
    "clippy": ["cargo", "clippy", "--workspace", "--all-targets", "--offline", "--locked", "--", "-D", "warnings"],
    "check": ["cargo", "run", "--release", "-p", "xtask", "--offline", "--locked", "--", "check"],
    "workspace": ["cargo", "test", "--workspace", "--offline", "--locked"],
    "engine": ["cargo", "run", "--release", "-p", "xtask", "--offline", "--locked", "--", "test", "engine"],
    "application": ["cargo", "run", "--release", "-p", "xtask", "--offline", "--locked", "--", "test", "application"],
    "application-wayland": ["cargo", "run", "--release", "-p", "xtask", "--offline", "--locked", "--", "test", "application-wayland"],
    "wayland-input": ["cargo", "run", "--release", "-p", "xtask", "--offline", "--locked", "--", "test", "wayland-input"],
    "wayland": ["cargo", "run", "--release", "-p", "xtask", "--offline", "--locked", "--", "test", "wayland"],
    "package": ["cargo", "run", "--release", "-p", "xtask", "--offline", "--locked", "--", "package", "arch"],
    "qml-perf-dpr1": ["target/qml-tests/qml_tests", "-input", "crates/ferric-browser-engine-qt/tests/qml-perf"],
    "qml-perf-dpr2": ["target/qml-tests/qml_tests", "-input", "crates/ferric-browser-engine-qt/tests/qml-perf"],
    "rust-perf-debug": ["cargo", "test", "-p", "ferric-browser-engine-qt", "--lib", "s17_rust_hint_pipeline_performance", "--offline", "--locked", "--", "--ignored", "--nocapture"],
    "rust-perf-release": ["cargo", "test", "--release", "-p", "ferric-browser-engine-qt", "--lib", "s17_rust_hint_pipeline_performance", "--offline", "--locked", "--", "--ignored", "--nocapture"],
}

def capture(args, sdk=False):
    result = subprocess.run(([str(WRAPPER)] if sdk else []) + args,
                            cwd=ROOT, env=ENV, text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    return {"command": args, "exit_code": result.returncode, "output": result.stdout}

def inventory():
    names = sorted(set(subprocess.check_output(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"], cwd=ROOT
    ).split(b"\0")) - {b""})
    source = {}
    for name in names:
        path = ROOT / os.fsdecode(name)
        if path.is_symlink():
            raise RuntimeError("Source symlink: " + str(path))
        source[os.fsdecode(name)] = hashlib.sha256(path.read_bytes()).hexdigest()
    return source

def artifacts():
    paths = [Path(ENV["CARGO_TARGET_DIR"]) / profile / name
             for profile in ["debug", "release"]
             for name in ["xtask", "ferric-browser", "libferric_browser_engine_qt.a"]]
    paths += [ROOT / "target" / directory / name for directory, name in [
        ("qml-tests", "qml_tests"), ("native-insert", "native_insert"),
        ("application-tests", "application_tests"),
        ("application-tests", "context_reload_tests"),
        ("application-tests", "startup_configuration_tests"),
    ]]
    paths.append(ROOT / "target/release/ferric-browser")
    result = {}
    for path in paths:
        if path.is_file():
            with path.open("rb") as stream:
                result[str(path)] = hashlib.file_digest(stream, "sha256").hexdigest()
    return result

if len(sys.argv) != 2 or sys.argv[1] not in COMMANDS:
    raise SystemExit("Choose one stage: " + ", ".join(COMMANDS))
stage = sys.argv[1]
command = COMMANDS[stage]
comparator = None
if stage in ("qml-perf-dpr1", "qml-perf-dpr2"):
    comparator = json.loads((BASE / "p00-pinned-qml-comparator-inputs.json").read_text())
    artifact_directory = Path(comparator["artifact_directory"])
    for name, facts in comparator["files"].items():
        actual = hashlib.sha256((artifact_directory / name).read_bytes()).hexdigest()
        if actual != facts["sha256"]:
            raise RuntimeError("QML comparator input changed: " + name)
    ENV.update(
        QT_QPA_PLATFORM="offscreen", QT_QPA_PLATFORMTHEME="", QT_QUICK_BACKEND="software",
        QSG_RHI_BACKEND="opengl", QML_XHR_ALLOW_FILE_READ="1",
        QTWEBENGINE_CHROMIUM_FLAGS="--disable-gpu", QT_SCALE_FACTOR=stage[-1],
        FERRIC_TEST_ARTIFACT_DIR=str(artifact_directory),
    )
    ENV.pop("QTWEBENGINE_DISABLE_SANDBOX", None)
if subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip() != PIN:
    raise RuntimeError("Pinned checkout HEAD changed")
if subprocess.check_output(["git", "status", "--porcelain=v1", "--untracked-files=all"], cwd=ROOT):
    raise RuntimeError("Original baseline checkout must be clean")
source = inventory()
manifest = BASE / "p00-pinned-original-source.sha256"
manifest_bytes = "".join(digest + "  " + name + "\n" for name, digest in source.items()).encode()
if manifest_bytes != (SOURCE_DIR / "p00-pinned-original-source.sha256").read_bytes():
    raise RuntimeError("Source does not match the retained original manifest")
if manifest.exists() and manifest.read_bytes() != manifest_bytes:
    raise RuntimeError("Original source inventory changed")
manifest.write_bytes(manifest_bytes)
record_path = BASE / ("p00-pinned-" + stage + ".json")
log_path = BASE / ("p00-pinned-" + stage + ".log")
if record_path.exists() or log_path.exists():
    raise RuntimeError("Preserve prior attempt; choose a new evidence name before retrying")
record = {
    "stage": stage, "status": "running", "pid": os.getpid(),
    "started": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "revision": PIN, "cwd": str(ROOT), "command": command,
    "invocation": ["nice", "-n", "10", str(WRAPPER)] + command,
    "source_files": len(source), "source_manifest": str(manifest),
    "source_manifest_sha256": hashlib.sha256(manifest_bytes).hexdigest(),
    "harness_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    "artifacts_before": artifacts(),
    "build_output_alias": {
        "path": str(ROOT / "target/release"),
        "target": str((ROOT / "target/release").resolve()),
        "is_symlink": (ROOT / "target/release").is_symlink(),
        "setup_record": str(BASE / "p00-pinned-build-path-alias.json"),
    },
    "harness_overlay": [], "log": str(log_path),
    "environment": {key: ENV.get(key) for key in [
        "CARGO_TARGET_DIR", "CARGO_BUILD_JOBS", "CARGO_NET_OFFLINE", "CC", "CXX",
        "CCACHE_DIR", "CCACHE_CONFIGPATH", "FERRIC_QTWAYLAND_PREFIX",
        "QT_QPA_PLATFORM", "QT_QUICK_BACKEND", "WAYLAND_DISPLAY", "XDG_SESSION_TYPE",
        "QT_QPA_PLATFORMTHEME", "QSG_RHI_BACKEND", "QML_XHR_ALLOW_FILE_READ",
        "QTWEBENGINE_CHROMIUM_FLAGS", "QT_SCALE_FACTOR", "FERRIC_TEST_ARTIFACT_DIR",
    ]},
    "fingerprints": [capture(args, sdk) for args, sdk in [
        (["uname", "-srmo"], False), (["rustc", "-vV"], False),
        (["cargo", "-V"], False), (["c++", "--version"], False),
        (["qmake6", "-query"], True), (["hyprctl", "version"], False),
        (["hyprctl", "-j", "devices"], False),
    ]],
    "runtime_metadata": "Per-test QPA/input/viewport/DPR/zoom is reported by each harness; functional results establish no latency or physical-input qualification.",
}
if comparator is not None:
    record["comparator_inputs"] = comparator
    record["runtime_metadata"] = "Original component suite, including separately labelled instrumented records. Use the pinned candidate/current source rows for the site-policy baseline."
if stage.startswith("rust-perf-"):
    record["runtime_metadata"] = "Original S17 Rust component samples. Build profile is explicit in the command."
if comparator is not None or stage.startswith("rust-perf-"):
    record["load_before"] = Path("/proc/loadavg").read_text().strip()

def save():
    temporary = record_path.with_suffix(".json.tmp")
    temporary.write_text(json.dumps(record, indent=2) + "\n")
    temporary.replace(record_path)

save()
print(json.dumps({"stage": stage, "status": "running", "pid": os.getpid(), "record": str(record_path)}), flush=True)
start = time.monotonic()
with log_path.open("x") as log:
    child = subprocess.Popen(record["invocation"], cwd=ROOT, env=ENV, stdout=log, stderr=subprocess.STDOUT)
    record["child_pid"] = child.pid
    save()
    code = child.wait()
source_matches = source == inventory()
if comparator is not None:
    record["comparator_postcheck_exact_match"] = all(
        hashlib.sha256((artifact_directory / name).read_bytes()).hexdigest() == facts["sha256"]
        for name, facts in comparator["files"].items()
    )
    if not record["comparator_postcheck_exact_match"] and code == 0:
        code = 2
record.update(exit_code=code, seconds=round(time.monotonic() - start, 3),
              finished=datetime.datetime.now(datetime.timezone.utc).isoformat(),
              status="passed" if code == 0 and source_matches else "failed",
              source_postcheck_exact_match=source_matches,
              artifacts_after=artifacts(),
              log_sha256=hashlib.sha256(log_path.read_bytes()).hexdigest())
save()
print(json.dumps({key: record[key] for key in ["stage", "status", "exit_code", "seconds", "source_postcheck_exact_match"]}), flush=True)
sys.exit(code if record["source_postcheck_exact_match"] else 2)

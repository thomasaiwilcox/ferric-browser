"""KDE-free Wayland qualification v2; historical KF6 schemas stay unchanged."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import math
import statistics
import subprocess
import sys
import time
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[2]
SCHEMA = "ferric.kde-free-wayland.v2"
FORBIDDEN_RUNTIME = re.compile(r"lib(?:KF[56]|KWayland|Plasma|kwallet|kio)|/(?:kf[56]|kwindowsystem|plasma)/", re.I)
REQUIRED_RUNS = {
    "fmt", "clippy", "rust", "xtask", "native", "portal", "application",
    "public-page-host", "qml", "everyday", "wayland", "blocking", "hyprland",
    "sanitizers", "fresh-build", "install",
}


def sha(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def snapshot():
    paths = subprocess.check_output(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"], cwd=ROOT
    ).decode().split("\0")
    # Build/product/qualification inputs; historical receipts and new evidence
    # are deliberately outside this manifest to avoid circular hashes.
    roots = {"crates", "helpers", "packaging", "tools", ".github", ".cargo"}
    files = {
        name: sha(ROOT / name) for name in sorted(set(paths))
        if name and (Path(name).parts[0] in roots or name in {
            "AGENTS.md", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"
        }) and (ROOT / name).is_file()
    }
    # A later evidence-only documentation commit does not change compiled
    # inputs. Keep the last product/build revision plus the complete content
    # manifest; any source, packaging or qualification-tool edit changes it.
    revision = subprocess.check_output(
        ["git", "log", "-1", "--format=%H", "--", *sorted(roots),
         "AGENTS.md", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"], cwd=ROOT).decode().strip()
    return {"revision": revision,
            "inputs": files,
            "sha256": hashlib.sha256(json.dumps(files, sort_keys=True).encode()).hexdigest()}


def policy():
    problems = []
    # Inspect dependency declarations and actual includes, not upstream notices,
    # negative runtime assertions, prose or historical qualification evidence.
    paths = [ROOT / "Cargo.toml", ROOT / "packaging/PKGBUILD", ROOT / "packaging/dependencies.toml"]
    paths += list((ROOT / "crates").rglob("Cargo.toml"))
    paths += list((ROOT / "crates").rglob("CMakeLists.txt"))
    paths += list((ROOT / ".github").rglob("*.yml"))
    paths += list((ROOT / ".github/ci").glob("*"))
    paths += list((ROOT / "tools/performance").glob("*.sh"))
    dependency = re.compile(r"\bKF[56](?:::|[A-Za-z_]+)|KWayland|KWindowSystem|\bk(?:io|wallet|config|coreaddons|guiaddons|notifications|service|parts|xmlgui|iconthemes|widgetsaddons|irigami)\b|(?:[\w-]+\.)?kde\.org|extra-cmake-modules|plasma-wayland-protocols", re.I)
    for path in paths:
        if path.is_file() and dependency.search(path.read_text()):
            problems.append(str(path.relative_to(ROOT)))
    for path in (ROOT / "crates").rglob("*"):
        if path.suffix not in {".cpp", ".h", ".rs"}:
            continue
        text = path.read_text()
        if re.search(r"#include\s*[<\"](?:KWayland|KWindowSystem|KF[56]/)", text):
            problems.append(str(path.relative_to(ROOT)))
        if path.name == "build.rs" and dependency.search(text):
            problems.append(str(path.relative_to(ROOT)))
    if problems:
        raise ValueError("KDE dependency requires explicit user approval: " + ", ".join(problems))
    return {"schema": SCHEMA, "policy": "passed"}


def reference(path):
    path = Path(path).resolve()
    return {"path": str(path), "sha256": sha(path)}


def checked(ref):
    path = Path(ref["path"])
    if not path.is_file() or sha(path) != ref["sha256"]:
        raise ValueError(f"Missing or changed evidence: {path}")
    return path


def record(args):
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    before = snapshot()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        raise ValueError("A real command is required")
    start = time.monotonic()
    started_ns = time.time_ns()
    log = output.with_suffix(".log")
    with log.open("w") as stream:
        result = subprocess.run(command, cwd=ROOT, stdout=stream, stderr=subprocess.STDOUT)
    receipt = {"schema": SCHEMA, "source": before, "source_after": snapshot(),
               "command": command, "returncode": result.returncode,
               "seconds": time.monotonic() - start, "log": reference(log)}
    errors = []
    if args.artifact:
        if args.artifact.is_file():
            receipt["artifact"] = reference(args.artifact)
        else:
            errors.append("Command did not produce its artifact")
    if args.junit:
        if not args.junit.is_file() or args.junit.stat().st_mtime_ns < started_ns:
            errors.append("Test command did not produce a fresh JUnit report")
        else:
            receipt["test_report"] = reference(args.junit)
    if errors:
        receipt["capture_errors"] = errors
    output.write_text(json.dumps(receipt, indent=2) + "\n")
    return result.returncode or int(bool(errors))


def sdk(args):
    prefix = args.prefix.resolve()
    version = subprocess.check_output([str(prefix / "bin/qmake6"), "-query", "QT_VERSION"]).decode().strip()
    if version != "6.11.2":
        raise ValueError("Only Qt 6.11.2 is qualified")
    header = prefix / "include/QtGui/6.11.2/QtGui/qpa/qplatformwindow_p.h"
    protocol_root = ROOT / "crates/ferric-browser-engine-qt/protocols"
    provenance = json.loads((protocol_root / "provenance.json").read_text())
    for entry in provenance["inputs"]:
        if sha(protocol_root / entry["file"]) != entry["sha256"]:
            raise ValueError("Changed pinned protocol input")
    generated = []
    for entry in provenance["inputs"]:
        for suffix in ["client.h", "server.h", "protocol.c"]:
            path = args.generated / f'{Path(entry["file"]).stem}-{suffix}'
            if path.stat().st_size == 0:
                raise ValueError("Empty generated protocol input")
            generated.append(reference(path))
    return {"schema": SCHEMA, "source": snapshot(), "qt": version,
            "private_header": reference(header), "protocol_provenance": reference(protocol_root / "provenance.json"),
            "libraries": {name: reference(prefix / f"lib/lib{name}.so.6") for name in
                          ["Qt6Core", "Qt6Gui", "Qt6WaylandClient", "Qt6WebEngineCore"]},
            "generated": generated,
            "scanner": subprocess.check_output(["wayland-scanner", "--version"], stderr=subprocess.STDOUT).decode().strip()}


def runtime(args):
    installed = args.artifact.resolve()
    process = Path(f"/proc/{args.pid}")
    if sha(process / "exe") != sha(installed):
        raise ValueError("Runtime process is not the installed artifact")
    maps = (process / "maps").read_text()
    closure = subprocess.check_output(["ldd", str(installed)], text=True)
    if FORBIDDEN_RUNTIME.search(maps + closure) or "not found" in closure:
        raise ValueError("Runtime closure contains KDE or unresolved libraries")
    if "libQt6Gui" not in maps or not re.search(r"/platforms/libqwayland[^/\s]*\.so", maps):
        raise ValueError("Runtime receipt must exercise the native Wayland GUI process")
    return {"schema": SCHEMA, "source": snapshot(), "phase": args.phase,
            "artifact": reference(installed), "maps": maps, "ldd": closure,
            "pid": args.pid,
            "libraries": {match[0]: reference(match[1]) for match in
                          re.findall(r"lib(Qt6\w+)\.so\.6\s+=>\s+(\S+)", closure)}}


def verify(path):
    receipt = json.loads(path.read_text())
    source = snapshot()
    if set(receipt) != {"schema", "source", "sdk", "installed_artifact", "runs", "runtime", "performance"}:
        raise ValueError("Wrong v2 receipt fields; historical KF6 fields are not v2 evidence")
    if receipt["schema"] != SCHEMA or receipt["source"] != source:
        raise ValueError("Wrong schema or source manifest")
    sdk_receipt = json.loads(checked(receipt["sdk"]).read_text())
    if sdk_receipt["schema"] != SCHEMA or sdk_receipt["source"] != source or sdk_receipt["qt"] != "6.11.2":
        raise ValueError("Unqualified SDK/source")
    checked(sdk_receipt["private_header"])
    for ref in sdk_receipt["libraries"].values():
        checked(ref)
    checked(sdk_receipt["protocol_provenance"])
    if len(sdk_receipt["generated"]) != 9:
        raise ValueError("Incomplete generated protocol inventory")
    for generated in sdk_receipt["generated"]:
        checked(generated)
    artifact = checked(receipt["installed_artifact"])
    if set(receipt["runs"]) != REQUIRED_RUNS:
        raise ValueError("Incomplete qualification run inventory")
    for name, ref in receipt["runs"].items():
        run = json.loads(checked(ref).read_text())
        if run["schema"] != SCHEMA or run["source"] != source or run["source_after"] != source or run["returncode"] != 0 or run.get("capture_errors"):
            raise ValueError(f"Failed or stale run: {name}")
        checked(run["log"])
        command = run["command"]
        if name == "fmt" and command != ["cargo", "fmt", "--all", "--", "--check"]:
            raise ValueError("The formatting gate must run the requested workspace command")
        if name == "clippy" and (command[:2] != ["cargo", "clippy"] or not all(x in command for x in ["--workspace", "--all-targets", "--locked", "-D", "warnings"])):
            raise ValueError("Incomplete Clippy gate")
        if name == "rust" and command != ["cargo", "test", "--workspace", "--locked"]:
            raise ValueError("Incomplete workspace Rust test gate")
        if name == "xtask" and command != ["cargo", "xtask", "check"]:
            raise ValueError("Incomplete repository gate")
        if name in {"fresh-build", "install"}:
            checked(run["artifact"])
            if run["artifact"]["sha256"] != receipt["installed_artifact"]["sha256"]:
                raise ValueError("Installed artifact differs from the qualified build/install output")
        if name in {"native", "portal", "application", "public-page-host", "qml", "everyday", "hyprland"}:
            # The named run also supplies a hashed nonempty JUnit/CTest report.
            report = ET.parse(checked(run["test_report"])).getroot()
            cases = list(report.iter("testcase"))
            if not cases or any(list(case.iter("failure")) + list(case.iter("error")) + list(case.iter("skipped")) for case in cases):
                raise ValueError(f"Failed, skipped or empty test inventory: {name}")
            names = {case.get("name") for case in cases}
            if name == "native" and not {"native_wayland_full", "native_wayland_no_tags", "native_wayland_no_exporter", "native_wayland_neither"} <= names:
                raise ValueError("Missing native protocol/lifecycle variants")
            if name == "application" and "application_native_freshness" not in names:
                raise ValueError("Missing actual Qt source/peer freshness regression")
    phases = set()
    for ref in receipt["runtime"]:
        run = json.loads(checked(ref).read_text())
        if run["schema"] != SCHEMA or run["source"] != source or checked(run["artifact"]) != artifact:
            raise ValueError("Stale runtime/artifact receipt")
        if FORBIDDEN_RUNTIME.search(run["maps"] + run["ldd"]) or "not found" in run["ldd"]:
            raise ValueError("Forbidden runtime closure")
        for name, ref in sdk_receipt["libraries"].items():
            loaded = run["libraries"][name]
            checked(loaded)
            if loaded["sha256"] != ref["sha256"]:
                raise ValueError(f"Runtime used a different Qt SDK library: {name}")
        phases.add(run["phase"])
    if phases != {"file-dialog", "window-mode"}:
        raise ValueError("Exercise both file dialogs and window mode")
    verify_performance(json.loads(checked(receipt["performance"]).read_text()), source,
                       receipt["installed_artifact"])
    return policy()


def verify_performance(performance, source, artifact):
    if performance["schema"] != SCHEMA or performance["candidate_source"] != source:
        raise ValueError("Performance is not bound to this candidate")
    if performance.get("pilot") is not False or (performance["pairs"], performance["warmups"], performance["samples"]) != (5, 10, 100):
        raise ValueError("Incomplete paired performance measurements")
    for field in ["baseline_artifact", "candidate_artifact", "probe"]:
        checked(performance[field])
    if performance["candidate_artifact"]["sha256"] != artifact["sha256"]:
        raise ValueError("Performance used a different installed artifact")
    required = {"startup-1", "startup-20", "creation", "recreation", "idle-1", "idle-20"}
    if set(performance["workloads"]) != required:
        raise ValueError("Incomplete performance workload inventory")
    for name, workload in performance["workloads"].items():
        idle = name.startswith("idle")
        fields = {"tree_cpu_percent", "gui_wakeups_per_second"} if idle else {"milliseconds"}
        if set(workload["metrics"]) != fields:
            raise ValueError(f"Incomplete measured metrics: {name}")
        rows = [json.loads(checked(ref).read_text()) for ref in workload["raw_receipts"]]
        if len(rows) != 10 or {(r["label"], r["pair"]) for r in rows} != {(label, pair) for label in ["baseline", "candidate"] for pair in range(5)}:
            raise ValueError(f"Missing independent process pairs: {name}")
        for row in rows:
            if row["source"] != source or row["workload"] != name or row["artifact"] != performance[f'{row["label"]}_artifact'] or row["probe"] != performance["probe"]:
                raise ValueError(f"Stale raw performance provenance: {name}")
            if (row["warmups"], row["samples"]) != (10, 100) or len(row["values"]) != (1 if idle else 100) or (not idle and len(row["warmup_values"]) != 10):
                raise ValueError(f"Incomplete raw samples/warmups: {name}")
            if len(row["processes"]) != (110 if name.startswith("startup") else 1):
                raise ValueError(f"Incomplete process inventory: {name}")
            if not idle and any(not math.isfinite(v["milliseconds"]) or v["milliseconds"] <= 0 for v in row["values"]):
                raise ValueError(f"Invalid latency samples: {name}")
            if name == "recreation" and {v["operation"] for v in row["values"]} != {"role", "surface"}:
                raise ValueError("Both actual role and surface recreation must be measured")
        for field, claimed in workload["metrics"].items():
            def summary(row):
                values = sorted(v[field] for v in row["values"])
                return values[94] if not idle else statistics.mean(values)
            actual = {label: statistics.median(summary(r) for r in rows if r["label"] == label)
                      for label in ["baseline", "candidate"]}
            if any(actual[label] != claimed[label] for label in actual):
                raise ValueError(f"Summary does not match raw measurements: {name}")
            if idle:
                repetitions = [json.loads(checked(ref).read_text()) for ref in claimed["baseline_raw_receipts"]]
                if len(repetitions) != 5 or {r["pair"] for r in repetitions} != {f"repeat-{i}" for i in range(5)} or any(not r["baseline_repetition"] or r["label"] != "baseline" or r["workload"] != name or r["source"] != source or r["probe"] != performance["probe"] or r["artifact"] != performance["baseline_artifact"] or len(r["processes"]) != 1 for r in repetitions):
                    raise ValueError(f"Incomplete baseline-only idle repetitions: {name}")
                before = [summary(r) for r in repetitions]
                if before != claimed["baseline_repetitions"] or max(r["finished_ns"] for r in repetitions) >= min(r["started_ns"] for r in rows):
                    raise ValueError(f"Idle variability was not established before comparison: {name}")
                if not min(before) <= actual["candidate"] <= max(before):
                    raise ValueError(f"Unresolved idle variability/regression: {name}/{field}")
            elif actual["candidate"] - actual["baseline"] > 1 and actual["candidate"] > actual["baseline"] * 1.05:
                raise ValueError(f"Latency regression: {name}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    sub.add_parser("policy")
    sub.add_parser("snapshot")
    run = sub.add_parser("record")
    run.add_argument("--output", type=Path, required=True)
    run.add_argument("--artifact", type=Path)
    run.add_argument("--junit", type=Path)
    run.add_argument("command", nargs=argparse.REMAINDER)
    sdk_parser = sub.add_parser("sdk")
    sdk_parser.add_argument("--prefix", type=Path, required=True)
    sdk_parser.add_argument("--generated", type=Path, required=True)
    runtime_parser = sub.add_parser("runtime")
    runtime_parser.add_argument("--pid", type=int, required=True)
    runtime_parser.add_argument("--artifact", type=Path, required=True)
    runtime_parser.add_argument("--phase", choices=["file-dialog", "window-mode"], required=True)
    check = sub.add_parser("verify")
    check.add_argument("receipt", type=Path)
    args = parser.parse_args()
    if args.action == "record":
        return record(args)
    result = {"policy": policy, "snapshot": snapshot,
              "sdk": lambda: sdk(args), "runtime": lambda: runtime(args),
              "verify": lambda: verify(args.receipt)}[args.action]()
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError, ET.ParseError) as error:
        print(f"KDE-free qualification failed: {error}", file=sys.stderr)
        sys.exit(1)

#!/usr/bin/env python3
"""Owned-bridge qualification: frozen products, identical event-driven probe.

Five alternating pairs; ten excluded warmups and 100 samples for each latency
workload. Startup samples each use an independent browser process. Creation and
role/surface recreation sample actual product windows in independent paired
browser processes. Idle baseline repetitions precede all candidate measurement.
The existing disposable-profile quit fixture handles rendered confirmations.
"""
import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess
import tempfile
import time
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "qualification"))
import kde_free as qualification


def atomic(path, value):
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(value))
    temporary.replace(path)


def wait_for(operation, child, timeout=40):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if child.poll() is not None:
            raise RuntimeError(f"Owned browser exited early: {child.returncode}")
        result = operation()
        if result is not None:
            return result
        time.sleep(0.005)
    raise TimeoutError("Owned browser did not reach the native measurement boundary")


def cpu_snapshot(root):
    todo, result = [root], {}
    while todo:
        pid = todo.pop()
        if pid in result:
            continue
        try:
            fields = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
            result[pid] = (int(fields[19]), int(fields[11]) + int(fields[12]))
            for task in Path(f"/proc/{pid}/task").iterdir():
                todo.extend(map(int, (task / "children").read_text().split()))
        except FileNotFoundError:
            continue
    return result


class Product:
    def __init__(self, binary, plugins, pages, log, baseline_plugins=None):
        self.binary = binary
        self.log = log
        self.profile = tempfile.TemporaryDirectory(prefix="ferric-browser-ui-perf-")
        self.root = Path(self.profile.name)
        (self.root / ".ferric-performance-fixture").write_text("Ferric performance fixture only.\n")
        if pages > 12:
            (self.root / "confirm-page-windows").write_text(f"Open up to {pages} performance fixture page windows.\n")
        env = dict(os.environ)
        env.update(QT_QPA_PLATFORM="wayland", QT_QPA_PLATFORMTHEME="",
                   QT_QPA_GENERIC_PLUGINS="ferricperfquitfixture",
                   FERRIC_PERF_NATIVE_PROBE="1", FERRIC_PERF_QUIT_ROOT=str(self.root),
                   FERRIC_PERF_EXPECTED_PAGES=str(pages))
        env["QT_PLUGIN_PATH"] = os.pathsep.join(str(p) for p in
            [plugins, baseline_plugins, env.get("QT_PLUGIN_PATH")] if p)
        if baseline_plugins:
            # Historical artifact only: do not resolve its matching library
            # against the host's newer, incompatible Qt/KF installation.
            env["LD_LIBRARY_PATH"] = str(baseline_plugins.parents[1]) + os.pathsep + env.get("LD_LIBRARY_PATH", "")
        self.env = env
        self.sequence = 0
        self.stream = log.open("w")
        started = time.perf_counter_ns()
        self.child = subprocess.Popen(
            [str(binary), "--basedir", str(self.root), "--set", 'tabs.presentation="windows"',
             "open", *(["about:blank"] * pages)], env=env, stdin=subprocess.DEVNULL,
            stdout=self.stream, stderr=subprocess.STDOUT)
        try:
            self.startup = wait_for(lambda: self.read("native-startup.json"), self.child,
                                    timeout=120 if pages > 1 else 40)
            self.startup["milliseconds"] = (time.perf_counter_ns() - started) / 1e6
            self.startup["pid"] = self.child.pid
            if self.startup["windows"] != pages:
                raise RuntimeError("Wrong native startup window count")
            # The native boundary includes exposure and a rendered frame.
            # Independently corroborate the normal product page workload.
            deadline = time.monotonic() + 40
            while True:
                output = subprocess.run([str(binary), "--basedir", str(self.root), "query", "tabs", "--format", "json"],
                                        capture_output=True, text=True, timeout=10, env=self.env)
                if output.returncode == 0:
                    tabs = json.loads(output.stdout).get("tabs", [])
                    if len(tabs) == pages and all(t.get("loading") in {"idle", "complete"} for t in tabs):
                        break
                if time.monotonic() >= deadline:
                    raise RuntimeError(f"Native windows mapped but product pages did not settle: {output.returncode}, {output.stdout}, {output.stderr}")
                time.sleep(0.05)
            (self.root / "confirm-page-windows").unlink(missing_ok=True)
        except BaseException:
            self.abort()
            raise

    def read(self, name):
        path = self.root / name
        return json.loads(path.read_text()) if path.is_file() else None

    def command(self, operation):
        self.sequence += 1
        atomic(self.root / "native-command.json", {"sequence": self.sequence, "operation": operation})
        def response():
            result = self.read("native-response.json")
            return result if result and result["sequence"] == self.sequence else None
        result = wait_for(response, self.child)
        if not result["ok"]:
            raise RuntimeError(f"Native probe rejected {operation}: {result}")
        return result

    def stop(self):
        atomic_file = self.root / "confirm-quit.tmp"
        atomic_file.write_text("Confirm this performance fixture's quit.\n")
        atomic_file.replace(self.root / "confirm-quit")
        subprocess.run([str(self.binary), "--basedir", str(self.root), "command", "--", "quit"],
                       check=True, stdout=self.stream, stderr=subprocess.STDOUT, timeout=10, env=self.env)
        code = self.child.wait(timeout=40)
        if code != 0 or not (self.root / "quit-confirmed").is_file():
            raise RuntimeError(f"Product shutdown failed: {code}")
        self.stream.close()
        self.profile.cleanup()

    def abort(self):
        if self.child.poll() is None:
            self.child.terminate()
            try:
                self.child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.child.kill()
                self.child.wait()
        self.stream.close()
        self.profile.cleanup()


def measure(args, name, label, pair, idle_repetition=False):
    output = args.output / f"{name}-{pair}-{label}.json"
    if output.exists():
        previous = json.loads(output.read_text())
        if (previous["artifact"] != qualification.reference(getattr(args, label)) or previous["source"] != args.source
            or previous["probe"] != qualification.reference(args.probe)
            or (previous["warmups"], previous["samples"]) != (args.warmups, args.samples)):
            raise ValueError("Cannot resume measurements after an artifact/source change")
        return previous
    pages = 20 if name.endswith("-20") else 1
    result = {"source": args.source, "artifact": qualification.reference(getattr(args, label)),
              "probe": qualification.reference(args.probe), "workload": name, "label": label,
              "pair": pair, "warmups": args.warmups, "samples": args.samples,
              "load_average": os.getloadavg(), "baseline_repetition": idle_repetition,
              "started_ns": time.time_ns(), "values": [], "warmup_values": [], "processes": []}
    starts = args.warmups + args.samples if name.startswith("startup") else 1
    for index in range(starts):
        product = Product(getattr(args, label), args.plugins, pages,
                          args.output / f"{name}-{pair}-{label}-{index}.log",
                          args.baseline_plugins if label == "baseline" else None)
        try:
            result["processes"].append({"pid": product.child.pid,
                "birth_ticks": cpu_snapshot(product.child.pid)[product.child.pid][0]})
            if name.startswith("startup"):
                result["warmup_values" if index < args.warmups else "values"].append(product.startup)
            elif name in {"creation", "recreation"}:
                for sample in range(args.warmups + args.samples):
                    # Both actual Qt native transitions are measured, in a fixed
                    # alternating order shared by the baseline and candidate.
                    operation = "create" if name == "creation" else ("role" if sample % 2 == 0 else "surface")
                    value = product.command(operation)
                    result["warmup_values" if sample < args.warmups else "values"].append(value)
                    if name == "creation":
                        product.command("cleanup")
                        wait_for(lambda: True if product.command("state")["windows"] == pages else None, product.child)
            else:
                # Let deferred page/role work settle before the idle interval.
                time.sleep(2)
                product.command("idle-start")
                before = cpu_snapshot(product.child.pid)
                start = time.monotonic()
                time.sleep(args.idle_seconds)
                after = cpu_snapshot(product.child.pid)
                elapsed = time.monotonic() - start
                awake = product.command("idle-stop")
                if before.keys() - after.keys() or any(after[p][0] != v[0] for p, v in before.items()):
                    raise RuntimeError("Process exited/reused during idle: complete CPU accounting unavailable")
                ticks = sum(v[1] - before.get(p, (v[0], 0))[1] for p, v in after.items())
                result["values"].append({"seconds": elapsed,
                    "tree_cpu_percent": 100 * ticks / os.sysconf("SC_CLK_TCK") / elapsed,
                    "gui_wakeups_per_second": awake["dispatcher_awake"] / awake["seconds"],
                    "cpu_before": before, "cpu_after": after, "dispatcher": awake})
            product.stop()
        except BaseException:
            product.abort()
            raise
    result["finished_ns"] = time.time_ns()
    atomic(output, result)
    print(f"Completed {name} {pair} {label}", flush=True)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--plugins", type=Path, required=True)
    parser.add_argument("--baseline-plugins", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--idle-seconds", type=int, default=20)
    parser.add_argument("--pilot", action="store_true", help="One sample only; never qualification evidence")
    args = parser.parse_args()
    for name in ["baseline", "candidate", "plugins", "output"]:
        setattr(args, name, getattr(args, name).resolve())
    args.output.mkdir(parents=True, exist_ok=True)
    args.probe = args.plugins / "generic/libferric_perf_quit_plugin.so"
    args.source = qualification.snapshot()
    args.samples, args.warmups, pairs = (1, 0, 1) if args.pilot else (100, 10, 5)
    if not args.pilot:
        active = subprocess.check_output(["ps", "-eo", "comm="], text=True).splitlines()
        if {"cargo", "rustc", "cc1", "cc1plus", "ninja", "make"} & {name.strip() for name in active}:
            raise RuntimeError("Compilation must stop before performance qualification")
    workloads = ["startup-1", "startup-20", "creation", "recreation", "idle-1", "idle-20"]
    repetitions = {}
    # Establish idle variability with independent baseline-only processes first.
    for name in ["idle-1", "idle-20"]:
        repetitions[name] = [measure(args, name, "baseline", f"repeat-{index}", True)
                             for index in range(pairs)]
    report = {"schema": qualification.SCHEMA, "candidate_source": args.source,
              "pairs": pairs, "warmups": args.warmups, "samples": args.samples,
              "baseline_artifact": qualification.reference(args.baseline),
              "candidate_artifact": qualification.reference(args.candidate),
              "probe": qualification.reference(args.probe),
              "pilot": args.pilot, "workloads": {},
              "method": "same test-only plugin explicitly maps committed registered background page owners; all native page windows exposed/frameSwapped; GUI dispatcher awake events, no added idle timer; five alternating independent process blocks; each startup sample launches a new product process"}
    for name in workloads:
        rows = []
        for pair in range(pairs):
            for label in (["baseline", "candidate"] if pair % 2 == 0 else ["candidate", "baseline"]):
                rows.append(measure(args, name, label, pair))
        entry = {"raw_receipts": [qualification.reference(args.output / f'{name}-{r["pair"]}-{r["label"]}.json') for r in rows]}
        fields = ["tree_cpu_percent", "gui_wakeups_per_second"] if name.startswith("idle") else ["milliseconds"]
        entry["metrics"] = {}
        for field in fields:
            # Retain per-process p95 for latency; compare paired process medians.
            def summary(row):
                values = sorted(v[field] for v in row["values"])
                return values[(95 * len(values) + 99) // 100 - 1] if field == "milliseconds" else statistics.mean(values)
            values = {label: statistics.median(summary(r) for r in rows if r["label"] == label)
                      for label in ["baseline", "candidate"]}
            if name.startswith("idle"):
                values["baseline_repetitions"] = [summary(r) for r in repetitions[name]]
                values["baseline_raw_receipts"] = [qualification.reference(args.output / f'{name}-{r["pair"]}-baseline.json') for r in repetitions[name]]
            entry["metrics"][field] = values
        report["workloads"][name] = entry
        atomic(args.output / "comparison.json", report)


if __name__ == "__main__":
    main()

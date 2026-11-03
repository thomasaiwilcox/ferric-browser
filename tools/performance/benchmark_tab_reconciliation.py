#!/usr/bin/env python3
"""Measure two controller versions in an owned worktree with a qualified QML runner.

Run with the SDK/display environment already selected and no compilation running.
Temporarily switches only the controller import; refuses an already edited file
and restores the original unless another writer has changed it in the meantime.
"""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path
from benchmark_matcher import compare

SOURCE = Path("crates/ferric-browser-engine-qt/qml/components/FerricTabViewController.qml")
TEST = Path("crates/ferric-browser-engine-qt/tests/qml-perf/tst_tab_reconciliation_performance.qml")
MARKER = "FERRIC_TAB_RECONCILIATION_JSON "


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", required=True)
    parser.add_argument("--candidate-file", required=True, type=Path)
    parser.add_argument("--runner", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    original = SOURCE.read_bytes()
    versions = {"baseline": subprocess.check_output(["git", "show", f"{args.baseline}:{SOURCE}"]),
                "candidate": args.candidate_file.read_bytes()}
    if original != versions["baseline"]:
        parser.error("controller differs from the pinned baseline; preserve that edit first")
    if args.output.exists():
        parser.error("output already exists")
    logs = args.output.with_suffix(".logs")
    logs.mkdir()
    runs = []
    active = original
    try:
        for pair in range(5):
            run = {}
            order = ["baseline", "candidate"] if pair % 2 == 0 else ["candidate", "baseline"]
            for name in order:
                if SOURCE.read_bytes() != active:
                    raise RuntimeError("controller changed by another writer; stopping")
                active = versions[name]
                SOURCE.write_bytes(active)
                result = subprocess.run([str(args.runner.resolve()), "-input", str(TEST)],
                                        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False)
                (logs / f"{pair}-{name}.log").write_bytes(result.stdout)
                if result.returncode:
                    raise RuntimeError(f"QML run failed; inspect {logs}/{pair}-{name}.log")
                rows = [json.loads(line.split(MARKER, 1)[1])
                        for line in result.stdout.decode().splitlines() if MARKER in line]
                if len(rows) != 6:
                    raise RuntimeError("QML runner did not report all six workloads")
                run[name] = [{"query": row["scenario"], "count": row["count"],
                              "samples_ms": row["samples_ms"]} for row in rows]
            runs.append(run)
            print(f"Completed alternating controller pair {pair+1}/5", flush=True)
    finally:
        if SOURCE.read_bytes() == active:
            SOURCE.write_bytes(original)
        else:
            print("Preserved a concurrent controller edit; original was not restored", flush=True)
    report = {"baseline_commit": args.baseline,
              "source_sha256": {name: hashlib.sha256(data).hexdigest() for name, data in versions.items()},
              "method": "5 alternating process pairs; actual controller/ListModel, Item views; 10 warmups + 100 samples; excludes WebEngine rendering",
              "runs": runs, "comparison": compare(runs)}
    args.output.write_text(json.dumps(report, indent=2)+"\n")


if __name__ == "__main__":
    main()

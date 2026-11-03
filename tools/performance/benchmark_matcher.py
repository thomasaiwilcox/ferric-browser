#!/usr/bin/env python3
"""Compare the actual Rust matcher against a pinned commit, without Qt or Cargo.

Run after other builds have finished. This is a synthetic matcher benchmark,
not complete hint activation or end-to-end Switcher latency.
"""
import argparse
import hashlib
import json
import platform
import os
import datetime
import random
import statistics
import subprocess
import tempfile
from pathlib import Path

SOURCE = "crates/ferric-browser-core/src/switcher.rs"
BENCH = r'''
mod switcher;
use std::{hint::black_box, time::Instant};
fn main() {
    for count in [100, 500, 1000, 5000] {
        let candidates = (0..count).map(|i| format!("Synthetic target {i} CAFÉ 東京")).collect::<Vec<_>>();
        for text in ["synthetic target 42", "st42", "CAFÉ 東京", "no-match-xyz"] {
            let query = switcher::tokenize_query(text);
            let mut samples = Vec::new();
            for iteration in 0..110 {
                let started = Instant::now();
                for candidate in &candidates {
                    black_box(switcher::rank(black_box(&query), "", black_box(candidate), &[]));
                }
                if iteration >= 10 { samples.push(started.elapsed().as_secs_f64()*1000.0); }
            }
            println!("{{\"query\":\"{text}\",\"count\":{count},\"samples_ms\":{samples:?}}}");
        }
    }
}
'''


def command(*args):
    return subprocess.check_output(args, text=True).strip()


def p95(samples):
    return sorted(samples)[(len(samples) * 95 + 99) // 100 - 1]


def compare(runs):
    rng = random.Random(0)
    result = []
    for index, first in enumerate(runs[0]["baseline"]):
        row = {"query": first["query"], "count": first["count"]}
        for name, statistic in [("median", statistics.median), ("p95", p95)]:
            before = [statistic(run["baseline"][index]["samples_ms"]) for run in runs]
            after = [statistic(run["candidate"][index]["samples_ms"]) for run in runs]
            deltas = [a - b for a, b in zip(after, before)]
            boot = sorted(statistics.mean(rng.choices(deltas, k=len(deltas))) for _ in range(10000))
            baseline = statistics.median(before)
            candidate = statistics.median(after)
            row[name] = {"before_ms": baseline, "after_ms": candidate,
                         "change_percent": (candidate / baseline - 1) * 100,
                         "paired_mean_delta_95ci_ms": [boot[249], boot[9749]],
                         "passes_gain_gate": candidate <= baseline * .9 and baseline - candidate >= 1 and boot[9749] < 0,
                         "regression": candidate > baseline * 1.05 and candidate - baseline > 1 and boot[249] > 0}
        result.append(row)
    return result


def read_optional(path):
    try:
        return Path(path).read_text().strip()
    except OSError:
        return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("output already exists")
    baseline = command("git", "rev-parse", args.baseline)
    sources = {"baseline": subprocess.check_output(["git", "show", f"{baseline}:{SOURCE}"]),
               "candidate": Path(SOURCE).read_bytes()}
    runs = []
    with tempfile.TemporaryDirectory(prefix="ferric-matcher-perf-") as directory:
        root = Path(directory)
        for name, source in sources.items():
            folder = root / name
            folder.mkdir()
            (folder / "switcher.rs").write_bytes(source)
            (folder / "bench.rs").write_text(BENCH)
            subprocess.run(["rustc", "-O", "--edition=2024", str(folder / "bench.rs"), "-o", str(folder / "bench")], check=True)
        for pair in range(5):
            run = {"load_average_before": os.getloadavg()}
            order = ["baseline", "candidate"] if pair % 2 == 0 else ["candidate", "baseline"]
            for name in order:
                run[name] = [json.loads(line) for line in command(str(root / name / "bench")).splitlines()]
            runs.append(run)
            print(f"Completed alternating pair {pair + 1}/5", flush=True)
    report = {"baseline_commit": baseline, "candidate_commit": command("git", "rev-parse", "HEAD"),
              "candidate_dirty": command("git", "status", "--porcelain"),
              "source_sha256": {name: hashlib.sha256(source).hexdigest() for name, source in sources.items()},
              "rustc": command("rustc", "--version"), "machine": platform.platform(),
              "method": "rustc -O; 5 alternating pairs; 10 warmups + 100 samples per workload; one sample ranks every candidate; paired bootstrap over run deltas, seed 0",
              "timestamp_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "cpu_info": (read_optional('/proc/cpuinfo') or 'unavailable').split('\n\n', 1)[0],
              "governor": read_optional('/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor'),
              "benchmark_source": BENCH, "runs": runs, "comparison": compare(runs)}
    with args.output.open("x") as output:
        json.dump(report, output, indent=2)
        output.write("\n")


if __name__ == "__main__":
    main()

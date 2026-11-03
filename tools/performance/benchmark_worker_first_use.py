#!/usr/bin/env python3
"""Measure construction and first-use overhead of the actual bounded worker.

Synthetic owned integer jobs isolate OS thread creation; this does not measure
filesystem, printing, or browser-visible completion latency. Run without builds.
"""
import argparse
import hashlib
import json
import statistics
import subprocess
import tempfile
from pathlib import Path

SOURCE = "crates/ferric-browser-runtime/src/worker.rs"
BENCH = r'''
#[allow(dead_code)] mod worker;
use std::{hint::black_box, time::{Duration, Instant}};
fn wait(worker: &mut worker::SingleFlightWorker<u32,u32>) {
    let started=Instant::now();
    loop {
        match worker.poll() {
            worker::Poll::Ready(value) => { assert_eq!(value,42);return; },
            worker::Poll::Pending if started.elapsed()<Duration::from_secs(2) => std::thread::yield_now(),
            _ => panic!("owned job did not complete")
        }
    }
}
fn main() {
    for sample in -10..100 {
        let started=Instant::now();
        let mut worker=CONSTRUCT;
        let constructor=started.elapsed().as_secs_f64()*1000.;
        let submit=Instant::now();
        black_box(&mut worker).submit(black_box(21)).unwrap();
        let first_submit=submit.elapsed().as_secs_f64()*1000.;
        wait(&mut worker);
        let first_delivery=started.elapsed().as_secs_f64()*1000.;
        let submit=Instant::now();
        black_box(&mut worker).submit(black_box(21)).unwrap();
        let warm_submit=submit.elapsed().as_secs_f64()*1000.;
        wait(&mut worker);
        if sample>=0 { println!("{{\"constructor_ms\":{constructor},\"first_submit_ms\":{first_submit},\"first_delivery_ms\":{first_delivery},\"warm_submit_ms\":{warm_submit}}}"); }
    }
}
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("output already exists")
    sources = {"baseline": subprocess.check_output(["git", "show", f"{args.baseline}:{SOURCE}"]),
               "candidate": Path(SOURCE).read_bytes()}
    runs = []
    with tempfile.TemporaryDirectory(prefix="ferric-worker-perf-") as temporary:
        root = Path(temporary)
        for name, source in sources.items():
            folder=root/name; folder.mkdir()
            (folder/"worker.rs").write_bytes(source)
            constructor = 'worker::SingleFlightWorker::spawn("worker-perf", |value| value*2).unwrap()' if name == "baseline" else 'worker::SingleFlightWorker::deferred("worker-perf", |value| value*2)'
            (folder/"main.rs").write_text(BENCH.replace("CONSTRUCT",constructor))
            subprocess.run(["rustc","-O","--edition=2024",str(folder/"main.rs"),"-o",str(folder/"bench")],check=True)
        for pair in range(5):
            run={}
            for name in (["baseline","candidate"] if pair%2==0 else ["candidate","baseline"]):
                run[name]=[json.loads(line) for line in subprocess.check_output([str(root/name/"bench")],text=True).splitlines()]
            runs.append(run)
    metrics = {}
    for metric in runs[0]["baseline"][0]:
        metrics[metric]={side:statistics.median(statistics.median(row[metric] for row in run[side]) for run in runs) for side in sources}
    args.output.write_text(json.dumps({"baseline_commit":args.baseline,"source_sha256":{name:hashlib.sha256(source).hexdigest() for name,source in sources.items()},
        "method":"rustc -O, five alternating process pairs; 10 warmups + 100 samples; owned integer jobs; domain I/O excluded",
        "benchmark_source":BENCH,"runs":runs,"median_ms":metrics},indent=2)+"\n")
    print(json.dumps(metrics,indent=2))


if __name__ == "__main__":
    main()

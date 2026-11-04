#!/usr/bin/env python3
"""Compare frozen release browsers using the same native UI harness and fixture.

Select the qualified SDK and owned display before invoking. No builds should run
while sampling. Shutdown failures remain explicit nulls in each raw report.
"""
import argparse
import hashlib
import json
import os
import random
import statistics
import subprocess
from pathlib import Path

METRICS = [("threads","process_tree","threads"),("pss_kib","process_tree","pss_kib"),
           ("rss_kib","process_tree","rss_kib"),("main_cpu_percent","idle.main_cpu","value_percent"),
           ("tree_cpu_percent","idle.process_tree_cpu","value_percent")]


def metric(report, name, field):
    return next(row[field] for row in report["measurements"] if row["name"] == name)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline",required=True,type=Path)
    parser.add_argument("--candidate",required=True,type=Path)
    parser.add_argument("--plugin-dir",required=True,type=Path)
    parser.add_argument("--pages",type=int,default=20,choices=[1,5,10,20])
    parser.add_argument("--presentation",default="windows",choices=["internal","windows"])
    parser.add_argument("--idle-seconds",type=int,default=20)
    parser.add_argument("--output",required=True,type=Path)
    args=parser.parse_args()
    if args.output.exists():parser.error("output already exists")
    versions={"baseline":args.baseline.resolve(),"candidate":args.candidate.resolve()}
    fingerprints={name:{binary:hashlib.sha256((folder/binary).read_bytes()).hexdigest()
                        for binary in ["xtask","ferric-browser"]} for name,folder in versions.items()}
    if fingerprints["baseline"]["xtask"] != fingerprints["candidate"]["xtask"]:
        parser.error("comparisons require an identical harness")
    logs=args.output.with_suffix(".runs");logs.mkdir()
    runs=[]
    for pair in range(5):
        run={"load_average_before":os.getloadavg()}
        for name in (["baseline","candidate"] if pair%2==0 else ["candidate","baseline"]):
            output=logs/f"{pair}-{name}.json"
            command=["dbus-run-session","--",str(versions[name]/"xtask"),"perf","--suite","ui",
                     "--samples","30","--pages",str(args.pages),"--presentation",args.presentation,
                     "--idle-seconds",str(args.idle_seconds),"--quit-plugin-dir",str(args.plugin_dir.resolve()),
                     "--output",str(output.resolve())]
            with output.with_suffix(".log").open("w") as stream:
                subprocess.run(command,stdout=stream,stderr=subprocess.STDOUT,check=True,timeout=180)
            run[name]=json.loads(output.read_text())["runs"][0]
        if run["baseline"]["runtime"] != run["candidate"]["runtime"]:
            raise RuntimeError("runtime graphics facts changed between paired samples")
        runs.append(run)
        print(f"Completed alternating UI pair {pair+1}/5",flush=True)
    rng=random.Random(0);comparison={}
    for key,name,field in METRICS:
        before=[metric(run["baseline"],name,field) for run in runs]
        after=[metric(run["candidate"],name,field) for run in runs]
        if any(value is None for value in before+after):
            comparison[key]={"available":False,"reason":"at least one run omitted the complete metric"};continue
        delta=[a-b for a,b in zip(after,before)]
        bootstrap=sorted(statistics.mean(rng.choices(delta,k=5)) for _ in range(10000))
        comparison[key]={"before":statistics.median(before),"after":statistics.median(after),
                         "paired_mean_delta_95ci":[bootstrap[249],bootstrap[9749]]}
    report={"fingerprints":fingerprints,"method":"five alternating independent process pairs; paired bootstrap seed0; warm shared OS/shader caches",
            "workload":{"pages":args.pages,"presentation":args.presentation,"idle_seconds":args.idle_seconds},
            "runs":runs,"comparison":comparison}
    args.output.write_text(json.dumps(report,indent=2)+"\n")
    print(json.dumps(comparison,indent=2))


if __name__ == "__main__":
    main()

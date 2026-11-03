#!/usr/bin/env python3
"""Compare frozen native Switcher fixtures in five alternating process pairs.

Build each fixture against its corresponding release engine archive. Their
embedded-resource parity checks must pass against the current source checkout.
Run with the qualified SDK selected, with no compilation during sampling.
"""
import argparse
import hashlib
import json
import os
import random
import statistics
import subprocess
from pathlib import Path


def percentile95(values):
    return sorted(values)[(len(values) * 95 + 99) // 100 - 1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', required=True, type=Path)
    parser.add_argument('--candidate', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    if args.output.exists():
        parser.error('output already exists')
    binaries = {name: path.resolve() for name, path in
                [('baseline', args.baseline), ('candidate', args.candidate)]}
    logs = args.output.with_suffix('.runs')
    logs.mkdir()
    env = os.environ | {'QT_QPA_PLATFORM': 'offscreen', 'QT_QUICK_BACKEND': 'software',
                        'QT_QPA_PLATFORMTHEME': '', 'QTWEBENGINE_CHROMIUM_FLAGS': '--disable-gpu',
                        'FERRIC_PERF_SWITCHER': '1'}
    runs = []
    for pair in range(5):
        run = {'load_average_before': os.getloadavg()}
        for name in (['baseline', 'candidate'] if pair % 2 == 0 else ['candidate', 'baseline']):
            output = logs / f'{pair}-{name}.log'
            result = subprocess.run(['dbus-run-session', '--', str(binaries[name]),
                                     'performanceLargeHistory'], env=env, text=True,
                                    stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=180)
            output.write_text(result.stdout)
            result.check_returncode()
            rows = [json.loads(line.split('FERRIC_SWITCHER_PERFORMANCE_JSON ', 1)[1])
                    for line in result.stdout.splitlines() if 'FERRIC_SWITCHER_PERFORMANCE_JSON ' in line]
            storage = [json.loads(line.split('FERRIC_STORAGE_POLL_PERFORMANCE_JSON ', 1)[1])
                       for line in result.stdout.splitlines() if 'FERRIC_STORAGE_POLL_PERFORMANCE_JSON ' in line]
            if len(rows) != 3 or len(storage) != 1 or any(len(r['uncached_ms']) != 100 for r in rows):
                raise RuntimeError(f'{name} did not produce the complete fixture samples')
            run[name] = {'queries': rows, 'storage': storage[0]}
        runs.append(run)
        print(f'Completed alternating native pair {pair+1}/5', flush=True)
    rng = random.Random(0)
    comparisons = []
    for index, workload in enumerate(runs[0]['baseline']['queries']):
        for field in ['uncached_ms', 'cached_ms']:
            row = {'database_rows': workload['database_rows'], 'library_limit': workload['library_limit'],
                   'path': field}
            for label, statistic in [('median', statistics.median), ('p95', percentile95)]:
                before = [statistic(run['baseline']['queries'][index][field]) for run in runs]
                after = [statistic(run['candidate']['queries'][index][field]) for run in runs]
                delta = [a-b for a, b in zip(after, before)]
                bootstrap = sorted(statistics.mean(rng.choices(delta, k=5)) for _ in range(10000))
                b, a = statistics.median(before), statistics.median(after)
                row[label] = {'before_ms': b, 'after_ms': a, 'change_percent': (a/b-1)*100,
                              'paired_mean_delta_95ci_ms': [bootstrap[249], bootstrap[9749]],
                              'passes_gain_gate': a <= b*.9 and b-a >= 1 and bootstrap[9749] < 0,
                              'regression': a > b*1.05 and a-b > 1 and bootstrap[249] > 0}
            comparisons.append(row)
    report = {'method': 'five alternating independent process pairs; ten warmups and 100 samples per query path; paired bootstrap seed0; software Qt fixture, excludes visible frame rendering',
              'binary_sha256': {name: hashlib.sha256(path.read_bytes()).hexdigest() for name, path in binaries.items()},
              'runs': runs, 'comparison': comparisons}
    args.output.write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps(comparisons, indent=2))


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Summarize an exact-source S17 Rust performance runner record."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import os

SOURCE_DIR = Path(__file__).resolve().parent
BASE = Path(os.environ.get('FERRIC_P00_ARTIFACT_DIR', str(SOURCE_DIR.parents[2] / 'target/site-policy-baseline'))).resolve()
PIN = 'f2c5d6deebd97eb861084503f25613a6a819949d'
MARKER = 'FERRIC_RUST_PERFORMANCE_JSON '
METRICS = ('decode_ms', 'labels_ms', 'interaction_setup_ms', 'ranking_ms', 'snapshot_ms')
COUNTS = (500, 1000, 5000)


def summarize(profile):
    stem = BASE / f'p00-pinned-rust-perf-{profile}'
    record_path = stem.with_suffix('.json')
    log_path = stem.with_suffix('.log')
    record = json.loads(record_path.read_text())
    log = log_path.read_bytes()
    log_hash = hashlib.sha256(log).hexdigest()
    if (record['status'] != 'passed' or record['exit_code'] != 0
            or record['revision'] != PIN or not record['source_postcheck_exact_match']
            or record['harness_overlay'] or record['log_sha256'] != log_hash
            or ('--release' in record['command']) != (profile == 'release')):
        raise ValueError('Runner record does not establish a passing pinned invocation')
    workloads = []
    for line in log.decode().splitlines():
        if not line.startswith(MARKER):
            continue
        row = json.loads(line.removeprefix(MARKER))
        count = row['count']
        samples = row['samples']
        if (row['scenario'] != f'rust-{count}' or row['warmups'] != 10 or len(samples) != 100
                or any(sample['count'] != count or sample['matches'] <= 0 for sample in samples)):
            raise ValueError(f'Malformed workload: {row["scenario"]}')
        metrics = {}
        for metric in METRICS:
            values = [sample[metric] for sample in samples]
            if any(type(value) not in (int, float) or not math.isfinite(value) or value < 0
                   for value in values):
                raise ValueError(f'Malformed timing: {count} {metric}')
            values.sort()
            metrics[metric] = {'p95_ms': values[math.ceil(.95 * len(values)) - 1], 'max_ms': values[-1]}
        workloads.append({'count': count, 'payload_bytes': row['payload_bytes'],
                          'warmups': row['warmups'], 'samples': len(samples), 'metrics': metrics})
    if tuple(workload['count'] for workload in workloads) != COUNTS:
        raise ValueError('Expected one workload for each prescribed count in test order')
    summary = {'revision': PIN, 'profile': profile, 'record': record_path.name,
               'raw_log': log_path.name, 'raw_log_sha256': log_hash,
               'method': 'Nearest-rank p95 and max, with no outlier removal',
               'workloads': workloads}
    destination = BASE / f'{stem.name}-summary.json'
    with destination.open('x') as stream:
        json.dump(summary, stream, indent=2)
        stream.write('\n')
    for row in workloads:
        print(row['count'], ' '.join(f"{name}={values['p95_ms']:.6f}/{values['max_ms']:.6f}"
                                   for name, values in row['metrics'].items()))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('profile', choices=('debug', 'release'))
    summarize(parser.parse_args().profile)

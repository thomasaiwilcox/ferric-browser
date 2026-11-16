#!/usr/bin/env python3
"""Verify and summarize a pinned activation baseline at one DPR."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import os
import re

SOURCE_DIR = Path(__file__).resolve().parent
BASE = Path(os.environ.get('FERRIC_P00_ARTIFACT_DIR', str(SOURCE_DIR.parents[2] / 'target/site-policy-baseline'))).resolve() / 'p00-v9-full-lifetime-activation-1'
WORKLOADS = ((500, .8), (500, 1), (500, 1.25), (1000, 1), (5000, 1))
JOURNEY = 'selected-hint-enter-to-dom-activation'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def nonnegative(value):
    return type(value) in (int, float) and math.isfinite(value) and value >= 0


def exact_receipt(value, index, count):
    if not (isinstance(value, dict) and type(value.get('count')) is int and value['count'] == index + 1
            and type(value.get('id')) is str and type(value.get('trusted')) is bool):
        return False
    match = re.fullmatch(r'target-(0|[1-9][0-9]*)', value['id'])
    return bool(match and int(match[1]) < count and value.get('name') == f'Target {match[1]}')


def summarize(row, events, dpr):
    count, zoom = row['count'], row['zoom']
    if (row['journey'] != JOURNEY or row['dpr'] != dpr or row['qpa'] != 'offscreen'
            or row['run_kind'] != 'performance-baseline' or row['warmups'] != 10
            or row['requested_measured_samples'] != 100 or row['poll_interval_ms'] != 5):
        raise RuntimeError(f'Wrong activation scope for {count} at {zoom}')
    sequence = [event for event in events if (event['count'], event['zoom']) == (count, zoom)]
    samples = row['samples_ms']
    receipts = row['activation_receipts']
    if (len(sequence) > 110 or len(samples) != row['completed_measured_samples'] or len(receipts) != len(samples)
            or any(type(event['iteration']) is not int or event['iteration'] != index
                   or event['smoke'] is not False or event['warmup'] is not (index < 10)
                   or type(event['pre_activation_session_id']) is not str or not event['pre_activation_session_id']
                   or (nonnegative(event['elapsed_ms']) and not exact_receipt(event['activation_receipt'], index, count))
                   for index, event in enumerate(sequence))):
        raise RuntimeError(f'Malformed activation sample events for {count} at {zoom}')
    measured = [event for event in sequence if not event['warmup'] and nonnegative(event['elapsed_ms'])
                and (row['failures'] == 0 or event['iteration'] < row['failed_sample_index'])]
    if [event['elapsed_ms'] for event in measured] != samples or [event['activation_receipt'] for event in measured] != receipts:
        raise RuntimeError(f'Activation sample events disagree with summary row {count} at {zoom}')
    p95 = maximum = no_policy = matched = None
    if row['stage'] == 'complete':
        if (row['failures'] != 0 or len(samples) != 100 or len(sequence) != 110
                or any(not nonnegative(event['elapsed_ms']) for event in sequence)):
            raise RuntimeError('Completed activation workload lacks the prescribed samples')
        ordered = sorted(samples)
        p95, maximum = ordered[94], ordered[-1]
        if (p95, maximum) != (row['p95_ms'], row['max_ms']):
            raise RuntimeError('Reported activation percentiles disagree with retained samples')
        no_policy = p95 + max(2, .1 * p95)
        matched = p95 + max(10, .25 * p95)
    elif row['failures'] != 1 or row['p95_ms'] != -1 or row['max_ms'] != -1:
        raise RuntimeError('Incomplete activation workload claims a percentile')
    return {'dpr': dpr, 'count': count, 'zoom': zoom, 'stage': row['stage'],
            'samples': len(samples), 'observed_warmup_events': sum(event['warmup'] for event in sequence),
            'p95_ms': p95, 'max_ms': maximum, 'no_policy_p95_reference_ms': no_policy,
            'matched_p95_reference_ms': matched, 'observed_targets': sorted({item['id'] for item in receipts}),
            'observed_is_trusted': sorted({item['trusted'] for item in receipts}),
            'failed_sample_index': row['failed_sample_index'], 'failed_endpoint': row['failed_endpoint']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('dpr', type=int, choices=(1, 2))
    parser.add_argument('--matrix-dir', type=Path, default=BASE)
    args = parser.parse_args()
    dpr = args.dpr
    base = args.matrix_dir.resolve()
    matrix = json.loads((base/'matrix.json').read_text())
    (binding,) = (row for row in matrix['runs'] if row['kind'] == 'activation' and row['dpr'] == dpr)
    folder = base/f'activation-dpr{dpr}'
    meta = folder/'run-metadata.json'
    raw = folder/'raw-results.json'
    events_path = folder/'sample-events.json'
    runtime = json.loads(meta.read_text())
    if (binding.get('source_artifact_binding_matches') is not True or binding.get('runtime_record_sha256') != digest(meta)
            or binding.get('status') not in ('passed', 'failed')
            or type(binding.get('exit_code')) is not int or not 0 <= binding['exit_code'] <= 255
            or type(runtime.get('exit_code')) is not int or type(runtime.get('child_exit_code')) is not int
            or runtime.get('interrupted') or runtime.get('harness_error')
            or runtime.get('child_exit_code') != binding.get('exit_code')
            or runtime.get('exit_code') != binding.get('exit_code')
            or (binding['status'] == 'passed') != (binding['exit_code'] == 0)
            or runtime['timed_out'] or runtime['parse_errors'] or runtime['run_kind'] != 'performance-baseline'):
        raise RuntimeError('Activation run lacks a terminal unprofiled build binding')
    rows = json.loads(raw.read_text())
    events = json.loads(events_path.read_text())
    if (tuple((row['count'], row['zoom']) for row in rows) != WORKLOADS
            or any(event['journey'] != JOURNEY or (event['count'], event['zoom']) not in WORKLOADS for event in events)):
        raise RuntimeError('Activation workload matrix does not match the prescribed scope')
    workloads = [summarize(row, events, dpr) for row in rows]
    summary = {'build_record_sha256': matrix['build_record_sha256'], 'runtime_record_sha256': digest(meta),
               'raw_results_sha256': digest(raw), 'raw_events_sha256': digest(events_path),
               'raw_log_sha256': digest(folder/'test.log'), 'boundary': rows[0]['boundary'],
               'method': 'Nearest-rank p95 and max, no outlier removal', 'workloads': workloads}
    with (folder/'verified-baselines.json').open('x') as stream:
        json.dump(summary, stream, indent=2)
        stream.write('\n')
    for row in workloads:
        print(row['count'], row['zoom'], row['stage'], row['samples'], row['p95_ms'], row['max_ms'])


if __name__ == '__main__':
    main()

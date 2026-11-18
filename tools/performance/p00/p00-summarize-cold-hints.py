#!/usr/bin/env python3
"""Verify and summarize an exact-build cold Hint workload at one DPR."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import os

SOURCE_DIR = Path(__file__).resolve().parent
BASE = Path(os.environ.get('FERRIC_P00_ARTIFACT_DIR', str(SOURCE_DIR.parents[2] / 'target/site-policy-baseline'))).resolve() / 'p00-v7-plain-baselines-1'
WORKLOADS = ((500, .8), (500, 1), (500, 1.25), (1000, 1), (5000, 1))
JOURNEY = 'cold-document-hint-to-usable-render'


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def valid_timing(value):
    return type(value) in (int, float) and math.isfinite(value) and value >= 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('dpr', type=int, choices=(1, 2))
    parser.add_argument('--matrix-dir', type=Path, default=BASE)
    args = parser.parse_args()
    dpr = args.dpr
    base = args.matrix_dir.resolve()
    folder = base / f'hints-dpr{dpr}'
    matrix = json.loads((base/'matrix.json').read_text())
    (receipt,) = (row for row in matrix['runs'] if row['kind'] == 'hints' and row['dpr'] == dpr)
    record = folder/'run-metadata.json'
    metadata = json.loads(record.read_text())
    if (receipt.get('source_artifact_binding_matches') is not True
            or receipt.get('runtime_record_sha256') != digest(record)
            or metadata['timed_out'] or metadata['parse_errors']
            or metadata['run_kind'] != 'performance-baseline'):
        raise RuntimeError('No terminal unprofiled result with the exact build binding')
    rows = json.loads((folder/'raw-results.json').read_text())
    events = json.loads((folder/'sample-events.json').read_text())
    if tuple((row['count'],row['zoom']) for row in rows) != WORKLOADS:
        raise RuntimeError('Cold Hint workload matrix is missing or duplicated')
    if any(event['journey'] != JOURNEY or (event['count'],event['zoom']) not in WORKLOADS for event in events):
        raise RuntimeError('Sample event does not match the cold workload matrix')
    results = []
    for raw in rows:
        count, zoom = raw['count'], raw['zoom']
        samples = raw['samples_ms']
        markers = raw['rendered_marker_counts']
        sequence = [event for event in events if (event['count'], event['zoom']) == (count, zoom)]
        if (raw['journey'] != JOURNEY or raw['dpr'] != dpr or raw['qpa'] != 'offscreen'
                or raw['warmups'] != 10 or raw['requested_measured_samples'] != 100
                or raw['completed_measured_samples'] != len(samples) or len(samples) > 100
                or not all(valid_timing(value) for value in samples)
                or [event['iteration'] for event in sequence] != list(range(len(sequence)))
                or any(event['smoke'] is not False or event['warmup'] != (event['iteration'] < 10) for event in sequence)
                or [event['elapsed_ms'] for event in sequence if not event['warmup'] and valid_timing(event['elapsed_ms'])] != samples
                or len(markers) != len(samples) or any(type(value) is not int or not 1 <= value <= count for value in markers)
                or (count == 500 and any(value != 500 for value in markers))):
            raise RuntimeError(f'Invalid DPR {dpr} cold workload {count} at zoom {zoom}')
        complete = raw['stage'] in ('complete', 'thresholds')
        ceiling = 200 if count == 500 else None
        p95 = maximum = planning_ceiling = None
        if complete:
            if len(sequence) != 110 or len(samples) != 100 or len(raw['page_viewports']) != 100:
                raise RuntimeError('Completed workload lacks the prescribed samples or viewport evidence')
            ordered = sorted(samples)
            p95, maximum = ordered[94], ordered[-1]
            if (p95,maximum) != (raw['p95_ms'],raw['max_ms']):
                raise RuntimeError('Cold Hint reported percentiles do not match retained samples')
            planning_ceiling = p95 + max(2, .1*p95)
            if ceiling is not None:
                planning_ceiling = min(ceiling, planning_ceiling)
            threshold_failed = ceiling is not None and p95 > ceiling
            if (raw['stage'] == 'thresholds') != threshold_failed or raw['failures'] != int(threshold_failed):
                raise RuntimeError('Cold Hint status disagrees with the absolute threshold')
            status = 'failed absolute ceiling' if threshold_failed else 'measured'
        else:
            if raw['failures'] != 1 or raw['p95_ms'] != -1 or raw['max_ms'] != -1:
                raise RuntimeError('Incomplete workload claims a percentile')
            status = 'unavailable'
        results.append({'count': count, 'zoom': zoom, 'dpr': dpr, 'status': status, 'stage': raw['stage'],
                        'observed_warmups': sum(event['warmup'] for event in sequence), 'samples': len(samples),
                        'p95_ms': p95, 'max_ms': maximum, 'absolute_p95_ceiling_ms': ceiling,
                        'no_policy_p95_ceiling_ms': planning_ceiling, 'marker_counts': sorted(set(markers)),
                        'failed_sample_index': raw['failed_sample_index'], 'failed_endpoint': raw['failed_endpoint']})
    summary = {'build_record_sha256': matrix['build_record_sha256'], 'runtime_record_sha256': digest(record),
               'raw_log_sha256': digest(folder/'test.log'), 'raw_results_sha256': digest(folder/'raw-results.json'),
               'method': 'Nearest-rank p95 and max, no outlier removal', 'boundary': rows[0]['boundary'],
               'workloads': results}
    with (base/f'cold-hint-dpr{dpr}-baselines.json').open('x') as stream:
        json.dump(summary, stream, indent=2)
        stream.write('\n')
    for row in results:
        print(row['count'], row['zoom'], row['status'], row['samples'], row['p95_ms'], row['max_ms'], row['marker_counts'])


if __name__ == '__main__':
    main()

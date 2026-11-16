#!/usr/bin/env python3
"""Verify and summarize pinned ready and queued Hint timings at one DPR."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import os

SOURCE_DIR = Path(__file__).resolve().parent
BASE = Path(os.environ.get('FERRIC_P00_ARTIFACT_DIR', str(SOURCE_DIR.parents[2] / 'target/site-policy-baseline'))).resolve() / 'p00-v10-ready-rooted-baselines-1'
JOURNEYS = ('ready-prefix-to-hint-render', 'ready-selection-to-hint-render',
            'ready-query-to-hint-render', 'queued-prefix-to-hint-render')
SIZES = ((500, .8), (500, 1), (500, 1.25), (1000, 1), (5000, 1))


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def valid_timing(value):
    return type(value) in (int, float) and math.isfinite(value) and value >= 0


def key(row):
    if type(row['journey']) is not str or type(row['count']) is not int or not valid_timing(row['zoom']):
        raise RuntimeError('Untyped ready workload identity')
    return row['journey'], row['count'], row['zoom']


def verify_event(event, index, count, queued):
    session, epoch = event['session_id'], event['collection_epoch']
    prepared_session, prepared_epoch = event['prepared_session_id'], event['prepared_collection_epoch']
    if (type(event['iteration']) is not int or event['iteration'] != index or event['warmup'] is not (index < 10)
            or event['smoke'] is not False or not valid_timing(event['elapsed_ms'])
            or type(epoch) is not int or type(prepared_epoch) is not int
            or type(session) is not str or not session or type(event['rendered_marker_count']) is not int
            or not 0 < event['rendered_marker_count'] <= count):
        raise RuntimeError(f'Malformed ready sample event {index}')
    if queued:
        if (prepared_session or type(event['previous_session_id']) is not str or not event['previous_session_id']
                or session == event['previous_session_id'] or epoch <= prepared_epoch
                or event['genuinely_queued'] is not True or event['no_session_before_queued_key'] is not True):
            raise RuntimeError(f'Queued sample {index} lacks its pending-key and new-session proof')
    elif (session != prepared_session or epoch != prepared_epoch
            or (count == 500 and (type(event['prepared_marker_count']) is not int
                or event['prepared_marker_count'] != 500))):
        raise RuntimeError(f'Ready sample {index} does not match its rendered starting state')


def summarize(row, events, dpr):
    journey, count, zoom = key(row)
    queued = journey.startswith('queued-')
    sequence = [event for event in events if key(event) == (journey, count, zoom)]
    samples = row['samples_ms']
    if (row['dpr'] != dpr or row['qpa'] != 'offscreen' or row['run_kind'] != 'performance-baseline'
            or row['warmups'] != 10 or row['requested_measured_samples'] != 100
            or len(samples) != row['completed_measured_samples'] or len(samples) > 100 or len(sequence) > 110
            or any(type(event['iteration']) is not int or event['iteration'] != index
                   for index, event in enumerate(sequence))):
        raise RuntimeError(f'Invalid ready sample matrix: {journey}, {count}, {zoom}')
    accepted_end = row['failed_sample_index'] + int(row['stage'] == 'untimed-reset')
    accepted = [event for event in sequence if row['failures'] == 0 or event['iteration'] < accepted_end]
    for index, event in enumerate(accepted):
        verify_event(event, index, count, queued)
    measured = accepted[10:]
    for event_key, row_key in (('elapsed_ms', 'samples_ms'), ('session_id', 'session_ids'),
                               ('collection_epoch', 'collection_epochs'), ('rendered_marker_count', 'rendered_marker_counts')):
        if [event[event_key] for event in measured] != row[row_key]:
            raise RuntimeError(f'Accepted ready events disagree with {row_key}: {journey}, {count}, {zoom}')
    p95 = maximum = no_policy = None
    if row['stage'] == 'complete':
        if row['failures'] != 0 or len(accepted) != 110 or len(samples) != 100:
            raise RuntimeError('Completed ready workload lacks its full sample set')
        ordered = sorted(samples)
        p95, maximum = ordered[94], ordered[-1]
        if (p95, maximum) != (row['p95_ms'], row['max_ms']):
            raise RuntimeError('Ready percentiles disagree with raw samples')
        no_policy = p95 + max(2, .1 * p95)
    elif row['failures'] != 1 or row['p95_ms'] != -1 or row['max_ms'] != -1:
        raise RuntimeError('Incomplete ready workload claims a percentile')
    return {'journey': journey, 'dpr': dpr, 'count': count, 'zoom': zoom, 'stage': row['stage'],
            'valid_warmups': min(10, len(accepted)), 'samples': len(samples), 'p95_ms': p95, 'max_ms': maximum,
            'no_policy_p95_reference_ms': no_policy, 'boundary': row['boundary'],
            'rendered_marker_counts': sorted(set(row['rendered_marker_counts'])),
            'failed_sample_index': row['failed_sample_index'], 'failed_endpoint': row['failed_endpoint']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('dpr', type=int, choices=(1, 2))
    parser.add_argument('--matrix-dir', type=Path, default=BASE)
    args = parser.parse_args()
    base = args.matrix_dir.resolve()
    matrix = json.loads((base/'matrix.json').read_text())
    (binding,) = (row for row in matrix['runs'] if row['kind'] == 'ready' and row['dpr'] == args.dpr)
    folder = base/f'ready-dpr{args.dpr}'
    meta, raw, events_path = (folder/name for name in ('run-metadata.json', 'raw-results.json', 'sample-events.json'))
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
        raise RuntimeError('Ready run lacks its terminal unprofiled source and artifact binding')
    rows, events = json.loads(raw.read_text()), json.loads(events_path.read_text())
    expected = {(journey, count, zoom) for journey in JOURNEYS for count, zoom in SIZES}
    if len(rows) != len(expected) or {key(row) for row in rows} != expected or any(key(event) not in expected for event in events):
        raise RuntimeError('Ready workload matrix is incomplete or duplicated')
    workloads = [summarize(row, events, args.dpr) for row in rows]
    summary = {'build_record_sha256': matrix['build_record_sha256'], 'runtime_record_sha256': digest(meta),
               'raw_results_sha256': digest(raw), 'raw_events_sha256': digest(events_path),
               'raw_log_sha256': digest(folder/'test.log'), 'method': 'Nearest-rank p95 and max, no outlier removal',
               'workloads': workloads}
    with (folder/'verified-baselines.json').open('x') as stream:
        json.dump(summary, stream, indent=2)
        stream.write('\n')
    for row in workloads:
        print(row['journey'], row['count'], row['zoom'], row['stage'], row['samples'], row['p95_ms'], row['max_ms'])


if __name__ == '__main__':
    main()

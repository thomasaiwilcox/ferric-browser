#!/usr/bin/env python3
"""Audit exact-build rooted rendering or provider observations."""
import argparse
import hashlib
import json
import math
from pathlib import Path

SHAPES = ('shadow-2', 'slots', 'same-origin-2', 'foreign-2', 'clipping')
SIZES = ((500, .8), (500, 1), (500, 1.25), (1000, 1), (5000, 1))


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def valid_timing(value):
    return type(value) in (int, float) and math.isfinite(value) and value >= 0


def workload_key(row, provider):
    if type(row['count']) is not int or not valid_timing(row['zoom']) or type(row['shape']) is not str:
        raise RuntimeError('Untyped root workload dimensions')
    key = (row['shape'], row['count'], row['zoom'])
    if provider:
        if type(row['metric']) is not str or type(row['independent_small_control']) is not bool:
            raise RuntimeError('Untyped provider workload dimensions')
        key = (row['metric'], *key, row['independent_small_control'])
    return key


def facts_match(facts, row):
    count, shape = row['count'], row['shape']
    depth = 2 if shape == 'shadow-2' else 1 if shape == 'slots' else 0
    expected = {'targetCount': count, 'requested': count,
                'shadowDepthMin': depth, 'shadowDepthMax': depth, 'shadowRoots': count * depth,
                'slots': count if shape == 'slots' else 0,
                'frameHops': 2 if shape in ('same-origin-2', 'foreign-2') else 0,
                'clippedSentinels': 2 if shape == 'clipping' else 0,
                'physicalButtons': count + (2 if shape == 'clipping' else 0)}
    return (isinstance(facts, dict) and facts.get('shape') == shape
            and all(type(facts.get(key)) is int and facts[key] == value for key, value in expected.items())
            and facts.get('freshControl') is row.get('independent_small_control', False)
            and type(facts.get('layoutGeneration')) is int and facts['layoutGeneration'] >= 0
            and all(isinstance(facts.get(viewport), dict)
                    and all(valid_timing(facts[viewport].get(key)) and facts[viewport][key] > 0
                            for key in ('width', 'height')) for viewport in ('viewport', 'leafViewport')))


def event_key(event, provider):
    journey = event['journey']
    if not provider:
        if journey != 'rooted-buttons-command-to-usable-render':
            raise RuntimeError('Unexpected rooted render event')
        return workload_key(event, False), 'render'
    journeys = {'rooted-provider-collection': ('collect', 'collect'),
                'rooted-provider-fresh-collection-setup': ('fresh', 'collect'),
                'rooted-provider-fresh': ('fresh', 'fresh')}
    if journey not in journeys:
        raise RuntimeError('Unexpected rooted provider event')
    metric, operation = journeys[journey]
    if ((operation == 'collect' or 'metric' in event) and event.get('metric') != metric):
        raise RuntimeError('Provider event metric contradicts its journey')
    return workload_key(dict(event, metric=metric), True), operation


def verify_events(row, events, provider, smoke):
    warmups, requested = (1, 2) if smoke else (10, 100)
    complete = row['stage'] in ('smoke-complete', 'complete', 'thresholds')
    subject = workload_key(row, provider)
    groups = {'collect': [], 'fresh': []} if provider else {'render': []}
    order = []
    for event in events:
        key, operation = event_key(event, provider)
        if key == subject:
            groups[operation].append(event)
            order.append((operation, event['iteration']))
    if provider and row['metric'] == 'fresh':
        expected_order = [(operation, index) for index in range(warmups + requested)
                          for operation in ('collect', 'fresh')]
        if order != expected_order[:len(order)]:
            raise RuntimeError('Fresh provider events do not follow their own collection setup')
    for operation, sequence in groups.items():
        active = operation != 'fresh' or row['metric'] == 'fresh'
        wanted = complete and active
        if wanted and len(sequence) != warmups + requested:
            raise RuntimeError('Completed rooted workload lacks its complete event sequence')
        if provider and row['metric'] == 'collect' and operation == 'fresh' and sequence:
            raise RuntimeError('Collection-only workload contains fresh events')
        accepted_count = warmups + requested if wanted else 0
        if not complete:
            stage, failed_index = row['stage'], row['failed_sample_index']
            stages = ({'setup', 'fresh-document', 'warmup-collection', 'measured-collection',
                       'warmup-fresh', 'measured-fresh', 'untimed-cleanup'} if provider else
                      {'setup', 'fresh-document', 'warmup-entry', 'measured-entry', 'untimed-reset'})
            if (stage not in stages or type(failed_index) is not int
                    or (provider and row['metric'] == 'collect' and stage in ('warmup-fresh', 'measured-fresh'))
                    or (stage == 'setup' and failed_index != -1)
                    or (stage != 'setup' and not 0 <= failed_index < warmups + requested)
                    or len(sequence) > failed_index + 1):
                raise RuntimeError('Rooted events contradict the recorded failure point')
            current_accepted = (stage == 'untimed-cleanup' if provider else stage == 'untimed-reset')
            if provider and operation == 'collect' and row['metric'] == 'fresh':
                current_accepted |= stage in ('warmup-fresh', 'measured-fresh')
            if active and failed_index >= 0:
                accepted_count = failed_index + int(current_accepted)
        for index, event in enumerate(sequence):
            if (type(event['iteration']) is not int or event['iteration'] != index
                    or index >= warmups + requested or event['warmup'] is not (index < warmups)
                    or event['smoke'] is not smoke
                    or type(event['elapsed_ms']) not in (int, float)
                    or not math.isfinite(event['elapsed_ms'])
                    or event['elapsed_ms'] < -1):
                raise RuntimeError('Malformed or duplicated rooted sample event')
            if operation != 'fresh' and not facts_match(event['fixture_root_facts'], row):
                raise RuntimeError('Rooted event fixture facts contradict its subject')
        if operation == 'render':
            values = row['samples_ms']
            fields = (('rendered_marker_count', 'rendered_marker_counts'),
                      ('fixture_root_facts', 'fixture_root_facts'),
                      ('collection_truncated', 'collection_truncated'))
        elif operation == 'collect':
            values = row['collection_samples_ms']
            fields = (('inspected_nodes', 'inspected_nodes'),
                      ('fixture_root_facts', 'fixture_root_facts'),
                      ('truncated', 'collection_truncated'))
        else:
            values, fields = row['fresh_samples_ms'], ()
        if len(sequence) < accepted_count or len(values) != max(0, accepted_count - warmups):
            raise RuntimeError('Rooted accepted samples contradict the recorded failure point')
        measured = sequence[warmups:warmups + len(values)]
        if [event['elapsed_ms'] for event in measured] != values:
            raise RuntimeError('Rooted event timings disagree with accepted row samples')
        for event_field, row_field in fields:
            if [event[event_field] for event in measured] != row[row_field]:
                raise RuntimeError('Rooted events disagree with ' + row_field)
        # Failed attempts can emit an event before a later assertion rejects
        # it. Only completed rows and explicitly retained samples imply success.
        accepted = sequence[:accepted_count]
        for event in accepted:
            if not valid_timing(event['elapsed_ms']):
                raise RuntimeError('Accepted rooted event has no valid latency')
            if operation == 'render':
                markers = event['rendered_marker_count']
                if (event['command_accepted'] is not True or type(markers) is not int
                        or not 0 < markers <= row['count'] or (row['count'] == 500 and markers != 500)
                        or type(event['collection_truncated']) is not bool):
                    raise RuntimeError('Accepted rooted render event lacks its endpoint')
            elif (type(event['callback_count']) is not int or event['callback_count'] != 1
                  or event['error'] != ''):
                raise RuntimeError('Accepted provider event lacks one successful callback')
            if operation == 'collect':
                if (type(event['candidate_count']) is not int or event['candidate_count'] != row['count']
                        or type(event['inspected_nodes']) is not int
                        or not row['count'] <= event['inspected_nodes'] <= 50000
                        or type(event['truncated']) is not bool):
                    raise RuntimeError('Accepted provider event lacks its complete collection')
            elif operation == 'fresh' and event['visible'] is not True:
                raise RuntimeError('Accepted fresh event lacks a visible proof')
    return sum(len(sequence) for sequence in groups.values())


def verify_row(row, dpr, provider, smoke):
    warmups, requested = (1, 2) if smoke else (10, 100)
    expected_kind = 'functional-smoke-not-baseline' if smoke else 'performance-baseline'
    samples = row['samples_ms']
    journey = ('rooted-provider-' + ('collection' if row['metric'] == 'collect' else 'fresh')
               if provider else 'rooted-buttons-command-to-usable-render')
    if (row['dpr'] != dpr or row['qpa'] != 'offscreen' or row['run_kind'] != expected_kind
            or row['journey'] != journey
            or row['warmups'] != warmups or row['requested_measured_samples'] != requested
            or len(samples) != row['completed_measured_samples'] or len(samples) > requested
            or not all(valid_timing(value) for value in samples)):
        raise RuntimeError(f'Invalid rooted samples: {workload_key(row, provider)}')
    complete = row['stage'] in ('smoke-complete', 'complete', 'thresholds')
    p95 = maximum = no_policy = matched = None
    absolute = 200 if not provider and row['count'] == 500 else None
    if provider:
        collections, fresh = row['collection_samples_ms'], row['fresh_samples_ms']
        measuring_fresh = row['metric'] == 'fresh'
        if (samples != (fresh if measuring_fresh else collections)
                or len(collections) != row['completed_collection_samples'] or len(fresh) != row['completed_fresh_samples']
                or not all(valid_timing(value) for value in collections + fresh)
                or (not measuring_fresh and fresh)):
            raise RuntimeError('Root provider primary and setup samples disagree')
        if complete and (len(collections) != requested or row['cleanup_failures'] != 0
                         or (measuring_fresh and row['fresh_availability'] != 'observed-valid')):
            raise RuntimeError('Completed provider workload lacks setup, fresh proof or cleanup')
    if complete:
        if len(samples) != requested or len(row['fixture_root_facts']) != requested:
            raise RuntimeError('Completed rooted workload lacks samples or fixture facts')
        if not all(facts_match(fact, row) for fact in row['fixture_root_facts']):
            raise RuntimeError('Root fixture facts do not match the prescribed subject')
        if not provider:
            markers, paths = row['rendered_marker_counts'], row['observed_frame_paths']
            expected_path = ['0.0.0'] if row['shape'] == 'same-origin-2' else ['0']
            if (len(markers) != requested or paths != [expected_path] * requested
                    or any(type(count) is not int or not 0 < count <= row['count'] for count in markers)
                    or (row['count'] == 500 and markers != [500] * requested)):
                raise RuntimeError('Root render markers or frame paths disagree with the fixture')
        if smoke:
            if row['failures'] != 0 or row['stage'] != 'smoke-complete':
                raise RuntimeError('Smoke row has the wrong completion stage')
        else:
            ordered = sorted(samples)
            p95, maximum = ordered[94], ordered[-1]
            if (p95, maximum) != (row['p95_ms'], row['max_ms']):
                raise RuntimeError('Rooted percentile does not match retained samples')
            threshold_failed = absolute is not None and p95 > absolute
            if row['failures'] != int(threshold_failed) or (row['stage'] == 'thresholds') != threshold_failed:
                raise RuntimeError('Rooted outcome disagrees with the absolute gate')
            no_policy = p95 + max(2, .1 * p95)
            matched = p95 + max(10, .25 * p95)
            if absolute is not None:
                no_policy, matched = min(absolute, no_policy), min(absolute, matched)
    elif row['failures'] != 1 or any(row[key] not in (-1, None) for key in ('p95_ms', 'max_ms')):
        raise RuntimeError('Incomplete rooted workload claims a percentile')
    keys = ('journey', 'shape', 'count', 'zoom', 'dpr', 'stage', 'failures', 'boundary',
            'failed_sample_index', 'failed_endpoint')
    result = {key: row[key] for key in keys}
    result.update(samples=len(samples), p95_ms=p95, max_ms=maximum,
                  absolute_p95_ceiling_ms=absolute, no_policy_p95_reference_ms=no_policy,
                  matched_p95_reference_ms=matched)
    if provider:
        for key in ('metric', 'independent_small_control', 'completed_collection_samples',
                    'completed_fresh_samples', 'fresh_availability', 'cleanup_failures'):
            result[key] = row[key]
    else:
        result['observed_marker_counts'] = sorted(set(row['rendered_marker_counts']))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('matrix_dir', type=Path)
    parser.add_argument('kind', choices=('roots', 'root-stages'))
    parser.add_argument('dpr', type=int, choices=(1, 2))
    args = parser.parse_args()
    matrix = json.loads((args.matrix_dir/'matrix.json').read_text())
    (binding,) = (row for row in matrix['runs'] if row['kind'] == args.kind and row['dpr'] == args.dpr)
    folder = args.matrix_dir/f'{args.kind}-dpr{args.dpr}'
    meta, raw, event_path = (folder/name for name in ('run-metadata.json', 'raw-results.json', 'sample-events.json'))
    runtime = json.loads(meta.read_text())
    if (binding.get('source_artifact_binding_matches') is not True or binding.get('runtime_record_sha256') != digest(meta)
            or binding.get('status') not in ('passed', 'failed')
            or type(binding.get('exit_code')) is not int or not 0 <= binding['exit_code'] <= 255
            or type(runtime.get('exit_code')) is not int or type(runtime.get('child_exit_code')) is not int
            or runtime.get('interrupted') or runtime.get('harness_error')
            or runtime.get('child_exit_code') != binding.get('exit_code')
            or runtime.get('exit_code') != binding.get('exit_code')
            or (binding['status'] == 'passed') != (binding['exit_code'] == 0)
            or runtime['timed_out'] or runtime['parse_errors']):
        raise RuntimeError('Rooted run lacks a terminal exact build binding')
    smoke = runtime['run_kind'] == 'functional-smoke-not-baseline'
    provider = args.kind == 'root-stages'
    expected = {(shape, count, zoom) for shape in SHAPES for count, zoom in SIZES}
    if provider:
        expected = {(metric, *key, False) for metric in ('collect', 'fresh') for key in expected}
        expected.update(('fresh', shape, 1, 1, True) for shape in SHAPES)
    rows, events = json.loads(raw.read_text()), json.loads(event_path.read_text())
    if len(rows) != len(expected) or {workload_key(row, provider) for row in rows} != expected:
        raise RuntimeError('Rooted fixture matrix is incomplete or duplicated')
    if any(event_key(event, provider)[0] not in expected for event in events):
        raise RuntimeError('Rooted event belongs to an unexpected workload')
    workloads = [verify_row(row, args.dpr, provider, smoke) for row in rows]
    event_count = sum(verify_events(row, events, provider, smoke) for row in rows)
    if event_count != len(events):
        raise RuntimeError('Rooted sample events are not fully accounted for')
    summary = {'build_record_sha256': matrix['build_record_sha256'], 'runtime_record_sha256': digest(meta),
               'raw_results_sha256': digest(raw), 'raw_log_sha256': digest(folder/'test.log'),
               'raw_events_sha256': digest(event_path), 'verified_sample_events': event_count,
               'method': 'Functional smoke, no percentiles' if smoke else 'Nearest-rank p95 and max, no outlier removal',
               'workloads': workloads}
    with (folder/'rooted-audit.json').open('x') as stream:
        json.dump(summary, stream, indent=2)
        stream.write('\n')
    print(len(workloads), sum(row['failures'] == 0 for row in workloads), f'{args.kind} DPR {args.dpr}')


if __name__ == '__main__':
    main()

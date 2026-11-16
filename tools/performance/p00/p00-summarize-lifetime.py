#!/usr/bin/env python3
"""Verify the P00 lifecycle observations and summarize sampled resource trends."""
import argparse
import collections
import hashlib
import json
from pathlib import Path
import os

SOURCE_DIR = Path(__file__).resolve().parent
BASE = Path(os.environ.get('FERRIC_P00_ARTIFACT_DIR', str(SOURCE_DIR.parents[2] / 'target/site-policy-baseline'))).resolve() / 'p00-v9-full-lifetime-activation-1'
JOURNEYS = {
    'ordinary-established-lifetime': (1000, 100, 1),
    'foreign-established-lifetime': (1000, 100, 2),
    'shared-profile-window-lifetime': (100, 10, 0),
    'private-profile-window-lifetime': (100, 10, 0),
}
OWNED = {'hint_visible': False, 'leases': 0, 'mode': 'normal', 'pending_inputs': 0,
         'pending_view': False, 'poll_pending': False, 'provider_view': False,
         'refresh_pending': False, 'root_owner': False, 'scoped_owner': False,
         'sources': 0, 'tracking_view': False}
RESOURCES = ('pss_kib', 'rss_kib', 'fd_count', 'threads', 'process_count')


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def summarize(row):
    journey = row['journey']
    requested, step, realm_count = JOURNEYS[journey]
    snapshots = row['snapshots']
    wanted = list(range(0, requested + 1, step))
    if (row['run_kind'] != 'lifecycle-baseline' or row['dpr'] != 1 or row['qpa'] != 'offscreen'
            or row['stage'] != 'complete' or row['failures'] != 0
            or row['requested_cycles'] != requested or row['completed_cycles'] != requested
            or row['warmups'] != 10 or row['completed_warmups'] != 10
            or [snapshot['cycle'] for snapshot in snapshots] != wanted):
        raise RuntimeError(f'Incomplete lifecycle matrix: {journey}')
    resources = []
    for snapshot in snapshots:
        observed = snapshot['process_resources']
        if not (observed['complete'] is True and observed['pss_complete'] is True
                and len(observed['processes']) == observed['process_count']):
            raise RuntimeError(f'Incomplete process observations: {journey}')
        for key in RESOURCES[:-1]:
            if (type(observed[key]) is not int or observed[key] < 0
                    or any(process.get('available') is not True or process.get('identity_stable') is not True
                           or type(process.get(key)) is not int or process[key] < 0 for process in observed['processes'])
                    or observed[key] != sum(process[key] for process in observed['processes'])):
                raise RuntimeError(f'Incomplete process {key} observations: {journey}')
        resources.append({'cycle': snapshot['cycle'], **{key: observed[key] for key in RESOURCES},
                          'qt_descendants': snapshot['qt_descendants'], 'qt_windows': snapshot['qt_windows']})
    result = {'journey': journey, 'cycles': requested, 'warmups': 10, 'checkpoints': resources}
    if realm_count:
        documents = [realm['document'] for realm in snapshots[0]['realms']]
        for snapshot in snapshots:
            realms = snapshot['realms']
            if (snapshot['cleanup_clean'] is not True or snapshot['owned'] != OWNED
                    or len(realms) != realm_count or [realm['document'] for realm in realms] != documents):
                raise RuntimeError(f'Unexpected renderer ownership subject: {journey}')
            for realm in realms:
                if (realm['available'] is not True or realm['shape_valid'] is not True or realm['clean'] is not True
                        or realm['before_close']['document'] != realm['document']
                        or any(realm[key] != 0 for key in ('elements', 'observers', 'windows'))
                        or realm['tracking'] is not False or realm['scope'] is not False):
                    raise RuntimeError(f'Renderer cleanup not established: {journey}')
        result['same_documents_after_close'] = len(documents)
        result['observed_enumerated_renderer_and_browser_state_clean'] = True
    else:
        outcomes = row['outcomes']
        baseline_ledger = snapshots[0]['ledger']
        if (len(outcomes) != requested + 10 or [outcome['index'] for outcome in outcomes] != list(range(-10, requested))
                or any(outcome['warmup'] is not (outcome['index'] < 0) or outcome['released'] is not True
                       or outcome['initial_rendered_close_confirmed'] is not True or outcome['view_destroyed'] is not True
                       or outcome['ledger_returned_to_baseline'] is not True or outcome['observed_ledger'] != baseline_ledger
                       or outcome['live_windows'] != 1 for outcome in outcomes)
                or any(snapshot['ledger'] != baseline_ledger or snapshot['live_windows'] != 1 for snapshot in snapshots)):
            raise RuntimeError(f'Window teardown outcome missing: {journey}')
        result['window_outcomes_including_warmups'] = dict(collections.Counter(
            'host_destroyed' if outcome['host_destroyed'] else 'host_hidden' if outcome['host_hidden'] else 'other'
            for outcome in outcomes))
        result['private_or_shared_group_destroyed'] = sum(outcome['group_destroyed'] for outcome in outcomes)
    peak = max(resources, key=lambda item: item['pss_kib'])
    result['pss_peak'] = {'cycle': peak['cycle'], 'kib': peak['pss_kib']}
    result['pss_final_minus_warmed_kib'] = resources[-1]['pss_kib'] - resources[0]['pss_kib']
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--matrix-dir", type=Path, default=BASE)
    args = parser.parse_args()
    base = args.matrix_dir.resolve()
    matrix = json.loads((base/'matrix.json').read_text())
    (receipt,) = (row for row in matrix['runs'] if row['kind'] == 'lifetime' and row['dpr'] == 1)
    folder = base/'lifetime-dpr1'
    metadata = folder/'run-metadata.json'
    raw = folder/'raw-results.json'
    if (receipt['status'] != 'passed' or receipt['source_artifact_binding_matches'] is not True
            or receipt['runtime_record_sha256'] != digest(metadata)):
        raise RuntimeError('Lifecycle run lacks a terminal passing build binding')
    rows = json.loads(raw.read_text())
    if [row['journey'] for row in rows] != list(JOURNEYS):
        raise RuntimeError('Unexpected lifecycle journeys')
    summary = {'build_record_sha256': matrix['build_record_sha256'], 'runtime_record_sha256': digest(metadata),
               'raw_results_sha256': digest(raw), 'raw_log_sha256': digest(folder/'test.log'),
               'scope': 'Observed process and enumerated ownership state at lifecycle checkpoints. No detached-DOM or allocation attribution.',
               'workloads': [summarize(row) for row in rows]}
    with (folder/'verified-lifetime.json').open('x') as stream:
        json.dump(summary, stream, indent=2)
        stream.write('\n')
    for row in summary['workloads']:
        print(row['journey'], row['cycles'], row['pss_peak'], row['pss_final_minus_warmed_kib'],
              row.get('window_outcomes_including_warmups'))


if __name__ == '__main__':
    main()

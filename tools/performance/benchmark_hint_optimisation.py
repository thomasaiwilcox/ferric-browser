#!/usr/bin/env python3
"""Alternate grants to two current-source native runners; record exact projections."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import time


def rows(path):
    marker = 'FERRIC_OPT_SAMPLE '
    return [json.loads(line.split(marker, 1)[1]) for line in path.read_text().splitlines() if marker in line]


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--controls', action='store_true')
    parser.add_argument('--smoke', action='store_true')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    root = Path(__file__).resolve().parents[2]
    cases = [(kind, 300, scroll) for kind in ['collection', 'full'] for scroll in [0, 1800]]
    if args.controls:
        cases = [('none', 8, 0), ('none', 300, 0), ('keys', 300, 0), ('anchor', 300, 0),
                 ('collection', 30, 0), ('collection', 100, 0)]
    warmups, measured = (1, 2) if args.smoke else (5, 10 if args.controls else 20)
    summary = {'baseline_sha256': digest(args.baseline), 'candidate_sha256': digest(args.candidate),
               'endpoint': 'constructed Qt input to synchronized/swapped labels; excludes compositor presentation',
               'cases': []}
    try:
        for kind, cards, scroll in cases:
            processes, logs, gates = {}, {}, {}
            label = f'{kind}-{cards}-{scroll}'
            try:
                for role, binary in [('baseline', args.baseline), ('candidate', args.candidate)]:
                    gate = args.output / f'{label}-{role}-gate'
                    gate.mkdir()
                    gates[role] = gate
                    path = args.output / f'{label}-{role}.log'
                    logs[role] = path
                    env = dict(os.environ, QT_QPA_PLATFORM='offscreen', QT_QUICK_BACKEND='software',
                               QT_QPA_PLATFORMTHEME='', QTWEBENGINE_CHROMIUM_FLAGS='--disable-gpu',
                               QV4_FORCE_INTERPRETER='1', FERRIC_OPT_PERF='1', FERRIC_OPT_CASE=kind,
                               FERRIC_OPT_CARDS=str(cards), FERRIC_OPT_SCROLL=str(scroll),
                               FERRIC_OPT_SAMPLES=str(measured), FERRIC_OPT_GATE=str(gate.resolve()))
                    if args.smoke:
                        env['FERRIC_OPT_SMOKE'] = '1'
                    stream = path.open('w')
                    process = subprocess.Popen([str(root / 'tools/performance/with_qt611.sh'),
                                                str(binary.resolve()), 'hintCollectionOptimisationPerformance',
                                                '-maxwarnings', '0'], env=env, stdout=stream, stderr=subprocess.STDOUT)
                    processes[role] = (process, stream)
                paired = []
                for index in range(warmups + measured):
                    order = ['baseline', 'candidate'] if index % 2 == 0 else ['candidate', 'baseline']
                    sample = {}
                    for role in order:
                        (gates[role] / f'go-{index}').touch()
                        until = time.monotonic() + 45
                        while True:
                            found = next((r for r in rows(logs[role]) if r['iteration'] == index), None)
                            if found is not None:
                                sample[role] = found
                                break
                            process = processes[role][0]
                            if process.poll() is not None or time.monotonic() >= until:
                                raise RuntimeError(f'{label}/{role} did not complete sample {index}: {logs[role]}')
                            time.sleep(.01)
                    if sample['baseline']['projection'] != sample['candidate']['projection']:
                        raise RuntimeError(f'{label} projection mismatch at {index}')
                    if sample['baseline']['viewport'] != sample['candidate']['viewport']:
                        raise RuntimeError(f'{label} viewport mismatch at {index}')
                    if index >= warmups:
                        paired.append({role: sample[role]['ms'] for role in ['baseline', 'candidate']})
                for process, _ in processes.values():
                    if process.wait(timeout=20) != 0:
                        raise RuntimeError(f'{label} runner failed; inspect logs')
                base = statistics.median(p['baseline'] for p in paired)
                candidate = statistics.median(p['candidate'] for p in paired)
                result = {'case': label, 'pairs': paired, 'baseline_median_ms': base,
                          'candidate_median_ms': candidate, 'gain_ms': base - candidate,
                          'gain_percent': (base - candidate) * 100 / base,
                          'projections_equal': True, 'warmups': warmups}
                summary['cases'].append(result)
                print(json.dumps({k: v for k, v in result.items() if k != 'pairs'}), flush=True)
            finally:
                for process, stream in processes.values():
                    if process.poll() is None:
                        process.terminate()
                        try:
                            process.wait(timeout=10)
                        except subprocess.TimeoutExpired:
                            process.kill()
                            process.wait(timeout=5)
                    stream.close()
    finally:
        (args.output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')


if __name__ == '__main__':
    main()

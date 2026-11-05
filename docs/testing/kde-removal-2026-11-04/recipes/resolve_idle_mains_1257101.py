"""Reassess idle after the frozen baseline also exceeded its early range.

Retain all four actual latency workloads and the passing twenty-window idle
workload unchanged. Collect five fresh one-window baseline controls immediately
before five alternating one-window idle pairs.
Use sixty-second intervals for both products, improving CPU counter resolution
from 0.05 to 0.0167 percentage points. Keep the original verifier unchanged.
"""
from pathlib import Path
from types import SimpleNamespace
import importlib.util
import json
import statistics

root = Path.cwd()
spec = importlib.util.spec_from_file_location('pair', root / 'tools/performance/kde_free_pair.py')
pair = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pair)
q = pair.qualification
source = q.snapshot()
assert source['revision'] == '1257101f6eead8e369d2ced87fd773ad6850da5b'
original_path = root / 'target/kde-free-performance-mains-1257101/comparison.json'
original = json.loads(original_path.read_text())
assert set(original['workloads']) == {'startup-1', 'startup-20', 'creation', 'recreation', 'idle-1', 'idle-20'}
assert original['candidate_source'] == source and original['pilot'] is False
assert (original['pairs'], original['warmups'], original['samples']) == (5, 10, 100)
args = SimpleNamespace(
    baseline=q.checked(original['baseline_artifact']),
    candidate=q.checked(original['candidate_artifact']),
    probe=q.checked(original['probe']),
    plugins=root / 'target/kde-free-performance-helper/plugins',
    baseline_plugins=Path('/home/tom/Projects/ferric-everyday-work/builds/kwindowsystem-qt6112-F01b/prefix/lib/qt6/plugins'),
    output=root / 'target/kde-free-performance-idle-resolution-1257101',
    source=source, warmups=10, samples=100, idle_seconds=60,
)
args.output.mkdir(parents=True, exist_ok=True)
report = {key: value for key, value in original.items() if key != 'workloads'}
report['workloads'] = {name: value for name, value in original['workloads'].items() if name != 'idle-1'}
report['prior_measurement'] = q.reference(original_path)
report['method'] += '; one-window idle reassessment: preserve original latency and twenty-window idle blocks; fresh baseline controls immediately precede five paired measurements, sixty-second intervals for both products, original unchanged variability limit'
investigation = {'source': source, 'original_comparison': q.reference(original_path), 'reason': 'Both frozen-baseline paired idle measurements and the candidate exceeded the early baseline-only range after six hours; original result remains inconclusive. Fresh controls precede each fresh comparison; sixty-second CPU intervals improve counter resolution. No thresholds, product or tracked harness source changed.', 'original_idle_metrics': {name: value['metrics'] for name, value in original['workloads'].items() if name.startswith('idle')}}
pair.atomic(args.output / 'investigation.json', investigation)
for name in ['idle-1']:
    repetitions = [pair.measure(args, name, 'baseline', f'repeat-{index}', True) for index in range(5)]
    rows = []
    for index in range(5):
        for label in (['baseline', 'candidate'] if index % 2 == 0 else ['candidate', 'baseline']):
            rows.append(pair.measure(args, name, label, index))
    entry = {'raw_receipts': [q.reference(args.output / f'{name}-{row["pair"]}-{row["label"]}.json') for row in rows], 'metrics': {}}
    for field in ['tree_cpu_percent', 'gui_wakeups_per_second']:
        def summary(row):
            return statistics.mean(value[field] for value in row['values'])
        entry['metrics'][field] = {label: statistics.median(summary(row) for row in rows if row['label'] == label) for label in ['baseline', 'candidate']}
        entry['metrics'][field]['baseline_repetitions'] = [summary(row) for row in repetitions]
        entry['metrics'][field]['baseline_raw_receipts'] = [q.reference(args.output / f'{name}-{row["pair"]}-baseline.json') for row in repetitions]
    report['workloads'][name] = entry
    pair.atomic(args.output / 'comparison.json', report)
assert q.snapshot() == source
q.verify_performance(report, source, original['candidate_artifact'])
print('All six workloads pass the unchanged performance verifier, including fresh sixty-second idle controls', flush=True)

from pathlib import Path
import argparse
import importlib.util
import json
import subprocess
import xml.etree.ElementTree as ET

parser = argparse.ArgumentParser()
parser.add_argument('--preflight', action='store_true')
args = parser.parse_args()
root = Path.cwd()
out = root / 'target/kde-free-evidence/qualified-1257101'
spec = importlib.util.spec_from_file_location('qualification', root / 'tools/qualification/kde_free.py')
q = importlib.util.module_from_spec(spec)
spec.loader.exec_module(q)
source = q.snapshot()
assert source['revision'] == '1257101f6eead8e369d2ced87fd773ad6850da5b'
sdk = json.loads((out / 'sdk.json').read_text())
assert sdk['source'] == source
q.checked(sdk['private_header'])
for ref in sdk['libraries'].values():
    q.checked(ref)
baseline = q.baseline_control(q.reference(out / 'baseline.json'), source, sdk)
mapping = {'application': 'application-final-complete',
           'public-page-host': 'public-page-host-resumed',
           'everyday': 'everyday-final-resumed', 'hyprland': 'hyprland-resumed'}
runs = {}
for name in sorted(q.REQUIRED_RUNS):
    path = out / (mapping.get(name, name) + '.json')
    run = json.loads(path.read_text())
    assert run['source'] == run['source_after'] == source and not run.get('capture_errors'), name
    q.checked(run['log'])
    if name == 'public-page-host':
        assert run['returncode'] in {0, 8}
        print('Preserved public-host baseline failures:', q.compare_baseline(
            ET.parse(q.checked(run['test_report'])).getroot(), baseline), flush=True)
    else:
        assert run['returncode'] == 0, name
        if 'test_report' in run:
            report = ET.parse(q.checked(run['test_report'])).getroot()
            assert list(report.iter('testcase'))
            assert not list(report.iter('failure')) + list(report.iter('error')) + list(report.iter('skipped')), name
    runs[name] = q.reference(path)
artifact = q.reference(root / 'target/kde-free-install-1257101/usr/bin/ferric-browser')
assert artifact['sha256'] == 'fdd8955dc8f5ec84e5316b4d57e9665887f883b9a96f3675a8c6002fea36bb4e'
for name in ['fresh-build', 'install']:
    assert json.loads(q.checked(runs[name]).read_text())['artifact']['sha256'] == artifact['sha256']
print('All 16 source-bound functional/build/install receipts verified', flush=True)
if args.preflight:
    raise SystemExit(0)
performance_run = json.loads((out / 'performance-idle-resolution.json').read_text())
assert performance_run['returncode'] == 0 and not performance_run.get('capture_errors')
assert performance_run['source'] == performance_run['source_after'] == source
q.checked(performance_run['log'])
performance = root / 'target/kde-free-performance-idle-resolution-1257101/comparison.json'
q.verify_performance(json.loads(performance.read_text()), source, artifact)
receipt = {'schema': q.SCHEMA, 'source': source, 'baseline': q.reference(out / 'baseline.json'),
           'sdk': q.reference(out / 'sdk.json'), 'installed_artifact': artifact, 'runs': runs,
           'runtime': [q.reference(out / ('runtime-' + phase + '.json')) for phase in ['window-mode', 'file-dialog']],
           'performance': q.reference(performance)}
path = out / 'qualification.json'
assert not path.exists(), 'Preserve existing qualification; do not overwrite it'
path.write_text(json.dumps(receipt, indent=2) + '\n')
subprocess.run(['python3', 'tools/qualification/kde_free.py', 'record',
                '--output', str(out / 'verification.json'), '--',
                'python3', 'tools/qualification/kde_free.py', 'verify', str(path)], check=True)
print('Final successor qualification passed', flush=True)

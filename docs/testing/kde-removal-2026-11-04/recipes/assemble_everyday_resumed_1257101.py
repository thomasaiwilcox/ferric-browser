from pathlib import Path
import copy
import importlib.util
import json
import subprocess
import xml.etree.ElementTree as ET

root = Path.cwd()
out = root / 'target/kde-free-evidence/qualified-1257101'
spec = importlib.util.spec_from_file_location('qualification', root / 'tools/qualification/kde_free.py')
q = importlib.util.module_from_spec(spec)
spec.loader.exec_module(q)
source = q.snapshot()
assert source['revision'] == '1257101f6eead8e369d2ced87fd773ad6850da5b'
discovery = json.loads(subprocess.check_output([
    'ctest', '--test-dir', 'target/kde-free-application/everyday', '--show-only=json-v1'
], text=True))
expected = {test['name'] for test in discovery['tests']}
assert len(expected) == 17
retry = {'everyday_T101_hyprland_identity_grouping', 'everyday_T102_hyprland_identity_grouping'}
cases, failed, receipts = {}, set(), []
for name in ['everyday', 'everyday-focus-resumed']:
    path = out / (name + '.json')
    run = json.loads(path.read_text())
    assert run['source'] == run['source_after'] == source and not run.get('capture_errors')
    q.checked(run['log'])
    rows = list(ET.parse(q.checked(run['test_report'])).getroot().iter('testcase'))
    names = {case.get('name') for case in rows}
    assert len(names) == len(rows)
    assert names == (expected if name == 'everyday' else retry)
    assert run['returncode'] == (8 if name == 'everyday' else 0)
    for case in rows:
        key = case.get('name')
        assert not list(case.iter('error')) + list(case.iter('skipped'))
        if case.find('failure') is not None:
            assert name == 'everyday' and key in retry
            output = json.loads(case.findtext('system-out'))
            assert output['reason'] == 'owned-focus-precondition-not-met'
            failed.add(key)
        if name == 'everyday-focus-resumed':
            assert case.find('failure') is None
            output = json.loads(case.findtext('system-out'))
            case_id = key.split('_')[1]
            assert output[case_id] == 'pass' and output['focus_events'] == 0
        cases[key] = copy.deepcopy(case)
    receipts.append(q.reference(path))
assert failed == retry and set(cases) == expected
assert all(not list(c.iter('failure')) + list(c.iter('error')) + list(c.iter('skipped')) for c in cases.values())
suite = ET.Element('testsuite', name='everyday: actual original successes and unchanged-source unlocked desktop reruns',
                   tests=str(len(cases)), failures='0', errors='0', skipped='0')
props = ET.SubElement(suite, 'properties')
for index, ref in enumerate(receipts):
    ET.SubElement(props, 'property', name=f'actual_run_receipt_{index}', value=json.dumps(ref))
for case in cases.values():
    suite.append(case)
ET.ElementTree(suite).write(out / 'everyday-final-resumed.xml', encoding='utf-8', xml_declaration=True)
(out / 'everyday-attempts-resumed.json').write_text(json.dumps({
    'source': source, 'actual_runs': receipts, 'assembler': q.reference(Path(__file__)),
    'rerun_cases': sorted(retry), 'accepted_cases': sorted(cases),
    'method': 'Keep the 15 original successful complete entries. Replace only the two entries blocked by the locked desktop with their actual successful unmodified complete reruns after user unlock. Preserve original failures and both source-identical run receipts.'
}, indent=2) + '\n')
print('17 actual successful CTest entries; T101 and T102 passed original unmodified tests on the unlocked desktop')

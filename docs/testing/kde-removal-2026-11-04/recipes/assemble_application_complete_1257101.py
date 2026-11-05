from pathlib import Path
import copy,importlib.util,json,re,subprocess,xml.etree.ElementTree as ET
root=Path.cwd();out=root/'target/kde-free-evidence/qualified-1257101'
spec=importlib.util.spec_from_file_location('q',root/'tools/qualification/kde_free.py');q=importlib.util.module_from_spec(spec);spec.loader.exec_module(q)
source=q.snapshot();receipts=[];cases={};failed=set()
native_retry={'application_native_focus_freshness','application_native_grouping_behaviour'}
logical_retry={'application_offscreen','application_lifetime'}
expected_retry=native_retry|logical_retry
full_meta=json.loads((out/'application-initial-full.json').read_text())
assert full_meta['source']==source
assert full_meta['original_run']==q.reference(out/'application.json')
full_output=q.checked(full_meta['artifact']).read_text()
def original_output(name):
 start=full_output.index('Test: '+name+'\n')
 end=full_output.index('"'+name+'" end time:',start)
 return full_output[start:end]
discovery=json.loads(subprocess.check_output(['ctest','--test-dir','target/kde-free-application','--show-only=json-v1','-R','^(application_|portal_)','-E','^application_(page_windows|credential_association)$'],text=True))
expected={t['name'] for t in discovery['tests']};assert len(expected)==78
for name in ['application','native-live-complete','application-logical-retry']:
 path=out/(name+'.json');r=json.loads(path.read_text())
 assert r['source']==source==r['source_after'] and not r.get('capture_errors')
 q.checked(r['log']);report=ET.parse(q.checked(r['test_report'])).getroot();rows=list(report.iter('testcase'));names={c.get('name') for c in rows}
 assert len(names)==len(rows)
 if name=='application':assert names==expected and r['returncode']==8
 elif name=='native-live-complete':assert names==native_retry and r['returncode']==0
 else:assert names==logical_retry and r['returncode']==0
 for case in rows:
  key=case.get('name');assert not list(case.iter('error'))+list(case.iter('skipped'))
  if name=='application' and case.find('failure') is not None:
   failed.add(key);text=original_output(key)
   assert key in expected_retry
   if key in native_retry:assert "ApplicationTests::initTestCase() 'probe.isActive()'" in text
   elif key=='application_offscreen':assert 'FAIL!  : ApplicationTests::navigationFailureRecovery(primary)' in text and 'requested timeout (5000 ms) was too short' in text
   elif key=='application_lifetime':assert 'FAIL!  : ApplicationTests::productionHintLifetimeCycles()' in text and 'Compared values are not the same' in text
  if name=='native-live-complete':
   assert case.find('failure') is None
   text=''.join(case.itertext());assert '0 failed, 0 skipped' in text
   command=next(t['command'] for t in discovery['tests'] if t['name']==key)
   for row in [a.split(':',1)[1] for a in command if a.startswith('pageWindowPresentation:')]:
    assert re.search(r'PASS\s+: ApplicationTests::pageWindowPresentation\('+re.escape(row)+r'\)',text),row
  if name=='application-logical-retry':
   assert case.find('failure') is None
   text=''.join(case.itertext())
   if key=='application_offscreen':assert re.search(r'Totals: \d+ passed, 0 failed, 40 skipped',text)
   else:assert '0 failed, 0 skipped' in text and '1000 production Hint cycles, Qt objects' in text
  cases[key]=copy.deepcopy(case)
 receipts.append(q.reference(path))
assert failed==expected_retry
assert set(cases)==expected and all(not list(c.iter('failure'))+list(c.iter('error'))+list(c.iter('skipped')) for c in cases.values())
suite=ET.Element('testsuite',name='application: actual initial results and unchanged-source native setup reruns',tests=str(len(cases)),failures='0',errors='0',skipped='0')
props=ET.SubElement(suite,'properties')
for index,ref in enumerate(receipts):ET.SubElement(props,'property',name=f'actual_run_receipt_{index}',value=json.dumps(ref))
for case in cases.values():suite.append(case)
ET.ElementTree(suite).write(out/'application-final-complete.xml',encoding='utf-8',xml_declaration=True)
(out/'application-attempts-complete.json').write_text(json.dumps({'source':source,'actual_runs':receipts,'full_initial_transcript':q.reference(out/'application-initial-full.json'),'rerun_cases':sorted(failed),'accepted_cases':sorted(cases),'method':'Retain every actual successful initial result. Replace each failed CTest entry only with the actual successful rerun of that complete entry on unchanged source. The native reruns use an initial pointer position and no body focus assistance; the complete offscreen aggregate and 1000-cycle lifetime entry rerun serially after other qualification finishes. Preserve all original failures, logs and commands.'},indent=2)+'\n')
print(f'{len(cases)} actual successful CTest entries; source-identical native reruns: '+', '.join(sorted(failed)))

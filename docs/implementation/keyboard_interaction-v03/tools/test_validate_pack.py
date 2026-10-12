#!/usr/bin/env python3
"""Negative checks for the pack validator; never executes Ferric or mutates source pack."""
from pathlib import Path
import importlib.util
import json
import shutil
import tempfile
import unittest

HERE=Path(__file__).resolve().parent
ROOT=HERE.parent
spec=importlib.util.spec_from_file_location('pack_validator',HERE/'validate_pack.py')
assert spec and spec.loader
module=importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

class PackValidatorTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory(prefix='ferric-pack-test-')
        self.root=Path(self.tmp.name)/'pack'
        shutil.copytree(ROOT,self.root)
    def tearDown(self): self.tmp.cleanup()
    def edit(self,name,fn):
        path=self.root/name; data=json.loads(path.read_text()); fn(data)
        path.write_text(json.dumps(data))
    def fails_with(self,fragment,**kw):
        report=module.validate(self.root,**kw)
        self.assertFalse(report['ok'],report)
        self.assertTrue(any(fragment in x for x in report['errors']),report)
    def test_pristine_structure(self): self.assertTrue(module.validate(self.root)['ok'])
    def test_unknown_dependency(self):
        self.edit('plan.json',lambda x:x['slices'][1]['depends_on'].append('S99'))
        self.fails_with('unknown dependency')
    def test_dependency_cycle(self):
        self.edit('plan.json',lambda x:x['slices'][0]['depends_on'].append('S01'))
        self.fails_with('Dependency cycle')
    def test_missing_source_coverage(self):
        self.edit('TRACEABILITY.json',lambda x:x['mappings'].pop())
        self.fails_with('Traceability coverage')
    def test_path_escape(self):
        self.edit('plan.json',lambda x:x['slices'][0].update(card='../outside.md'))
        self.fails_with('Path escapes')
    def test_unknown_test(self):
        self.edit('plan.json',lambda x:x['slices'][0]['test_ids'].append('T999'))
        self.fails_with('unknown/missing test')
    def test_fabricated_matrix_result(self):
        self.edit('TEST_MATRIX.json',lambda x:x['tests'][0].update(status='passed'))
        self.fails_with('matrix must remain a plan')
    def test_completion_without_evidence(self):
        self.edit('progress.json',lambda x:x['slices'][0].update(status='complete',evidence=None))
        self.fails_with('terminal status needs evidence',evidence=True)
    def test_unimplemented_delivery_not_complete(self):
        self.fails_with('implementation not complete',strict_completion=True)
    def test_research_go_without_required_proof(self):
        sid='S20'
        (self.root/'evidence/cap.md').write_text('Example report for a negative validator test only')
        record={'schema_version':1,'slice_id':sid,'status':'complete','commit':'0'*40,
                'environment':{'fixture':'negative-test'},'test_results':[],
                'capability_decision':'go','capability_report':'evidence/cap.md'}
        (self.root/'evidence/S20.json').write_text(json.dumps(record))
        self.edit('progress.json',lambda x:x['slices'][20].update(status='complete',evidence='evidence/S20.json',capability_decision='go'))
        self.fails_with('missing passed evidence',evidence=True)
    def test_no_go_cannot_mean_only_missing_infrastructure(self):
        sid='S20'
        (self.root/'evidence/cap.md').write_text('Example report for a negative validator test only')
        record={'schema_version':1,'slice_id':sid,'status':'research_no_go','commit':'0'*40,
                'environment':{'fixture':'negative-test'},'test_results':[],
                'capability_decision':'no_go','capability_report':'evidence/cap.md'}
        (self.root/'evidence/S20.json').write_text(json.dumps(record))
        self.edit('progress.json',lambda x:x['slices'][20].update(status='research_no_go',evidence='evidence/S20.json',capability_decision='no_go'))
        self.fails_with('no-go needs concrete evidence',evidence=True)
    def test_duplicate_slice(self):
        self.edit('plan.json',lambda x:x['slices'].append(dict(x['slices'][0])))
        self.fails_with('Duplicate slice')

if __name__=='__main__': unittest.main(verbosity=2)

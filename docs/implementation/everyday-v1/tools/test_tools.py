#!/usr/bin/env python3
"""Local tests use disposable fake repositories, never live browser data or network."""
from __future__ import annotations
import copy,importlib.util,json,os,subprocess,sys,tempfile,unittest
from pathlib import Path
from unittest.mock import patch
sys.path.insert(0,str(Path(__file__).resolve().parent))
from common import PACK,CLONE,git,load,write_json,digest,packet,validate_repo,safe_relative,repo_snapshot
import bootstrap,run_gate,cleanup,ready,validate_pack,preflight,qt_include_guard

class TempRepo(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory(prefix='ferric-pack-test-');self.root=Path(self.tmp.name);self.repo=self.root/'repo';self.repo.mkdir()
        git(self.repo,'init','--initial-branch=main')
        git(self.repo,'config','user.name','Pack Fixture');git(self.repo,'config','user.email','fixture@example.invalid')
        (self.repo/'fixture.txt').write_text('synthetic test repository\n');git(self.repo,'add','fixture.txt');git(self.repo,'commit','-m','fixture')
        git(self.repo,'remote','add','origin',CLONE)
        git(self.repo,'update-ref','refs/remotes/origin/main',git(self.repo,'rev-parse','HEAD'))
    def tearDown(self):self.tmp.cleanup()
    def command(self,code,out='gate',**kwargs):
        return run_gate.run(self.repo,self.root/out,'fixture',[sys.executable,'-c',code],kwargs.pop('timeout',3),**kwargs)

class StructureTests(unittest.TestCase):
    def initial_state_for_plan(self,plan):
        state=copy.deepcopy(load(PACK/'control/state.initial.json'));prior=state['slices'];slices={}
        for task in plan['slices']:
            current=prior.get(task['id'],{'status':'not_started','attempts':0,'author':None,'reviewer':None,'evidence':None,'blocker':None})
            if task.get('superseded_by'):
                current={**current,'status':'superseded','supersession':{'amendment':task['amendment'],'superseded_by':task['superseded_by']}}
            slices[task['id']]=current
        state['slices']=slices
        return state
    def f02e2_a42_historical_task(self):
        task=copy.deepcopy(next(t for t in load(PACK/'control/plan.json')['slices'] if t['id']=='F02e2'))
        task['allowed_paths']=[p for p in task['allowed_paths'] if p!='crates/ferric-browser-engine-qt/src/browser_ui_input.rs']
        task['read_anchors']=[p for p in task['read_anchors'] if p!='crates/ferric-browser-engine-qt/src/browser_ui_input.rs']
        task['locks']=['public-page-host','bridge','test-registration']
        task['max_changed_product_files']=9
        task['path_budget_amendment']='A42'
        task['new_paths_notice']='A42 approves engine_requests.rs as path 9 (ninth product path).'
        return task
    def test_structure_matches_a44_review_state(self):
        errors=validate_pack.validate_structure()
        amendment=(PACK/'control/AMENDMENTS.md').read_text().split('## A44',1)[1].split('\n## ',1)[0]
        if '**Status:** independently reviewed and accepted' in amendment:
            self.assertEqual(errors,[])
        else:
            self.assertEqual(errors,['F02e2: A44 path exception is missing its accepted amendment record'])
    def test_all_packets_bounded(self):
        plan=load(PACK/'control/plan.json')
        ids=[t['id'] for t in plan['slices']]
        self.assertTrue(ids)
        self.assertEqual(len(ids),len(set(ids)))
        for t in plan['slices']:self.assertLessEqual(len(packet(t).encode()),24000,t['id'])
    def test_goal_limit(self):self.assertLess(len((PACK/'GOAL_PROMPT.txt').read_bytes()),1000)
    def test_cycle_rejected(self):
        p=load(PACK/'control/plan.json');p['slices'][0]['depends_on']=['X06']
        self.assertTrue(any('cycle' in e for e in validate_pack.validate_structure(plan=p)))
    def test_unknown_dependency_rejected(self):
        p=load(PACK/'control/plan.json');p['slices'][0]['depends_on']=['missing']
        self.assertTrue(any('unknown dependency' in e for e in validate_pack.validate_structure(plan=p)))
    def test_path_escape_rejected(self):
        with self.assertRaises(ValueError):safe_relative(PACK,'../secrets')
    def test_a38_exact_path_budget_exception(self):
        plan=load(PACK/'control/plan.json');task=next(t for t in plan['slices'] if t['id']=='F02e1')
        self.assertEqual(validate_pack.validate_edit_path_budget(task,PACK),[])
        too_many=copy.deepcopy(task);too_many['allowed_paths'].append('crates/example/extra.cpp')
        self.assertTrue(any('exact nine-path' in e for e in validate_pack.validate_edit_path_budget(too_many,PACK)))
        wrong_path=copy.deepcopy(task);wrong_path['allowed_paths'][-1]='crates/example/other.cpp'
        self.assertTrue(any('exact nine-path' in e for e in validate_pack.validate_edit_path_budget(wrong_path,PACK)))
    def test_a38_exception_requires_marker_and_scope(self):
        plan=load(PACK/'control/plan.json');task=next(t for t in plan['slices'] if t['id']=='F02e1')
        unamended=copy.deepcopy(task);unamended.pop('path_budget_amendment')
        self.assertTrue(any('edit-path budget exceeded' in e for e in validate_pack.validate_edit_path_budget(unamended,PACK)))
        other_task=copy.deepcopy(task);other_task['id']='W04'
        self.assertTrue(any('unapproved edit-path budget amendment' in e for e in validate_pack.validate_edit_path_budget(other_task,PACK)))
    def test_a42_exact_path_budget_exception(self):
        task=self.f02e2_a42_historical_task()
        live=validate_pack.validate_edit_path_budget(task,PACK)
        amendment=(PACK/'control/AMENDMENTS.md').read_text().split('## A42',1)[1].split('\n## ',1)[0]
        if '**Status:** independently reviewed and accepted' in amendment:
            self.assertEqual(live,[])
        else:
            self.assertTrue(any('missing its accepted amendment record' in e for e in live))
        too_many=copy.deepcopy(task);too_many['allowed_paths'].append('crates/example/extra.cpp')
        self.assertTrue(any('exact nine-path' in e for e in validate_pack.validate_edit_path_budget(too_many,PACK)))
        wrong_path=copy.deepcopy(task);wrong_path['allowed_paths'][-1]='crates/example/other.cpp'
        self.assertTrue(any('exact nine-path' in e for e in validate_pack.validate_edit_path_budget(wrong_path,PACK)))
        wrong_cap=copy.deepcopy(task);wrong_cap['max_changed_product_files']=8
        self.assertTrue(any('exact nine-path' in e for e in validate_pack.validate_edit_path_budget(wrong_cap,PACK)))
        wrong_task=copy.deepcopy(task);wrong_task['id']='W04'
        self.assertTrue(any('unapproved edit-path budget amendment' in e for e in validate_pack.validate_edit_path_budget(wrong_task,PACK)))
        wrong_amendment=copy.deepcopy(task);wrong_amendment['path_budget_amendment']='A99'
        self.assertTrue(any('unapproved edit-path budget amendment' in e for e in validate_pack.validate_edit_path_budget(wrong_amendment,PACK)))
        with tempfile.TemporaryDirectory() as tmp:
            pack=Path(tmp);control=pack/'control';control.mkdir()
            amendments=(PACK/'control/AMENDMENTS.md').read_text()
            a42_lines=amendments.split('## A42',1)[1].split('\n## ',1)[0].splitlines()
            status_line=next(line for line in a42_lines if line.startswith('- **Status:**'))
            pending_status='- **Status:** draft for independent review; do not dispatch the next F02e2 attempt until the exact pack change is accepted.'
            accepted_status='**Status:** independently reviewed and accepted by /root/d28_review; decision recorded at `evidence/F02e2-a1/PLAN_REVIEW-A42-final.json`.'
            self.assertIn(status_line,amendments)
            write_json(control/'source-map.execution.json',load(PACK/'control/source-map.execution.json'))
            (control/'AMENDMENTS.md').write_text(amendments.replace(status_line,pending_status,1))
            pending=validate_pack.validate_edit_path_budget(task,pack)
            self.assertTrue(any('missing its accepted amendment record' in e for e in pending))
            (control/'AMENDMENTS.md').write_text(amendments.replace(status_line,'- '+accepted_status,1))
            self.assertEqual(validate_pack.validate_edit_path_budget(task,pack),[])
    def test_a42_exception_requires_exact_amendment_metadata(self):
        task=self.f02e2_a42_historical_task()
        unamended=copy.deepcopy(task);unamended.pop('path_budget_amendment')
        self.assertTrue(any('edit-path budget exceeded' in e for e in validate_pack.validate_edit_path_budget(unamended,PACK)))
        with tempfile.TemporaryDirectory() as tmp:
            pack=Path(tmp);control=pack/'control';control.mkdir()
            amendments=(PACK/'control/AMENDMENTS.md').read_text()
            pending_status='**Status:** draft for independent review; do not dispatch the next F02e2 attempt until the exact pack change is accepted.'
            accepted_status='**Status:** independently reviewed and accepted by /root/d28_review; decision recorded at `evidence/F02e2-a1/PLAN_REVIEW-A42-final.json`.'
            (control/'AMENDMENTS.md').write_text(amendments.replace(pending_status,accepted_status,1))
            source_map=load(PACK/'control/source-map.execution.json')
            record=next(x for x in source_map['execution_amendments'] if x.get('id')=='A42')
            record['added_paths']=['crates/ferric-browser-engine-qt/src/browser_ui_tab_state.rs']
            write_json(control/'source-map.execution.json',source_map)
            errors=validate_pack.validate_edit_path_budget(task,pack)
            self.assertTrue(any('source-map owner record' in e for e in errors))
    def test_a44_exact_ten_path_scope_and_lock(self):
        task=next(t for t in load(PACK/'control/plan.json')['slices'] if t['id']=='F02e2')
        self.assertEqual(validate_pack.validate_edit_path_budget(task,PACK),[])
        self.assertEqual(len(task['allowed_paths']),10)
        self.assertIn('crates/ferric-browser-engine-qt/src/browser_ui_input.rs',task['allowed_paths'])
        self.assertEqual(task['locks'],['public-page-host','bridge','test-registration','browser-ui-input'])
        too_many=copy.deepcopy(task);too_many['allowed_paths'].append('crates/example/extra.cpp')
        self.assertTrue(any('exact ten-path' in e for e in validate_pack.validate_edit_path_budget(too_many,PACK)))
        wrong_path=copy.deepcopy(task);wrong_path['allowed_paths'][-1]='crates/example/other.cpp'
        self.assertTrue(any('exact ten-path' in e for e in validate_pack.validate_edit_path_budget(wrong_path,PACK)))
        wrong_lock=copy.deepcopy(task);wrong_lock['locks'].remove('browser-ui-input')
        self.assertTrue(any('browser-ui-input' in e for e in validate_pack.validate_edit_path_budget(wrong_lock,PACK)))
        wrong_cap=copy.deepcopy(task);wrong_cap['max_changed_product_files']=9
        self.assertTrue(any('exact ten-path' in e for e in validate_pack.validate_edit_path_budget(wrong_cap,PACK)))
        wrong_task=copy.deepcopy(task);wrong_task['id']='W04'
        self.assertTrue(any('unapproved edit-path budget amendment' in e for e in validate_pack.validate_edit_path_budget(wrong_task,PACK)))
        with tempfile.TemporaryDirectory() as tmp:
            pack=Path(tmp);control=pack/'control';control.mkdir()
            amendments=(PACK/'control/AMENDMENTS.md').read_text()
            a44_lines=amendments.split('## A44',1)[1].split('\n## ',1)[0].splitlines()
            status_line=next(line for line in a44_lines if line.startswith('- **Status:**'))
            write_json(control/'source-map.execution.json',load(PACK/'control/source-map.execution.json'))
            (control/'AMENDMENTS.md').write_text(amendments.replace(status_line,'- **Status:** staged for independent review.',1))
            self.assertTrue(any('missing its accepted amendment record' in e for e in validate_pack.validate_edit_path_budget(task,pack)))
            (control/'AMENDMENTS.md').write_text(amendments)
            source_map=load(PACK/'control/source-map.execution.json')
            record=next(x for x in source_map['execution_amendments'] if x.get('id')=='A44')
            record['added_paths']=[]
            write_json(control/'source-map.execution.json',source_map)
            self.assertTrue(any('source-map owner record' in e for e in validate_pack.validate_edit_path_budget(task,pack)))
    def test_initial_not_complete(self):
        p=load(PACK/'control/plan.json');e=validate_pack.validate_state(p,PACK/'control/state.initial.json',True)
        self.assertTrue(any('not accepted' in x for x in e))
    def test_initial_ready(self):
        self.assertEqual(ready.suggest(load(PACK/'control/plan.json'),load(PACK/'control/state.initial.json'))['suggestions'],['S00'])
    def test_file_conflicts_even_without_named_lock(self):
        p={'limits':{'max_noncoordinator_agents':6},'slices':[{'id':x,'depends_on':[],'locks':[],'allowed_paths':['shared.rs']} for x in ('A','B')]}
        s={'active_locks':{},'slices':{x:{'status':'ready'} for x in ('A','B')}}
        self.assertEqual(ready.suggest(p,s)['suggestions'],['A'])
    def test_capacity_exhausted(self):
        p={'limits':{'max_noncoordinator_agents':1},'slices':[{'id':x,'depends_on':[],'locks':[],'allowed_paths':[x]} for x in ('A','B')]}
        s={'slices':{'A':{'status':'in_progress'},'B':{'status':'ready'}}}
        self.assertEqual(ready.suggest(p,s)['suggestions'],[])
    def test_attempt_increase_without_exception_rejected(self):
        p=load(PACK/'control/plan.json')
        p.get('attempt_exceptions',{}).pop('W01b',None)
        next(t for t in p['slices'] if t['id']=='W01b')['max_attempts']=4
        self.assertTrue(any('increase lacks explicit attempt exception' in e for e in validate_pack.validate_attempt_limits(p,{t['id']:t for t in p['slices']})))
    def test_exception_cannot_be_assigned_to_unapproved_task(self):
        p=load(PACK/'control/plan.json');t=next(t for t in p['slices'] if t['id']=='F02c');t['max_attempts']=4
        p['attempt_exceptions']={'F02c':{'amendment':'A37','default_limit':3,'authorized_limit':4,'reason':'fabricated fixture exception'}}
        self.assertTrue(any('not authorized for F02c' in e for e in validate_pack.validate_attempt_limits(p,{x['id']:x for x in p['slices']})))
    def test_global_attempt_limit_cannot_be_raised(self):
        p=load(PACK/'control/plan.json');p['limits']['max_attempts']=4
        self.assertTrue(any('Global max_attempts must remain 3' in e for e in validate_pack.validate_attempt_limits(p,{t['id']:t for t in p['slices']})))
    def test_attempt3_to4_authorized_but5_rejected(self):
        p=load(PACK/'control/plan.json');t=next(t for t in p['slices'] if t['id']=='W01b');t['max_attempts']=4
        p.setdefault('attempt_exceptions',{})['W01b']={'amendment':'A37','default_limit':3,'authorized_limit':4,'reason':'one bounded repair after independent review'}
        self.assertEqual(validate_pack.validate_attempt_limits(p,{x['id']:x for x in p['slices']}),[])
        with tempfile.TemporaryDirectory() as tmp:
            state=self.initial_state_for_plan(p);path=Path(tmp)/'control/state.json';path.parent.mkdir()
            for attempts,valid in ((3,True),(4,True),(5,False)):
                state['slices']['W01b']['attempts']=attempts;write_json(path,state)
                errors=validate_pack.validate_state(p,path,pack=PACK)
                self.assertEqual(not any('W01b: attempt limit' in e for e in errors),valid)
    def test_f02e2_a48_final_attempt6_exact_exception_and_no_seventh(self):
        p=load(PACK/'control/plan.json')
        tasks={x['id']:x for x in p['slices']}
        self.assertEqual(p['limits']['max_attempts'],3)
        self.assertEqual(p['attempt_exceptions']['P00a']['amendment'],'A37')
        self.assertEqual(p['attempt_exceptions']['W01b']['amendment'],'A37')
        self.assertEqual(tasks['F02e2']['max_attempts'],6)
        entry=p['attempt_exceptions']['F02e2']
        self.assertEqual(set(entry),{'amendment','default_limit','authorized_limit','reason'})
        self.assertEqual(entry['amendment'],'A48')
        self.assertEqual(entry['default_limit'],3)
        self.assertEqual(entry['authorized_limit'],6)
        self.assertTrue(entry['reason'].strip())
        self.assertEqual(validate_pack.validate_attempt_limits(p,tasks),[])
        for wrong_amendment in ('A37','A42','A44','A47'):
            wrong=json.loads(json.dumps(p))
            wrong['attempt_exceptions']['F02e2']['amendment']=wrong_amendment
            self.assertTrue(any('F02e2: attempt exception must cite A48' in e for e in validate_pack.validate_attempt_limits(wrong,{x['id']:x for x in wrong['slices']})))
        amendment=(PACK/'control/AMENDMENTS.md').read_text()
        a43=amendment.split('## A43',1)[1].split('\n## ',1)[0]
        a44=amendment.split('## A44',1)[1].split('\n## ',1)[0]
        a47=amendment.split('## A47',1)[1].split('\n## ',1)[0]
        a48=amendment.split('## A48',1)[1].split('\n## ',1)[0]
        self.assertIn('no fifth attempt',a43)
        self.assertIn('attempt 4/4 is exhausted',a44)
        self.assertIn('attempt 6',a47)
        self.assertIn('attempt 7',a48.lower())
        self.assertEqual(validate_pack.validate_source_attempt_extension_A48(p,PACK),[])
        extension=copy.deepcopy(p);extension['source_attempt_extensions_A48']['F02e2']['attempt_seven_authorized']=True
        self.assertTrue(any('invalid attempt_seven_authorized' in e for e in validate_pack.validate_source_attempt_extension_A48(extension,PACK)))
        widened=copy.deepcopy(p);widened['source_attempt_extensions_A48']['F02e2']['allowed_source_paths'].append('crates/example/extra.cpp')
        self.assertTrue(any('invalid allowed_source_paths' in e for e in validate_pack.validate_source_attempt_extension_A48(widened,PACK)))
        with tempfile.TemporaryDirectory() as tmp:
            state=self.initial_state_for_plan(p);path=Path(tmp)/'control/state.json';path.parent.mkdir()
            for attempts,valid in ((3,True),(5,True),(6,True),(7,False)):
                state['slices']['F02e2']['attempts']=attempts;write_json(path,state)
                errors=validate_pack.validate_state(p,path,pack=PACK)
                self.assertEqual(not any('F02e2: attempt limit' in e for e in errors),valid)
    def test_a45_native_retry_is_exact_same_source_and_not_attempt6(self):
        p=load(PACK/'control/plan.json')
        self.assertEqual(validate_pack.validate_native_gate_retries(p,PACK),[])
        retry=p['native_gate_retries']['F02e2']
        self.assertEqual(retry['source_attempt'],5)
        self.assertEqual(retry['max_same_source_retries'],1)
        self.assertEqual(retry['candidate_sha'],'58c047c04072bb7efa382e3cd7ca65a11da1b117')
        self.assertEqual(retry['candidate_tree'],'f5cc15efd18496bb209d25866d060347157a00bd')
        self.assertTrue(retry['same_source_only'])
        self.assertFalse(retry['source_changes_allowed'])
        self.assertFalse(retry['attempt_six_authorized'])
        self.assertTrue(retry['fresh_target'].endswith('native-F02e2-a5-qt6112-qmake'))
        self.assertEqual(retry['cargo_target_dir'],retry['fresh_target']+'/cargo-target')
        self.assertEqual(retry['cmake_dir'],retry['fresh_target']+'/cmake')
        self.assertEqual(retry['pkg_config_path'],retry['pkg_config_libdir'])
        self.assertEqual(retry['cmake_prefix_path'],'/home/tom/.local/share/ferric-browser-runtime/qt/6.11.2/gcc_64')
        self.assertTrue(retry['qt6_dir'].endswith('/lib/cmake/Qt6'))
        self.assertEqual(retry['strict_qt_guard_tool'],'tools/run_gate.py --qt-include-prefix')
        self.assertEqual(retry['emit_cxx_commands_env'],'CC_ENABLE_DEBUG_OUTPUT=1')
        self.assertEqual(retry['only_test_id'],'T014e2')
        for key,value in (('max_same_source_retries',2),('candidate_sha','f'*40),
                          ('source_changes_allowed',True),('attempt_six_authorized',True),
                          ('qmake','/usr/bin/qmake6'),('pkg_config_libdir','/usr/lib/pkgconfig'),
                          ('qt6_dir','/usr/lib/cmake/Qt6'),
                          ('strict_qt_guard_tool','none')):
            wrong=copy.deepcopy(p);wrong['native_gate_retries']['F02e2'][key]=value
            self.assertTrue(validate_pack.validate_native_gate_retries(wrong,PACK))
        wrong=copy.deepcopy(p);wrong['native_gate_retries']['W04']=wrong['native_gate_retries'].pop('F02e2')
        self.assertTrue(any('not authorized' in e for e in validate_pack.validate_native_gate_retries(wrong,PACK)))
        missing=copy.deepcopy(p);missing.pop('native_gate_retries')
        self.assertTrue(any('lacks its exact native-gate retry entry' in e for e in validate_pack.validate_native_gate_retries(missing,PACK)))
        with tempfile.TemporaryDirectory() as tmp:
            pack=Path(tmp);control=pack/'control';control.mkdir()
            amendments=(PACK/'control/AMENDMENTS.md').read_text()
            a45=amendments.split('## A45',1)[1].split('\n## ',1)[0]
            status=next(line for line in a45.splitlines() if line.startswith('- **Status:**'))
            (control/'AMENDMENTS.md').write_text(amendments.replace(status,'- **Status:** staged for independent review.',1))
            write_json(control/'source-map.execution.json',load(PACK/'control/source-map.execution.json'))
            errors=validate_pack.validate_native_gate_retries(p,pack)
            self.assertTrue(any('lacks its accepted amendment record' in e for e in errors))
            (control/'AMENDMENTS.md').write_text(amendments)
            source_map=load(PACK/'control/source-map.execution.json')
            record=next(x for x in source_map['execution_amendments'] if x.get('id')=='A45')
            record['added_paths']=['crates/example/unapproved.rs']
            write_json(control/'source-map.execution.json',source_map)
            errors=validate_pack.validate_native_gate_retries(p,pack)
            self.assertTrue(any('source-map retry record' in e for e in errors))
            source_map=load(PACK/'control/source-map.execution.json')
            source_map['execution_amendments'].append(copy.deepcopy(next(
                x for x in source_map['execution_amendments'] if x.get('id')=='A45')))
            write_json(control/'source-map.execution.json',source_map)
            errors=validate_pack.validate_native_gate_retries(p,pack)
            self.assertTrue(any('exactly one A44 record and one A45 retry record' in e for e in errors))
        missing_task=copy.deepcopy(p)
        missing_task['slices']=[task for task in missing_task['slices'] if task.get('id')!='F02e2']
        errors=validate_pack.validate_native_gate_retries(missing_task,PACK)
        self.assertTrue(any("retain A44's historical 5/5" in e for e in errors))
    def test_a46_final_native_retry_scope_is_bounded(self):
        p=load(PACK/'control/plan.json');retry=p['native_gate_retry_extensions']['F02e2']
        expected_paths=[
            'docs/implementation/everyday-v1/tools/qt_include_guard.py',
            'docs/implementation/everyday-v1/tools/run_gate.py',
            'docs/implementation/everyday-v1/tools/test_tools.py',
            'evidence/A46/watch_qt_includes.py',
            'evidence/A46/run_native_retry.py'
        ]
        self.assertEqual(retry['amendment'],'A46')
        self.assertEqual(retry['same_source_retry_number'],2)
        self.assertEqual(retry['cumulative_max_same_source_retries'],2)
        self.assertEqual(retry['source_attempt_count'],5)
        self.assertEqual(retry['candidate_sha'],'58c047c04072bb7efa382e3cd7ca65a11da1b117')
        self.assertEqual(retry['candidate_tree'],'f5cc15efd18496bb209d25866d060347157a00bd')
        self.assertFalse(retry['source_changes_allowed']);self.assertFalse(retry['attempt_six_authorized'])
        self.assertEqual(retry['tooling_paths'],expected_paths);self.assertEqual(retry['tooling_file_cap'],5)
        self.assertEqual(retry['product_path_cap'],10);self.assertEqual(retry['product_paths_added_or_changed'],[])
        self.assertEqual(retry['existing_product_locks_unchanged'],['public-page-host','bridge','test-registration','browser-ui-input'])
        source_map=load(PACK/'control/source-map.execution.json')
        a46=[x for x in source_map['execution_amendments'] if x.get('id')=='A46']
        self.assertEqual(len(a46),1);self.assertEqual(a46[0]['tooling_paths'],expected_paths)
        self.assertEqual(a46[0]['added_paths'],[]);self.assertEqual(a46[0]['added_locks'],[])
    def test_a47_pkg_config_retry_scope_is_bounded(self):
        p=load(PACK/'control/plan.json')
        retry=p['native_gate_retry_extensions_A47']['F02e2']
        self.assertEqual(retry['same_source_retry_number'],3)
        self.assertEqual(retry['cumulative_max_same_source_retries'],3)
        self.assertEqual(retry['A45_same_source_retry_count_used'],1)
        self.assertEqual(retry['A46_same_source_retry_count_used'],2)
        self.assertEqual(retry['candidate_sha'],'58c047c04072bb7efa382e3cd7ca65a11da1b117')
        self.assertEqual(retry['candidate_tree'],'f5cc15efd18496bb209d25866d060347157a00bd')
        self.assertFalse(retry['source_changes_allowed']);self.assertFalse(retry['attempt_six_authorized'])
        self.assertFalse(retry['retry_four_authorized'])
        qt_pc='/home/tom/.local/share/ferric-browser-runtime/qt/6.11.2/gcc_64/lib/pkgconfig'
        self.assertEqual(retry['pkg_config_path'],qt_pc)
        self.assertEqual(retry['pkg_config_libdir'],qt_pc+':/usr/lib/pkgconfig:/usr/share/pkgconfig')
        self.assertEqual(retry['tooling_file_cap'],3)
        self.assertEqual(retry['tooling_paths'],[
            'docs/implementation/everyday-v1/tools/test_tools.py',
            'evidence/A47/watch_qt_includes.py',
            'evidence/A47/run_native_retry.py'])
        self.assertEqual(retry['product_paths_added_or_changed'],[])
        self.assertEqual(retry['existing_product_locks_unchanged'],['public-page-host','bridge','test-registration','browser-ui-input'])
        a46=p['native_gate_retry_extensions']['F02e2']
        self.assertEqual(a46['same_source_retry_number'],2)
        manifest_path=PACK.parents[3]/'evidence/A47/NATIVE_COMMANDS-A47-r1.json'
        manifest=load(manifest_path)
        self.assertEqual(manifest['amendment'],'A47');self.assertEqual(manifest['retry_number'],3)
        self.assertEqual(manifest['sdk']['pkg_config_path'],qt_pc)
        self.assertEqual(manifest['sdk']['pkg_config_libdir'],retry['pkg_config_libdir'])
        for gate in manifest['gates']:
            env=gate.get('environment',{})
            self.assertEqual(env.get('PKG_CONFIG_PATH'),qt_pc)
            self.assertEqual(env.get('PKG_CONFIG_LIBDIR'),retry['pkg_config_libdir'])
        versions_receipt=load(PACK.parents[3]/'evidence/A47/pkgconfig-modversions-r1/receipt.json')
        versions=(PACK.parents[3]/'evidence/A47/pkgconfig-modversions-r1/output.log').read_text().split()
        self.assertEqual(versions_receipt['result'],'passed')
        self.assertEqual(versions,['6.11.2']*4+['6.31.0'])
        cflags_receipt=load(PACK.parents[3]/'evidence/A47/pkgconfig-cflags-r1/receipt.json')
        cflags=(PACK.parents[3]/'evidence/A47/pkgconfig-cflags-r1/output.log').read_text().split()
        self.assertEqual(cflags_receipt['result'],'passed')
        self.assertTrue(any(x.startswith('-I/usr/include/KF6/KWindowSystem') for x in cflags))
        self.assertTrue(all(not x.startswith('-I') or '/Qt' not in x and '/qt' not in x or x[2:].startswith('/home/tom/.local/share/ferric-browser-runtime/qt/6.11.2/gcc_64/') for x in cflags))
        source_map=load(PACK/'control/source-map.execution.json')
        records=[x for x in source_map['execution_amendments'] if x.get('id')=='A47']
        self.assertEqual(len(records),1);self.assertEqual(records[0]['pkg_config_libdir'],retry['pkg_config_libdir'])
        watcher=PACK.parents[3]/'evidence/A47/watch_qt_includes.py'
        spec=importlib.util.spec_from_file_location('a47_watch_qt_includes',watcher)
        module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
        self.assertIs(module.qt_include_escape,qt_include_guard.qt_include_escape)
    def test_attempt_limit_hides_only_exhausted_implementation_dispatch(self):
        task={'id':'W01b','kind':'implementation','allowed_paths':['route.rs'],'depends_on':[],'locks':[],'max_attempts':4}
        p={'limits':{'max_noncoordinator_agents':6,'max_attempts':3},'attempt_exceptions':{'W01b':{'amendment':'A37','default_limit':3,'authorized_limit':4,'reason':'bounded repair'}},'slices':[task]}
        s={'active_locks':{},'slices':{'W01b':{'status':'ready','attempts':3}}}
        self.assertEqual(ready.suggest(p,s)['suggestions'],['W01b'])
        s['slices']['W01b']['attempts']=4
        self.assertEqual(ready.suggest(p,s)['suggestions'],[])
        p['slices']=[{'id':'AGG','kind':'aggregate','allowed_paths':['aggregate.md'],'depends_on':[],'locks':[],'max_attempts':3}]
        s={'active_locks':{},'slices':{'AGG':{'status':'ready','attempts':3}}}
        self.assertEqual(ready.suggest(p,s)['suggestions'],['AGG'])
        p['slices']=[{'id':'LOW','kind':'implementation','allowed_paths':['low.rs'],'depends_on':[],'locks':[],'max_attempts':2}]
        s={'active_locks':{},'slices':{'LOW':{'status':'ready','attempts':2}}}
        self.assertEqual(ready.suggest(p,s)['suggestions'],[])

class SupersessionTests(unittest.TestCase):
    def fixture(self):
        tasks={'OLD':{'id':'OLD','amendment':'A37','superseded_by':['NEW'],'depends_on':[],'test_ids':['T-OLD']},
               'NEW':{'id':'NEW','depends_on':[],'test_ids':['T-NEW']}}
        tests=[{'id':'T-OLD','slice':'OLD','requirement':'REQ','level':'unit','result':'not_run','superseded_by':['T-NEW']},
               {'id':'T-NEW','slice':'NEW','requirement':'REQ','level':'unit','result':'not_run'}]
        return tasks,tests
    def test_supersession_mapping_missing_id_rejected(self):
        tasks,tests=self.fixture();tasks['OLD']['superseded_by']=['MISSING']
        self.assertTrue(any('unknown superseding task' in e for e in validate_pack.validate_supersession_structure(tasks,tests)))
        tasks,tests=self.fixture();tests[0]['superseded_by']=['MISSING']
        self.assertTrue(any('unknown replacement test' in e for e in validate_pack.validate_supersession_structure(tasks,tests)))
    def test_supersession_mapping_cycle_rejected(self):
        tasks,tests=self.fixture();tasks['NEW'].update(amendment='A37',superseded_by=['OLD'])
        tests[1]['superseded_by']=['T-OLD']
        errors=validate_pack.validate_supersession_structure(tasks,tests)
        self.assertTrue(any('supersession cycle' in e for e in errors))
    def test_dependency_must_be_rewired_from_superseded_task(self):
        tasks,tests=self.fixture();tasks['DOWNSTREAM']={'id':'DOWNSTREAM','depends_on':['OLD'],'test_ids':[]}
        self.assertTrue(any('rewire to its replacement' in e for e in validate_pack.validate_supersession_structure(tasks,tests)))
    def test_historical_dependency_requires_explicit_replacement_edge(self):
        tasks,tests=self.fixture();tasks['DOWNSTREAM']={'id':'DOWNSTREAM','depends_on':[],'historical_depends_on':['OLD'],'test_ids':[]}
        self.assertTrue(any('not explicitly rewired' in e for e in validate_pack.validate_supersession_structure(tasks,tests)))
        tasks['DOWNSTREAM']['depends_on']=['NEW']
        self.assertFalse(any('not explicitly rewired' in e for e in validate_pack.validate_supersession_structure(tasks,tests)))
    def test_superseded_state_requires_accepted_surviving_replacement(self):
        tasks,tests=self.fixture();plan={'slices':list(tasks.values())}
        state={'slices':{'OLD':{'status':'superseded','supersession':{'amendment':'A37','superseded_by':['NEW']}},
                         'NEW':{'status':'ready'}}}
        self.assertTrue(any('replacement dependency NEW is not accepted' in e for e in validate_pack.validate_supersession_state(plan,state,strict=True)))
        state['slices']['NEW']['status']='accepted'
        self.assertEqual(validate_pack.validate_supersession_state(plan,state,strict=True),[])
    def test_superseded_task_cannot_be_unlocked_by_fake_accepted_replacement(self):
        tasks,tests=self.fixture();plan={'limits':{'max_attempts':3},'slices':list(tasks.values())}
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);pack=root/'pack';(pack/'control').mkdir(parents=True)
            write_json(pack/'control/test_matrix.json',{'tests':tests})
            state={'slices':{'OLD':{'status':'superseded','attempts':0,'author':'historical-author','reviewer':'historical-reviewer','evidence':'evidence/retained-old.json','supersession':{'amendment':'A37','superseded_by':['NEW']}},
                             'NEW':{'status':'accepted','attempts':0,'author':'a','reviewer':'r','evidence':None}}}
            path=root/'control/state.json';path.parent.mkdir(exist_ok=True);write_json(path,state)
            errors=validate_pack.validate_state(plan,path,pack=pack)
            self.assertTrue(any(e.startswith('NEW:') and 'missing evidence' in e for e in errors))
            self.assertFalse(any(e.startswith('OLD:') and 'not accepted' in e for e in errors))
            state['slices']['OLD']['status']='accepted';write_json(path,state)
            errors=validate_pack.validate_state(plan,path,pack=pack)
            self.assertTrue(any(e.startswith('OLD:') and 'must retain state status superseded' in e for e in errors))
            self.assertFalse(any(e.startswith('OLD:') and 'evidence' in e for e in errors))

class SetupTests(TempRepo):
    def test_dirty_checkout_refused_untouched(self):
        sentinel=self.repo/'local-draft';sentinel.write_text('keep me')
        with self.assertRaises(ValueError):bootstrap.bootstrap(self.root/'run',self.repo)
        self.assertEqual(sentinel.read_text(),'keep me');self.assertEqual(git(self.repo,'branch','--show-current'),'main')
        self.assertFalse((self.root/'run').exists())
    def test_wrong_remote_refused(self):
        git(self.repo,'remote','set-url','origin','https://github.com/example/other.git')
        with self.assertRaises(ValueError):validate_repo(self.repo,clean=True)
    def test_nonempty_workspace_refused(self):
        w=self.root/'run';w.mkdir();(w/'keep').write_text('keep')
        with self.assertRaises(ValueError):bootstrap.bootstrap(w,self.repo)
        self.assertEqual((w/'keep').read_text(),'keep')
    def test_bootstrap_existing_without_network(self):
        real=bootstrap.git
        def fake(repo,*args,**kw):
            if args and args[0]=='fetch':return ''
            return real(repo,*args,**kw)
        with patch.object(bootstrap,'git',side_effect=fake):ctx=bootstrap.bootstrap(self.root/'run',self.repo)
        self.assertEqual(ctx['branch'],'work/everyday-v1')
        self.assertTrue((self.repo/'docs/implementation/everyday-v1/START_HERE.md').exists())
        self.assertTrue((self.root/'run/control/state.json').exists())
        self.assertEqual(git(self.repo,'rev-parse','main'),ctx['head_at_setup'])
    def test_bootstrap_suppresses_checkout_hook(self):
        flag=self.root/'hook-ran';hook=self.repo/'.git/hooks/post-checkout'
        hook.write_text('#!/bin/sh\ntouch "'+str(flag)+'"\n');hook.chmod(0o755)
        real=bootstrap.git
        with patch.object(bootstrap,'git',side_effect=lambda r,*a,**k: '' if a[0]=='fetch' else real(r,*a,**k)):
            bootstrap.bootstrap(self.root/'run',self.repo)
        self.assertFalse(flag.exists())
    def test_existing_branch_refused(self):
        git(self.repo,'branch','work/everyday-v1')
        with self.assertRaises(ValueError):bootstrap.bootstrap(self.root/'run',self.repo)
    def test_preflight_records_actual_source(self):
        with patch.object(preflight,'version',return_value={'fixture':True}):r=preflight.inventory(self.repo)
        self.assertEqual(r['source']['sha'],git(self.repo,'rev-parse','HEAD'))
        self.assertTrue(r['source_anchors']);self.assertFalse(r['authoring_commit_available'])

class GateTests(TempRepo):
    def test_positive_summary_and_digest(self):
        r=self.command("print('test result: ok. 3 passed; 0 failed; 0 ignored')",require_tests=True)
        self.assertEqual(r['result'],'passed');self.assertEqual(r['log_sha256'],digest(self.root/'gate/output.log'))
    def test_failure_exit_not_success(self):
        r=self.command("print('1 passed');raise SystemExit(2)",require_tests=True)
        self.assertEqual(r['result'],'failed')
    def test_failure_summary_even_zero_exit(self):
        r=self.command("print('Totals: 1 passed, 1 failed, 0 skipped')",require_tests=True)
        self.assertEqual(r['result'],'failed')
    def test_zero_tests_rejected(self):
        r=self.command("print('test result: ok. 0 passed; 0 failed; 0 ignored')",require_tests=True)
        self.assertEqual(r['result'],'failed')
    def test_skip_rejected(self):
        r=self.command("print('Totals: 2 passed, 0 failed, 1 skipped')",require_tests=True)
        self.assertEqual(r['result'],'failed')
    def test_timeout_rejected(self):
        r=self.command('import time;time.sleep(20)',timeout=.2)
        self.assertTrue(r['timed_out']);self.assertEqual(r['result'],'failed')
    def test_qt_include_guard_stops_owned_process_immediately(self):
        prefix=self.root/'qt/6.11.2/gcc_64'
        r=self.command("import time;print('running: c++ -I/usr/include/qt6 -c x.cpp',flush=True);time.sleep(20)",qt_include_prefix=prefix)
        self.assertEqual(r['result'],'failed')
        self.assertEqual(r['qt_include_guard'],'stopped')
        self.assertEqual(r['qt_include_violation'],'/usr/include/qt6')
        self.assertLess(r['elapsed_seconds'],3)
    def test_qt_include_guard_accepts_paths_inside_pinned_prefix(self):
        prefix=self.root/'sdk/qt/6.11.2/gcc_64'
        r=self.command(f"print('running: c++ -I{prefix}/include/QtCore -c x.cpp')",qt_include_prefix=prefix)
        self.assertEqual(r['result'],'passed')
        self.assertEqual(r['qt_include_guard'],'clear')
        self.assertIsNone(r['qt_include_violation'])
    def test_qt_include_guard_rejects_external_symlink_alias(self):
        prefix=self.root/'sdk/qt/6.11.2/gcc_64';prefix.mkdir(parents=True)
        alias=self.root/'system-qt';alias.symlink_to(prefix,target_is_directory=True)
        r=self.command(f"print('running: c++ -I{alias}/include/QtCore -c x.cpp')",qt_include_prefix=prefix)
        self.assertEqual(r['result'],'failed')
        self.assertEqual(r['qt_include_guard'],'stopped')
    def test_qt_include_guard_allows_generated_cxx_qt_wrapper_headers(self):
        prefix=self.root/'sdk/qt/6.11.2/gcc_64'
        wrapper=self.root/'builds/native-F02e2-a5-qt6112/cargo-target/release/build/cxx-qt-lib/out/cxxqtbuild/include'
        r=self.command(f"print('running: c++ -I{prefix}/include/QtCore -I{wrapper} -c x.cpp')",qt_include_prefix=prefix)
        self.assertEqual(r['result'],'passed')
        self.assertEqual(r['qt_include_guard'],'clear')
        self.assertIsNone(r['qt_include_violation'])
    def test_shared_qt_classifier_covers_system_pinned_and_generated_paths(self):
        prefix=self.root/'sdk/qt/6.11.2/gcc_64';prefix.mkdir(parents=True)
        wrapper=self.root/'builds/native-F02e2-a5-qt6112/cargo-target/build/cxx-qt-lib/out/cxxqtbuild/include'
        self.assertEqual(qt_include_guard.qt_include_escape(['c++','-I/usr/include/qt6','-c','x.cpp'],prefix),'/usr/include/qt6')
        self.assertIsNone(qt_include_guard.qt_include_escape(['c++',f'-I{prefix}/include/QtCore','-c','x.cpp'],prefix))
        self.assertIsNone(qt_include_guard.qt_include_escape(['c++',f'-I{wrapper}','-c','x.cpp'],prefix))
    def test_live_process_monitor_uses_shared_qt_classifier(self):
        watcher=PACK.parents[3]/'evidence/A46/watch_qt_includes.py'
        spec=importlib.util.spec_from_file_location('a46_watch_qt_includes',watcher)
        module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
        self.assertIs(module.qt_include_escape,qt_include_guard.qt_include_escape)
    def test_timeout_when_parent_exits_before_child(self):
        r=self.command("import subprocess,sys;subprocess.Popen([sys.executable,'-c','import time;time.sleep(20)'])",timeout=.3)
        self.assertTrue(r['timed_out']);self.assertEqual(r['result'],'failed')
    def test_log_overflow_rejected(self):
        r=self.command("print('x'*10000)",max_log_bytes=128)
        self.assertTrue(r['output_limit_exceeded']);self.assertEqual((self.root/'gate/output.log').stat().st_size,128)
    def test_output_not_overwritten(self):
        self.command("print('one')")
        with self.assertRaises(ValueError):self.command("print('two')")
    def test_dirty_source_recorded(self):
        (self.repo/'fixture.txt').write_text('new value')
        self.assertTrue(self.command("print('done')")['source_before']['dirty'])
    def test_summary_parsers(self):
        for x in ['100% tests passed, 0 tests failed out of 7\n','Totals: 6 passed, 0 failed, 0 skipped','Ran 12 tests in 0.1s\nOK']:
            self.assertGreater(run_gate.test_counts(x)[0],0)

class CleanupTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.w=Path(self.tmp.name)/'run';(self.w/'builds/safe').mkdir(parents=True);(self.w/'control').mkdir()
        write_json(self.w/'.ferric-workspace.json',{'run_id':'fixture'})
        write_json(self.w/'control/builds.json',{'run_id':'fixture','artifacts':[{'name':'safe','active':False,'retain':False}]})
        write_json(self.w/'builds/safe/.ferric-build-owner.json',{'run_id':'fixture'})
        (self.w/'builds/safe/build.bin').write_text('output');(self.w/'keep-source').write_text('keep')
    def tearDown(self):self.tmp.cleanup()
    def test_dry_run_keeps_data(self):
        self.assertEqual(cleanup.clean(self.w,[])['eligible'],['safe']);self.assertTrue((self.w/'builds/safe/build.bin').exists())
    def test_apply_owned_only(self):
        cleanup.clean(self.w,['safe'],True);self.assertFalse((self.w/'builds/safe').exists());self.assertEqual((self.w/'keep-source').read_text(),'keep')
    def test_unknown_or_escape_refused(self):
        with self.assertRaises(ValueError):cleanup.clean(self.w,['../../'],True)
    def test_active_lock_refused(self):
        (self.w/'control/build-active.lock').write_text('active')
        with self.assertRaises(ValueError):cleanup.clean(self.w,[])
    def test_owner_mismatch_refused(self):
        write_json(self.w/'builds/safe/.ferric-build-owner.json',{'run_id':'other'})
        with self.assertRaises(ValueError):cleanup.clean(self.w,[])
    def test_source_checkout_refused(self):
        (self.w/"builds/safe/.git").mkdir()
        with self.assertRaises(ValueError):cleanup.clean(self.w,["safe"],True)
        self.assertTrue((self.w/"builds/safe/build.bin").exists())
    def test_active_registry_refused(self):
        write_json(self.w/"control/builds.json",{"run_id":"fixture","artifacts":[{"name":"safe","active":True}]})
        with self.assertRaises(ValueError):cleanup.clean(self.w,[])
    def test_symlink_build_refused(self):
        (self.w/'builds/alias').symlink_to(self.w/'builds/safe',target_is_directory=True)
        write_json(self.w/'control/builds.json',{'run_id':'fixture','artifacts':[{'name':'alias','active':False}]})
        with self.assertRaises(ValueError):cleanup.clean(self.w,[])

class EvidenceTests(TempRepo):
    def make_evidence(self,level='unit'):
        sha=git(self.repo,'rev-parse','HEAD');(self.root/'evidence').mkdir()
        task={'id':'Z','test_ids':['T001']};test={'T001':{'level':level}}
        review={'reviewer':'reviewer-session','source_sha':sha,'decision':'accept'}
        write_json(self.root/'evidence/review.json',review)
        r=self.command("print('Totals: 2 passed, 0 failed, 0 skipped')",out='evidence/gate',require_tests=True)
        art={'path':'evidence/gate/receipt.json','sha256':digest(self.root/'evidence/gate/receipt.json'),'kind':'command_receipt'}
        ev={'slice_id':'Z','source_sha':sha,'implementation_sha':sha,'author':'author-session','reviewer':'reviewer-session',
            'review_path':'evidence/review.json','review_sha256':digest(self.root/'evidence/review.json'),
            'cases':[{'test_id':'T001','level':level,'result':'passed','source_sha':sha,'assertion':'fixture assertion','artifacts':[art]}]}
        write_json(self.root/'evidence/Z.json',ev)
        state={'status':'accepted','author':'author-session','reviewer':'reviewer-session','evidence':'evidence/Z.json','source_sha':sha}
        return task,state,test,ev
    def test_valid_unit_ledger(self):
        t,s,ts,e=self.make_evidence();self.assertEqual(validate_pack.validate_acceptance(t,s,self.root,ts),[])
    def test_valid_component_ledger(self):
        t,s,ts,e=self.make_evidence('component')
        self.assertEqual(validate_pack.validate_acceptance(t,s,self.root,ts),[])
    def test_component_requires_executable_evidence(self):
        t,s,ts,e=self.make_evidence('component')
        e['cases'][0]['artifacts'][0]['kind']='report'
        write_json(self.root/'evidence/Z.json',e)
        self.assertTrue(any('executable evidence missing' in x for x in validate_pack.validate_acceptance(t,s,self.root,ts)))
    def check_component_test_guard(self,field,value):
        t,s,ts,e=self.make_evidence('component')
        receipt=self.root/'evidence/gate/receipt.json'
        rc=json.loads(receipt.read_text());rc[field]=value;write_json(receipt,rc)
        e['cases'][0]['artifacts'][0]['sha256']=digest(receipt)
        write_json(self.root/'evidence/Z.json',e)
        self.assertTrue(any('no executed tests or skipped tests' in x for x in validate_pack.validate_acceptance(t,s,self.root,ts)))
    def test_component_rejects_zero_tests(self):
        self.check_component_test_guard('observed_test_summary_count',0)
    def test_component_rejects_skipped_tests(self):
        self.check_component_test_guard('observed_skips',1)
    def test_self_review_rejected(self):
        t,s,ts,e=self.make_evidence();s['reviewer']=s['author'];self.assertTrue(validate_pack.validate_acceptance(t,s,self.root,ts))
    def test_log_tamper_rejected(self):
        t,s,ts,e=self.make_evidence();(self.root/'evidence/gate/output.log').write_text('forged')
        self.assertTrue(validate_pack.validate_acceptance(t,s,self.root,ts))
    def test_native_cannot_be_unit_receipt(self):
        t,s,ts,e=self.make_evidence('native');self.assertTrue(any('qualification' in x for x in validate_pack.validate_acceptance(t,s,self.root,ts)))
    def test_physical_requires_device_record(self):
        t,s,ts,e=self.make_evidence('physical')
        q={'source_sha':s['source_sha'],'level':'physical','passed':True,'operator':'fixture','environment':{'kind':'test'},'observations':['synthetic validator test'],'devices':[]}
        write_json(self.root/'evidence/q.json',q)
        e['cases'][0]['artifacts'].append({'path':'evidence/q.json','sha256':digest(self.root/'evidence/q.json'),'kind':'qualification_record'})
        write_json(self.root/'evidence/Z.json',e)
        self.assertTrue(any('devices' in x for x in validate_pack.validate_acceptance(t,s,self.root,ts)))

if __name__=='__main__':unittest.main(verbosity=2)

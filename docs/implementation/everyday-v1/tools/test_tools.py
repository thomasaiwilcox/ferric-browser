#!/usr/bin/env python3
"""Local tests use disposable fake repositories, never live browser data or network."""
from __future__ import annotations
import copy,importlib.util,json,os,subprocess,sys,tempfile,unittest
from pathlib import Path
from unittest.mock import patch
sys.path.insert(0,str(Path(__file__).resolve().parent))
from common import PACK,CLONE,git,load,write_json,digest,packet,validate_repo,safe_relative,repo_snapshot,workspace_root
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
                if task['id']=='F02e2' and task.get('amendment')=='A52':
                    current={**current,'status':'failed','attempts':6,
                             'terminal_failure':validate_pack.A50_TERMINAL_FAILURE.copy(),
                             'supersession':{'amendment':'A52','superseded_by':['F02e2s'],
                                             'transition':'pending_successor_acceptance'}}
                elif task['id']=='F02e2r' and task.get('amendment')=='A52':
                    current={**current,'status':'failed','attempts':3,
                             'supersession':{'amendment':'A52','superseded_by':['F02e2s'],
                                             'transition':'pending_successor_acceptance'}}
                elif task['id']=='F02e2' and task.get('amendment')=='A50':
                    current={**current,'status':'failed','attempts':6,
                             'terminal_failure':validate_pack.A50_TERMINAL_FAILURE.copy(),
                             'supersession':{'amendment':'A50','superseded_by':['F02e2r'],
                                             'transition':'pending_successor_acceptance'}}
                else:
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
    def test_a49_static_qml_prerequisite_is_separate_and_matrix_failure_is_exact(self):
        p=load(PACK/'control/plan.json')
        self.assertEqual(validate_pack.validate_harness_prerequisite_split_A49(p,PACK),[])
        tasks={x['id']:x for x in p['slices']}
        self.assertEqual(tasks['F02e2']['test_ids'],['T014e2'])
        self.assertEqual(tasks['F02e2q']['test_ids'],['T014e2q'])
        self.assertNotIn('max_attempts',tasks['F02e2q'])
        widened=copy.deepcopy(p)
        next(x for x in widened['slices'] if x['id']=='F02e2q')['test_ids']=['T014e2']
        self.assertTrue(any('F02e2q:' in e for e in validate_pack.validate_harness_prerequisite_split_A49(widened,PACK)))
        tests=load(PACK/'control/test_matrix.json')['tests']
        self.assertEqual(validate_pack.validate_matrix_execution_status(tests),[])
        changed=copy.deepcopy(tests)
        next(x for x in changed if x['id']=='T014e2')['binding']=next(x for x in changed if x['id']=='T014e2')['binding'].replace('2e35b498','0e35b498')
        self.assertTrue(any('T014e2: A48 historical failure' in e for e in validate_pack.validate_matrix_execution_status(changed)))
        changed=copy.deepcopy(tests)
        next(x for x in changed if x['id']=='T014e2q')['result']='passed'
        self.assertTrue(any('only the exact A48 T014e2 failure' in e for e in validate_pack.validate_matrix_execution_status(changed)))
    def test_a50_successor_keeps_terminal_parent_closed_until_new_task_accepts(self):
        p=load(PACK/'control/plan.json')
        self.assertEqual(validate_pack.validate_successor_recovery_A50(p,PACK),[])
        self.assertEqual(validate_pack.validate_successor_recovery_A52(p,PACK),[])
        tasks={t['id']:t for t in p['slices']}
        self.assertEqual(tasks['F02e2']['max_attempts'],6)
        self.assertEqual(tasks['F02e2']['superseded_by'],['F02e2s'])
        self.assertEqual(tasks['F02e2r']['test_ids'],['T014e2r'])
        self.assertEqual(tasks['F02e2r']['allowed_paths'],validate_pack.A50_F02E2_PATHS_ORDERED)
        self.assertEqual(tasks['F02e2r']['locks'],validate_pack.A44_LOCKS)
        self.assertEqual(tasks['F02e2r']['superseded_by'],['F02e2s'])
        self.assertEqual(tasks['F02e2s']['test_ids'],['T014e2s'])
        self.assertEqual(tasks['F02e2s']['allowed_paths'],validate_pack.A50_F02E2_PATHS_ORDERED)
        self.assertEqual(tasks['F02e2s']['locks'],validate_pack.A44_LOCKS)
        self.assertEqual(tasks['F02e3']['depends_on'],['F02e2s'])
        self.assertEqual(tasks['F02e3']['historical_depends_on'],['F02e2','F02e2r'])

        wrong=copy.deepcopy(p)
        next(t for t in wrong['slices'] if t['id']=='F02e2r')['test_ids']=['T014e2']
        self.assertTrue(any('F02e2r:' in e for e in validate_pack.validate_successor_recovery_A50(wrong,PACK)))
        wrong=copy.deepcopy(p)
        next(t for t in wrong['slices'] if t['id']=='F02e2r')['locks'].append('integration')
        self.assertTrue(any('F02e2r:' in e for e in validate_pack.validate_successor_recovery_A50(wrong,PACK)))
        wrong=copy.deepcopy(p)
        next(t for t in wrong['slices'] if t['id']=='F02e3')['depends_on']=['F02e2']
        self.assertTrue(any('F02e3:' in e for e in validate_pack.validate_successor_recovery_A50(wrong,PACK)))
        tests=load(PACK/'control/test_matrix.json')['tests']
        changed=copy.deepcopy(tests)
        next(t for t in changed if t['id']=='T014e2r')['slice']='F02e2'
        with tempfile.TemporaryDirectory() as tmp:
            pack=Path(tmp);control=pack/'control';control.mkdir()
            (control/'AMENDMENTS.md').write_text((PACK/'control/AMENDMENTS.md').read_text())
            write_json(control/'source-map.execution.json',load(PACK/'control/source-map.execution.json'))
            write_json(control/'test_matrix.json',{'tests':changed})
            self.assertTrue(any('T014e2r:' in e for e in validate_pack.validate_successor_recovery_A50(p,pack)))

        plan={'slices':[
            {'id':'F02e2','amendment':'A50','superseded_by':['F02e2r']},
            {'id':'F02e2r','depends_on':[]},
        ]}
        state={'slices':{
            'F02e2':{'status':'failed','attempts':6,'terminal_failure':validate_pack.A50_TERMINAL_FAILURE.copy(),
                     'supersession':{'amendment':'A50','superseded_by':['F02e2r'],'transition':'pending_successor_acceptance'}},
            'F02e2r':{'status':'ready'},
        }}
        self.assertEqual(validate_pack.validate_supersession_state(plan,state),[])
        premature=copy.deepcopy(state)
        premature['slices']['F02e2']['status']='superseded'
        premature['slices']['F02e2']['supersession']['transition']='successor_accepted'
        self.assertTrue(any('status must remain failed' in e for e in validate_pack.validate_supersession_state(plan,premature)))
        accepted=copy.deepcopy(state)
        accepted['slices']['F02e2r']['status']='accepted'
        accepted['slices']['F02e2']['status']='superseded'
        accepted['slices']['F02e2']['supersession']['transition']='successor_accepted'
        self.assertEqual(validate_pack.validate_supersession_state(plan,accepted,strict=True),[])
        exhausted=copy.deepcopy(state)
        exhausted['slices']['F02e2']['attempts']=7
        self.assertTrue(any('exhausted 6/6' in e for e in validate_pack.validate_supersession_state(plan,exhausted)))
        mutated=copy.deepcopy(state)
        mutated['slices']['F02e2']['terminal_failure']['receipt_sha256']='0'*64
        self.assertTrue(any('exact immutable A48' in e for e in validate_pack.validate_supersession_state(plan,mutated)))
    def test_a51_continuation_is_bound_to_the_exact_failed_a3_artifacts(self):
        p=load(PACK/'control/plan.json')
        self.assertEqual(validate_pack.validate_continuation_A51(p,PACK),[])
        self.assertEqual(p['native_continuation_A51']['attempt_identity'],{
            'slice':'F02e2r','source_amendment':'A50','attempt':3,'attempt_limit':3,'counter_increment':False})

        changed=copy.deepcopy(p)
        changed['native_continuation_A51']['candidate']['source_sha']='0'*40
        self.assertTrue(any('A51: continuation metadata' in e for e in validate_pack.validate_continuation_A51(changed,PACK)))
        changed=copy.deepcopy(p)
        changed['native_continuation_A51']['artifacts']['integration_executable_sha256']='0'*64
        self.assertTrue(any('A51: continuation metadata' in e for e in validate_pack.validate_continuation_A51(changed,PACK)))
        changed=copy.deepcopy(p)
        changed['native_continuation_A51']['allowed_gates'].append('engine rebuild')
        self.assertTrue(any('A51: continuation metadata' in e for e in validate_pack.validate_continuation_A51(changed,PACK)))

        with tempfile.TemporaryDirectory() as tmp:
            pack=Path(tmp);control=pack/'control';control.mkdir()
            for name in ('AMENDMENTS.md','state.initial.json','source-map.execution.json','test_matrix.json'):
                (control/name).write_bytes((PACK/'control'/name).read_bytes())
            source=load(control/'source-map.execution.json')
            next(x for x in source['execution_amendments'] if x['id']=='A51')['continuation']['artifact_hashes']['link_command_sha256']='0'*64
            write_json(control/'source-map.execution.json',source)
            self.assertTrue(any('A51: source-map continuation record' in e for e in validate_pack.validate_continuation_A51(p,pack)))

    def test_a51_successor_acceptance_requires_passed_continuation_and_independent_review(self):
        incomplete={'slices':{'F02e2r':{'status':'accepted','attempts':3}}}
        self.assertTrue(any('separately reviewed continuation record' in e
                            for e in validate_pack.validate_continuation_state_A51(incomplete,Path('/tmp'))))
        altered={'slices':{'F02e2r':{'status':'accepted','attempts':4,'A51_continuation':{
            'amendment':'A51','source_amendment':'A50','attempt':4,'attempt_limit':3,
            'attempt_count_after':4,'candidate':validate_pack.A51_CANDIDATE.copy(),
            'artifacts':validate_pack.A51_ARTIFACTS.copy(),'preserved_A50_run':validate_pack.A51_A50_RUN.copy(),
            'preserved_A50_lease_closure':validate_pack.A51_A50_CLOSURE.copy(),'result':'passed',
            'product_tests_run':['T014e2r','T014e2'],'old_T014e2_ran':True,
            'source_changed':False,'build_or_configure_ran':False}}}}
        errors=validate_pack.validate_continuation_state_A51(altered,Path('/tmp'))
        self.assertTrue(any('attempt 3/3' in e for e in errors))
        self.assertTrue(any('continuation field attempt' in e for e in errors))
        self.assertTrue(any('product_tests_run' in e for e in errors))
    def test_a52_preserves_both_failed_histories_and_blocks_early_supersession(self):
        p=load(PACK/'control/plan.json')
        prior=load(PACK/'control/state.initial.json')['slices']
        states={t['id']:copy.deepcopy(prior.get(t['id'],{'status':'not_started','attempts':0})) for t in p['slices']}
        states['F02e2'].update({'status':'failed','attempts':6,
            'terminal_failure':validate_pack.A50_TERMINAL_FAILURE.copy(),
            'supersession':{'amendment':'A52','superseded_by':['F02e2s'],'transition':'pending_successor_acceptance'}})
        states['F02e2r'].update({'status':'failed','attempts':3,
            'supersession':{'amendment':'A52','superseded_by':['F02e2s'],'transition':'pending_successor_acceptance'}})
        state={'slices':states}
        self.assertEqual(validate_pack.validate_supersession_state(p,state),[])
        premature=copy.deepcopy(state)
        for sid in ('F02e2','F02e2r'):
            premature['slices'][sid]['status']='superseded'
            premature['slices'][sid]['supersession']['transition']='successor_accepted'
        self.assertTrue(any('status must remain failed' in e
                            for e in validate_pack.validate_supersession_state(p,premature)))
        accepted=copy.deepcopy(state)
        accepted['slices']['F02e2s']['status']='accepted'
        for sid in ('F02e2','F02e2r'):
            accepted['slices'][sid]['status']='superseded'
            accepted['slices'][sid]['supersession']['transition']='successor_accepted'
        self.assertEqual(validate_pack.validate_supersession_state(p,accepted),[])
        changed=copy.deepcopy(state);changed['slices']['F02e2r']['attempts']=4
        self.assertTrue(any('exhausted attempt count 3/3' in e
                            for e in validate_pack.validate_supersession_state(p,changed)))
        wrong=copy.deepcopy(p);next(t for t in wrong['slices'] if t['id']=='F02e2s')['allowed_paths'].append('crates/unauthorized.rs')
        self.assertTrue(any('F02e2s:' in e for e in validate_pack.validate_successor_recovery_A52(wrong,PACK)))
        wrong=copy.deepcopy(p);wrong['successor_recovery_A52']['terminal_history']['F02e2r']['A51_continuation_sha256']='0'*64
        self.assertTrue(any('metadata differs' in e for e in validate_pack.validate_successor_recovery_A52(wrong,PACK)))
        wrong=copy.deepcopy(p);next(t for t in wrong['slices'] if t['id']=='F02e3')['depends_on']=['F02e2r']
        self.assertTrue(any('F02e3:' in e for e in validate_pack.validate_successor_recovery_A52(wrong,PACK)))

    def test_a52_old_a51_failure_evidence_remains_valid_after_successor_acceptance(self):
        root=workspace_root()
        state=load(root/'control/state.json')
        old=state['slices']['F02e2r']
        old['status']='superseded'
        old['supersession']={'amendment':'A52','superseded_by':['F02e2s'],'transition':'successor_accepted'}
        state['slices']['F02e2s']={'status':'accepted'}
        self.assertEqual(validate_pack.validate_continuation_state_A51(state,root),[])

    def test_a52_acceptance_requires_independent_exact_t014e2s_gate_review(self):
        names=validate_pack.A52_NATIVE_GATE_SEQUENCE
        source='a'*40;tree='b'*40
        task=next(t for t in load(PACK/'control/plan.json')['slices'] if t['id']=='F02e2s')
        record={'status':'accepted','author':'worker','attempts':1,'evidence':'evidence/F02e2s-a1/EVIDENCE.json'}

        def build_case(root):
            case=root/'evidence/F02e2s-a1';native=case/'native';native.mkdir(parents=True)
            target_dir=root/'builds/native-F02e2s-a1-qt6112-qmake';target_dir.mkdir(parents=True)
            target=str(target_dir.resolve())
            review_path=case/'GATE_EVIDENCE_REVIEW.json';run_path=native/'NATIVE_RUN-A52-a1.json'
            closure_path=native/'NATIVE_LEASE_CLOSED-A52-a1.json';monitor_path=native/'live-monitor.json'
            artifacts={}
            exe=target_dir/'public_page_host_integration_tests';exe.write_bytes(b'fixture executable');exe.chmod(0o755)
            kf6_library='/runtime/lib/libKF6WindowSystem.so.6.28.0'
            link=target_dir/'link.txt';link.write_text(f'libferric_browser_engine_qt.a {kf6_library}\n')
            for index,name in enumerate(names):
                if name=='read-only-sdk-selection-preflight':
                    path=case/'native/sdk-preflight.json'
                    write_json(path,{'result':'passed','exit_code':0,'no_compilation':True,'target':target,'attempt':1,
                        'source_before':{'candidate':{'sha':source,'tree':tree,'dirty':False}},
                        'source_after':{'candidate':{'sha':source,'tree':tree,'dirty':False}},
                        'sdk':{'qt_version':'6.11.2','version':'6.28.0','abi':'Qt_6.11','library':kf6_library}})
                elif name=='robust-executable-link-order-verification':
                    path=case/'native/link-order.json'
                    write_json(path,{'result':'passed','source_sha':source,'source_tree':tree,'target':target,
                        'checks':{'engine_archive_before_kf6windowsystem':True,'matching_kf6_runtime':True,
                                  'qt_version':'6.11.2','kf6_window_system_qt_abi':'Qt_6.11',
                                  'kf6_window_system_library':kf6_library},
                        'artifacts':{'executable':{'path':str(exe.relative_to(root)),'sha256':digest(exe)},
                                     'link_command':{'path':str(link.relative_to(root)),'sha256':digest(link)}}})
                else:
                    gate_dir=native/f'gate-{index}';gate_dir.mkdir()
                    log=gate_dir/'output.log'
                    if name=='nonzero-ctest-discovery':
                        log.write_text('Test project\n  Test #9: T014e2s\n  Total Tests: 9\n')
                        command=['ctest','--test-dir',str(target_dir/'cmake'),'-N']
                    elif name=='T014e2s-serial':
                        log.write_text('1/1 Test #9: T014e2s Passed\n100% tests passed, 0 tests failed out of 1\n')
                        command=['ctest','--test-dir',str(target_dir/'cmake'),'--output-on-failure','--parallel','1','--tests-regex','^T014e2s$']
                    elif name=='exact-source-release-engine-build':
                        log.write_text('Finished release build\n');command=['cargo','build','--release','--target-dir',str((target_dir/'cargo-target').resolve())]
                    elif name=='public-page-host-harness-configure':
                        log.write_text('Configuring harness\n');command=['cmake','-S','tests/public-page-host','-B',str(target_dir/'cmake')]
                    else:
                        log.write_text('Built integration target\n');command=['cmake','--build',str(target_dir/'cmake'),'--target','public_page_host_integration_tests']
                    receipt={'result':'passed','exit_code':0,'timed_out':False,'output_limit_exceeded':False,'observed_failed':0,
                        'source_before':{'sha':source,'tree':tree,'dirty':False},'source_after':{'sha':source,'tree':tree,'dirty':False},
                        'log':'output.log','log_sha256':digest(log),'command':command,'qt_include_guard':'clear',
                        'observed_test_summary_count':1 if name=='T014e2s-serial' else 0,'observed_skips':0,
                        'native_target':target,'execution_binding':{'native_target':target}}
                    if name=='exact-source-release-engine-build':
                        cargo_target=str((target_dir/'cargo-target').resolve())
                        receipt['execution_binding'].update({'cargo_target_dir':cargo_target,'environment':{'CARGO_TARGET_DIR':cargo_target}})
                    elif name in {'public-page-host-harness-configure','integration-target-build','nonzero-ctest-discovery','T014e2s-serial'}:
                        receipt['execution_binding']['cmake_binary_dir']=str((target_dir/'cmake').resolve())
                    if name=='T014e2s-serial':receipt['require_tests']=True
                    path=gate_dir/'receipt.json';write_json(path,receipt)
                artifacts[name]=path
            gate_records=[];review_gates=[]
            for index,name in enumerate(names):
                path=artifacts[name];sha=digest(path)
                gate={'name':name,'result':'passed','target':target,'receipt':str(path.relative_to(root)),'receipt_sha256':sha}
                reviewer_gate={'name':name,'decision':'pass','target':target,'receipt':gate['receipt'],'receipt_sha256':sha}
                if name=='nonzero-ctest-discovery':
                    gate.update({'registered_tests':9,'T014e2s_registered':True})
                    reviewer_gate.update({'registered_tests':9,'T014e2s_registered':True})
                if name=='T014e2s-serial':
                    gate.update({'test_id':'T014e2s','run_count':1,'failed':0,'skipped':0,'parallelism':1})
                    reviewer_gate.update({'test_id':'T014e2s','executed_test_count':1,'failures':0,'skips':0,'parallelism':1})
                gate_records.append(gate);review_gates.append(reviewer_gate)
            monitor={'result':'clear','source_sha':source,'source_tree':tree,'attempt':1,'artifact':target_dir.name}
            write_json(monitor_path,monitor)
            run={'result':'passed','slice':'F02e2s','amendment':'A52','attempt':1,'attempt_limit':3,'source_sha':source,'source_tree':tree,
                'product_tests_run':['T014e2s'],'target':target,'gates':gate_records,'lease_released':True,
                'active_build_processes_remaining':[],'live_process_monitor':{'result':'clear','path':str(monitor_path.relative_to(root)),'sha256':digest(monitor_path)}}
            closure={'artifact':target_dir.name,'source_sha':source,'source_tree':tree,'attempt':1,
                'live_process_monitor':str(monitor_path.relative_to(root)),'live_process_monitor_sha256':digest(monitor_path),
                'active_lock_released':True,'active_build_processes_remaining':[]}
            review={'reviewer':'independent-reviewer','decision':'accept',
                'candidate':{'slice':'F02e2s','amendment':'A52','attempt':1,'commit':source,'tree':tree},'gates':review_gates,
                'run_and_closure':{'target':target,'all_commands_passed':True,'lease_released':True,
                    'active_build_processes_remaining':[],'old_T014e2_rerun':False,'old_T014e2r_ran':False,'other_product_tests_run':False}}
            write_json(run_path,run);write_json(closure_path,closure);write_json(review_path,review)
            evidence_path=case/'EVIDENCE.json'
            evidence={'source_sha':source,'source_tree':tree,'reviewer':'independent-reviewer',
                'gate_evidence_review':'evidence/F02e2s-a1/GATE_EVIDENCE_REVIEW.json','gate_evidence_review_sha256':digest(review_path),
                'native_run':'evidence/F02e2s-a1/native/NATIVE_RUN-A52-a1.json','native_run_sha256':digest(run_path),
                'lease_closure':'evidence/F02e2s-a1/native/NATIVE_LEASE_CLOSED-A52-a1.json','lease_closure_sha256':digest(closure_path)}
            write_json(evidence_path,evidence)
            return {'root':root,'run':run,'review':review,'closure':closure,'evidence':evidence,'artifacts':artifacts,
                    'run_path':run_path,'review_path':review_path,'closure_path':closure_path,'evidence_path':evidence_path}

        def seal(case):
            for gate,review_gate in zip(case['run']['gates'],case['review']['gates']):
                path=safe_relative(case['root'],gate['receipt']);sha=digest(path)
                gate['receipt_sha256']=sha;review_gate['receipt_sha256']=sha
            write_json(case['run_path'],case['run']);write_json(case['closure_path'],case['closure'])
            write_json(case['review_path'],case['review'])
            case['evidence']['native_run_sha256']=digest(case['run_path'])
            case['evidence']['lease_closure_sha256']=digest(case['closure_path'])
            case['evidence']['gate_evidence_review_sha256']=digest(case['review_path'])
            write_json(case['evidence_path'],case['evidence'])

        with tempfile.TemporaryDirectory() as tmp:
            case=build_case(Path(tmp))
            self.assertEqual(validate_pack.validate_acceptance_A52(task,record,case['root']),[])

        for name in names:
            for mutation in ('missing gate','failed gate','missing receipt','wrong receipt artifact','wrong source'):
                with self.subTest(gate=name,mutation=mutation), tempfile.TemporaryDirectory() as tmp:
                    case=build_case(Path(tmp));index=names.index(name);gate=case['run']['gates'][index]
                    receipt_path=safe_relative(case['root'],gate['receipt'])
                    if mutation=='missing gate':
                        case['run']['gates'].pop(index);case['review']['gates'].pop(index)
                    elif mutation=='failed gate':
                        gate['result']='failed';case['review']['gates'][index]['decision']='fail'
                    elif mutation=='missing receipt':receipt_path.unlink()
                    elif mutation=='wrong receipt artifact':
                        receipt_path.write_bytes(receipt_path.read_bytes()+b' tampered')
                    else:
                        receipt=load(receipt_path)
                        if name=='robust-executable-link-order-verification':receipt['source_sha']='c'*40
                        elif name=='read-only-sdk-selection-preflight':receipt['source_before']['candidate']['sha']='c'*40
                        else:receipt['source_before']['sha']='c'*40
                        write_json(receipt_path,receipt);seal(case)
                    if mutation in {'missing gate','failed gate','wrong source'}:seal(case)
                    errors=validate_pack.validate_acceptance_A52(task,record,case['root'])
                    self.assertTrue(errors, f'{name} accepted with {mutation}')

        for name in names[1:4]+names[5:]:
            for mutation in ('wrong target binding','wrong invocation target'):
                with self.subTest(gate=name,mutation=mutation), tempfile.TemporaryDirectory() as tmp:
                    case=build_case(Path(tmp));receipt_path=case['artifacts'][name];receipt=load(receipt_path)
                    other=str((case['root']/'builds/other-target').resolve())
                    if mutation=='wrong target binding':
                        receipt['native_target']=other
                    else:
                        command=receipt['command']
                        if name=='exact-source-release-engine-build':
                            command[command.index('--target-dir')+1]=str((Path(other)/'cargo-target').resolve())
                        else:
                            option='-B' if name=='public-page-host-harness-configure' else '--build' if name=='integration-target-build' else '--test-dir'
                            command[command.index(option)+1]=str((Path(other)/'cmake').resolve())
                    write_json(receipt_path,receipt);seal(case)
                    errors=validate_pack.validate_acceptance_A52(task,record,case['root'])
                    self.assertTrue(errors, f'{name} accepted with {mutation}')

        with tempfile.TemporaryDirectory() as tmp:
            case=build_case(Path(tmp));receipt_path=case['artifacts']['exact-source-release-engine-build']
            receipt=load(receipt_path);receipt['execution_binding']['environment']['CARGO_TARGET_DIR']='/tmp/stale-cargo-target'
            write_json(receipt_path,receipt);seal(case)
            self.assertTrue(validate_pack.validate_acceptance_A52(task,record,case['root']))

        for mutation in ('misordered actual link command','same-basename foreign KF6 library',
                         'missing executable artifact','missing link-command artifact','wrong matching library'):
            with self.subTest(link_artifact=mutation), tempfile.TemporaryDirectory() as tmp:
                case=build_case(Path(tmp));report_path=case['artifacts']['robust-executable-link-order-verification']
                report=load(report_path);files=report['artifacts']
                if mutation=='misordered actual link command':
                    link_path=safe_relative(case['root'],files['link_command']['path'])
                    link_path.write_text('/runtime/lib/libKF6WindowSystem.so.6.28.0 libferric_browser_engine_qt.a\n')
                    files['link_command']['sha256']=digest(link_path);write_json(report_path,report);seal(case)
                elif mutation=='same-basename foreign KF6 library':
                    link_path=safe_relative(case['root'],files['link_command']['path'])
                    link_path.write_text(f"libferric_browser_engine_qt.a /other/lib/libKF6WindowSystem.so.6.28.0\n")
                    files['link_command']['sha256']=digest(link_path);write_json(report_path,report);seal(case)
                elif mutation in {'missing executable artifact','missing link-command artifact'}:
                    name='executable' if mutation=='missing executable artifact' else 'link_command'
                    safe_relative(case['root'],files[name]['path']).unlink();seal(case)
                else:
                    report['checks']['kf6_window_system_library']='/wrong/libKF6WindowSystem.so.6.28.0'
                    write_json(report_path,report);seal(case)
                self.assertTrue(validate_pack.validate_acceptance_A52(task,record,case['root']))

        with tempfile.TemporaryDirectory() as tmp:
            case=build_case(Path(tmp));case['review']['reviewer']='worker';seal(case)
            self.assertTrue(any('independently accept' in e for e in validate_pack.validate_acceptance_A52(task,record,case['root'])))
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
        manifest_path=workspace_root()/'evidence/A47/NATIVE_COMMANDS-A47-r1.json'
        manifest=load(manifest_path)
        self.assertEqual(manifest['amendment'],'A47');self.assertEqual(manifest['retry_number'],3)
        self.assertEqual(manifest['sdk']['pkg_config_path'],qt_pc)
        self.assertEqual(manifest['sdk']['pkg_config_libdir'],retry['pkg_config_libdir'])
        for gate in manifest['gates']:
            env=gate.get('environment',{})
            self.assertEqual(env.get('PKG_CONFIG_PATH'),qt_pc)
            self.assertEqual(env.get('PKG_CONFIG_LIBDIR'),retry['pkg_config_libdir'])
        versions_receipt=load(workspace_root()/'evidence/A47/pkgconfig-modversions-r1/receipt.json')
        versions=(workspace_root()/'evidence/A47/pkgconfig-modversions-r1/output.log').read_text().split()
        self.assertEqual(versions_receipt['result'],'passed')
        self.assertEqual(versions,['6.11.2']*4+['6.31.0'])
        cflags_receipt=load(workspace_root()/'evidence/A47/pkgconfig-cflags-r1/receipt.json')
        cflags=(workspace_root()/'evidence/A47/pkgconfig-cflags-r1/output.log').read_text().split()
        self.assertEqual(cflags_receipt['result'],'passed')
        self.assertTrue(any(x.startswith('-I/usr/include/KF6/KWindowSystem') for x in cflags))
        self.assertTrue(all(not x.startswith('-I') or '/Qt' not in x and '/qt' not in x or x[2:].startswith('/home/tom/.local/share/ferric-browser-runtime/qt/6.11.2/gcc_64/') for x in cflags))
        source_map=load(PACK/'control/source-map.execution.json')
        records=[x for x in source_map['execution_amendments'] if x.get('id')=='A47']
        self.assertEqual(len(records),1);self.assertEqual(records[0]['pkg_config_libdir'],retry['pkg_config_libdir'])
        watcher=workspace_root()/'evidence/A47/watch_qt_includes.py'
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

class ProfileIdentityA53Tests(unittest.TestCase):
    def setUp(self):
        self.plan=load(PACK/'control/plan.json')
        self.contract=validate_pack.profile_identity_contract(PACK)
        self.tmp=tempfile.TemporaryDirectory(prefix='ferric-a53-test-')
        self.root=Path(self.tmp.name)
        self.addCleanup(self.tmp.cleanup)

    def state(self):
        return {'slices':{'F02e2s':copy.deepcopy(self.contract['initial_parent']),
                          'F02e2i':copy.deepcopy(load(PACK/'control/state.initial.json')['slices']['F02e2i'])}}

    def test_approved_scope_dependency_and_case_contract(self):
        self.assertEqual(validate_pack.validate_profile_identity_A53(self.plan,PACK),[])
        mutations=[('F02e2i','allowed_paths',[]),('F02e2i','max_attempts',5),
                   ('F02e2i','locks',[]),('F02e2s','depends_on',['F02e1','F02e2q'])]
        for sid,key,value in mutations:
            with self.subTest(sid=sid,key=key):
                p=copy.deepcopy(self.plan)
                next(t for t in p['slices'] if t['id']==sid)[key]=value
                self.assertTrue(validate_pack.validate_profile_identity_A53(p,PACK))
        with self.subTest('missing amendment'):
            p=copy.deepcopy(self.plan);p.pop('profile_instance_prerequisite_A53')
            self.assertTrue(validate_pack.validate_profile_identity_A53(p,PACK))

    def test_contract_file_and_initial_history_cannot_be_rewritten(self):
        from shutil import copytree
        pack=self.root/'pack';copytree(PACK/'control',pack/'control')
        for name,mutate in [
            ('profile-identity.A53.json',lambda d:d['identity_cases'].pop()),
            ('state.initial.json',lambda d:d['slices']['F02e2s'].update(attempts=0)),
            ('test_matrix.json',lambda d:d['tests'].__setitem__(slice(None),[x for x in d['tests'] if x['id']!='T014e2i']))]:
            path=pack/'control'/name;original=path.read_bytes();data=load(path);mutate(data);write_json(path,data)
            with self.subTest(name=name):self.assertTrue(validate_pack.validate_profile_identity_A53(self.plan,pack))
            path.write_bytes(original)

    def test_parent_stays_reserved_and_waits_without_locks(self):
        self.assertEqual(validate_pack.validate_profile_identity_state_A53(self.plan,self.state(),self.root,True),[])
        for changes in [{'attempts':0},{'attempts':4},{'status':'in_progress'},
                        {'native_work_authorized':True},{'source_changes_authorized':True},{'preserved_failures_A53':{}}]:
            state=self.state();state['slices']['F02e2s'].update(changes)
            with self.subTest(changes=changes):
                self.assertTrue(validate_pack.validate_profile_identity_state_A53(self.plan,state,self.root,True))
        state=self.state();state['active_locks']={'bridge':{'slice':'F02e2s'}}
        self.assertTrue(validate_pack.validate_profile_identity_state_A53(self.plan,state,self.root,True))

    def test_continued_parent_uses_exact_accepted_child_base(self):
        state=self.state();child=state['slices']['F02e2i'];parent=state['slices']['F02e2s']
        child.update(status='accepted',source_sha='a'*40,source_tree='b'*40)
        parent.update(status='in_progress',base_sha='a'*40,base_tree='b'*40)
        # This helper's non-initial path also verifies immutable failed evidence.
        # Isolate that boundary with disposable copies and matching hashes.
        c=copy.deepcopy(self.contract)
        for name,refs in c['metadata']['preserved_failures'].items():
            run=self.root/refs['run'];closure=self.root/refs['closure']
            write_json(run,{'result':'failed','lease_released':True,'active_build_processes_remaining':[],
                            'product_tests_run':['T014e2s'] if name=='attempt2' else []})
            write_json(closure,{'active_lock_released':True,'active_build_processes_remaining':[]})
            refs.update(run_sha256=digest(run),closure_sha256=digest(closure))
        request=self.root/c['metadata']['scope_request']['path'];write_json(request,{'fixture':'scope stop'})
        c['metadata']['scope_request']['sha256']=digest(request)
        parent['preserved_failures_A53']=copy.deepcopy(c['metadata']['preserved_failures'])
        parent['scope_request_A53']=copy.deepcopy(c['metadata']['scope_request'])
        with patch.object(validate_pack,'profile_identity_contract',return_value=c):
            self.assertEqual(validate_pack.validate_profile_identity_state_A53(self.plan,state,self.root),[])
            parent['base_tree']='c'*40
            self.assertTrue(validate_pack.validate_profile_identity_state_A53(self.plan,state,self.root))
            parent['base_tree']='b'*40
            request.write_text('changed scope evidence')
            self.assertTrue(validate_pack.validate_profile_identity_state_A53(self.plan,state,self.root))
            write_json(request,{'fixture':'scope stop'})
            run.write_text('changed failed run')
            self.assertTrue(validate_pack.validate_profile_identity_state_A53(self.plan,state,self.root))

    def acceptance_fixture(self):
        repository=self.root/'worktrees'/'identity'
        source_file=repository/self.contract['task']['allowed_paths'][1]
        if not repository.exists():
            repository.mkdir(parents=True)
            git(repository,'init','--initial-branch=main')
            git(repository,'config','user.name','Pack Fixture');git(repository,'config','user.email','fixture@example.invalid')
            source_file.parent.mkdir(parents=True);source_file.write_text('fixture baseline\n')
            git(repository,'add','.');git(repository,'commit','-m','fixture baseline')
            self.identity_fixture_contract=copy.deepcopy(self.contract)
            self.identity_fixture_contract['metadata'].update(source_base_sha=git(repository,'rev-parse','HEAD'),
                                                              source_base_tree=git(repository,'rev-parse','HEAD^{tree}'))
        source_file.write_text(source_file.read_text()+'fixture child change\n')
        git(repository,'add','.');git(repository,'commit','-m','fixture child')
        source=git(repository,'rev-parse','HEAD');tree=git(repository,'rev-parse','HEAD^{tree}')
        target=str(self.root/'builds'/'identity-target')
        case_map={case:'profile_session::tests::profile_instance_identity_'+case for case in self.contract['identity_cases']}
        log_text='\n'.join('test '+name+' ... ok' for name in case_map.values())+'\ntest result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n'
        gates={}
        for name in ['focused','application_regression']:
            path=self.root/'evidence'/name/'receipt.json';path.parent.mkdir(parents=True,exist_ok=True)
            log=path.parent/'output.log';log.write_text(log_text)
            cmd=['cargo','+stable','test','--package','ferric-browser-application','--lib','--locked','--offline','--jobs','2','--target-dir',target]
            if name=='focused':cmd+=['profile_instance_identity']
            cmd+=['--','--nocapture']
            snapshot={'sha':source,'tree':tree,'dirty':False}
            receipt={'cwd':str(repository),'result':'passed','exit_code':0,'timed_out':False,'output_limit_exceeded':False,
                     'require_tests':True,'observed_test_summary_count':8,'observed_skips':0,'observed_failed':0,
                     'source_before':snapshot,'source_after':snapshot,'command':cmd,
                     'execution_binding':{'cargo_target_dir':target,'environment':{'CARGO_TARGET_DIR':target}},
                     'log':'output.log','log_sha256':digest(log)}
            write_json(path,receipt);gates[name]={'receipt':str(path.relative_to(self.root)),'sha256':digest(path)}
        review={'decision':'accept','reviewer':'independent-reviewer',
                'candidate':{'slice':'F02e2i','commit':source,'tree':tree,'attempt':1},
                'identity_case_tests':case_map,'unit_gates':gates}
        evidence={'source_repository':str(repository),'source_sha':source,'source_tree':tree,'reviewer':review['reviewer'],
                  'identity_case_tests':case_map,'unit_gates':gates,
                  'identity_evidence_review':'evidence/review.json'}
        record={'status':'accepted','attempts':1,'author':'worker','reviewer':review['reviewer'],
                'source_sha':source,'source_tree':tree,'evidence':'evidence/EVIDENCE.json'}
        return record,evidence,review

    def save_acceptance(self,evidence,review):
        write_json(self.root/'evidence/review.json',review)
        evidence['identity_evidence_review_sha256']=digest(self.root/'evidence/review.json')
        write_json(self.root/'evidence/EVIDENCE.json',evidence)

    def check_acceptance(self,record):
        with patch.object(validate_pack,'profile_identity_contract',return_value=self.identity_fixture_contract):
            return validate_pack.validate_identity_acceptance_A53(self.contract['task'],record,self.root)

    def test_unrelated_commit_and_extra_product_path_rejected(self):
        for mutation in ['unrelated','extra_path','wrong_cwd']:
            with self.subTest(mutation=mutation):
                record,evidence,review=self.acceptance_fixture();repository=Path(evidence['source_repository'])
                if mutation=='unrelated':
                    source=git(repository,'commit-tree',evidence['source_tree'],'-m','unrelated fixture commit')
                elif mutation=='extra_path':
                    (repository/'extra.rs').write_text('outside scope\n')
                    git(repository,'add','.');git(repository,'commit','-m','out of scope fixture')
                    source=git(repository,'rev-parse','HEAD')
                else:source=evidence['source_sha']
                tree=git(repository,'rev-parse',f'{source}^{{tree}}')
                record.update(source_sha=source,source_tree=tree);evidence.update(source_sha=source,source_tree=tree)
                review['candidate'].update(commit=source,tree=tree)
                for ref in evidence['unit_gates'].values():
                    path=self.root/ref['receipt'];receipt=load(path)
                    for key in ['source_before','source_after']:receipt[key].update(sha=source,tree=tree)
                    if mutation=='wrong_cwd':receipt['cwd']=str(self.root)
                    write_json(path,receipt);ref['sha256']=digest(path)
                self.save_acceptance(evidence,review)
                self.assertTrue(self.check_acceptance(record))
                if mutation=='extra_path':
                    (repository/'extra.rs').unlink();git(repository,'add','.');git(repository,'commit','-m','restore fixture scope')

    def test_exact_independently_reviewed_two_gate_identity_evidence(self):
        record,evidence,review=self.acceptance_fixture();self.save_acceptance(evidence,review)
        self.assertEqual(self.check_acceptance(record),[])
        record['author']=review['reviewer'];self.assertTrue(self.check_acceptance(record))

    def test_missing_case_wrong_review_source_and_changed_receipt_rejected(self):
        for mutation in ['case','source','receipt','review_digest','record_tree']:
            with self.subTest(mutation=mutation):
                record,evidence,review=self.acceptance_fixture()
                if mutation=='case':
                    evidence['identity_case_tests'].pop('weak_lifetime_and_renewal')
                if mutation=='source':review['candidate']['commit']='c'*40
                if mutation=='record_tree':record['source_tree']='c'*40
                self.save_acceptance(evidence,review)
                if mutation=='receipt':
                    (self.root/evidence['unit_gates']['focused']['receipt']).write_text('{}')
                if mutation=='review_digest':(self.root/'evidence/review.json').write_text('{}')
                self.assertTrue(self.check_acceptance(record))

    def test_rehashed_bad_gate_and_missing_executed_case_rejected(self):
        for mutation in ['failed','empty','skipped','dirty','source','narrow_full','target','log','unexecuted_case']:
            with self.subTest(mutation=mutation):
                record,evidence,review=self.acceptance_fixture()
                name='application_regression' if mutation=='narrow_full' else 'focused'
                ref=evidence['unit_gates'][name];path=self.root/ref['receipt'];receipt=load(path)
                if mutation=='failed':receipt['exit_code']=1
                if mutation=='empty':receipt['observed_test_summary_count']=0
                if mutation=='skipped':receipt['observed_skips']=1
                if mutation=='dirty':receipt['source_after']['dirty']=True
                if mutation=='source':receipt['source_before']['tree']='c'*40
                if mutation=='narrow_full':receipt['command'].insert(-2,'profile_instance_identity')
                if mutation=='target':receipt['execution_binding']['environment']['CARGO_TARGET_DIR']='/tmp/foreign'
                if mutation in ['log','unexecuted_case']:
                    log=path.parent/'output.log'
                    if mutation=='log':log.write_text('tampered log')
                    else:
                        log.write_text('\n'.join(log.read_text().splitlines()[1:])+'\n')
                        receipt['log_sha256']=digest(log)
                write_json(path,receipt);ref['sha256']=digest(path)
                self.save_acceptance(evidence,review)
                self.assertTrue(self.check_acceptance(record))


class IdentityRepairA54Tests(unittest.TestCase):
    def setUp(self):
        self.plan=load(PACK/'control/plan.json');self.contract=validate_pack.identity_repair_contract_A54(PACK)
        temporary=tempfile.TemporaryDirectory(prefix='ferric-a54-test-');self.addCleanup(temporary.cleanup)
        self.root=Path(temporary.name)

    def fixture(self):
        c=copy.deepcopy(self.contract);state={'slices':{'F02e2i':copy.deepcopy(c['initial_child'])},'active':[],'active_locks':{}}
        # Disposable evidence bytes replace fixture hashes only. The separate
        # structural test verifies the real immutable canonical contract.
        for number,record in c['history'].items():
            for name,ref in record['files'].items():
                path=self.root/ref['path'];write_json(path,{'fixture':number,'file':name});ref['sha256']=digest(path)
        patch_path=self.root/c['metadata']['repair_patch']['path'];patch_path.parent.mkdir(parents=True,exist_ok=True)
        patch_path.write_text('disposable exact cfg-test patch\n');c['metadata']['repair_patch']['sha256']=digest(patch_path)
        review_path=self.root/'evidence/pack-review.json'
        review={'decision':'accept','reviewer':'independent-pack-reviewer','candidate':{'commit':'a'*40,'tree':'b'*40}}
        write_json(review_path,review)
        state['slices']['F02e2i']['source_repair_A54_authority']={
            'review':'evidence/pack-review.json','review_sha256':digest(review_path),'pack_commit':'a'*40,'pack_tree':'b'*40}
        return c,state

    def check_state(self,c,state):
        with patch.object(validate_pack,'identity_repair_contract_A54',return_value=c):
            return validate_pack.validate_identity_repair_state_A54(self.plan,state,self.root)

    def assign(self,c,state):
        child=state['slices']['F02e2i'];meta=c['metadata'];child.update(attempts=3,status='ready',author='fixture-worker',source_sha=None)
        assignment={'slice':'F02e2i','amendment':'A54','attempt':4,'attempt_limit':4,'worker':'fixture-worker',
                    'base_sha':meta['source_base_sha'],'base_tree':meta['source_base_tree'],
                    'reapply_source_sha':meta['reapply_source_sha'],'reapply_source_tree':meta['reapply_source_tree'],
                    'repair_patch_sha256':meta['repair_patch']['sha256'],'expected_lib_sha256':meta['expected_lib_sha256'],
                    'unchanged_session_sha256':meta['unchanged_session_sha256'],'pack_commit':'a'*40,'pack_tree':'b'*40,
                    'run_nonce':'f'*32}
        assignment['lifecycle_seed_sha256']=validate_pack.identity_attempt_event_hash_A54(validate_pack.identity_attempt_seed_A54(assignment['run_nonce']))
        path=self.root/meta['source4_assignment'];write_json(path,assignment)
        child.update(assignment=meta['source4_assignment'],assignment_sha256=digest(path))
        validate_pack.append_identity_attempt_event_A54(self.root,child,c,'assigned')
        child.update(attempts=4,status='in_progress')

    def terminal(self,c,state):
        meta=c['metadata'];path=self.root/meta['terminal_marker'];write_json(path,{'slice':'F02e2i','attempt':4,'result':'failed','no_retry':True,
            'failed_artifacts':validate_pack.identity_failed_artifacts_A54(self.root,meta)})
        child=state['slices']['F02e2i'];child.update(status='failed',source_changes_authorized=False,compile_test_authorized=False)
        child['attempt_history']['4']={'attempt':4,'result':'failed_terminal','assignment_sha256':child['assignment_sha256'],
                                      'terminal':{'path':meta['terminal_marker'],'sha256':digest(path)}}
        validate_pack.append_identity_attempt_event_A54(self.root,child,c,'terminal_failed',meta['terminal_marker'])

    def product_fixture(self,c):
        repository=self.root/'worktrees/identity';repository.mkdir(parents=True)
        git(repository,'init','--initial-branch=main');git(repository,'config','user.name','Pack Fixture');git(repository,'config','user.email','fixture@example.invalid')
        paths={'crates/ferric-browser-application/src/lib.rs':'expected cfg tests\n',
               'crates/ferric-browser-application/src/profile_session.rs':'unchanged actual session\n'}
        for path,body in paths.items():
            file=repository/path;file.parent.mkdir(parents=True,exist_ok=True);file.write_text(body)
        c['metadata']['expected_lib_sha256']=digest(repository/'crates/ferric-browser-application/src/lib.rs')
        c['metadata']['unchanged_session_sha256']=digest(repository/'crates/ferric-browser-application/src/profile_session.rs')
        git(repository,'add','.');git(repository,'commit','-m','exact fixture source')
        return repository,git(repository,'rev-parse','HEAD'),git(repository,'rev-parse','HEAD^{tree}'),paths

    def test_exact_named_exception_and_narrow_a53_overlay(self):
        self.assertEqual(validate_pack.validate_identity_repair_A54(self.plan,PACK),[])
        self.assertEqual(validate_pack.validate_profile_identity_A53(self.plan,PACK),[])
        task=next(t for t in self.plan['slices'] if t['id']=='F02e2i')
        self.assertEqual(validate_pack.effective_attempt_limit(self.plan,task),4)
        for mutation in ['global','parent','child5','wrong_amendment','bool','missing_exception','other_task']:
            with self.subTest(mutation=mutation):
                plan=copy.deepcopy(self.plan)
                if mutation=='global':plan['limits']['max_attempts']=4
                if mutation=='parent':next(t for t in plan['slices'] if t['id']=='F02e2s')['max_attempts']=4
                if mutation=='child5':next(t for t in plan['slices'] if t['id']=='F02e2i')['max_attempts']=5
                if mutation=='wrong_amendment':plan['attempt_exceptions']['F02e2i']['amendment']='A53'
                if mutation=='bool':plan['attempt_exceptions']['F02e2i']['authorized_limit']=True
                if mutation=='missing_exception':plan['attempt_exceptions'].pop('F02e2i')
                if mutation=='other_task':plan['attempt_exceptions']['F02c']=plan['attempt_exceptions']['F02e2i'].copy()
                errors=validate_pack.validate_identity_repair_A54(plan,PACK)+validate_pack.validate_attempt_limits(plan,{t['id']:t for t in plan['slices']})
                self.assertTrue(errors)
        plan=copy.deepcopy(self.plan);next(t for t in plan['slices'] if t['id']=='F02e2i')['locks']=[]
        self.assertTrue(validate_pack.validate_profile_identity_A53(plan,PACK))

    def test_canonical_contract_and_initial_history_are_immutable(self):
        from shutil import copytree
        pack=self.root/'pack';copytree(PACK/'control',pack/'control')
        for name in ['profile-identity.A53.json','profile-identity-repair.A54.json']:
            path=pack/'control'/name;raw=path.read_bytes();path.write_text('{}')
            with self.subTest(name=name):
                if name=='profile-identity.A53.json':self.assertTrue(validate_pack.validate_profile_identity_A53(self.plan,pack))
                else:self.assertTrue(validate_pack.validate_identity_repair_A54(self.plan,pack))
            path.write_bytes(raw)
        path=pack/'control/state.initial.json';initial=load(path);initial['slices']['F02e2i']['attempts']=0;write_json(path,initial)
        self.assertTrue(validate_pack.validate_identity_repair_A54(self.plan,pack))

    def test_preserved_history_patch_and_budget_uncertainty_cannot_be_changed(self):
        c,state=self.fixture();self.assertEqual(self.check_state(c,state),[])
        for mutation in ['missing_history','changed_state','changed_budget','changed_patch','uncertainty']:
            with self.subTest(mutation=mutation):
                c,state=self.fixture();child=state['slices']['F02e2i']
                if mutation=='missing_history':child['attempt_history'].pop('1')
                if mutation=='changed_state':child['attempt_history']['3']['candidate_sha']='c'*40
                if mutation=='changed_budget':(self.root/c['history']['1']['files']['SOURCE_BUDGET.json']['path']).write_text('pretend exact zero')
                if mutation=='changed_patch':(self.root/c['metadata']['repair_patch']['path']).write_text('different correction')
                if mutation=='uncertainty':child['preserved_source_failures_A54']['1']['budget_uncertain']=False
                self.assertTrue(self.check_state(c,state))

    def test_single_assignment_requires_accepted_authority_and_no_reset(self):
        c,state=self.fixture();child=state['slices']['F02e2i'];child['status']='ready'
        self.assertEqual(self.check_state(c,state),[])
        child.pop('source_repair_A54_authority');self.assertTrue(self.check_state(c,state))
        c,state=self.fixture();self.assign(c,state);self.assertEqual(self.check_state(c,state),[])
        for attempt in [0,2,3,5,True]:
            wrong=copy.deepcopy(state);wrong['slices']['F02e2i']['attempts']=attempt
            with self.subTest(attempt=attempt):self.assertTrue(self.check_state(c,wrong))
        wrong=copy.deepcopy(state);wrong['slices']['F02e2i']['status']='ready';self.assertTrue(self.check_state(c,wrong))
        path=self.root/c['metadata']['source4_assignment'];assignment=load(path);assignment['repair_patch_sha256']='0'*64;write_json(path,assignment)
        state['slices']['F02e2i']['assignment_sha256']=digest(path)
        self.assertTrue(self.check_state(c,state))

    def test_failed_fixed_gate_or_review_cannot_be_retried_or_accepted(self):
        for kind in ['focused','source_review','integrated_source_review','malformed']:
            with self.subTest(kind=kind):
                # Each independent instance gets its own directory to preserve
                # source4's unique assignment and terminal evidence.
                temporary=tempfile.TemporaryDirectory(prefix='ferric-a54-terminal-');self.addCleanup(temporary.cleanup)
                self.root=Path(temporary.name);c,state=self.fixture();self.assign(c,state)
                if kind=='focused':
                    write_json(self.root/c['metadata']['unit_gate_receipts']['focused'],{'result':'failed','exit_code':101})
                    write_json(self.root/'evidence/alternate-retry/receipt.json',{'result':'passed','exit_code':0})
                elif kind=='malformed':
                    path=self.root/c['metadata']['unit_gate_receipts']['application_regression'];path.parent.mkdir(parents=True,exist_ok=True);path.write_text('{incomplete failed receipt')
                else:write_json(self.root/c['metadata'][kind],{'recommendation':'request_changes'})
                self.assertTrue(self.check_state(c,state))
                self.terminal(c,state);self.assertEqual(self.check_state(c,state),[])
                state['slices']['F02e2i']['status']='accepted';self.assertTrue(self.check_state(c,state))
                state['slices']['F02e2i'].update(status='failed',source_changes_authorized=True)
                self.assertTrue(self.check_state(c,state))

    def test_terminal_journal_rejects_deleted_files_history_and_replay(self):
        c,state=self.fixture();self.assign(c,state);meta=c['metadata'];child=state['slices']['F02e2i']
        receipt_path=self.root/meta['unit_gate_receipts']['focused'];write_json(receipt_path,{'result':'failed','exit_code':101})
        self.terminal(c,state);self.assertEqual(self.check_state(c,state),[])
        terminal_path=self.root/meta['terminal_marker'];terminal_bytes=terminal_path.read_bytes();receipt_bytes=receipt_path.read_bytes()
        terminal_path.unlink();receipt_path.unlink()
        child.update(status='in_progress',source_changes_authorized=True,compile_test_authorized=True)
        self.assertTrue(self.check_state(c,state))
        with self.assertRaises((OSError,ValueError)):
            validate_pack.append_identity_attempt_event_A54(self.root,child,c,'source_review_started')
        terminal_path.write_bytes(terminal_bytes);receipt_path.write_bytes(receipt_bytes);child.update(status='failed',source_changes_authorized=False,compile_test_authorized=False)
        self.assertEqual(self.check_state(c,state),[])
        history=child['attempt_history'].pop('4');self.assertTrue(self.check_state(c,state));child['attempt_history']['4']=history
        journal=self.root/meta['lifecycle'];raw=journal.read_bytes();journal.unlink();self.assertTrue(self.check_state(c,state))
        with self.assertRaises(ValueError):validate_pack.append_identity_attempt_event_A54(self.root,child,c,'assigned')
        saved_binding=child.pop('lifecycle_A54')
        with self.assertRaises(ValueError):validate_pack.append_identity_attempt_event_A54(self.root,child,c,'assigned')
        child['lifecycle_A54']=saved_binding
        journal.write_bytes(raw.splitlines(keepends=True)[0]);self.assertTrue(self.check_state(c,state))
        journal.write_bytes(raw);self.assertEqual(self.check_state(c,state),[])
        with self.assertRaises(ValueError):validate_pack.append_identity_attempt_event_A54(self.root,child,c,'source_review_started')

    def test_compile_requires_fixed_bound_reviews_and_ordered_lifecycle(self):
        c,state=self.fixture();repository,source,tree,_=self.product_fixture(c);self.assign(c,state)
        meta=c['metadata'];child=state['slices']['F02e2i']
        child.update(worktree=str(repository),source_sha=source,source_tree=tree,implementation_sha=source,implementation_tree=tree,compile_test_authorized=True)
        self.assertTrue(self.check_state(c,state));child['compile_test_authorized']=False
        alternate='evidence/F02e2i-a4/alternate/SOURCE_REVIEW.json';write_json(self.root/alternate,{'recommendation':'request_changes'})
        child['source_review']=alternate;self.assertTrue(self.check_state(c,state));child.pop('source_review')
        validate_pack.append_identity_attempt_event_A54(self.root,child,c,'source_review_started')
        for key,phase in [('source_review','source_review_passed'),('integrated_source_review','integrated_review_passed')]:
            if key=='integrated_source_review':validate_pack.append_identity_attempt_event_A54(self.root,child,c,'integration_started')
            path=self.root/meta[key];write_json(path,{'decision':'accept','reviewer':'independent-reviewer','source_sha':source,'source_tree':tree})
            child[key]=meta[key];child[key+'_sha256']=digest(path)
            validate_pack.append_identity_attempt_event_A54(self.root,child,c,phase,meta[key])
        child['compile_test_authorized']=True
        self.assertEqual(self.check_state(c,state),[])
        child['status']='accepted';self.assertTrue(self.check_state(c,state));child['status']='in_progress'
        fixed=self.root/meta['source_review'];saved=fixed.read_bytes();fixed.unlink();self.assertTrue(self.check_state(c,state));fixed.write_bytes(saved)
        child['integrated_source_review_sha256']='0'*64;self.assertTrue(self.check_state(c,state))

    def test_independent_audit_rejects_combined_journal_state_history_rollback(self):
        c,state=self.fixture();self.assign(c,state);meta=c['metadata'];child=state['slices']['F02e2i']
        journal=self.root/meta['lifecycle'];seed=journal.read_bytes();seed_state=copy.deepcopy(child)
        receipt=self.root/meta['unit_gate_receipts']['focused'];write_json(receipt,{'result':'failed','exit_code':101})
        self.terminal(c,state);self.assertEqual(self.check_state(c,state),[])
        (self.root/meta['terminal_marker']).unlink();receipt.unlink();journal.write_bytes(seed)
        child.clear();child.update(seed_state);child.update(status='in_progress',source_changes_authorized=True,compile_test_authorized=True)
        self.assertTrue(self.check_state(c,state))
        with self.assertRaises(ValueError):validate_pack.append_identity_attempt_event_A54(self.root,child,c,'source_review_started')
        # Even rolling back just the mutable audit head cannot erase the
        # independently retained per-event refs.
        audit=self.root/meta['audit_repository'];assignment=load(self.root/meta['source4_assignment'])
        prefix=meta['audit_ref_prefix']+assignment['run_nonce']
        seed_oid=subprocess.check_output(['git','--git-dir',str(audit),'rev-parse',prefix+'/events/000'],text=True).strip()
        subprocess.run(['git','-c','core.hooksPath=/dev/null','--git-dir',str(audit),'update-ref',prefix+'/head',seed_oid],check=True)
        self.assertTrue(self.check_state(c,state))
        with self.assertRaises(ValueError):validate_pack.append_identity_attempt_event_A54(self.root,child,c,'source_review_started')
        (audit/'.ferric-a54-owner.json').unlink();self.assertTrue(self.check_state(c,state))

    def test_append_refuses_bad_phase_failed_proof_and_alternate_path_before_writing(self):
        c,state=self.fixture();self.assign(c,state);child=state['slices']['F02e2i'];meta=c['metadata'];journal=self.root/meta['lifecycle']
        before=journal.read_bytes()
        with self.assertRaises(ValueError):validate_pack.append_identity_attempt_event_A54(self.root,child,c,'focused_started')
        self.assertEqual(journal.read_bytes(),before)
        validate_pack.append_identity_attempt_event_A54(self.root,child,c,'source_review_started');before=journal.read_bytes()
        write_json(self.root/meta['source_review'],{'decision':'accept','recommendation':'request_changes'})
        with self.assertRaises(ValueError):validate_pack.append_identity_attempt_event_A54(self.root,child,c,'source_review_passed',meta['source_review'])
        with self.assertRaises(ValueError):validate_pack.append_identity_attempt_event_A54(self.root,child,c,'source_review_passed','evidence/alternate-review.json')
        self.assertEqual(journal.read_bytes(),before)

    def test_actual_commit_content_and_fixed_gate_target_are_bound(self):
        c=copy.deepcopy(self.contract);repository,source,tree,paths=self.product_fixture(c)
        evidence={'source_repository':str(repository),'source_sha':source,'source_tree':tree,'implementation_sha':source,'implementation_tree':tree,
                  'identity_evidence_review':c['metadata']['gate_evidence_review'],'review_path':c['metadata']['gate_evidence_review'],'unit_gates':{}}
        for key,path_key in [('worker_source_review','source_review'),('integrated_source_review','integrated_source_review')]:
            path=self.root/c['metadata'][path_key];write_json(path,{'decision':'accept','reviewer':'independent-source-reviewer','source_sha':source,'source_tree':tree})
            evidence[key]={'path':c['metadata'][path_key],'sha256':digest(path)}
        for name,path in c['metadata']['unit_gate_receipts'].items():
            write_json(self.root/path,{'execution_binding':{'cargo_target_dir':str((self.root/c['metadata']['cargo_target_relative']).resolve())}})
            evidence['unit_gates'][name]={'receipt':path}
        evidence_path=self.root/'evidence/EVIDENCE.json';write_json(evidence_path,evidence)
        record={'status':'accepted','attempts':4,'author':'fixture-worker','evidence':'evidence/EVIDENCE.json'};task={'id':'F02e2i'}
        def check():
            with patch.object(validate_pack,'identity_repair_contract_A54',return_value=c):
                return validate_pack.validate_identity_repair_acceptance_A54(self.plan,task,record,self.root)
        self.assertEqual(check(),[])
        original=copy.deepcopy(evidence['worker_source_review']);evidence['worker_source_review']['path']='evidence/alternate-source-review.json';write_json(evidence_path,evidence);self.assertTrue(check())
        evidence['worker_source_review']=original
        original=copy.deepcopy(evidence['integrated_source_review']);evidence['integrated_source_review']['sha256']='0'*64;write_json(evidence_path,evidence);self.assertTrue(check())
        evidence['integrated_source_review']=original;write_json(evidence_path,evidence)
        evidence['unit_gates']['focused']['receipt']='evidence/alternate/receipt.json';write_json(evidence_path,evidence);self.assertTrue(check())
        evidence['unit_gates']['focused']['receipt']=c['metadata']['unit_gate_receipts']['focused'];write_json(evidence_path,evidence)
        receipt_path=self.root/c['metadata']['unit_gate_receipts']['focused'];receipt=load(receipt_path);receipt['execution_binding']['cargo_target_dir']=str(self.root/'builds/other');write_json(receipt_path,receipt);self.assertTrue(check())
        for path in paths:
            with self.subTest(path=path):
                (repository/path).write_text('unapproved production or assertion edit\n');git(repository,'add','.');git(repository,'commit','-m','unapproved fixture source')
                with self.assertRaises(ValueError):validate_pack.validate_identity_product_A54(repository,git(repository,'rev-parse','HEAD'),c)
                (repository/path).write_text(paths[path]);git(repository,'add','.');git(repository,'commit','-m','restore fixture content')


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
        watcher=workspace_root()/'evidence/A46/watch_qt_includes.py'
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

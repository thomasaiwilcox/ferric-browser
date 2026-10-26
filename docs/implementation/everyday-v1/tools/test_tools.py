#!/usr/bin/env python3
"""Local tests use disposable fake repositories, never live browser data or network."""
from __future__ import annotations
import copy,json,os,subprocess,sys,tempfile,unittest
from pathlib import Path
from unittest.mock import patch
sys.path.insert(0,str(Path(__file__).resolve().parent))
from common import PACK,CLONE,git,load,write_json,digest,packet,validate_repo,safe_relative,repo_snapshot
import bootstrap,run_gate,cleanup,ready,validate_pack,preflight

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
    def test_structure(self):self.assertEqual(validate_pack.validate_structure(),[])
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

#!/usr/bin/env python3
"""Offline, read-only structural/integrity/evidence-record validation.

This checks a specification pack and record completeness, not browser correctness
or the truth of user-entered test results. Python 3.10+; standard library only.
"""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import re
import sys
from typing import Any

STATUSES = {'not_started','in_progress','blocked_environment','blocked_specification',
            'implemented_unqualified','complete','research_no_go','not_applicable'}
LEVELS = {'static-inspection','pure-unit','qml-component','webengine-fixture',
          'native-qt-offscreen','native-wayland','live-site','performance'}
KINDS = {'foundation','implementation','gated_implementation','research','release'}
RESULTS = {'passed','failed','not_run','unavailable'}
BASELINE = '4337056f632de0c710943fa6df76c67cf14b8b47'
REQUIRED_SOURCE_ITEMS = ({f'A-{i:02}' for i in range(1,21)} |
                         {f'F-{i:02}' for i in range(1,9)} |
                         {f'WEBAPP-{i:03}' for i in range(1,23)} |
                         {'B-01','B-10','B-11','B-12','B-13'})

def checked_path(root: Path, relative: str) -> Path:
    if not isinstance(relative, str) or not relative or Path(relative).is_absolute():
        raise ValueError(f'Expected a nonempty relative path: {relative!r}')
    path = (root / relative).resolve()
    if not path.is_relative_to(root.resolve()):
        raise ValueError(f'Path escapes pack: {relative}')
    return path

def read_json(root: Path, name: str) -> Any:
    return json.loads(checked_path(root, name).read_text(encoding='utf-8'))

def validate(root: Path, integrity: bool = False, evidence: bool = False,
             strict_completion: bool = False) -> dict[str, Any]:
    errors: list[str] = []
    warnings: list[str] = []
    def check(ok: bool, message: str) -> None:
        if not ok:
            errors.append(message)
    try:
        proposal = checked_path(root,'PROPOSAL.md').read_text(encoding='utf-8')
        plan = read_json(root,'plan.json')
        matrix = read_json(root,'TEST_MATRIX.json')
        trace = read_json(root,'TRACEABILITY.json')
        progress = read_json(root,'progress.json')
    except (OSError, ValueError, TypeError) as exc:
        return {'ok':False,'errors':[str(exc)],'warnings':[],
                'scope':'pack records only; not Ferric correctness'}
    for label, data in [('plan',plan),('matrix',matrix),('trace',trace),('progress',progress)]:
        check(isinstance(data,dict) and data.get('schema_version') == 1,
              f'{label}: unsupported schema')
    if errors:
        return {'ok':False,'errors':errors,'warnings':warnings}
    check(plan.get('baseline') == BASELINE, 'Unexpected pinned baseline')
    check(plan.get('publishing_authorized') is False, 'Publishing must not be authorised')
    check(plan.get('default_execution') == 'serial_dependency_order', 'Default scheduling must be serial')
    contracts = set(re.findall(r'^## (C\d{2}) —',proposal,re.MULTILINE))
    check(contracts == {f'C{i:02}' for i in range(1,25)}, 'Canonical contracts must be C01–C24 exactly')
    invariants = set(re.findall(r'^\| (UI-\d{3}) \|',proposal,re.MULTILINE))
    check(invariants == {f'UI-{i:03}' for i in range(1,27)}, 'Unified invariants must be UI-001–026 exactly')
    slices = plan.get('slices',[])
    tests = matrix.get('tests',[])
    mappings = trace.get('mappings',[])
    check(isinstance(slices,list) and len(slices)>0, 'Missing slice array')
    check(isinstance(tests,list) and len(tests)>0, 'Missing test array')
    if errors:
        return {'ok':False,'errors':errors,'warnings':warnings}
    ids = [s.get('id') for s in slices]
    tids = [t.get('id') for t in tests]
    check(len(ids)==len(set(ids)), 'Duplicate slice ID')
    check(len(tids)==len(set(tids)), 'Duplicate test ID')
    smap = {s['id']:s for s in slices if 'id' in s}
    tmap = {t['id']:t for t in tests if 'id' in t}
    used_tests: set[str] = set()
    for test in tests:
        tid=test.get('id','?')
        check(bool(re.fullmatch(r'T\d{3}',str(tid))),f'Invalid test ID {tid}')
        levels=test.get('required_levels',[])
        check(bool(levels) and set(levels)<=LEVELS,f'{tid}: invalid evidence classes')
        check(bool(test.get('assertion')),f'{tid}: missing assertion')
        check(test.get('status')=='not_run',f'{tid}: matrix must remain a plan, not a claimed test result')
    for s in slices:
        sid=s.get('id','?')
        check(bool(re.fullmatch(r'S\d{2}',str(sid))),f'Invalid slice ID {sid}')
        check(s.get('kind') in KINDS,f'{sid}: invalid kind')
        check(bool(s.get('locks')) and len(s['locks'])==len(set(s['locks'])),f'{sid}: missing/duplicate locks')
        check(bool(s.get('contracts')) and set(s['contracts'])<=contracts,f'{sid}: unknown/missing contract')
        check(bool(s.get('test_ids')) and set(s['test_ids'])<=set(tids),f'{sid}: unknown/missing test')
        used_tests.update(s.get('test_ids',[]))
        check(bool(s.get('source_items')) and set(s['source_items'])<=REQUIRED_SOURCE_ITEMS,
              f'{sid}: unknown/missing source item')
        for dep in s.get('depends_on',[]):
            check(dep in smap, f'{sid}: unknown dependency {dep}')
            check(dep != sid, f'{sid}: self dependency')
        cond=s.get('condition')
        if cond:
            check(cond.get('slice') in s.get('depends_on',[]),f'{sid}: conditional gate must be a direct dependency')
            check(cond.get('decision')=='go',f'{sid}: unsupported conditional gate')
        try:
            card=checked_path(root,s.get('card','')).read_text(encoding='utf-8')
            check(card.startswith('# '+str(sid)+' —'),f'{sid}: card identity mismatch')
            for heading in ['## Intent and write boundary','## Implementation order','## Do not do',
                            '## Tests and evidence','## Exit and handoff']:
                check(heading in card,f'{sid}: card missing {heading}')
            for cid in s.get('contracts',[]):
                check(cid in card,f'{sid}: card omits {cid}')
            for tid in s.get('test_ids',[]):
                check(f'**{tid} —' in card,f'{sid}: card omits test {tid}')
        except (OSError,ValueError,TypeError) as exc:
            errors.append(f'{sid}: {exc}')
    check(used_tests==set(tids),'Test matrix has orphan or unknown tests')
    # Detect dependency cycles, including non-numeric/topologically reordered plans.
    visiting: set[str]=set(); visited: set[str]=set()
    def visit(sid: str) -> None:
        if sid in visiting:
            errors.append(f'Dependency cycle at {sid}'); return
        if sid in visited or sid not in smap:
            return
        visiting.add(sid)
        for dep in smap[sid].get('depends_on',[]):
            visit(dep)
        visiting.remove(sid); visited.add(sid)
    for sid in smap:
        visit(sid)
    source_ids=[m.get('source_item') for m in mappings]
    check(len(source_ids)==len(set(source_ids)), 'Duplicate traceability item')
    check(set(source_ids)==REQUIRED_SOURCE_ITEMS,'Traceability coverage must include all 55 source items')
    for m in mappings:
        item=m.get('source_item','?')
        check(bool(m.get('contracts')) and set(m['contracts'])<=contracts,f'{item}: bad contract mapping')
        check(bool(m.get('slices')) and set(m['slices'])<=set(ids),f'{item}: bad slice mapping')
    records=progress.get('slices',[])
    pids=[r.get('id') for r in records]
    check(set(pids)==set(ids) and len(pids)==len(set(pids)), 'Progress must contain each slice once')
    pmap={r['id']:r for r in records if 'id' in r}
    for r in records:
        check(r.get('status') in STATUSES,f'{r.get("id")}: unknown progress status')
    # Evidence validation is opt-in: an untouched specification is not completed implementation.
    if evidence or strict_completion:
        for sid,s in smap.items():
            r=pmap.get(sid,{})
            status=r.get('status')
            if strict_completion:
                check(status in {'complete','research_no_go','not_applicable'},f'{sid}: implementation not complete ({status})')
            ref=r.get('evidence')
            if not ref:
                check(status not in {'complete','research_no_go','not_applicable'},f'{sid}: terminal status needs evidence')
                continue
            try:
                e=read_json(root,ref)
                check(e.get('slice_id')==sid,f'{sid}: evidence slice mismatch')
                check(e.get('status')==status,f'{sid}: evidence/progress status mismatch')
                check(bool(re.fullmatch(r'[0-9a-f]{40}',str(e.get('commit','')))),f'{sid}: actual commit required')
                check(bool(e.get('environment')),f'{sid}: actual environment required')
                results=e.get('test_results',[])
                passed=set()
                for result in results:
                    tid=result.get('test_id'); level=result.get('level')
                    check(tid in tmap,f'{sid}: unknown evidence test {tid}')
                    check(level in LEVELS,f'{sid}: invalid evidence level {level}')
                    check(result.get('result') in RESULTS,f'{sid}: invalid result')
                    check(bool(result.get('command')),f'{sid}/{tid}: actual command/source operation required')
                    if result.get('result')=='passed':
                        passed.add((tid,level))
                        log=result.get('log')
                        check(bool(log) and checked_path(root,log).is_file(),f'{sid}/{tid}: passing evidence log missing')
                    elif result.get('result') in {'not_run','unavailable'}:
                        check(bool(result.get('reason')),f'{sid}/{tid}: missing not-run reason')
                decision=e.get('capability_decision')
                if status=='research_no_go':
                    check(s['kind'] in {'research','gated_implementation'},f'{sid}: core implementation cannot be completed by no-go')
                    check(decision=='no_go',f'{sid}: no-go decision required')
                    check(any(x.get('result') in {'passed','failed'} and x.get('log') and checked_path(root,x['log']).is_file() for x in results),f'{sid}: no-go needs concrete evidence, not only missing infrastructure')
                if status=='not_applicable':
                    cond=s.get('condition')
                    check(bool(cond),f'{sid}: not_applicable only allowed with a specified conditional predecessor')
                    if cond:
                        pred=pmap.get(cond['slice'],{})
                        check(pred.get('status')=='research_no_go',f'{sid}: predecessor must have an evidenced no-go')
                if s['kind']=='gated_implementation' and s.get('condition'):
                    pred=pmap.get(s['condition']['slice'],{})
                    if pred.get('status') in {'blocked_environment','implemented_unqualified'} and pred.get('capability_decision') is None:
                        check(status=='blocked_environment',f'{sid}: undecided environment-blocked predecessor requires blocked_environment')
                if s['kind']=='research' and status=='complete':
                    check(decision in {'go','no_go','no_need'},f'{sid}: research decision required')
                if decision is not None and status in {'complete','research_no_go'}:
                    check(r.get('capability_decision')==decision,f'{sid}: capability decision differs between progress and evidence')
                if status in {'research_no_go','not_applicable'} or (s['kind'] in {'research','gated_implementation'} and status=='complete'):
                    report=e.get('capability_report')
                    check(bool(report) and checked_path(root,report).is_file(),f'{sid}: capability report missing')
                if status=='complete' and (s['kind']!='research' or decision=='go'):
                    for tid in s['test_ids']:
                        for level in tmap[tid]['required_levels']:
                            check((tid,level) in passed,f'{sid}: missing passed evidence {tid}/{level}')
                    if s['kind']=='gated_implementation':
                        check(decision=='go',f'{sid}: gated production completion requires go')
                if status=='complete' and s['kind']=='research' and decision in {'no_go','no_need'}:
                    check(any(x.get('result') in {'passed','failed'} and x.get('log') and checked_path(root,x['log']).is_file() for x in results),f'{sid}: research conclusion requires traceable evidence')
            except (OSError,ValueError,TypeError,KeyError) as exc:
                errors.append(f'{sid}: invalid evidence record: {exc}')
    if integrity:
        try:
            lines=checked_path(root,'SHA256SUMS.txt').read_text(encoding='utf-8').splitlines()
            names=[]
            for line in lines:
                expected,name=line.split('  ',1)
                check(bool(re.fullmatch(r'[0-9a-f]{64}',expected)),f'Invalid hash for {name}')
                f=checked_path(root,name)
                names.append(name)
                check(f.is_file(),f'Integrity file missing: {name}')
                if f.is_file():
                    check(hashlib.sha256(f.read_bytes()).hexdigest()==expected,f'Integrity mismatch: {name}')
            check(len(names)==len(set(names)),'Duplicate integrity entry')
            required={'PROPOSAL.md','SOURCE_MAP.md','TRACEABILITY.json','plan.json','TEST_MATRIX.json',
                      'GOAL_PROMPT.md','README.md','SLICE_INDEX.md','tools/validate_pack.py','tools/test_validate_pack.py'}
            required.update(s['card'] for s in slices)
            check(required<=set(names),'Normative files missing from integrity list')
        except (OSError,ValueError,TypeError) as exc:
            errors.append(f'Integrity: {exc}')
    if not strict_completion:
        warnings.append('Structural success is not implementation/test completion; Ferric correctness is not checked.')
    return {'ok':not errors,'errors':errors,'warnings':warnings,
            'counts':{'contracts':len(contracts),'invariants':len(invariants),'slices':len(slices),
                      'planned_tests':len(tests),'source_items':len(mappings)},
            'checks':{'integrity':integrity,'evidence_records':evidence or strict_completion,
                      'strict_completion':strict_completion},
            'scope':'pack records only; not Ferric correctness'}

def main() -> int:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root',type=Path,default=Path(__file__).resolve().parents[1])
    parser.add_argument('--integrity',action='store_true')
    parser.add_argument('--evidence',action='store_true')
    parser.add_argument('--strict-completion',action='store_true')
    parser.add_argument('--json',action='store_true')
    args=parser.parse_args()
    try:
        result=validate(args.root,args.integrity,args.evidence,args.strict_completion)
    except (OSError,ValueError,TypeError,KeyError) as exc:
        result={'ok':False,'errors':[f'Malformed pack: {exc}'],'warnings':[]}
    if args.json:
        print(json.dumps(result,indent=2))
    else:
        print('PASS: pack checks' if result['ok'] else 'FAIL: pack checks')
        for err in result['errors']: print('ERROR:',err)
        for warning in result['warnings']: print('NOTE:',warning)
        if 'counts' in result: print(json.dumps(result['counts'],sort_keys=True))
    return 0 if result['ok'] else 1

if __name__=='__main__':
    sys.exit(main())

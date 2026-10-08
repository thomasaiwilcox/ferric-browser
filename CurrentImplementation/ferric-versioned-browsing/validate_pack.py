#!/usr/bin/env python3
"""Read-only validation/task extraction for this proposal pack. Does not run Ferric."""
from __future__ import annotations
import argparse
import collections
import json
import re
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent

def read_json(name: str) -> dict[str, Any]:
    with (ROOT / name).open(encoding='utf-8') as handle:
        data = json.load(handle)
    if not isinstance(data, dict):
        raise ValueError(f'{name}: expected a JSON object')
    return data

def validate() -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    ledger = read_json('IMPLEMENTATION_BACKLOG.json')
    gates = read_json('CAPABILITY_GATES.json')
    audit = read_json('BASELINE_AUDIT.json')
    if ledger['baseline_main'] != audit['main_sha'] or gates['baseline_main'] != audit['main_sha']:
        raise ValueError('Baseline SHA mismatch across pack files')
    tasks = ledger.get('tasks', [])
    by_id = {t['id']: t for t in tasks}
    if len(by_id) != len(tasks):
        raise ValueError('Duplicate task ID')
    phases = {p['id'] for p in ledger['phases']}
    states = set(ledger['status_vocabulary'])
    capabilities = gates['capabilities']
    for task in tasks:
        tid = task['id']
        if task['phase'] not in phases or task['status'] not in states:
            raise ValueError(f'{tid}: unknown phase/status')
        for dep in task['depends_on']:
            if dep not in by_id or dep == tid:
                raise ValueError(f'{tid}: invalid dependency {dep}')
        for cap in task['required_capabilities']:
            if cap not in capabilities:
                raise ValueError(f'{tid}: unknown capability {cap}')
        for key in ('implementation_steps', 'test_requirements', 'acceptance_criteria'):
            if not task.get(key) or not all(isinstance(x, str) and x.strip() for x in task[key]):
                raise ValueError(f'{tid}: empty or malformed {key}')
        if task['status'] == 'blocked' and not task.get('blocker'):
            raise ValueError(f'{tid}: blocked task needs a concrete blocker')
        if task['status'] == 'verified' and not task.get('tests_run'):
            raise ValueError(f'{tid}: verified task needs actual test/evidence results')
    visiting: set[str] = set()
    visited: set[str] = set()
    def visit(tid: str) -> None:
        if tid in visiting:
            raise ValueError(f'Dependency cycle at {tid}')
        if tid in visited:
            return
        visiting.add(tid)
        for dep in by_id[tid]['depends_on']:
            visit(dep)
        visiting.remove(tid)
        visited.add(tid)
    for tid in by_id:
        visit(tid)
    ancestor_cache: dict[str, set[str]] = {}
    def ancestors(tid: str) -> set[str]:
        if tid not in ancestor_cache:
            result: set[str] = set()
            for dep in by_id[tid]['depends_on']:
                result.add(dep)
                result.update(ancestors(dep))
            ancestor_cache[tid] = result
        return ancestor_cache[tid]
    for task in tasks:
        for path, origin in task.get('planned_path_origins', {}).items():
            introduced_by = origin['introduced_by']
            if introduced_by != task['id'] and introduced_by not in ancestors(task['id']):
                raise ValueError(f"{task['id']}: planned module {path} has no dependency on {introduced_by}")
    proposal = (ROOT / 'PROPOSAL.md').read_text(encoding='utf-8')
    rendered = (ROOT / 'IMPLEMENTATION_SLICES.md').read_text(encoding='utf-8')
    headings = re.findall(r'^#### (VB-[0-9]+(?:[a-z])?) —', proposal, re.M)
    if collections.Counter(headings) != collections.Counter(by_id.keys()):
        raise ValueError('Proposal task appendix does not match ledger task IDs')
    other = re.findall(r'^#### (VB-[0-9]+(?:[a-z])?) —', rendered, re.M)
    if collections.Counter(other) != collections.Counter(by_id.keys()):
        raise ValueError('Standalone task appendix does not match ledger')
    for text, label in ((proposal, 'proposal'), (rendered, 'slice appendix')):
        for task in tasks:
            pattern = rf"#### {task['id']} —[^\n]*\n+\*\*Dependencies:\*\* ([^\n]*)"
            match = re.search(pattern, text)
            actual = set(re.findall(r'VB-[0-9]+', match.group(1))) if match else None
            if actual != set(task['depends_on']):
                raise ValueError(f"{label}: dependency text mismatch for {task['id']}")
    for name in ('PROPOSAL.md','AGENT_HANDOFF.md','IMPLEMENTATION_SLICES.md','README.md'):
        text = (ROOT / name).read_text(encoding='utf-8')
        fences = re.findall(r'^```', text, re.M)
        if len(fences) % 2:
            raise ValueError(f'{name}: unbalanced fenced code block')
    for cap, result in capabilities.items():
        if result['primitive_gate'] not in ('supported', 'unsupported', 'unqualified'):
            raise ValueError(f'{cap}: invalid gate status')
        if result['production_enabled']:
            release_id = result.get('release_qualification_id')
            release_task = by_id.get(release_id)
            if (result['primitive_gate'] != 'supported'
                    or not result['evidence_ids']
                    or release_task is None
                    or release_task['phase'] != 'P8'
                    or release_task['status'] != 'verified'
                    or not release_task['tests_run']
                    or not release_task['evidence_path']):
                raise ValueError(f'{cap}: production enabled without a verified P8 release qualification')
    summary = {
        'pack_validation': 'passed',
        'scope': 'Document/ledger integrity only; not a Ferric build or native test.',
        'baseline_main': audit['main_sha'],
        'tasks': len(tasks),
        'phases': len(phases),
        'dependency_edges': sum(len(t['depends_on']) for t in tasks),
        'dag_valid': True,
        'planned_module_prerequisites_valid': True,
        'task_appendices_match_ledger': True,
        'markdown_fences_balanced': True,
        'proposal_word_count': len(proposal.split()),
        'statuses': dict(collections.Counter(t['status'] for t in tasks)),
        'enabled_production_capabilities': [k for k,v in capabilities.items() if v['production_enabled']],
    }
    return ledger, gates, summary

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group()
    group.add_argument('--next', action='store_true', help='Print the next dependency/capability-ready task')
    group.add_argument('--task', help='Print one task by ID')
    group.add_argument('--summary', action='store_true', help='Print phase and status summary')
    args = parser.parse_args()
    try:
        ledger, gates, report = validate()
        tasks = ledger['tasks']
        by_id = {t['id']:t for t in tasks}
        if args.task:
            task = by_id.get(args.task)
            if task is None:
                raise ValueError(f'Unknown task: {args.task}')
            print(json.dumps(task, indent=2, ensure_ascii=False))
        elif args.next:
            ready = [t for t in tasks if t['status'] == 'not-started'
                     and all(by_id[d]['status'] == 'verified' for d in t['depends_on'])
                     and all(gates['capabilities'][c]['primitive_gate'] == 'supported'
                             for c in t['required_capabilities'])]
            if ready:
                print(json.dumps(ready[0], indent=2, ensure_ascii=False))
            else:
                print(json.dumps({'next_task':None,
                    'reason':'No dependency/capability-ready task. Review in-progress work and concrete blockers; do not redefine blocked extensions as complete.'},indent=2))
        elif args.summary:
            report['phase_counts'] = dict(collections.Counter(t['phase'] for t in tasks))
            print(json.dumps(report, indent=2))
        else:
            print(json.dumps(report, indent=2))
        return 0
    except (OSError, ValueError, KeyError, TypeError, json.JSONDecodeError) as exc:
        print(f'Validation failed: {exc}', file=sys.stderr)
        return 1

if __name__ == '__main__':
    raise SystemExit(main())

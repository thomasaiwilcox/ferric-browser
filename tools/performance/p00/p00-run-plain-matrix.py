#!/usr/bin/env python3
"""Run selected P00 workloads serially against one exact plain build."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import runpy
import signal
import subprocess

SOURCE_DIR = Path(__file__).resolve().parent
BASE = Path(os.environ.get('FERRIC_P00_ARTIFACT_DIR', str(SOURCE_DIR.parents[2] / 'target/site-policy-baseline'))).resolve()
KINDS = ('command', 'hints', 'refresh', 'refresh-visible', 'ready', 'activation', 'roots', 'root-stages', 'lifetime')


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def workload(value):
    kind, separator, dpr = value.partition(':')
    if not separator or kind not in KINDS or dpr not in ('1', '2'):
        raise argparse.ArgumentTypeError('Use KIND:1 or KIND:2 for each workload')
    return kind, dpr


def save(path, value):
    temporary = path.with_suffix('.tmp')
    temporary.write_text(json.dumps(value, indent=2) + '\n')
    temporary.replace(path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--build', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    parser.add_argument('--smoke', action='store_true')
    parser.add_argument('workloads', type=workload, nargs='+')
    args = parser.parse_args()
    if len(args.workloads) != len(set(args.workloads)):
        parser.error('Workloads must be unique within a run')
    build_path = args.build.resolve()
    build = json.loads(build_path.read_text())
    if build['mode'] != 'plain' or build['status'] != 'passed' or not build['source_postcheck_exact_match']:
        raise RuntimeError('The selected plain build has no passing source receipt')
    helpers = runpy.run_path(str(SOURCE_DIR / 'p00-build-measurement.py'))
    root = helpers['ROOT']
    if Path(build['source_root']) != root or build['revision'] != helpers['PIN']:
        raise RuntimeError('The selected build did not use the pinned source directory')
    manifest_path = build_path.with_name('source-inputs.json')
    overlay = build['overlays'][0]['patch'].removeprefix('p00-measurement-overlay-')
    expected, overlays = helpers['expected_sources']('plain', overlay)
    if overlay == 'v7' and any(kind == 'refresh-visible' for kind, _ in args.workloads):
        parser.error('The 500-visible refresh row requires the v8 overlay')
    if (overlays != build['overlays'] or digest(manifest_path) != build['source_manifest_sha256']
            or json.loads(manifest_path.read_text()) != expected or helpers['inventory']() != expected):
        raise RuntimeError('Current source, approved overlay and build receipt disagree')
    binary = Path(build['binary'])
    producer = build['cargo_producer']
    archive = Path(producer['archive'])
    if digest(binary) != build['binary_sha256'] or digest(archive) != producer['archive_sha256']:
        raise RuntimeError('Current artifacts differ from the selected build')
    env = dict(os.environ, FERRIC_QTWAYLAND_PREFIX=os.environ.get('FERRIC_QTWAYLAND_PREFIX', str(BASE / 'qtwayland-sdk')))
    env.pop('FERRIC_P00_BROKER_TRACE', None)
    output = args.output_dir.resolve()
    output.mkdir(parents=True)
    receipt = output / 'matrix.json'
    summary = {'status': 'running', 'build_record': str(build_path), 'build_record_sha256': digest(build_path),
               'started': datetime.now(timezone.utc).isoformat(), 'pid': os.getpid(),
               'workloads': [f'{kind}:{dpr}' for kind, dpr in args.workloads], 'runs': []}
    runner = root / 'crates/ferric-browser-engine-qt/tests/application/run_site_policy_performance.py'

    def interrupted(_signum, _frame):
        raise KeyboardInterrupt('Measurement matrix interrupted')

    previous_term = signal.signal(signal.SIGTERM, interrupted)
    child = None
    try:
        for kind, dpr in args.workloads:
            folder = output / f'{kind}-dpr{dpr}'
            command = [str(helpers['WRAPPER']), 'python3', str(runner), '--binary', str(binary),
                       '--archive', str(archive), '--output-dir', str(folder), '--kind', kind, '--dpr', dpr]
            if args.smoke:
                command.append('--smoke')
            row = {'kind': kind, 'dpr': int(dpr), 'command': command, 'status': 'running'}
            summary['runs'].append(row)
            save(receipt, summary)
            child = subprocess.Popen(command, cwd=root, env=env, start_new_session=True)
            row['pid'] = child.pid
            save(receipt, summary)
            exit_code = child.wait()
            child = None
            record_path = folder / 'run-metadata.json'
            record = json.loads(record_path.read_text())
            binding_matches = (
                record['source_manifest_sha256'] == build['source_manifest_sha256']
                and record['head'] == build['revision'] and record['exit_code'] == exit_code
                and record['binary']['sha256'] == build['binary_sha256']
                and record['archive']['sha256'] == producer['archive_sha256']
                and record.get('source_postcheck_exact_match') is True
                and record.get('instrumented_production') is False and record.get('broker_tracing') is False
                and record.get('archive_preflight_exit_code') == 0)
            row.update(status='passed' if exit_code == 0 else 'failed', exit_code=exit_code,
                       source_artifact_binding_matches=binding_matches, runtime_record_sha256=digest(record_path))
            if not binding_matches or record.get('interrupted') or record.get('harness_error'):
                raise RuntimeError(f'{kind} DPR {dpr} lost the exact build binding or had a harness error')
        artifacts_match = digest(binary) == build['binary_sha256'] and digest(archive) == producer['archive_sha256']
        summary['artifact_postcheck_exact_match'] = artifacts_match
        if not artifacts_match:
            raise RuntimeError('The plain build artifacts changed during the matrix')
        summary['status'] = 'passed' if all(row['status'] == 'passed' for row in summary['runs']) else 'failed-workloads'
    except BaseException as error:
        status = 'interrupted' if isinstance(error, KeyboardInterrupt) else 'failed'
        if child is not None:
            child.terminate()
            child.wait()
        if summary['runs'] and summary['runs'][-1]['status'] == 'running':
            summary['runs'][-1]['status'] = status
        summary.update(status=status,
                       error=f'{type(error).__name__}: {error}')
    finally:
        signal.signal(signal.SIGTERM, previous_term)
        summary['finished'] = datetime.now(timezone.utc).isoformat()
        save(receipt, summary)
    print(receipt)
    return 0 if summary['status'] == 'passed' else 1


if __name__ == '__main__':
    raise SystemExit(main())

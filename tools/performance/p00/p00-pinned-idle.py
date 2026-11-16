#!/usr/bin/env python3
"""Measure the pinned browser on a blank page with no Hint session."""
import argparse
from datetime import datetime, timezone
import errno
import json
import math
import os
from pathlib import Path
import runpy
import shutil
import signal
import subprocess
import sys
import tempfile
import time

SOURCE_DIR = Path(__file__).resolve().parent
BASE = Path(os.environ.get('FERRIC_P00_ARTIFACT_DIR', str(SOURCE_DIR.parents[2] / 'target/site-policy-baseline'))).resolve()
HELPER_BUILD = BASE / 'p00-pinned-idle-helper'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('observation', choices=('before-lifetime', 'after-lifetime'))
    parser.add_argument('--attempt', type=int, default=1)
    parser.add_argument('--before-attempt', type=int, default=1)
    parser.add_argument('--build', type=Path, default=BASE / 'p00-v11-build-plain-1/build-record.json')
    parser.add_argument('--tools-record', type=Path, default=BASE / 'p00-pinned-build-tools.json',
                        help='Passing original-source command receipt containing the release xtask hash')
    args = parser.parse_args()
    helpers = runpy.run_path(str(SOURCE_DIR / 'p00-build-measurement.py'))
    root, wrapper, digest = (helpers[name] for name in ('ROOT', 'WRAPPER', 'digest'))
    build_path = args.build.resolve()
    build = json.loads(build_path.read_text())
    overlay = build['overlays'][0]['patch'].removeprefix('p00-measurement-overlay-')
    expected, overlays = helpers['expected_sources']('plain', overlay)
    if (build['revision'] != helpers['PIN'] or build['mode'] != 'plain' or build['status'] != 'passed'
            or not build['source_postcheck_exact_match'] or overlays != build['overlays']
            or helpers['inventory']() != expected
            or digest(build_path.with_name('source-inputs.json')) != build['source_manifest_sha256']
            or json.loads(build_path.with_name('source-inputs.json').read_text()) != expected):
        raise RuntimeError('Current source and plain build receipt disagree')
    browser = Path(build['cargo_producer']['browser'])
    xtask = browser.with_name('xtask')
    tools_record = args.tools_record.resolve()
    pinned_reference = json.loads(tools_record.read_text())
    if (pinned_reference.get('status') != 'passed'
            or type(pinned_reference.get('exit_code')) is not int or pinned_reference['exit_code'] != 0
            or pinned_reference.get('revision') != helpers['PIN']
            or pinned_reference.get('cwd') != str(root)
            or pinned_reference.get('source_postcheck_exact_match') is not True
            or pinned_reference.get('source_manifest_sha256') != digest(SOURCE_DIR / 'p00-pinned-original-source.sha256')):
        raise RuntimeError('The xtask receipt must establish a passing original-source build')
    artifacts = {browser: build['cargo_producer']['browser_sha256'],
                 xtask: pinned_reference['artifacts_after'][str(xtask)]}
    if any(digest(path) != sha for path, sha in artifacts.items()):
        raise RuntimeError('Browser or xtask does not match its pinned build record')
    suffix = '' if args.attempt == 1 else f'-attempt-{args.attempt}'
    output = BASE / f'p00-pinned-idle-{args.observation}{suffix}'
    output.mkdir()
    env = dict(os.environ, FERRIC_QTWAYLAND_PREFIX=os.environ.get('FERRIC_QTWAYLAND_PREFIX', str(BASE / 'qtwayland-sdk')),
               QT_QPA_PLATFORM='offscreen', QT_QUICK_BACKEND='software', QT_QPA_PLATFORMTHEME='',
               QTWEBENGINE_CHROMIUM_FLAGS='--disable-gpu', QT_SCALE_FACTOR='1')
    for key in ('FERRIC_P00_BROKER_TRACE', 'FERRIC_PERF_NATIVE_PROBE'):
        env.pop(key, None)
    plugin = HELPER_BUILD / 'plugins/generic/libferric_perf_quit_plugin.so'
    report_path = output / 'ui-perf.json'
    receipt_path = output / 'run-record.json'
    pid_path = output / 'xtask.pid'
    receipt = {'revision': build['revision'], 'build_record_sha256': digest(build_path),
               'tools_record': str(tools_record), 'tools_record_sha256': digest(tools_record),
               'source_manifest_sha256': build['source_manifest_sha256'], 'status': 'running',
               'artifacts': {str(path): sha for path, sha in artifacts.items()},
               'started': datetime.now(timezone.utc).isoformat(), 'pid': os.getpid(), 'commands': []}

    def save():
        temporary = receipt_path.with_suffix('.tmp')
        temporary.write_text(json.dumps(receipt, indent=2) + '\n')
        temporary.replace(receipt_path)

    def retire_group(child):
        try:
            os.killpg(child.pid, signal.SIGTERM)
            child.wait(timeout=5)
        except (ProcessLookupError, subprocess.TimeoutExpired):
            pass
        finally:
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            child.wait()

    def run(name, command, timeout=180):
        receipt['commands'].append(command)
        save()
        with (output / f'{name}.log').open('x') as log:
            child = subprocess.Popen(command, cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT,
                                     start_new_session=True)
            try:
                exit_code = child.wait(timeout=timeout)
            except BaseException:
                retire_group(child)
                raise
            if exit_code:
                retire_group(child)
                raise subprocess.CalledProcessError(exit_code, command)

    def interrupted(_signum, _frame):
        raise KeyboardInterrupt('Idle observation interrupted')

    previous_term = signal.signal(signal.SIGTERM, interrupted)
    runtime_dir = None
    try:
        if args.observation == 'before-lifetime':
            source = root / 'crates/ferric-browser-engine-qt/tests/performance-helpers'
            run('configure', [str(wrapper), 'cmake', '-G', 'Unix Makefiles', '-S', str(source), '-B', str(HELPER_BUILD)])
            run('build', [str(wrapper), 'cmake', '--build', str(HELPER_BUILD), '--parallel', '2'])
        plugin_hash = digest(plugin)
        receipt['plugin_sha256'] = plugin_hash
        comparison_keys = ('revision', 'artifacts', 'plugin_sha256')
        if args.observation == 'after-lifetime':
            before_suffix = '' if args.before_attempt == 1 else f'-attempt-{args.before_attempt}'
            before_path = BASE / f'p00-pinned-idle-before-lifetime{before_suffix}/run-record.json'
            before = json.loads(before_path.read_text())
            if (any(before.get(key) != receipt[key] for key in comparison_keys)
                    or before.get('source_postcheck_exact_match') is not True
                    or before.get('artifact_postcheck_exact_match') is not True):
                raise RuntimeError('Before/after observations require matching pinned sources and artifacts')
            verification_path = before_path.with_name('verified-report.json')
            if verification_path.is_file():
                verification = json.loads(verification_path.read_text())
                if (digest(before_path) != verification['runtime_record_sha256']
                        or digest(before_path.with_name('ui-perf.json')) != verification['report_sha256']):
                    raise RuntimeError('Before observation does not match its report verification')
                before.update(rendering_environment=verification['rendering_environment'],
                              runtime_facts=verification['runtime_facts'])
            elif (before['status'] != 'passed'
                    or digest(before_path.with_name('ui-perf.json')) != before.get('report_sha256')):
                raise RuntimeError('Before observation has no verified idle report')
        # The durable fixture includes the full TMPDIR in its Unix socket
        # path. The original profile name fits under /tmp directly.
        runtime_dir = Path(tempfile.mkdtemp(prefix='ferric-p00-idle-', dir='/tmp'))
        env['TMPDIR'] = '/tmp'
        env['XDG_RUNTIME_DIR'] = str(runtime_dir)
        receipt['load_before'] = Path('/proc/loadavg').read_text().strip()
        record_pid = ('import os, pathlib, sys\n'
                      'pathlib.Path(sys.argv[1]).write_text(str(os.getpid()))\n'
                      'os.execv(sys.argv[2], sys.argv[2:])')
        run('xtask', ['dbus-run-session', '--', sys.executable, '-c', record_pid, str(pid_path),
                      str(wrapper), str(xtask), 'perf', '--suite', 'ui',
                      '--samples', '30', '--repetitions', '2', '--idle-seconds', '120', '--pages', '1',
                      '--presentation', 'internal', '--page-url', 'about:blank',
                      '--quit-plugin-dir', str(HELPER_BUILD/'plugins'), '--output', str(report_path)], 720)
        report = json.loads(report_path.read_text())
        if report['suite'] != 'ui' or report['repetition_count'] != 2 or len(report['runs']) != 2:
            raise RuntimeError('Idle report lacks the requested independent repetitions')
        provenance = report['provenance']
        if provenance['source_commit'] != build['revision'] or provenance['qt'] != '6.11.2':
            raise RuntimeError('Idle report runtime does not match the pinned revision and SDK')
        receipt['rendering_environment'] = {key: provenance[key] for key in
            ('qt', 'qt_webengine', 'qpa_platform', 'quick_backend', 'scale_factor', 'chromium_flags')}
        receipt['runtime_facts'] = [row['runtime'] for row in report['runs']]
        receipt['resource_measurements_complete'] = []
        for row in report['runs']:
            measures = {item['name']: item for item in row['measurements']}
            if (row['method']['idle_window_seconds'] < 120 or measures['shutdown']['owned_quit_confirmed'] is not True
                    or measures['shutdown']['error'] is not None
                    or any(type(measures[name]['value_percent']) not in (int, float)
                           or not math.isfinite(measures[name]['value_percent'])
                           for name in ('idle.main_cpu', 'idle.process_tree_cpu'))):
                raise RuntimeError('Idle report lacks a complete CPU interval or rendered Quit proof')
            resources = measures['process_tree']
            receipt['resource_measurements_complete'].append(
                resources['unavailable_memory_processes'] == resources['unavailable_thread_processes'] == 0
                and all(type(resources[key]) is int and resources[key] >= 0
                        for key in ('rss_kib', 'pss_kib', 'threads', 'processes', 'renderers')))
        if args.observation == 'after-lifetime':
            for key in ('rendering_environment', 'runtime_facts'):
                if before.get(key) != receipt[key]:
                    raise RuntimeError('Before/after idle runtime facts differ')
        receipt.update(status='passed', report_sha256=digest(report_path))
    except BaseException as error:
        receipt.update(status='failed', error=f'{type(error).__name__}: {error}')
    finally:
        signal.signal(signal.SIGTERM, previous_term)
        try:
            if pid_path.is_file():
                owned_pid = int(pid_path.read_text())
                retired = []
                for folder in Path('/tmp').glob(f'ferric-browser-ui-perf-{owned_pid}-*'):
                    marker = folder / '.ferric-performance-fixture'
                    if (not folder.is_symlink() and not marker.is_symlink() and marker.is_file()
                            and marker.read_bytes() == b'Ferric performance fixture only.\n'):
                        shutil.rmtree(folder)
                        retired.append(str(folder))
                receipt['profiles_retired_after_exit'] = retired
            receipt['source_postcheck_exact_match'] = helpers['inventory']() == expected
            receipt['artifact_postcheck_exact_match'] = all(digest(path) == sha for path, sha in artifacts.items())
            if not receipt['source_postcheck_exact_match'] or not receipt['artifact_postcheck_exact_match']:
                receipt['status'] = 'failed'
        except Exception as error:
            receipt.update(status='failed', postcheck_error=f'{type(error).__name__}: {error}')
        if runtime_dir is not None:
            # The private session bus can have just stopped while its document
            # portal unmounts a temporary FUSE directory. Keep cleanup status
            # separate from already collected browser measurements.
            for attempt in range(8):
                try:
                    shutil.rmtree(runtime_dir, ignore_errors=False)
                    break
                except FileNotFoundError:
                    if not runtime_dir.exists():
                        break
                except OSError as error:
                    if error.errno not in (errno.EBUSY, errno.ECONNABORTED, errno.ENOTCONN):
                        receipt['runtime_cleanup_error'] = f'{runtime_dir}: {error}'
                        break
                if attempt == 7:
                    receipt['runtime_cleanup_error'] = f'Runtime directory remained busy: {runtime_dir}'
                else:
                    time.sleep(.5)
        receipt['finished'] = datetime.now(timezone.utc).isoformat()
        save()
    print(receipt_path)
    return 0 if receipt['status'] == 'passed' else 1


if __name__ == '__main__':
    raise SystemExit(main())

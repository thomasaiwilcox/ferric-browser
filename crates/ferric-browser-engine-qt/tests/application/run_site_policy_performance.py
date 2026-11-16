#!/usr/bin/env python3
"""Run opt-in production rendering measurements and preserve exact input metadata."""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import signal
import subprocess

ROOT = Path(__file__).resolve().parents[4]
BASELINE = 'f2c5d6deebd97eb861084503f25613a6a819949d'
MEASUREMENT_FILES = {
    'crates/ferric-browser-engine-qt/tests/application/tst_application.h',
    'crates/ferric-browser-engine-qt/tests/application/tst_application.cpp',
    'crates/ferric-browser-engine-qt/tests/application/CMakeLists.txt',
    'crates/ferric-browser-engine-qt/tests/application/tst_site_policy_performance.cpp',
    'crates/ferric-browser-engine-qt/tests/application/run_site_policy_performance.py',
    'crates/ferric-browser-engine-qt/tests/fixtures/site-policy/render-targets.html',
    'crates/ferric-browser-engine-qt/tests/fixtures/site-policy/render-refresh-targets.html',
    'crates/ferric-browser-engine-qt/tests/fixtures/site-policy/root-render-targets.html',
}
DIAGNOSTIC_FILES = {
    'crates/ferric-browser-engine-qt/src/hint_frame_broker.cpp',
    'crates/ferric-browser-engine-qt/src/hint_frame_broker.h',
    'crates/ferric-browser-engine-qt/qml/components/FerricHintFrameProvider.qml',
}


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def finite_number(value):
    try:
        return type(value) in (int, float) and math.isfinite(value)
    except OverflowError:
        return False


def valid_samples(value, expected):
    return (isinstance(value, list) and len(value) == expected
            and all(finite_number(item) and item >= 0 for item in value))


def source_inventory():
    names = subprocess.check_output(
        ['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard'], cwd=ROOT
    ).split(b'\0')
    result = {}
    for raw in sorted(set(names) - {b''}):
        name = os.fsdecode(raw)
        path = ROOT / name
        if path.is_symlink() or not path.is_file():
            raise RuntimeError('Source inventory requires a regular file: ' + name)
        result[name] = digest(path)
    return result


def production_differences(manifest):
    entries = subprocess.check_output(['git', 'ls-tree', '--full-tree', '-r', '-z', BASELINE], cwd=ROOT).split(b'\0')
    original = {}
    for entry in entries:
        if not entry:
            continue
        identity, raw_name = entry.split(b'\t', 1)
        mode, kind, oid = identity.split()
        if kind != b'blob' or mode not in (b'100644', b'100755'):
            raise RuntimeError('Original baseline requires regular source files')
        original[os.fsdecode(raw_name)] = oid.decode('ascii')
    differences = []
    for name in sorted((set(original) | set(manifest)) - MEASUREMENT_FILES):
        if name not in original or name not in manifest:
            differences.append(name)
            continue
        content = (ROOT / name).read_bytes()
        # Compare exact Git blob identity; the complete SHA-256 inventory is
        # independently retained for the actual run, including harness files.
        blob = b'blob ' + str(len(content)).encode('ascii') + b'\0' + content
        if hashlib.sha1(blob).hexdigest() != original[name]:
            differences.append(name)
    return differences


def terminate_group(child):
    try:
        os.killpg(child.pid, signal.SIGTERM)
    except ProcessLookupError:
        child.wait()
        return
    try:
        child.wait(timeout=5)
    except subprocess.TimeoutExpired:
        pass
    finally:
        # The main process may exit before its renderer children. Retire any
        # remaining members of this owned session even after main-process exit.
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        child.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--archive', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    parser.add_argument('--kind', choices=['command', 'hints', 'refresh', 'refresh-visible', 'ready', 'activation', 'roots', 'root-stages', 'lifetime', 'authoring-lifetime', 'routing', 'all'], default='all')
    parser.add_argument('--hint-policy', choices=['ordinary', 'nonmatching'], default='ordinary',
                        help='Hint-only paired control, or 256 valid nonmatching site rules')
    parser.add_argument('--hint-row', choices=['500-80', '500-100', '500-125', '1000-100', '5000-100'],
                        help='Isolate one existing Hint row without changing its samples or deadlines')
    parser.add_argument('--ready-row', action='append', choices=[
        f'{size}-{scale}-{operation}' for operation in ['prefix', 'selection', 'query', 'queued-prefix']
        for size, scale in [(500, 80), (500, 100), (500, 125), (1000, 100), (5000, 100)]],
        help='Select existing ready-input rows; repeat for multiple rows, with unchanged samples and deadlines')
    parser.add_argument('--activation-row', action='append', choices=['500-80', '500-100', '500-125', '1000-100', '5000-100'],
                        help='Select existing activation rows; repeat for multiple rows, with unchanged samples and deadlines')
    parser.add_argument('--activation-policy', choices=['ordinary', 'anchor'], default='ordinary',
                        help='Activation paired control, or one matched anchor selected through its reserved code')
    parser.add_argument('--dpr', type=int, choices=[1, 2], default=1)
    parser.add_argument('--smoke', action='store_true', help='Functional handshake only; never a baseline')
    parser.add_argument('--require-baseline', action='store_true',
                        help='Require the original production revision plus the enumerated measurement overlay')
    args = parser.parse_args()
    if args.kind != 'hints' and (args.hint_policy != 'ordinary' or args.hint_row):
        parser.error('--hint-policy and --hint-row apply only to --kind hints')
    if args.ready_row and (args.kind != 'ready' or len(args.ready_row) != len(set(args.ready_row))):
        parser.error('--ready-row requires --kind ready and distinct rows')
    if args.activation_row and (args.kind != 'activation' or len(args.activation_row) != len(set(args.activation_row))):
        parser.error('--activation-row requires --kind activation and distinct rows')
    if args.activation_policy != 'ordinary' and args.kind != 'activation':
        parser.error('--activation-policy requires --kind activation')
    if args.require_baseline and (args.hint_policy != 'ordinary' or args.activation_policy != 'ordinary'):
        parser.error('The original baseline requires the ordinary policy control')
    lifetime_kind = args.kind in ['lifetime', 'authoring-lifetime']
    try:
        git_root = Path(subprocess.check_output(
            ['git', 'rev-parse', '--show-toplevel'], cwd=ROOT, text=True, stderr=subprocess.STDOUT
        ).strip()).resolve()
    except subprocess.CalledProcessError:
        parser.error('Install this measurement harness in its actual repository worktree')
    if git_root != ROOT:
        parser.error('Measurement ROOT must equal the Git worktree root; nested archives/draft directories are not a baseline')
    if os.environ.get('FERRIC_P00_BROKER_TRACE') is not None and not args.smoke:
        parser.error('Instrumented broker tracing is diagnostic smoke only; remove the overlay for unprofiled timing')
    if os.environ.get('FERRIC_P15_OVERLAY_DIAGNOSTIC') is not None and not args.smoke:
        parser.error('Marker creation counting is diagnostic only; full baselines must be unprofiled')
    binary, archive, output = args.binary.resolve(), args.archive.resolve(), args.output_dir.resolve()
    if not binary.is_file() or not archive.is_file():
        parser.error('The matching production archive and built test binary are required')
    output.mkdir(parents=True, exist_ok=False)
    log_path = output / 'test.log'
    metadata = {
        'purpose': 'Measurement metadata, not a release qualification receipt',
        'started_utc': datetime.now(timezone.utc).isoformat(),
        'run_kind': 'functional-smoke-not-baseline' if args.smoke else 'lifecycle-baseline' if lifetime_kind else 'performance-baseline',
        'requested_dpr': args.dpr,
        'broker_tracing': os.environ.get('FERRIC_P00_BROKER_TRACE') is not None,
        'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
        'machine': {'platform': platform.platform(), 'cpuinfo': Path('/proc/cpuinfo').read_text().split('model name', 1)[-1].splitlines()[0],
                    'memory': Path('/proc/meminfo').read_text().splitlines()[0], 'loadavg': Path('/proc/loadavg').read_text().strip()},
        'binary': {'path': str(binary), 'sha256': digest(binary)},
        'archive': {'path': str(archive), 'sha256': digest(archive)},
        'input': 'Per journey: constructed QKeyEvent through production QWindow/router, or explicit public BrowserUi hint-buttons command for rooted-render rows, or direct production provider callbacks for component rows',
        'endpoint': 'Post-production-close resource observations; no latency percentile' if lifetime_kind else 'Synchronous native Qt routing return; renderer receipt outside timing' if args.kind == 'routing' else 'matching Qt Quick synchronized/swapped frame; excludes compositor presentation',
        'policy': {'full_command': '10warmups/1000samples', 'full_hint': '10warmups/100fresh-document samples per size/zoom',
                   'full_authoring_lifetime': '10warmups/1000 cycles; create draft, native capture selection, complete live preview, discard; natural cleanup before resource observations',
                   'full_native_routing': '10warmups/1000 press/release samples for raw and mapped blocks; renderer receipt outside timing; maximum press/release sample-index difference, nearest-rank p99',
                   'full_refresh': '10warmups/100samples; 1000 actual style mutations per sample; production passive refresh',
                   'full_ready': '10warmups/100samples per size/zoom/input operation; final activation/fresh proof separate',
                   'full_activation': '10warmups/100samples per size/zoom; selected button Enter through production fresh/action to observed exact DOM click receipt; polling/transport included',
                   'full_roots': '10warmups/100fresh-document samples per5root-shapes x5sizes/zooms; public hint-buttons command to exact production candidate/marker render endpoint',
                   'smoke': '1warmup/2samples; no percentile/gate claims', 'reset': 'outside measured interval',
                   'observer': 'inclusive latency; generation-wide observer cost recorded separately, never subtracted'},
        'environment': {}, 'exit_code': None, 'timed_out': False,
    }
    manifest = source_inventory()
    try:
        differences = production_differences(manifest)
    except subprocess.CalledProcessError as error:
        if args.require_baseline:
            raise
        differences = None
        metadata['production_comparison_error'] = str(error)
    production_ok = (not args.require_baseline or not differences
                     or (args.smoke and set(differences) <= DIAGNOSTIC_FILES))
    metadata['require_original_production'] = args.require_baseline
    metadata['production_baseline'] = BASELINE
    metadata['production_differences'] = differences
    metadata['instrumented_production'] = bool(differences) if differences is not None else None
    metadata['production_preflight_exit_code'] = 0 if production_ok else 1
    (output / 'source-inputs.json').write_text(json.dumps(manifest, indent=2) + '\n')
    metadata['source_manifest_sha256'] = digest(output / 'source-inputs.json')
    harness = Path(__file__).resolve().parent
    binary_inputs = [archive, harness / 'tst_application.cpp', harness / 'tst_application.h',
                     harness / 'tst_site_policy_performance.cpp', harness / 'CMakeLists.txt',
                     harness.parent / 'EngineArchive.cmake']
    outdated = [str(path) for path in binary_inputs if path.stat().st_mtime_ns > binary.stat().st_mtime_ns]
    metadata['binary_input_preflight'] = {'inputs': [str(path) for path in binary_inputs], 'newer_than_binary': outdated}
    flags = binary.parent / 'CMakeFiles/application_tests.dir/flags.make'
    if flags.is_file():
        metadata['harness_build_flags'] = flags.read_text()
    metadata['hint_policy'] = args.hint_policy
    metadata['hint_row'] = args.hint_row
    metadata['ready_rows'] = args.ready_row
    metadata['activation_rows'] = args.activation_row
    metadata['activation_policy'] = args.activation_policy
    env = dict(os.environ)
    env.pop('FERRIC_P15_ACTIVATION_POLICY', None)
    if args.kind == 'activation':
        env['FERRIC_P15_ACTIVATION_POLICY'] = args.activation_policy
    env.pop('FERRIC_P15_HINT_POLICY', None)
    if args.kind == 'hints':
        env['FERRIC_P15_HINT_POLICY'] = args.hint_policy
    env.pop('FERRIC_P00_LIFETIME', None)
    if lifetime_kind and not args.smoke:
        env['FERRIC_P00_LIFETIME'] = '1'
    env.pop('FERRIC_SITE_POLICY_PERF', None)
    env.pop('FERRIC_SITE_POLICY_PERF_SMOKE', None)
    env.pop('FERRIC_P15_NATIVE_ROUTING', None)
    if args.kind == 'routing':
        env['FERRIC_P15_NATIVE_ROUTING'] = '1'
    env['FERRIC_SITE_POLICY_PERF_SMOKE' if args.smoke else 'FERRIC_SITE_POLICY_PERF'] = '1'
    env.update(QT_QPA_PLATFORM='offscreen', QT_QUICK_BACKEND='software', QT_QPA_PLATFORMTHEME='',
               QTWEBENGINE_CHROMIUM_FLAGS='--disable-gpu', QTEST_FUNCTION_TIMEOUT='7200000' if args.kind == 'authoring-lifetime' else '3600000',
               QT_SCALE_FACTOR=str(args.dpr))
    for key in ['QT_QPA_PLATFORM', 'QT_QUICK_BACKEND', 'QT_QPA_PLATFORMTHEME', 'QTWEBENGINE_CHROMIUM_FLAGS',
                'QTEST_FUNCTION_TIMEOUT', 'FERRIC_SITE_POLICY_PERF', 'FERRIC_SITE_POLICY_PERF_SMOKE',
                'FERRIC_P00_LIFETIME', 'FERRIC_P15_NATIVE_ROUTING', 'FERRIC_P15_HINT_POLICY', 'FERRIC_P15_ACTIVATION_POLICY', 'FERRIC_P15_OVERLAY_DIAGNOSTIC', 'QSG_RENDER_LOOP', 'QMAKE', 'LD_LIBRARY_PATH', 'QT_SCALE_FACTOR', 'FERRIC_P00_BROKER_TRACE']:
        if key in env:
            metadata['environment'][key] = env[key]
    selectors = []
    if args.kind in ['command', 'all']:
        selectors.append('sitePolicyCommandRenderPerformance')
    if args.kind in ['hints', 'all']:
        selectors.append('sitePolicyHintRenderPerformance' + (':' + args.hint_row if args.hint_row else ''))
    if args.kind == 'all':
        selectors.append('sitePolicyPassiveRefreshPerformance')
    elif args.kind == 'refresh':
        selectors.append('sitePolicyPassiveRefreshPerformance:1000-targets')
    elif args.kind == 'refresh-visible':
        selectors.append('sitePolicyPassiveRefreshPerformance:500-visible')
    if args.kind in ['ready', 'all']:
        selectors.extend(['sitePolicyReadyHintPerformance:' + row for row in args.ready_row]
                         if args.ready_row else ['sitePolicyReadyHintPerformance'])
    if args.kind in ['activation', 'all']:
        selectors.extend(['sitePolicyActivationPerformance:' + row for row in args.activation_row]
                         if args.activation_row else ['sitePolicyActivationPerformance'])
    if args.kind in ['roots', 'all']:
        selectors.append('sitePolicyRootRenderPerformance')
    if args.kind in ['root-stages', 'all']:
        selectors.append('sitePolicyRootStagesPerformance')
    if args.kind == 'lifetime':
        selectors = ['sitePolicyOrdinaryLifetime', 'sitePolicyWindowLifetime']
    elif args.kind == 'authoring-lifetime':
        selectors = ['sitePolicyAuthoringLifetime']
    elif args.kind == 'routing':
        selectors = ['sitePolicyNativeProductionGesture:persistent-command']
    command = [str(binary), *selectors, '-nocrashhandler', '-maxwarnings', '0']
    metadata['command'] = command
    record = output / 'run-metadata.json'
    def save():
        record.write_text(json.dumps(metadata, indent=2) + '\n')
    save()
    def interrupted(_signum, _frame):
        raise KeyboardInterrupt('Performance fixture interrupted')
    previous_term = signal.signal(signal.SIGTERM, interrupted)
    child = None
    try:
        with log_path.open('w') as log:
            guard = ['cmake', '-DFERRIC_ARCHIVE_TO_CHECK=' + str(archive), '-DFERRIC_SOURCE_ROOT=' + str(ROOT),
                     '-P', str(ROOT / 'crates/ferric-browser-engine-qt/tests/EngineArchive.cmake')]
            if not production_ok:
                log.write('Production source differs from the pinned baseline: ' + ', '.join(differences) + '\n')
                log.flush()
                metadata['exit_code'] = 1
                preflight_code = 1
            elif outdated:
                log.write('Test binary predates required archive/harness inputs: ' + ', '.join(outdated) + '\n')
                log.flush()
                metadata['binary_input_preflight_exit_code'] = 1
                metadata['exit_code'] = 1
                preflight_code = 1
            else:
                metadata['binary_input_preflight_exit_code'] = 0
                preflight_code = subprocess.run(guard, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, check=False).returncode
                metadata['archive_preflight_exit_code'] = preflight_code
                metadata['exit_code'] = preflight_code
            if preflight_code == 0:
                child = subprocess.Popen(command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
                try:
                    metadata['exit_code'] = child.wait(timeout=7200)
                except subprocess.TimeoutExpired:
                    metadata['timed_out'] = True
                    terminate_group(child)
                    metadata['exit_code'] = child.returncode
    except BaseException as error:
        metadata['interrupted'] = isinstance(error, KeyboardInterrupt)
        metadata['harness_error'] = type(error).__name__ + ': ' + str(error)
        if child is not None:
            terminate_group(child)
        metadata['exit_code'] = 130 if isinstance(error, KeyboardInterrupt) else 1
    finally:
        signal.signal(signal.SIGTERM, previous_term)
        rows, samples, errors = [], [], []
        log_text = log_path.read_text(errors='replace')
        metadata['logging_limit_exceeded'] = 'Maximum amount of warnings exceeded' in log_text
        for number, line in enumerate(log_text.splitlines(), 1):
            for marker, destination in [('FERRIC_SITE_POLICY_RENDER_JSON ', rows),
                                        ('FERRIC_SITE_POLICY_SAMPLE_JSON ', samples),
                                        ('FERRIC_P00_LIFETIME_JSON ', rows),
                                        ('FERRIC_P00_LIFETIME_CHECKPOINT_JSON ', samples),
                                        ('FERRIC_P00_LIFETIME_WINDOW_CYCLE_JSON ', samples)]:
                if marker not in line:
                    continue
                try:
                    value = json.loads(line.split(marker, 1)[1])
                    if not isinstance(value, dict):
                        raise ValueError('Expected a JSON object record')
                    destination.append(value)
                except ValueError as error:
                    errors.append({'line': number, 'error': str(error), 'marker': marker})
        (output / 'sample-events.json').write_text(json.dumps(samples, indent=2) + '\n')
        (output / 'raw-results.json').write_text(json.dumps(rows, indent=2) + '\n')
        metadata['result_rows'] = len(rows)
        metadata['sample_events'] = len(samples)
        metadata['parse_errors'] = errors
        metadata['child_exit_code'] = child.returncode if child is not None else None
        expected = (1 if args.kind == 'authoring-lifetime' else 4) if lifetime_kind else ((1 if args.kind in ['command', 'routing', 'all'] else 0)
                    + ((1 if args.hint_row else 5) if args.kind in ['hints', 'all'] else 0)
                    + (2 if args.kind == 'all' else 1 if args.kind in ['refresh', 'refresh-visible'] else 0)
                    + ((len(args.ready_row) if args.ready_row else 20) if args.kind in ['ready', 'all'] else 0)
                    + ((len(args.activation_row) if args.activation_row else 5) if args.kind in ['activation', 'all'] else 0)
                    + (25 if args.kind in ['roots', 'all'] else 0)
                    + (55 if args.kind in ['root-stages', 'all'] else 0))
        validation_errors = []
        if metadata['logging_limit_exceeded']:
            validation_errors.append('QtTest suppressed required measurement records after its message limit')
            metadata.setdefault('harness_error', 'QtTest message limit suppressed measurement records')
        if len(rows) != expected:
            validation_errors.append(f'Expected {expected} result rows, received {len(rows)}')
        for row in rows:
            if row.get('run_kind') != metadata['run_kind']:
                validation_errors.append('Result kind does not match requested smoke/full mode')
            if row.get('failures') != 0:
                validation_errors.append('Result reports a failed stage')
            dpr = row.get('dpr')
            if not finite_number(dpr) or abs(dpr - args.dpr) > .01:
                validation_errors.append('Actual DPR does not match requested DPR')
            if lifetime_kind:
                window_row = row.get('journey') in ['shared-profile-window-lifetime', 'private-profile-window-lifetime']
                requested = 2 if args.smoke else 100 if window_row else 1000
                warmups = 1 if args.smoke else 10
                if (not all(type(row.get(key)) is int for key in ['completed_cycles', 'requested_cycles', 'completed_warmups', 'warmups'])
                    or row['completed_cycles'] != requested or row.get('requested_cycles') != requested or row.get('completed_warmups') != warmups
                    or row.get('warmups') != warmups):
                    validation_errors.append('Lifecycle cycles/warmups are incomplete')
                snapshots = row.get('snapshots')
                if not isinstance(snapshots, list) or not all(isinstance(item, dict) for item in snapshots):
                    validation_errors.append('Invalid lifecycle snapshots')
                    continue
                interval = 10 if window_row else 100
                wanted_cycles = {0, 2} if args.smoke else {0, *range(interval, requested + 1, interval)}
                actual_cycles = [item.get('cycle') for item in snapshots]
                if (not all(type(item) is int for item in actual_cycles)
                    or len(actual_cycles) != len(wanted_cycles) or set(actual_cycles) != wanted_cycles):
                    validation_errors.append('Lifecycle checkpoint matrix is incomplete or duplicated')
                for snapshot in snapshots:
                    resources = snapshot.get('process_resources')
                    if (not isinstance(resources, dict) or resources.get('complete') is not True
                        or resources.get('pss_complete') is not True):
                        validation_errors.append('Lifecycle process-resource snapshot is incomplete')
                    elif (not all(type(resources.get(key)) is int and resources[key] >= 0
                                  for key in ['rss_kib', 'pss_kib', 'threads', 'fd_count', 'process_count'])
                          or not isinstance(resources.get('processes'), list)
                          or resources['process_count'] != len(resources['processes'])
                          or resources['process_count'] < 1):
                        validation_errors.append('Invalid lifecycle aggregate resources')
                    if not all(type(snapshot.get(key)) is int and snapshot[key] >= 0
                               for key in ['qt_descendants', 'qt_windows']):
                        validation_errors.append('Invalid lifecycle Qt counts')
                    if not window_row and snapshot.get('cleanup_clean') is not True:
                        validation_errors.append('Lifecycle renderer/browser cleanup was not established')
                if window_row:
                    outcomes = row.get('outcomes')
                    if (not isinstance(outcomes, list) or len(outcomes) != requested + warmups
                        or not all(isinstance(item, dict) and item.get('released') is True
                                   and item.get('view_destroyed') is True
                                   and item.get('initial_rendered_close_confirmed') is True
                                   and item.get('ledger_returned_to_baseline') is True for item in outcomes)):
                        validation_errors.append('Window/profile release outcomes are incomplete')
                    elif (not all(type(item.get('index')) is int and type(item.get('warmup')) is bool
                                  and item['warmup'] == (item['index'] < 0) for item in outcomes)
                          or sorted(item['index'] for item in outcomes) != list(range(-warmups, requested))):
                        validation_errors.append('Window/profile release cycle matrix is incomplete or duplicated')
                continue
            requested = 2 if args.smoke else 1000 if row.get('journey') in ['normal-colon-to-command-render', 'synchronous-native-routing'] else 100
            if row.get('completed_measured_samples') != requested or not valid_samples(row.get('samples_ms'), requested):
                validation_errors.append('Result sample count is incomplete')
            if args.kind == 'routing':
                arrays = [row.get(key) for key in ['raw_samples_ms', 'mapped_samples_ms', 'raw_release_samples_ms', 'mapped_release_samples_ms']]
                if (row.get('journey') != 'synchronous-native-routing' or row.get('warmups') != (1 if args.smoke else 10)
                    or not all(valid_samples(values, requested) for values in arrays)):
                    validation_errors.append('Native routing raw/mapped press/release samples are incomplete')
                else:
                    raw, mapped, raw_release, mapped_release = arrays
                    added = [max(0, mapped[i]-raw[i], mapped_release[i]-raw_release[i]) for i in range(requested)]
                    if not all(math.isclose(a,b,rel_tol=1e-9,abs_tol=1e-9) for a,b in zip(added,row['samples_ms'])):
                        validation_errors.append('Native routing added-time samples disagree with raw measurements')
                    p99 = sorted(added)[math.ceil(requested*.99)-1]
                    if not finite_number(row.get('p99_ms')) or not math.isclose(row['p99_ms'],p99,rel_tol=1e-9,abs_tol=1e-9):
                        validation_errors.append('Native routing p99 calculation disagrees with raw measurements')
                    if not args.smoke and p99 > 1:
                        validation_errors.append('Native routing added p99 exceeds 1ms')
        if lifetime_kind:
            journeys = [row.get('journey') for row in rows]
            wanted = {'ordinary-established-lifetime', 'foreign-established-lifetime',
                      'shared-profile-window-lifetime', 'private-profile-window-lifetime'}
            if args.kind == 'authoring-lifetime':
                wanted = {'authoring-capture-preview-lifetime'}
            if (not all(type(item) is str for item in journeys)
                or len(journeys) != len(wanted) or set(journeys) != wanted):
                validation_errors.append('Lifecycle journey matrix is incomplete or duplicated')
        try:
            after = source_inventory()
            metadata['source_postcheck_exact_match'] = after == manifest
            if after != manifest:
                metadata['source_changed_paths'] = sorted(
                    name for name in set(after) | set(manifest) if after.get(name) != manifest.get(name)
                )
                validation_errors.append('Source inventory changed during measurement')
        except Exception as error:
            metadata['source_postcheck_exact_match'] = False
            metadata['source_postcheck_error'] = type(error).__name__ + ': ' + str(error)
            validation_errors.append('Source inventory could not be verified after measurement')
        root_shapes = ['shadow-2', 'slots', 'same-origin-2', 'foreign-2', 'clipping']
        root_sizes = [(500, .8), (500, 1), (500, 1.25), (1000, 1), (5000, 1)]
        if args.kind in ['activation', 'all']:
            if any(row.get('policy_case') != args.activation_policy for row in rows
                   if row.get('journey') == 'selected-hint-enter-to-dom-activation'):
                validation_errors.append('Activation policy does not match the requested workload')
            actual = [(row.get('count'), row.get('zoom')) for row in rows
                      if row.get('journey') == 'selected-hint-enter-to-dom-activation']
            wanted = {(int(name.split('-')[0]), int(name.split('-')[1]) / 100)
                      for name in args.activation_row} if args.activation_row else set(root_sizes)
            if (any(type(count) is not int or not finite_number(zoom) for count, zoom in actual)
                    or len(actual) != len(wanted) or set(actual) != wanted):
                validation_errors.append('Activation workload matrix is incomplete or duplicated')
        if args.kind in ['refresh', 'refresh-visible', 'all']:
            wanted = [500, 1000] if args.kind == 'all' else [500 if args.kind == 'refresh-visible' else 1000]
            refresh = [row for row in rows if row.get('journey') == '1000-mutations-to-passive-hint-render']
            counts = [row.get('count') for row in refresh]
            if not all(type(count) is int for count in counts) or sorted(counts) != wanted:
                validation_errors.append('Passive refresh workload matrix is incomplete or duplicated')
            for row in refresh:
                if type(row.get('mutations_per_sample')) is not int or row['mutations_per_sample'] != 1000:
                    validation_errors.append('Passive refresh mutation workload changed')
                if row.get('count') == 500:
                    markers = row.get('rendered_marker_counts')
                    completed = row.get('completed_measured_samples')
                    if (type(completed) is not int or not isinstance(markers, list) or len(markers) != completed
                            or any(type(value) is not int or value != 500 for value in markers)):
                        validation_errors.append('Passive refresh did not retain 500 rendered markers')
                    receipts = row.get('mutation_receipts')
                    warmups = 1 if args.smoke else 10
                    if (not isinstance(receipts, list) or len(receipts) != completed
                            or any(not isinstance(receipt, dict)
                                   or not all(type(receipt.get(key)) is int for key in ('revision', 'targets', 'mutations', 'total_mutations'))
                                   or receipt['revision'] != index + warmups + 1 or receipt['targets'] != 500
                                   or receipt['mutations'] != 1000 or receipt['total_mutations'] != 1000 * receipt['revision']
                                   for index, receipt in enumerate(receipts))):
                        validation_errors.append('Passive refresh is missing its exact mutation receipts')
        if args.kind in ['ready', 'all']:
            journeys = ('ready-prefix-to-hint-render', 'ready-selection-to-hint-render',
                        'ready-query-to-hint-render', 'queued-prefix-to-hint-render')
            actual = []
            for row in rows:
                if row.get('journey') not in journeys:
                    continue
                if type(row.get('count')) is not int or not finite_number(row.get('zoom')):
                    validation_errors.append('Invalid ready Hint workload fields')
                    continue
                actual.append((row['journey'], row['count'], row['zoom']))
                if row.get('failures') != 0:
                    continue
                warmups = 1 if args.smoke else 10
                requested = 2 if args.smoke else 100
                events = [event for event in samples if all(event.get(key) == row[key]
                          for key in ('journey', 'count', 'zoom'))]
                if (row.get('warmups') != warmups or len(events) != warmups + requested
                        or [event.get('elapsed_ms') for event in events[warmups:]] != row.get('samples_ms')):
                    validation_errors.append('Ready Hint sample event matrix is incomplete')
                    continue
                queued = row['journey'] == 'queued-prefix-to-hint-render'
                for index, event in enumerate(events):
                    session = event.get('session_id')
                    epoch = event.get('collection_epoch')
                    prepared_epoch = event.get('prepared_collection_epoch')
                    if (type(event.get('iteration')) is not int or event['iteration'] != index
                            or type(event.get('count')) is not int or not finite_number(event.get('zoom'))
                            or event.get('warmup') is not (index < warmups) or event.get('smoke') is not args.smoke
                            or not finite_number(event.get('elapsed_ms')) or event['elapsed_ms'] < 0
                            or type(session) is not str or not session
                            or type(epoch) is not int or type(prepared_epoch) is not int):
                        validation_errors.append('Invalid ready Hint sample event')
                        break
                    if queued:
                        if (event.get('no_session_before_queued_key') is not True
                                or event.get('genuinely_queued') is not True
                                or type(event.get('previous_session_id')) is not str or not event['previous_session_id']
                                or session == event.get('previous_session_id') or epoch <= prepared_epoch):
                            validation_errors.append('Queued Hint sample lacks a new session or real queued input')
                            break
                    elif (session != event.get('prepared_session_id') or epoch != prepared_epoch
                            or (row['count'] == 500 and (type(event.get('prepared_marker_count')) is not int
                                or event['prepared_marker_count'] != 500))):
                        validation_errors.append('Ready Hint sample does not match its rendered starting state')
                        break
            wanted = {(journey, count, zoom) for journey in journeys for count, zoom in root_sizes}
            if args.ready_row:
                selected = set()
                for name in args.ready_row:
                    size, scale, operation = name.split('-', 2)
                    journey = 'queued-prefix-to-hint-render' if operation == 'queued-prefix' else 'ready-' + operation + '-to-hint-render'
                    selected.add((journey, int(size), int(scale) / 100))
                wanted = selected
            if len(actual) != len(wanted) or set(actual) != wanted:
                validation_errors.append('Ready Hint workload matrix is incomplete or duplicated')
        if args.kind in ['roots', 'all']:
            actual = []
            for row in rows:
                if row.get('journey') != 'rooted-buttons-command-to-usable-render':
                    continue
                if type(row.get('shape')) is not str or type(row.get('count')) is not int or not finite_number(row.get('zoom')):
                    validation_errors.append('Invalid root render workload fields')
                    continue
                actual.append((row['shape'], row['count'], row['zoom']))
            wanted = {(shape, count, zoom) for shape in root_shapes for count, zoom in root_sizes}
            if len(actual) != len(wanted) or set(actual) != wanted:
                validation_errors.append('Root render workload matrix is incomplete or duplicated')
        if args.kind in ['root-stages', 'all']:
            component_rows = [row for row in rows if row.get('journey') in
                              ['rooted-provider-collection', 'rooted-provider-fresh']]
            actual = []
            for row in component_rows:
                if (type(row.get('metric')) is not str or type(row.get('shape')) is not str
                    or type(row.get('count')) is not int or not finite_number(row.get('zoom'))
                    or type(row.get('independent_small_control')) is not bool):
                    validation_errors.append('Invalid root component workload fields')
                    continue
                actual.append((row['metric'], row['shape'], row['count'], row['zoom'], row['independent_small_control']))
            wanted = {(metric, shape, count, zoom, False) for metric in ['collect', 'fresh']
                      for shape in root_shapes for count, zoom in root_sizes}
            wanted |= {('fresh', shape, 1, 1, True) for shape in root_shapes}
            if len(actual) != len(wanted) or set(actual) != wanted:
                validation_errors.append('Root component workload matrix is incomplete or duplicated')
            for row in component_rows:
                requested = 2 if args.smoke else 100
                if row.get('completed_collection_samples') != requested or not valid_samples(row.get('collection_samples_ms'), requested):
                    validation_errors.append('Root component collection observations are incomplete')
                expected_fresh = requested if row.get('metric') == 'fresh' else 0
                if row.get('completed_fresh_samples') != expected_fresh or not valid_samples(row.get('fresh_samples_ms'), expected_fresh):
                    validation_errors.append('Root component fresh observations are incomplete')
                if row.get('cleanup_failures') != 0:
                    validation_errors.append('Root component tracking cleanup failed')
        metadata['result_validation_errors'] = validation_errors
        if metadata['timed_out']:
            metadata['exit_code'] = 124
        elif metadata['exit_code'] is None:
            metadata['exit_code'] = 1
        elif metadata['exit_code'] < 0:
            metadata['exit_code'] = 128 - metadata['exit_code']
        elif (errors or validation_errors) and metadata['exit_code'] == 0:
            metadata['exit_code'] = 1
        metadata['finished_utc'] = datetime.now(timezone.utc).isoformat()
        save()
    print(record)
    return metadata['exit_code']


if __name__ == '__main__':
    raise SystemExit(main())

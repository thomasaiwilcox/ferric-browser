#!/usr/bin/env python3
"""Qualify original IPC receipts across settings and real page-window hops."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import select
import socket
import struct
import subprocess
import tempfile
import time
import uuid


def digest(path):
    result = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            result.update(block)
    return result.hexdigest()


class Client:
    def __init__(self, path):
        self.socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.socket.settimeout(12)
        self.socket.connect(str(path))
        self.prefix = uuid.uuid4().hex
        self.serial = 0
        self.rpc('hello', {'protocol_major': 3, 'protocol_minor': 0,
                           'client': 'settings-handoff-fixture'})

    def receive(self):
        def exact(size):
            parts = bytearray()
            while len(parts) < size:
                block = self.socket.recv(size - len(parts))
                assert block, 'IPC stream closed'
                parts.extend(block)
            return parts
        size = struct.unpack('>I', exact(4))[0]
        assert 0 < size <= 1024 * 1024, size
        return json.loads(exact(size))

    def rpc(self, method, params, allow_busy=False, expected_error=None):
        self.serial += 1
        request_id = f'{self.prefix}-{self.serial}'
        packet = json.dumps({'id': request_id, 'method': method, 'params': params}).encode()
        self.socket.sendall(struct.pack('>I', len(packet)) + packet)
        response = self.receive()
        assert response.get('id') == request_id, response
        if expected_error is not None:
            assert response.get('error', {}).get('code') == expected_error, response
            return response['error']
        if allow_busy and response.get('error', {}).get('code') == 'E_BUSY':
            return {}
        assert 'error' not in response, (method, params, response)
        return response['result']

    def drain(self, seconds):
        deadline = time.monotonic() + seconds
        events = []
        while time.monotonic() < deadline:
            if not select.select([self.socket], [], [], max(0, deadline - time.monotonic()))[0]:
                break
            event = self.receive()
            assert event.get('event_type') == 'operation.completed', event
            events.append(event)
        return events

    def close(self):
        self.socket.close()


def run_case(binary, interposer, settings_first, css, failing_tail, record, *, fault=False, close_owner=False, destroy_owner=False, prewrite_failure=False):
    case = ('settings-first' if settings_first else 'native-first') + ('-css' if css else '-live')
    if failing_tail:
        case += '-tail-failure'
    if fault:
        case += '-durability-uncertain'
    if close_owner:
        case += '-mutation-owner-close'
    if destroy_owner:
        case += '-destroyed-object'
    if prewrite_failure:
        case += '-prewrite-failure'
    record.update(case=case, status='failed', checks=[])
    with tempfile.TemporaryDirectory(prefix='ferric-settings-ipc-') as temporary, \
            tempfile.TemporaryDirectory(prefix='ferric-receipt-markers-') as marker_directory:
        root = Path(temporary)
        # The real watcher includes config parents. Keep all fixture sync
        # controls outside that hierarchy so arming a hold cannot request a
        # competing configuration/context reload.
        config = root / 'fixtures/config/fixture.toml'
        config.parent.mkdir(parents=True)
        config.write_text('schema_version=3\n[tabs]\npresentation="windows"\n'
                          '[hints.selectors.buttons]\nmode="extend"\nselectors=["#baseline"]\n')
        markers = Path(marker_directory)
        ready, release = markers / 'sync-ready', markers / 'sync-release'
        runtime = root / 'state/runtime-overrides.toml'
        # Initial profile/session work is allowed to finish. The optional file
        # gate makes the later hold conditional on actual override replacement.
        release.touch()
        env = os.environ.copy()
        for name in ('FERRIC_SETTINGS_SYNC_FAULT_PATH', 'FERRIC_SETTINGS_SYNC_FIXTURE'):
            env.pop(name, None)
        env.update(QT_QPA_PLATFORM='offscreen', QT_QPA_PLATFORMTHEME='', QT_QUICK_BACKEND='software',
                   QTWEBENGINE_CHROMIUM_FLAGS='--disable-gpu', FERRIC_BROWSER_ALLOW_NON_WAYLAND='1',
                   LD_PRELOAD=str(interposer), FERRIC_SETTINGS_SYNC_HOLD_PATH=str(runtime.parent),
                   FERRIC_SETTINGS_SYNC_HOLD_FILE=str(runtime),
                   FERRIC_SETTINGS_SYNC_READY_FILE=str(ready), FERRIC_SETTINGS_SYNC_RELEASE_FILE=str(release))
        fault_marker, expired_marker = markers / 'fault-armed', markers / 'hold-expired'
        env['FERRIC_SETTINGS_SYNC_HOLD_EXPIRED_FILE'] = str(expired_marker)
        if fault:
            env['FERRIC_SETTINGS_SYNC_FAULT_PATH'] = str(runtime.parent)
            env['FERRIC_SETTINGS_SYNC_FAULT_FILE'] = str(fault_marker)
        if prewrite_failure:
            # A directory at the exact final filename rejects replacement.
            # This case has no held or faulted post-replacement fsync.
            for name in ('FERRIC_SETTINGS_SYNC_HOLD_PATH', 'FERRIC_SETTINGS_SYNC_HOLD_FILE',
                         'FERRIC_SETTINGS_SYNC_READY_FILE', 'FERRIC_SETTINGS_SYNC_RELEASE_FILE'):
                env.pop(name, None)
        if close_owner:
            (markers / '.ferric-owned-fixture').write_text('Owned context-deletion test only.\n')
            env.update(FERRIC_OWNED_QUIT_ROOT=str(markers),
                       QT_QPA_GENERIC_PLUGINS='ferricownedquitfixture',
                       FERRIC_SETTINGS_SYNC_HOLD_THREAD='ferric-settings',
                       FERRIC_SETTINGS_SYNC_READY_FACTS=str(markers / 'held-writer-facts'),
                       FERRIC_SETTINGS_SYNC_COMPLETED_FACTS=str(markers / 'completed-writer-facts'))
            env['QT_PLUGIN_PATH'] = str(interposer.parent / 'plugins') + os.pathsep + env.get('QT_PLUGIN_PATH', '')
        prefix = [str(binary), '--basedir', str(root), '--instance', 'settings-receipt']
        with tempfile.TemporaryFile(mode='w+') as log:
            app = subprocess.Popen([*prefix, '--config', str(config), '--software-rendering',
                                    '--userscripts-off', 'open', 'rb://blank'],
                                   env=env, stdout=log, stderr=subprocess.STDOUT)
            control = events = None
            operation = None

            def wait_for(probe, accepted, seconds=10):
                deadline = time.monotonic() + seconds
                while True:
                    assert app.poll() is None, 'fixture browser exited'
                    value = probe()
                    if accepted(value):
                        return value
                    assert time.monotonic() < deadline, (case, value)
                    time.sleep(.025)

            def command(name, arguments, owner):
                return {'command': name, 'arguments': arguments,
                        'context': {'source': 'cli', 'window': owner}}

            try:
                sockets = wait_for(lambda: list(root.glob('runtime/instances/*/browser.sock')),
                                   bool, seconds=20)
                assert len(sockets) == 1, sockets
                control, events = Client(sockets[0]), Client(sockets[0])
                windows = wait_for(lambda: control.rpc('windows.query', {}, allow_busy=True),
                                   lambda value: len(value.get('live_registry', [])) == 1)
                primary = windows['live_registry'][0]['id']
                wait_for(lambda: control.rpc('tabs.query', {'window': primary}, allow_busy=True),
                         lambda value: len(value.get('tabs', [])) == 1 and value['tabs'][0].get('url') == 'rb://blank'
                         and value['tabs'][0]['loading'] == 'complete')
                control.rpc('command.execute', command('tab-open', {'input': 'rb://blank'}, primary))
                windows = wait_for(lambda: control.rpc('windows.query', {}),
                                   lambda value: len(value.get('live_registry', [])) == 2)
                child = next(row['id'] for row in windows['live_registry'] if row['id'] != primary)
                child_tabs = wait_for(lambda: control.rpc('tabs.query', {'window': child}, allow_busy=True),
                                      lambda value: len(value.get('tabs', [])) == 1 and value['tabs'][0]['url'] == 'rb://blank'
                                      and value['tabs'][0]['loading'] == 'complete')
                child_tab = child_tabs['tabs'][0]['id']
                assert not child_tabs['tabs'][0]['pinned'], child_tabs
                assert not runtime.exists(), 'setup unexpectedly wrote runtime settings'
                if close_owner:
                    plugin = interposer.parent / 'plugins/generic/libcontext_quit_test_plugin.so'
                    loaded = Path(f'/proc/{app.pid}/maps').read_text().splitlines()
                    assert any(line.endswith(' ' + str(plugin)) for line in loaded), 'exact owned close plugin was not loaded'
                    record['loaded_close_plugin'] = {'path': str(plugin), 'sha256': digest(plugin)}
                if fault:
                    fault_marker.touch()
                    record['fault_armed_after_durable_startup'] = True
                events.rpc('events.subscribe', {'event_types': ['operation.completed']})
                child_before = {row['operation_id'] for row in control.rpc('operations.query', {'window': child})['operations']}
                record['destination_operation_ids_before'] = sorted(child_before)
                ready.unlink(missing_ok=True)
                if prewrite_failure:
                    assert not any((settings_first, css, failing_tail, fault, close_owner, destroy_owner))
                    runtime.mkdir()
                    rejected_destination = runtime.stat()
                else:
                    release.unlink()
                key = 'hints.selectors.buttons' if css else 'content.zoom'
                value = '{mode="extend",selectors=["#ipc-handoff"]}' if css else '1.25'
                setting = command('set', {'key': key, 'value': value}, primary)
                focus = command('tab-focus', {'id': child_tab}, primary)
                chain = [setting, focus] if settings_first else [focus, setting]
                chain.append(command('tab-pin', {'id': child_tab, 'state': 'on'}, primary))
                if failing_tail:
                    chain.append(command('get', {'key': 'no.such.setting'}, primary))
                first = dict(chain[0], continuation=chain[1:])
                accepted = control.rpc('command.execute', first)
                operation = accepted['operation_id']
                record['accepted'] = accepted
                assert accepted['command_context']['operation_id'] == operation, accepted
                if prewrite_failure:
                    def original():
                        return control.rpc('operations.query', {'operation_id': operation})['operations']
                    terminal_rows = wait_for(original, lambda rows: len(rows) == 1
                                            and rows[0]['status'] in ('completed', 'failed'))
                    terminal = terminal_rows[0]
                    assert terminal['operation_id'] == operation and terminal['window_id'] == primary, terminal
                    assert terminal['status'] == 'failed' and terminal['error']['code'] == 'E_ENGINE', terminal
                    assert not terminal['cancellable'] and 'settings saved' not in terminal['detail'], terminal
                    for owner in (primary, child):
                        active = control.rpc('command.execute', command('get', {'key': key}, owner))
                        assert active['value'] == 1.0 and not active['pending'], active
                    after = runtime.stat()
                    assert runtime.is_dir() and list(runtime.iterdir()) == []
                    assert (after.st_dev, after.st_ino, after.st_mtime_ns) == (
                        rejected_destination.st_dev, rejected_destination.st_ino, rejected_destination.st_mtime_ns)
                    observed = []
                    deadline = time.monotonic() + 4
                    while not any(event['payload']['operation_id'] == operation for event in observed):
                        assert time.monotonic() < deadline, 'missing failed native root event'
                        observed.extend(events.drain(.1))
                    observed.extend(events.drain(2.25))
                    completions = [event['payload'] for event in observed if event['payload']['operation_id'] == operation]
                    assert len(completions) == 1, completions
                    assert completions[0]['window_id'] == primary and completions[0]['status'] == 'failed', completions
                    assert completions[0]['operation_sequence'] == terminal['sequence'], completions
                    assert completions[0]['error_code'] == 'E_ENGINE', completions
                    assert original() == terminal_rows
                    record.update(terminal=terminal, completion=completions[0], status='passed')
                    record['checks'].append('exact native root fails once before replacement; both owners retain active settings and destination directory remains unchanged')
                    return record
                wait_for(ready.exists, bool, seconds=5)
                held_deadline = time.monotonic() + 5
                if close_owner:
                    witness = (markers / 'held-writer-facts').read_text().splitlines()
                    assert witness == ['ferric-settings', str(runtime.parent)], witness
                    record['held_writer_thread_and_directory'] = witness
                assert ('#ipc-handoff' if css else '1.25') in runtime.read_text()
                wait_for(lambda: control.rpc('tabs.query', {'window': child}),
                         lambda value: value['tabs'][0]['pinned'], seconds=3)
                record['checks'].append('on-disk replacement and accepted tail while receipt is held')

                def operations(owner=None):
                    params = {'operation_id': operation}
                    if owner:
                        params['window'] = owner
                    return control.rpc('operations.query', params)['operations']

                running = None
                for _ in range(3):
                    rows = operations()
                    assert len(rows) == 1, rows
                    running = rows[0]
                    assert running['operation_id'] == operation, running
                    assert running['window_id'] == primary and running['status'] == 'running', running
                    assert not running['cancellable'], running
                    assert operations(primary) == rows, (operations(primary), rows)
                    assert operations(child) == [], operations(child)
                    child_ids = {row['operation_id'] for row in control.rpc('operations.query', {'window': child})['operations']}
                    assert child_ids == child_before, (child_before, child_ids)
                    time.sleep(.025)
                record['running'] = running
                record['checks'].append('original running noncancellable identity; no destination operation')
                competing_request = None
                if not any((settings_first, css, failing_tail, fault, close_owner)):
                    # The held mutation belongs to the child. Attempt a real
                    # competing persistent edit from the surviving issuer.
                    held_bytes = runtime.read_bytes()
                    competing_request = command('set', {'key': key, 'value': '1.5'}, primary)
                    competing = control.rpc('command.execute', competing_request)
                    competing_id = competing['operation_id']
                    assert competing_id != operation, competing
                    competing_rows = wait_for(
                        lambda: control.rpc('operations.query', {'operation_id': competing_id})['operations'],
                        lambda rows: len(rows) == 1 and rows[0]['status'] in ('completed', 'failed'), seconds=3)
                    assert competing_rows[0]['status'] == 'failed', competing_rows
                    assert competing_rows[0]['window_id'] == primary, competing_rows
                    assert competing_rows[0]['error']['code'] == 'E_CONFIG', competing_rows
                    assert runtime.read_bytes() == held_bytes
                    assert operations() == [running], (operations(), running)
                    record['competing_peer_write'] = competing_rows[0]
                    record['checks'].append('actual competing peer write rejected while original admitted writer remains pending')
                mutation_owner = primary if settings_first else child
                # Before close, inspect the actual live writer's candidate.
                # Surviving peers have no observer-local pending preview.
                held = control.rpc('command.execute', command('get', {'key': key}, mutation_owner))
                record['held_query'] = {name: held[name] for name in
                    ('value', 'pending_value', 'pending_state', 'candidate_stage', 'operation_id')}
                assert held['pending'] and held['pending_state'] == 'pending', held
                assert held['candidate_stage'] == 'write-pending', held
                if css:
                    assert held['value']['selectors'] == ['#baseline'], held
                    assert held['pending_value']['selectors'] == ['#baseline', '#ipc-handoff'], held
                    # The original UI received a real startup parser callback;
                    # the cloned child has its own parser context and no such
                    # active receipt. Its new desired CSS is checked locally.
                    assert held['validation']['engine_css'] == ('passed' if settings_first else 'unknown'), held
                    assert held['desired_validation']['engine_css'] == 'passed', held
                    assert held['validation']['live_document'] == 'unknown', held
                    assert held['desired_validation']['live_document'] == 'unknown', held
                else:
                    assert held['value'] == 1.0 and held['pending_value'] == 1.25, held
                assert operations() == [running], (operations(), running)
                assert operations(child) == [], operations(child)
                # command.execute admits its own synchronous query receipt.
                # That is distinct from duplicating the held mutation receipt.
                query_id = held['operation_id']
                assert query_id != operation, held
                query_rows = control.rpc('operations.query', {'operation_id': query_id})['operations']
                assert len(query_rows) == 1 and query_rows[0]['status'] == 'completed', query_rows
                assert query_rows[0]['window_id'] == mutation_owner, query_rows
                child_expected = child_before | ({query_id} if mutation_owner == child else set())
                child_ids = {row['operation_id'] for row in control.rpc('operations.query', {'window': child})['operations']}
                assert child_ids == child_expected, (child_expected, child_ids)
                record['checks'].append('independent read observes exact held writer; separate query receipt completes without changing mutation identity or active value')
                observed = events.drain(.1)
                assert not any(event['payload']['operation_id'] == operation for event in observed), observed
                record['checks'].append('no premature completion event')
                retired_before_release = False
                if close_owner:
                    assert not settings_first and not css, 'close row targets secondary live mutation'
                    closed = control.rpc('command.execute', command('window-close', {}, child))
                    record['close_accepted'] = closed
                    (markers / 'confirm-window-close').write_text(json.dumps({
                        'authorization': "Confirm this fixture's secondary window close.", 'window_id': child}))
                    def confirmation():
                        try:
                            return json.loads((markers / 'window-close-confirmed.json').read_text())
                        except (FileNotFoundError, json.JSONDecodeError):
                            return None
                    proof = wait_for(confirmation, lambda value: isinstance(value, dict)
                                     and value.get('presented') is True,
                                     seconds=max(.1, held_deadline - time.monotonic()))
                    assert proof['choice'] == 'Close window' and proof['window_id'] == child and proof['presented'], proof
                    record['close_confirmation'] = proof
                    retirement_deadline = min(held_deadline, time.monotonic() + 2)
                    while time.monotonic() < retirement_deadline:
                        registry = control.rpc('windows.query', {})
                        assert isinstance(registry.get('live_registry'), list), registry
                        assert any(row['id'] == primary for row in registry['live_registry']), registry
                        if not any(row['id'] == child for row in registry['live_registry']):
                            retired_before_release = True
                            break
                        time.sleep(.025)
                    facts = markers / 'quit-observed-state.json'
                    if retired_before_release:
                        closed_state = wait_for(lambda: json.loads(facts.read_text()) if facts.exists() else {},
                            lambda value: value.get('windowAlive') is False or
                                value.get('runtimeReleased') is True and value.get('viewAlive') is False,
                            seconds=max(.1, held_deadline - time.monotonic()))
                        record['closed_observer'] = closed_state
                    else:
                        record['closed_observer'] = json.loads(facts.read_text()) if facts.exists() else None
                    record['retired_before_receipt_release'] = retired_before_release
                    assert retired_before_release, ('owner did not retire; inspect real close prompt', record['closed_observer'])
                    if destroy_owner:
                        (markers / 'destroy-retired-window').write_text(child + '\n')
                        destroyed = wait_for(lambda: json.loads(facts.read_text()) if facts.exists() else {},
                            lambda value: value.get('windowAlive') is False and value.get('window_id') == child,
                            seconds=max(.1, held_deadline - time.monotonic()))
                        record['destroyed_observer'] = destroyed
                    assert runtime.read_text().find('1.25') >= 0
                    retained = operations()
                    record['after_close_request'] = retained
                    assert retained == [running], ('admitted write lost its original pending receipt before disk outcome', retained)
                    assert time.monotonic() < held_deadline, 'close setup exceeded its held-receipt budget'
                    assert not expired_marker.exists(), 'fixture hold expired during close setup'
                    record['checks'].append('exact native close confirmation; valid primary registry; original writer stays pending before actual receipt')
                assert not expired_marker.exists(), 'fixture hold expired before writer release'
                release.touch()
                if close_owner:
                    completed = markers / 'completed-writer-facts'
                    sync_result = wait_for(lambda: completed.read_text().splitlines() if completed.exists() else [],
                                          lambda value: value == ['-1' if fault else '0', str(runtime.parent)], seconds=3)
                    record['actual_settings_directory_sync_completed'] = sync_result
                    assert not expired_marker.exists(), 'fixture hold expired instead of explicit release'
                rows = wait_for(operations, lambda rows: len(rows) == 1
                                and rows[0]['status'] in ('completed', 'failed'), seconds=8)
                terminal = rows[0]
                assert terminal['operation_id'] == operation, terminal
                assert terminal['status'] == ('failed' if failing_tail or close_owner else 'completed'), terminal
                assert terminal['window_id'] == primary and terminal['sequence'] == running['sequence'], terminal
                if not close_owner:
                    assert operations(child) == [], operations(child)
                    child_ids = {row['operation_id'] for row in control.rpc('operations.query', {'window': child})['operations']}
                    assert child_ids == child_expected, (child_expected, child_ids)
                    record['destination_operation_ids_after'] = sorted(child_ids)
                record['terminal'] = terminal
                if close_owner:
                    assert 'settings saved' in terminal['detail'] and 'not active' in terminal['detail'], terminal
                    assert 'reload required' in terminal['detail'] and 'restart' in terminal['detail'], terminal
                    assert terminal['error']['code'] == 'E_CONFIG', terminal
                if fault:
                    assert 'durability uncertainty' in terminal.get('detail', ''), terminal
                    record['checks'].append('original native receipt preserves actual save uncertainty and terminal command outcome')
                deadline = time.monotonic() + 4
                while not any(event['payload']['operation_id'] == operation for event in observed):
                    assert time.monotonic() < deadline, 'missing original completion event'
                    observed.extend(events.drain(.1))
                # Include the native lease-expiry interval so a late outcome
                # callback cannot silently produce a second completion.
                observed.extend(events.drain(2.25))
                completions = [event['payload'] for event in observed if event['payload']['operation_id'] == operation]
                assert len(completions) == 1, completions
                assert completions[0]['window_id'] == primary, completions
                assert completions[0]['operation_sequence'] == running['sequence'], completions
                assert completions[0]['status'] == terminal['status'], completions
                record['completion'] = completions[0]
                assert operations() == rows, (operations(), rows)
                record['checks'].append('one original terminal receipt and matching event after release')
                def requested():
                    return control.rpc('command.execute', command('get', {'key': key}, primary if close_owner else mutation_owner))
                if css:
                    saved = wait_for(requested, lambda value: value.get('pending') is True)
                    assert saved['pending_value']['selectors'] == ['#baseline', '#ipc-handoff'], saved
                    assert saved['value']['selectors'] == ['#baseline'], saved
                    assert saved['value']['mode'] == 'extend', saved
                    assert saved['validation']['engine_css'] == held['validation']['engine_css'], saved
                    assert saved['desired_validation']['engine_css'] == 'passed', saved
                    record['checks'].append('actual CSS receipt stays desired while admitted and after saving; independent active proof retained')
                    record['selector_query_value'] = saved['value']
                    record['active_selectors_qualified_by_this_probe'] = True
                else:
                    saved = wait_for(requested, lambda value: value.get('value') == 1.25)
                record['checks'].append('save survived; expected active value or composed reload-pending metadata retained')
                if competing_request is not None:
                    # The public error deliberately hides internal diagnostics.
                    # A successful identical request after release establishes
                    # contention causally, without exposing private messages.
                    retry = control.rpc('command.execute', competing_request)
                    retry_id = retry['operation_id']
                    assert retry_id not in (operation, competing_id), retry
                    retry_rows = wait_for(
                        lambda: control.rpc('operations.query', {'operation_id': retry_id})['operations'],
                        lambda rows: len(rows) == 1 and rows[0]['status'] in ('completed', 'failed'))
                    assert retry_rows[0]['status'] == 'completed', retry_rows
                    assert retry_rows[0]['window_id'] == primary, retry_rows
                    for owner in (primary, child):
                        wait_for(lambda: control.rpc('command.execute', command('get', {'key': key}, owner)),
                                 lambda value: value.get('value') == 1.5)
                    assert runtime.read_bytes() != held_bytes and '1.5' in runtime.read_text()
                    assert operations() == rows, (operations(), rows)
                    observed.extend(events.drain(.2))
                    assert [event['payload'] for event in observed
                            if event['payload']['operation_id'] == operation] == completions
                    record['competing_request'] = competing_request
                    record['competing_peer_retry'] = retry_rows[0]
                    record['checks'].append('identical peer edit succeeds after release; original terminal receipt and event remain immutable')
                assert not expired_marker.exists(), 'expired fixture hold cannot qualify a terminal receipt'
                record['status'] = 'passed'
                return record
            except Exception:
                record['ready_seen'] = ready.exists()
                record['runtime_files'] = [str(path.relative_to(root)) for path in root.rglob('runtime-overrides.toml')]
                if control:
                    for name, method, params in [
                            ('failure_windows', 'windows.query', {}),
                            ('failure_tabs', 'tabs.query', {}),
                            ('failure_blocking', 'blocking.status', {}),
                            ('failure_operations', 'operations.query', {'operation_id': operation} if operation else {})]:
                        try:
                            record[name] = control.rpc(method, params)
                        except Exception as error:
                            record[name] = str(error)
                print(json.dumps(record), flush=True)
                raise
            finally:
                release.touch()
                if events:
                    events.close()
                if control:
                    control.close()
                app.terminate()
                try:
                    app.wait(timeout=8)
                except subprocess.TimeoutExpired:
                    app.kill()
                    app.wait(timeout=5)
                if record['status'] != 'passed':
                    log.seek(0)
                    print(log.read()[-12000:], flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--executable', required=True, type=Path)
    parser.add_argument('--interposer', required=True, type=Path)
    parser.add_argument('--record', type=Path)
    args = parser.parse_args()
    binary, interposer = args.executable.resolve(), args.interposer.resolve()
    assert binary.is_file() and interposer.is_file()
    plugin = interposer.parent / 'plugins/generic/libcontext_quit_test_plugin.so'
    assert plugin.is_file(), 'build the owned close-confirmation plugin with the application harness'
    record = {'binary': str(binary), 'sha256': digest(binary),
              'interposer_sha256': digest(interposer), 'scope': 'real IPC/Qt offscreen; no physical-focus qualification',
              'close_plugin': {'path': str(plugin), 'sha256': digest(plugin)},
              'cases': [], 'status': 'failed'}
    try:
        cases = [(first, css, False, False) for css in (False, True) for first in (False, True)]
        cases += [(False, False, True, False), (True, False, True, False),
                  (False, False, False, True), (True, False, False, True),
                  (False, False, True, True)]
        cases = [(*case, False, False) for case in cases]
        cases += [(False, False, False, False, True, False),
                  (False, False, False, True, True, True)]
        cases = [(*case, False) for case in cases]
        cases += [(False, False, False, False, False, False, True)]
        for settings_first, css, failing_tail, fault, close_owner, destroy_owner, prewrite_failure in cases:
            record['current_case'] = {'settings_first': settings_first, 'css': css, 'failing_tail': failing_tail, 'durability_fault': fault, 'close_owner': close_owner, 'destroy_owner': destroy_owner, 'prewrite_failure': prewrite_failure}
            case = {}
            record['cases'].append(case)
            run_case(binary, interposer, settings_first, css, failing_tail, case, fault=fault, close_owner=close_owner, destroy_owner=destroy_owner, prewrite_failure=prewrite_failure)
            print(json.dumps(case), flush=True)
        record['status'] = 'passed'
        record.pop('current_case')
    finally:
        if args.record:
            args.record.write_text(json.dumps(record, indent=2) + '\n')


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Exercise native multi-window IPC ownership with disposable browser profiles."""
import argparse
import hashlib
import json
import os
import socket
import struct
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]


def file_sha256(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--executable', type=Path, default=ROOT / 'target/release/ferric-browser')
    parser.add_argument('--record', type=Path)
    args = parser.parse_args()
    binary = args.executable.resolve()
    record = {'binary': str(binary), 'sha256': file_sha256(binary), 'checks': []}
    env = os.environ.copy()
    env.update(QT_QPA_PLATFORM='offscreen', QT_QPA_PLATFORMTHEME='', QT_QUICK_BACKEND='software', QTWEBENGINE_CHROMIUM_FLAGS='--disable-gpu', FERRIC_BROWSER_ALLOW_NON_WAYLAND='1')
    with tempfile.TemporaryDirectory(prefix='ferric-owner-ipc-') as temporary:
        prefix = [str(binary), '--basedir', temporary, '--instance', 'owner-routing']
        with tempfile.TemporaryFile(mode='w+') as log:
            app = subprocess.Popen([*prefix, '--software-rendering', '--userscripts-off', 'open', 'rb://blank'], env=env, stdout=log, stderr=subprocess.STDOUT)
            connection = None
            serial = 0

            def receive(length):
                data = b''
                while len(data) < length:
                    block = connection.recv(length - len(data))
                    assert block, 'IPC connection closed'
                    data += block
                return data

            def rpc(method, params, ok=True, allow_busy=False):
                nonlocal serial
                serial += 1
                packet = json.dumps({'id': str(serial), 'method': method, 'params': params}).encode()
                connection.settimeout(30 if method == 'diagnostics.get' else 12)
                connection.sendall(struct.pack('>I', len(packet)) + packet)
                length = struct.unpack('>I', receive(4))[0]
                assert length <= 1024 * 1024, length
                result = json.loads(receive(length))
                assert result['id'] == str(serial), result
                if allow_busy and result.get('error', {}).get('code') == 'E_BUSY':
                    return {}
                if ok:
                    assert 'error' not in result, (method, params, result)
                    return result['result']
                assert 'error' in result, (method, params, result)
                return result['error']

            def wait_for(probe, accepted, seconds=10):
                deadline = time.monotonic() + seconds
                while True:
                    assert app.poll() is None, 'browser exited'
                    value = probe()
                    if accepted(value):
                        return value
                    assert time.monotonic() < deadline, value
                    time.sleep(0.05)

            def command(name, arguments=None, window=None, **context):
                context['source'] = 'cli'
                if window:
                    context['window'] = window
                return rpc('command.execute', {'command': name, 'arguments': arguments or {}, 'context': context})

            def tabs(window):
                return rpc('tabs.query', {'window': window})['tabs']

            def identity(rows):
                return [{key: row[key] for key in ['id', 'window_id', 'url', 'selected', 'pinned', 'muted']} for row in rows]

            def completed_operation(operation_id):
                rows = rpc('operations.query', {'operation_id': operation_id})['operations']
                assert len(rows) == 1, rows
                return rows[0]['status']

            try:
                deadline = time.monotonic() + 20
                while connection is None:
                    assert app.poll() is None, 'browser exited'
                    sockets = list(Path(temporary).glob('runtime/instances/*/browser.sock'))
                    if sockets:
                        connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                        connection.settimeout(12)
                        connection.connect(str(sockets[0]))
                        break
                    assert time.monotonic() < deadline, 'socket unavailable'
                    time.sleep(0.05)
                rpc('hello', {'protocol_major': 2, 'protocol_minor': 0, 'client': 'live-owner-probe'})

                def ready_windows():
                    return rpc('windows.query', {}, allow_busy=True)
                initial = wait_for(ready_windows, lambda value: len(value.get('windows', [])) == 1)
                primary = initial['windows'][0]['id']
                command('open', {'input': 'rb://blank', 'target': 'window'}, primary)
                before = wait_for(ready_windows, lambda value: len(value.get('live_registry', [])) == 2 and all((row['id'] for row in value['live_registry'])))
                record['before'] = before
                ids = [row['id'] for row in before['live_registry']]
                assert len(set(ids)) == 2, ('duplicate live window IDs', ids)
                assert len(before['windows']) == 2, before
                child = next((row for row in before['live_registry'] if row['id'] != primary))
                child_id = child['id']
                child_token = child['owner_token']
                root_before = identity(tabs(primary))
                for selector in [child_id, child_token]:
                    count = len(tabs(child_id))
                    result = command('tab-open', {'input': 'rb://blank'}, selector)
                    assert result['command_context']['window_id'] == child_id, result
                    wait_for(lambda: tabs(child_id), lambda rows: len(rows) == count + 1)
                    assert identity(tabs(primary)) == root_before, 'primary changed during child command'
                    record['checks'].append('explicit command ' + selector)
                count = len(tabs(child_id))
                action = rpc('action.execute', {'action': 'browser.tab.open', 'arguments': {'input': 'rb://blank'}, 'context': {'window': child_token, 'source': 'cli'}})
                assert action['command_context']['window_id'] == child_id, action
                wait_for(lambda: tabs(child_id), lambda rows: len(rows) == count + 1)
                operations = rpc('operations.query', {})['operations']
                assert any((row['operation_id'] == action['operation_id'] for row in operations)), operations
                record['checks'].append('action destination and global operation observation')
                focus = command('window-focus', {'id': child_id}, primary)
                outcome = wait_for(lambda: completed_operation(focus['operation_id']),
                                   lambda status: status.startswith(('completed', 'failed')))
                assert outcome.startswith('completed'), outcome
                active = wait_for(lambda: rpc('tabs.query', {'active_only': True, 'window': 'active'}), lambda value: value.get('tabs', [{}])[0].get('window_id') == child_id)
                record['active_owner_before_diagnostics'] = active
                focused = rpc('tabs.query', {'active_only': True, 'window': 'last-focused'})
                assert focused['tabs'][0]['window_id'] == child_id, focused
                record['checks'].append('actual active and last-focused child selection')
                focus = rpc('window.focus', {})
                assert focus['window_id'] == child_id, focus
                outcome = wait_for(lambda: completed_operation(focus['operation_id']),
                                   lambda status: status.startswith(('completed', 'failed')))
                assert outcome.startswith('completed'), outcome
                record['checks'].append('focus command and child focus request complete on the issuing owner')
                diagnostics = rpc('diagnostics.get', {})
                assert diagnostics['collection']['status'] == 'complete', diagnostics.get('collection')
                record['checks'].append('secondary diagnostic client receives its completed reply')
                count = len(tabs(child_id))
                command('tab-new', window=child_id)
                wait_for(lambda: tabs(child_id), lambda rows: len(rows) == count + 1)
                record['checks'].append('explicit-owner native new tab')
                snapshot = identity(tabs(child_id))
                for context in [{'window': child_id, 'profile': 'missing-profile'}, {'window': child_id, 'context': 'missing-context'}, {'window': child_id, 'source': 'ui'}]:
                    rpc('command.execute', {'command': 'open', 'arguments': {'input': 'https://must-not-open.invalid/'}, 'context': context}, ok=False)
                    assert identity(tabs(child_id)) == snapshot, 'invalid destination policy changed child'
                    assert identity(tabs(primary)) == root_before, 'invalid destination policy changed primary'
                error = rpc('command.execute', {'command': 'tab-open', 'arguments': {'input': 'rb://blank'}, 'context': {'window': 'windowid-18446744073709551615'}}, ok=False)
                assert error['code'] == 'E_STALE_TARGET', error
                record['checks'].append('destination policy and absent-owner rejection preserve both windows')
                command('open', {'input': 'rb://blank', 'target': 'private-window'}, primary)
                windows = wait_for(ready_windows, lambda value: len(value.get('windows', [])) == 3)
                private = next((row for row in windows['live_registry'] if row['private']))
                assert tabs(private['id']) == [], 'private tabs exposed by default'
                error = rpc('tabs.query', {'window': private['id'], 'include_private': True}, ok=False)
                assert error['code'] in ['E_INVALID_ARGUMENT', 'E_DENIED'], error
                private_tabs = next((row for row in windows['windows'] if row['id'] == private['id']))['tabs']
                all_tabs = rpc('tabs.query', {})['tabs']
                assert not any((row['id'] in private_tabs for row in all_tabs)), 'global query exposed private tab'
                record['checks'].append('per-owner private-query policy and global filtering')
                private_operation = command('tab-open', {'input': 'rb://blank'}, private['id'])['operation_id']
                record['private_operation'] = private_operation
                public_operations = rpc('operations.query', {})['operations']
                assert all((row['operation_id'] != private_operation for row in public_operations)), 'private operations exposed without opt-in'
                for options in [{}, {'include_stderr': True}]:
                    hidden = rpc('operations.query', {'window': private['id'], **options})['operations']
                    assert hidden == [], hidden
                rpc('operations.query', {'window': private['id'], 'include_private': True}, ok=False)
                setting = subprocess.run([*prefix, 'command', '--window', private['id'], '--', 'set', '--temp', 'ipc.private_queries=true'], env=env, capture_output=True, text=True, timeout=12)
                assert setting.returncode == 0, (setting.stdout, setting.stderr)
                visible = rpc('operations.query', {'window': private['id'], 'include_private': True})['operations']
                assert any(row['operation_id'] == private_operation for row in visible), visible
                record['checks'].append('private operations require configured explicit opt-in')
                record['after'] = rpc('windows.query', {})
                record['status'] = 'passed'
            finally:
                if record.get('status') != 'passed':
                    record['status'] = 'failed'
                if connection:
                    connection.close()
                if args.record:
                    args.record.write_text(json.dumps(record, indent=2) + '\n')
                app.terminate()
                try:
                    app.wait(timeout=8)
                except subprocess.TimeoutExpired:
                    app.kill()
                    app.wait(timeout=5)
                if record.get('status') != 'passed':
                    log.seek(0)
                    print(log.read()[-12000:], file=sys.stderr)
    print(json.dumps({'status': record['status'], 'checks': record['checks']}, indent=2))


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Real close receipts, using keys only in an owned disposable compositor."""
import argparse
import http.server
import json
import os
from pathlib import Path
import re
import socket
import struct
import subprocess
import tempfile
import threading
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--browser', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    # Never target the user's desktop. native_session.py supplies both values.
    token = os.environ['FERRIC_DISPOSABLE_BUS_OWNER']
    assert re.fullmatch('[0-9a-f]{32}', token)
    assert Path(os.environ['XDG_RUNTIME_DIR']).name == 'fn-' + token[:6]
    instance = os.environ['HYPRLAND_INSTANCE_SIGNATURE']
    args.output.mkdir(parents=True, exist_ok=False)
    facts = {'passed': False, 'cases': [], 'keys': [], 'browser_reaped': False}
    owner = None
    streams = []
    admitted_operations = set()
    received_operations = set()
    serial = 0
    with tempfile.TemporaryDirectory(prefix='ferric-owned-close-') as temporary:
        root = Path(temporary)
        env = dict(os.environ, QT_QPA_PLATFORM='wayland', QT_QUICK_BACKEND='software', QT_QPA_PLATFORMTHEME='')
        for key in ('XDG_CONFIG_HOME', 'XDG_CACHE_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME'):
            folder = root / key
            folder.mkdir(mode=0o700)
            env[key] = str(folder)
        config = root / 'config.toml'
        config.write_text("schema_version=3\n[tabs]\npresentation='windows'\n")
        prefix = [str(args.browser.resolve()), '--basedir', str(root / 'profile'), '--config', str(config), '--instance', 'owned-close']

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                title = 'Owned close child' if self.path == '/child' else 'Owned close root'
                body = ('<!doctype html><title>' + title + '</title><p>Disposable close fixture</p>').encode()
                self.send_response(200)
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *_):
                pass

        server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        base = f'http://127.0.0.1:{server.server_port}'
        log = (args.output / 'browser.log').open('wb')

        def wait(probe, seconds=20):
            deadline = time.monotonic() + seconds
            while time.monotonic() < deadline:
                assert owner.poll() is None, ('owned browser exited early', owner.returncode)
                result = probe()
                if result:
                    return result
                time.sleep(0.05)
            raise AssertionError('owned close state timed out')

        def cli(*arguments, allow_busy=False):
            result = subprocess.run(prefix + list(arguments), env=env, text=True, capture_output=True, timeout=15)
            if result.returncode != 0 and allow_busy:
                error = json.loads(result.stderr).get('error', {})
                if error.get('code') == 'E_BUSY':
                    facts['startup_busy_retries'] = facts.get('startup_busy_retries', 0) + 1
                    return None
            assert result.returncode == 0, (arguments, result.returncode, result.stdout, result.stderr)
            return json.loads(result.stdout) if result.stdout.lstrip().startswith(('{', '[')) else result.stdout

        def tabs():
            return (cli('query', 'tabs', '--format', 'json', allow_busy=True) or {}).get('tabs', [])

        def frame(stream, allow_eof=False):
            def exact(size):
                data = bytearray()
                while len(data) < size:
                    part = stream.recv(size - len(data))
                    if not part and not data and allow_eof:
                        return None
                    assert part, 'stream ended before terminal close receipt'
                    data.extend(part)
                return bytes(data)
            header = exact(4)
            if header is None:
                return None
            size, = struct.unpack('>I', header)
            assert 0 < size <= 1024 * 1024
            return json.loads(exact(size))

        def rpc(stream, method, params):
            nonlocal serial
            serial += 1
            request_id = str(serial)
            payload = json.dumps({'id': request_id, 'method': method, 'params': params}).encode()
            stream.sendall(struct.pack('>I', len(payload)) + payload)
            response = frame(stream)
            assert response.get('id') == request_id, response
            assert not response.get('error'), response
            return response['result']

        def connect():
            paths = list((root / 'profile').rglob('browser.sock'))
            assert len(paths) == 1, paths
            stream = socket.socket(socket.AF_UNIX)
            stream.settimeout(15)
            stream.connect(str(paths[0]))
            streams.append(stream)
            rpc(stream, 'hello', {'protocol_major': 3, 'protocol_minor': 0, 'client': 'owned-close-receipt'})
            return stream

        def clients():
            result = subprocess.run(['/usr/bin/hyprctl', '--instance', instance, '-j', 'clients'], env=env,
                                    text=True, capture_output=True, timeout=5)
            assert result.returncode == 0, result.stderr
            return [row for row in json.loads(result.stdout) if row['pid'] == owner.pid]

        def decision(title, key):
            rows = wait(lambda: [row for row in clients() if title in row['title']])
            assert len(rows) == 1, rows
            address = rows[0]['address']
            assert re.fullmatch('0x[0-9a-fA-F]+', address)
            # Official Lua dispatch API, with a checked owned PID/address only:
            # https://wiki.hypr.land/Configuring/Basics/Dispatchers/
            for script in [f'hl.dispatch(hl.dsp.focus({{window="address:{address}"}}))',
                           f'hl.dispatch(hl.dsp.send_shortcut({{mods="",key="{key}",window="address:{address}"}}))']:
                result = subprocess.run(['/usr/bin/hyprctl', '--instance', instance, 'eval', script], env=env,
                                        text=True, capture_output=True, timeout=5)
                assert result.returncode == 0 and 'error' not in result.stdout.lower(), (result.stdout, result.stderr)
            facts['keys'].append({'pid': owner.pid, 'address': address, 'key': key})

        def terminal(subscription, operation_id, issuing_window):
            started = time.monotonic()
            while True:
                event = frame(subscription)
                event_operation = event.get('payload', {}).get('operation_id')
                if event_operation in admitted_operations:
                    assert event_operation not in received_operations, ('duplicate terminal receipt', event)
                    received_operations.add(event_operation)
                if event.get('event_type') == 'operation.completed' and event_operation == operation_id:
                    payload = event['payload']
                    assert payload['window_id'] == issuing_window, payload
                    facts.setdefault('notifications', []).append(event)
                    facts.setdefault('latency_ms', []).append(round((time.monotonic() - started) * 1000))
                    return payload

        try:
            owner = subprocess.Popen(prefix + ['--software-rendering', '--userscripts-off', 'open', base + '/root'], env=env,
                                     stdin=subprocess.DEVNULL, stdout=log, stderr=log, start_new_session=True)
            wait(lambda: list((root / 'profile').rglob('browser.sock')))
            wait(lambda: len(tabs()) == 1 and tabs()[0]['loading'] == 'complete')
            first = tabs()[0]
            cli('command', '--window', first['window_id'], '--', 'tab-open ' + base + '/child')
            current = wait(lambda: (rows if len(rows := tabs()) == 2 and all(row['loading'] == 'complete' for row in rows) else None))
            child = next(row for row in current if row['id'] != first['id'])
            request = connect()
            subscription = connect()
            rpc(subscription, 'events.subscribe', {'event_types': ['operation.completed']})

            def close(window, continuation=None, count=None):
                params = {'command': 'tab-close', 'arguments': {} if count is None else {'count': count}, 'context': {'window': window, 'source': 'cli'}}
                if continuation:
                    params['continuation'] = continuation
                response = rpc(request, 'command.execute', params)
                assert response['pending'] is True, response
                admitted_operations.add(response['operation_id'])
                time.sleep(0.35)  # Allow the actual owned QML decision to paint.
                return response['operation_id']

            operation = close(child['window_id'])
            decision('Owned close child', 'N')
            assert terminal(subscription, operation, child['window_id'])['status'] == 'cancelled'
            assert len(tabs()) == 2
            facts['cases'].append('cancel leaves both actual pages open with one original cancelled receipt')

            operation = close(first['window_id'], count=2)
            decision('Owned close child', 'Y')
            wait(lambda: len(tabs()) == 1)
            time.sleep(0.35)
            decision('Owned close root', 'N')
            assert terminal(subscription, operation, first['window_id'])['status'] == 'failed'
            partial = cli('query', 'operations', operation, '--format', 'json')['operations']
            assert len(partial) == 1 and partial[0]['status'] == 'failed' and 'Closed 1 of 2' in partial[0]['detail'], partial
            assert len(tabs()) == 1 and tabs()[0]['id'] == first['id']
            facts['cases'].append('accepted peer close followed by issuer cancellation reports Closed 1 of 2 on the original receipt')
            cli('command', '--window', first['window_id'], '--', 'tab-open ' + base + '/child')
            current = wait(lambda: (rows if len(rows := tabs()) == 2 and all(row['loading'] == 'complete' for row in rows) else None))
            child = next(row for row in current if row['id'] != first['id'])

            operation = close(child['window_id'], [{'command': 'jseval', 'arguments': {'world': 'page', 'script': 'document.title="Owned survivor after close"'}}])
            decision('Owned close child', 'Y')
            assert terminal(subscription, operation, child['window_id'])['status'] == 'completed'
            remaining = wait(lambda: (rows if len(rows := tabs()) == 1 and rows[0]['title'] == 'Owned survivor after close' else None))
            assert remaining[0]['id'] == first['id']
            receipt = cli('query', 'operations', operation, '--window', child['window_id'], '--format', 'json')['operations']
            assert len(receipt) == 1 and receipt[0]['status'] == 'completed', receipt
            facts['cases'].append('retired secondary issuer pushes its original receipt and runs the tail on the captured survivor')

            operation = close(first['window_id'])
            decision('Owned survivor after close', 'Y')
            assert terminal(subscription, operation, first['window_id'])['status'] == 'completed'
            # Terminal delivery precedes natural last-window exit, without a
            # query/manual refresh or forced cleanup supplying completion.
            assert owner.wait(timeout=15) == 0
            subscription.settimeout(2)
            while (event := frame(subscription, allow_eof=True)) is not None:
                event_operation = event.get('payload', {}).get('operation_id')
                assert event_operation not in admitted_operations, ('duplicate final terminal receipt', event)
            facts['subscription_drained_to_eof'] = True
            facts['cases'].append('last close drains terminal notification before natural process exit')
            ids = [event['payload']['operation_id'] for event in facts['notifications']]
            assert len(ids) == len(set(ids)) == 4
            facts['passed'] = True
        finally:
            for stream in streams:
                stream.close()
            if owner is not None and owner.poll() is None:
                facts['forced_cleanup'] = True
                owner.terminate()
                try:
                    owner.wait(timeout=8)
                except subprocess.TimeoutExpired:
                    owner.kill()
                    owner.wait(timeout=5)
            facts['browser_reaped'] = owner is None or owner.poll() is not None
            facts['browser_returncode'] = None if owner is None else owner.returncode
            server.shutdown()
            server.server_close()
            log.close()
            (args.output / 'receipt.json').write_text(json.dumps(facts, indent=2) + '\n')
    print(json.dumps(facts, indent=2))


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Exercise window-mode IPC against a disposable production browser instance.

This is command/ownership coverage on Qt's offscreen platform. Native focus and
physical credential qualification belong to the separate Wayland/device suites.
"""
import argparse
import http.server
import json
import os
from pathlib import Path
import signal
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
    args.output.mkdir(parents=True, exist_ok=True)
    facts = {'success': False, 'physical_qualification': False, 'cases': []}
    with tempfile.TemporaryDirectory(prefix='ferric-window-ipc-') as temporary:
        root = Path(temporary)
        env = dict(os.environ, QT_QPA_PLATFORM='offscreen', QT_QUICK_BACKEND='software',
                   QT_QPA_PLATFORMTHEME='', QTWEBENGINE_CHROMIUM_FLAGS='--disable-gpu')
        for key in ('XDG_RUNTIME_DIR', 'XDG_CONFIG_HOME', 'XDG_CACHE_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME'):
            directory = root / key
            directory.mkdir(mode=0o700)
            env[key] = str(directory)
        config = root / 'config.toml'
        config.write_text("schema_version=3\n[tabs]\npresentation='windows'\n")
        (root / 'contexts.toml').write_text("schema_version=3\n[[contexts]]\nname='defined'\nlabel='Defined'\nprofile='default'\n")
        prefix = [str(args.browser.resolve()), '--basedir', str(root / 'profile'),
                  '--config', str(config), '--instance', 'owned-window-commands']

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                body = b'<!doctype html><title>Owned window IPC</title><input id="draft">'
                self.send_response(200)
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *_):
                pass

        server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        base = f'http://127.0.0.1:{server.server_port}'
        owner = None
        log = (args.output / 'browser.log').open('wb')

        def cli(*arguments):
            result = subprocess.run(prefix + list(arguments), env=env, text=True,
                                    capture_output=True, timeout=20)
            assert result.returncode == 0, (arguments, result.returncode, result.stdout, result.stderr)
            return json.loads(result.stdout) if result.stdout.lstrip().startswith(('{', '[')) else result.stdout

        def wait(check):
            deadline = time.monotonic() + 30
            while time.monotonic() < deadline:
                assert owner.poll() is None, ('owned browser exited', owner.returncode)
                value = check()
                if value:
                    return value
                time.sleep(0.05)
            facts['diagnostic_operations'] = cli('query', 'operations', '--format', 'json')
            facts['diagnostic_tabs'] = tabs()
            raise AssertionError('owned IPC state did not reach the expected result')

        def tabs():
            result = subprocess.run(prefix + ['query', 'tabs', '--format', 'json'], env=env,
                                    text=True, capture_output=True, timeout=10)
            return json.loads(result.stdout)['tabs'] if result.returncode == 0 else []

        def complete_tabs(count):
            current = tabs()
            return current if len(current) == count and all(tab['loading'] == 'complete' for tab in current) else None

        def raw_request(params):
            # The sole browser socket is inside this fixture's private runtime.
            paths = list(root.rglob('browser.sock'))
            assert len(paths) == 1
            with socket.socket(socket.AF_UNIX) as stream:
                stream.settimeout(10)
                stream.connect(str(paths[0]))

                def exchange(method, params):
                    payload = json.dumps({'id': method, 'method': method, 'params': params}).encode()
                    stream.sendall(struct.pack('>I', len(payload)) + payload)

                    def read_exact(size):
                        data = bytearray()
                        while len(data) < size:
                            chunk = stream.recv(size - len(data))
                            assert chunk, 'IPC closed before response'
                            data.extend(chunk)
                        return bytes(data)
                    length, = struct.unpack('>I', read_exact(4))
                    assert 0 < length <= 1024 * 1024
                    return json.loads(read_exact(length))
                hello = exchange('hello', {'protocol_major': 3, 'protocol_minor': 0, 'client': 'owned-window-test'})
                assert not hello.get('error'), hello
                return exchange('command.execute', params)

        try:
            owner = subprocess.Popen(prefix + ['--software-rendering', '--userscripts-off', 'open', base + '/root'],
                                     env=env, stdin=subprocess.DEVNULL, stdout=log, stderr=log, start_new_session=True)
            first = wait(lambda: complete_tabs(1))[0]
            source = first['window_id']
            cli('command', '--window', source, '--', 'tab-open ' + base + '/child')
            current = wait(lambda: complete_tabs(2))
            child = next(tab for tab in current if tab['id'] != first['id'])
            assert child['window_id'] != source
            facts['cases'].append('typed page-window creation')
            before = sorted((tab['id'], tab['url'], tab['window_id'], tab.get('context_name')) for tab in tabs())
            rejected = raw_request({'command': 'tab-next', 'arguments': {},
                                    'context': {'window': source, 'context': 'defined', 'source': 'cli'}})
            assert rejected.get('error'), rejected
            assert 'page selection' in rejected['error'].get('details', {}).get('diagnostic_context', '').lower(), rejected
            assert sorted((tab['id'], tab['url'], tab['window_id'], tab.get('context_name')) for tab in tabs()) == before
            facts['cases'].append('IPC context selection rejects without mutation')
            focused = raw_request({'command': 'tab-focus', 'arguments': {'id': child['id']},
                                   'context': {'window': source, 'source': 'cli'}})
            assert not focused.get('error'), focused
            assert focused['result']['window_id'] == child['window_id'], focused
            facts['cases'].append('stable page ID resolves actual peer owner')
            operation_id = focused['result']['operation_id']

            def completed_handoff():
                records = cli('query', 'operations', operation_id, '--format', 'json')['operations']
                return next((record for record in records if record['status'] in ('completed', 'failed', 'cancelled')), None)
            terminal = wait(completed_handoff)
            assert terminal['status'] == 'completed', terminal
            assert terminal['detail'] in ('activated', 'activation outcome unknown'), terminal
            facts['cases'].append('operation completion comes from native activation receipt')
            # Literal CLI chains must cross native owners in one admission.
            # Independent requests would execute this script back on source.
            cli('command', '--window', source, '--',
                'tab-focus ' + child['id'] + ' ;; jseval --world page document.title=12345')
            changed = wait(lambda: next((tab for tab in tabs()
                                        if tab['id'] == child['id'] and tab['title'] == '12345'), None))
            assert changed['window_id'] == child['window_id']
            assert next(tab for tab in tabs() if tab['id'] == first['id'])['url'] == first['url']
            facts['cases'].append('literal CLI continuation executes on the selected native owner')
            before = sorted((tab['id'], tab['url'], tab['pinned']) for tab in tabs())
            rejected = raw_request({'command': 'tab-pin', 'arguments': {},
                                    'context': {'window': source, 'source': 'cli'},
                                    'continuation': [{'command': 'reload', 'arguments': {'unexpected': True}}]})
            assert rejected.get('error'), rejected
            assert sorted((tab['id'], tab['url'], tab['pinned']) for tab in tabs()) == before
            facts['cases'].append('invalid continuation rejects the complete envelope before its first mutation')
            rejected = raw_request({'command': 'tab-pin', 'arguments': {},
                                    'context': {'window': source, 'source': 'cli'},
                                    'continuation': [{'command': 'target-search', 'arguments': {}}]})
            assert rejected.get('error', {}).get('code') == 'E_DENIED', rejected
            assert sorted((tab['id'], tab['url'], tab['pinned']) for tab in tabs()) == before
            facts['cases'].append('local-only continuation rejects before any mutation with the denied category')
            multi = raw_request({'command': 'tab-focus', 'arguments': {'id': child['id']},
                                 'context': {'window': source, 'source': 'cli'},
                                 'continuation': [
                                     {'command': 'tab-focus', 'arguments': {'id': first['id']}},
                                     *[{'command': 'jseval', 'arguments': {'world': 'page', 'script': script}}
                                       for script in ('window.multiHop=1', 'window.multiHop+=1', 'document.title=12348')]]})
            assert not multi.get('error'), multi
            operation_id = multi['result']['operation_id']
            wait(lambda: next((tab for tab in tabs() if tab['id'] == first['id'] and tab['title'] == '12348'), None))
            terminal = wait(completed_handoff)
            assert terminal['status'] == 'completed', terminal
            facts['cases'].append('one original operation completes after a multi-hop continuation reaches its final owner')
            failed = raw_request({'command': 'tab-focus', 'arguments': {'id': child['id']},
                                  'context': {'window': source, 'source': 'cli'},
                                  'continuation': [{'command': 'tab-select', 'arguments': {'selector': '999999'}}]})
            assert not failed.get('error'), failed
            operation_id = failed['result']['operation_id']
            terminal = wait(completed_handoff)
            assert terminal['status'] == 'failed', terminal
            facts['cases'].append('later selection failure fails the original operation')
            before = sorted((tab['id'], tab['title'], tab['url']) for tab in tabs())
            rejected = raw_request({'command': 'tab-next', 'arguments': {},
                                    'context': {'window': source, 'source': 'cli'},
                                    'continuation': [{'command': 'repeat', 'arguments': {}},
                                                     {'command': 'jseval', 'arguments': {'world': 'page', 'script': 'document.title=99999'}}]})
            assert rejected.get('error', {}).get('code') == 'E_UNSUPPORTED', rejected
            assert sorted((tab['id'], tab['title'], tab['url']) for tab in tabs()) == before
            facts['cases'].append('unsupported nested workflow rejects before selection or document mutation')
            pinned = raw_request({'command': 'tab-pin', 'arguments': {'id': child['id'], 'state': 'on'},
                                  'context': {'window': source, 'source': 'cli'}})
            assert not pinned.get('error'), pinned
            assert pinned['result']['window_id'] == child['window_id'], pinned
            operation_id = pinned['result']['operation_id']
            wait(lambda: next((tab for tab in tabs() if tab['id'] == child['id'] and tab['pinned']), None))
            terminal = wait(completed_handoff)
            assert terminal['status'] == 'completed' and terminal['detail'] == 'page state updated', terminal
            facts['cases'].append('pin changes the actual peer owner and completes after its state update')
            muted = raw_request({'command': 'tab-mute', 'arguments': {'id': child['id'], 'state': 'on'},
                                 'context': {'window': source, 'source': 'cli'},
                                 'continuation': [{'command': 'jseval', 'arguments': {'world': 'page', 'script': 'document.title=23456'}}]})
            assert not muted.get('error'), muted
            operation_id = muted['result']['operation_id']
            wait(lambda: next((tab for tab in tabs() if tab['id'] == first['id'] and tab['title'] == '23456'), None))
            assert next(tab for tab in tabs() if tab['id'] == child['id'])['muted']
            assert next(tab for tab in tabs() if tab['id'] == child['id'])['title'] == '12345'
            assert wait(completed_handoff)['status'] == 'completed'
            facts['cases'].append('remote mute resumes the document continuation on its issuing owner')
            selected_mutation = raw_request({'command': 'tab-focus', 'arguments': {'id': child['id']},
                                             'context': {'window': source, 'source': 'cli'},
                                             'continuation': [
                                                 {'command': 'tab-pin', 'arguments': {'id': first['id'], 'state': 'on'}},
                                                 {'command': 'jseval', 'arguments': {'world': 'page', 'script': 'document.title=23457'}}]})
            assert not selected_mutation.get('error'), selected_mutation
            operation_id = selected_mutation['result']['operation_id']
            wait(lambda: next((tab for tab in tabs() if tab['id'] == child['id'] and tab['title'] == '23457'), None))
            assert next(tab for tab in tabs() if tab['id'] == first['id'])['pinned']
            assert next(tab for tab in tabs() if tab['id'] == first['id'])['title'] == '23456'
            terminal = wait(completed_handoff)
            assert terminal['status'] == 'completed' and terminal['detail'] in ('activated', 'activation outcome unknown'), terminal
            facts['cases'].append('selection and remote mutation share one final-owner activation receipt')
            cli('command', '--window', source, '--', 'open --target window ' + base + '/standalone')
            standalone = next(tab for tab in wait(lambda: complete_tabs(3)) if tab['id'] not in (first['id'], child['id']))
            rejected = raw_request({'command': 'tab-pin', 'arguments': {'id': standalone['id'], 'state': 'on'},
                                    'context': {'window': source, 'source': 'cli'}})
            assert rejected.get('error'), rejected
            assert not next(tab for tab in tabs() if tab['id'] == standalone['id'])['pinned']
            facts['cases'].append('peer mutation excludes a standalone window outside the current logical group')
            facts['success'] = True
        finally:
            if owner is not None:
                if owner.poll() is None:
                    os.killpg(owner.pid, signal.SIGTERM)
                try:
                    owner.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    os.killpg(owner.pid, signal.SIGKILL)
                    owner.wait(timeout=10)
                facts['owned_process_reaped'] = True
            log.close()
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)
            (args.output / 'facts.json').write_text(json.dumps(facts, indent=2) + '\n')
    print(json.dumps(facts))


if __name__ == '__main__':
    main()

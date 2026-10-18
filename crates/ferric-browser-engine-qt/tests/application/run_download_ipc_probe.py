#!/usr/bin/env python3
"""Verify live Downloads, registered actions and private policy through public IPC."""
import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import hashlib
import json
import os
from pathlib import Path
import select
import signal
import socket
import struct
import sqlite3
import subprocess
import threading
import tempfile
import time
import uuid

TOTAL = 1024 * 1024
FIRST = 256 * 1024


def receive(stream, count):
    value = bytearray()
    while len(value) < count:
        part = stream.recv(count - len(value))
        if not part:
            raise RuntimeError('Owned event endpoint closed before replying')
        value.extend(part)
    return value


def read_frame(stream):
    length = struct.unpack('>I', receive(stream, 4))[0]
    assert 0 < length <= 1024 * 1024, length
    return json.loads(receive(stream, length))


def rpc(stream, method, params):
    request_id = 'owned-download-' + uuid.uuid4().hex
    payload = json.dumps({'id': request_id, 'method': method, 'params': params}).encode()
    stream.sendall(struct.pack('>I', len(payload)) + payload)
    value = read_frame(stream)
    assert value['id'] == request_id, value
    return value


def connect(endpoint):
    stream = socket.socket(socket.AF_UNIX)
    stream.settimeout(12)
    stream.connect(str(endpoint))
    hello = rpc(stream, 'hello', {'protocol_major': 2, 'protocol_minor': 0,
                                   'client': 'ferric-owned-download-boundary'})
    assert 'error' not in hello, hello
    return stream


class Events:
    def __init__(self, endpoint):
        self.stream = connect(endpoint)
        subscribed = rpc(self.stream, 'events.subscribe', {'event_types': ['download.changed']})
        assert subscribed.get('result', {}).get('status') == 'subscribed', subscribed
        self.values, self.errors = [], []
        self.stop = threading.Event()
        self.thread = threading.Thread(target=self.collect)
        self.thread.start()

    def collect(self):
        try:
            while not self.stop.is_set():
                if select.select([self.stream], [], [], .2)[0]:
                    value = read_frame(self.stream)
                    assert value.get('event_type') == 'download.changed', value
                    if self.values:
                        assert value['instance_id'] == self.values[-1]['instance_id'], value
                        assert value['sequence'] > self.values[-1]['sequence'], value
                    self.values.append(value)
        except BaseException as error:
            if not self.stop.is_set():
                self.errors.append(repr(error))

    def close(self):
        self.stop.set()
        self.stream.shutdown(socket.SHUT_RDWR)
        self.thread.join(timeout=3)
        assert not self.thread.is_alive(), 'Owned subscriber did not stop'
        self.stream.close()


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        name = self.path.removeprefix('/')
        if name not in self.server.transfers:
            self.send_error(404)
            return
        transfer = self.server.transfers[name]
        try:
            if name == 'request-cancelled':
                transfer['started'].set()
                deadline = time.monotonic() + 30
                while not transfer['release'].is_set():
                    if select.select([self.connection], [], [], 0.05)[0]:
                        if not self.connection.recv(1, socket.MSG_PEEK):
                            transfer['disconnected'] = True
                            return
                    assert time.monotonic() < deadline, 'Owned before-headers request was not released'
            self.send_response(200)
            # Preserve binary responses for ordinary native downloads while the
            # explicit stream also supports renderable text without navigation.
            self.send_header('Content-Type', 'text/plain' if name == 'request-complete' else 'application/octet-stream')
            if not name.startswith('request-'):
                self.send_header('Content-Disposition', f'attachment; filename="{name}.bin"')
            self.send_header('Content-Length', str(TOTAL))
            self.end_headers()
            self.wfile.write(b'x' * FIRST)
            self.wfile.flush()
            transfer['started'].set()
            assert transfer['release'].wait(30), 'Owned transfer was not released'
            self.wfile.write(b'x' * (TOTAL - FIRST))
            self.wfile.flush()
        except (BrokenPipeError, ConnectionResetError):
            transfer['disconnected'] = True
        except BaseException as error:
            transfer['error'] = repr(error)
        finally:
            transfer['done'].set()

    def log_message(self, *args):
        pass


def run_case(executable, base, artifacts, enabled):
    base.mkdir()
    artifacts.mkdir()
    env = os.environ.copy()
    env.update(QT_QPA_PLATFORM='offscreen', QT_QPA_PLATFORMTHEME='', QT_QUICK_BACKEND='software',
               QTWEBENGINE_CHROMIUM_FLAGS='--disable-gpu', FERRIC_BROWSER_ALLOW_NON_WAYLAND='1')
    for key, folder in [('XDG_CONFIG_HOME', 'config'), ('XDG_CACHE_HOME', 'cache'),
                        ('XDG_DATA_HOME', 'data'), ('XDG_STATE_HOME', 'state'), ('XDG_RUNTIME_DIR', 'runtime')]:
        path = base / folder
        path.mkdir(mode=0o700)
        env[key] = str(path)
    normal = base / 'normal-files'
    private = base / 'private-files'
    normal.mkdir()
    private.mkdir()
    config = base / 'browser.toml'
    config.write_text('schema_version=3\n[downloads]\nask_destination=false\ndirectory='
                      + json.dumps(str(normal)) + '\n[ipc]\nprivate_queries=' + str(enabled).lower() + '\n')
    prefix = [str(executable), '--basedir', str(base / 'browser'), '--instance', 'download-boundary',
              '--config', str(config)]
    script_root = base / 'browser' / 'config' / 'userscripts'
    script_outputs = {}
    server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    server.transfers = {name: {key: threading.Event() for key in ('started', 'release', 'done')}
                        for name in ('normal-before', 'private-transfer', 'normal-after', 'request-cancelled', 'request-complete')}
    threading.Thread(target=server.serve_forever, daemon=True).start()
    facts = {'private_queries': enabled, 'queries': [], 'checks': []}
    events = None
    history_lock = None
    log_path = artifacts / 'browser.log'
    with log_path.open('w') as log:
        app = subprocess.Popen([*prefix, '--software-rendering', '--userscripts-off', 'open', 'rb://blank'],
                               env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        def wait_for(probe, accepted, seconds=20):
            deadline = time.monotonic() + seconds
            while True:
                assert app.poll() is None, 'Owned browser exited'
                if events:
                    assert not events.errors, events.errors
                value = probe()
                if accepted(value):
                    return value
                assert time.monotonic() < deadline, value
                time.sleep(.05)

        def cli(*arguments):
            result = subprocess.run([*prefix, *arguments], env=env, capture_output=True, text=True, timeout=12)
            assert result.returncode == 0, (arguments, result.stdout, result.stderr)
            if arguments[0] == 'query':
                value = json.loads(result.stdout)
                return value.get('result', value)
            return result.stdout

        try:
            def endpoint_ready():
                candidates = list((base / 'browser').glob('runtime/instances/*/browser.sock'))
                if len(candidates) != 1:
                    return None
                with connect(candidates[0]) as stream:
                    value = rpc(stream, 'tabs.query', {})
                rows = value.get('result', {}).get('tabs', [])
                if rows and rows[0].get('url') == 'rb://blank':
                    facts['primary_window'] = rows[0]['window_id']
                    return candidates[0]
            endpoint = wait_for(endpoint_ready, bool)
            assert cli('query', 'config', 'downloads.directory', '--format', 'json')['value'] == {'path': str(normal)}
            assert cli('query', 'config', 'downloads.ask_destination', '--format', 'json')['value'] is False
            assert cli('query', 'config', 'ipc.private_queries', '--format', 'json')['value'] is enabled
            facts['normal_folder_preflight'] = str(normal)
            script_root.mkdir(parents=True, mode=0o700)
            for name, allowed in [('download_allowed', True), ('download_denied', False)]:
                output = base / (name + '.json')
                script_outputs[name] = output
                argv = ['-c', 'cat > "$1"; printf \'%s\' \'{"type":"message","text":"Fixture done"}\'', 'fixture', str(output)]
                manifest = ('schema_version=1\nname=' + json.dumps(name) + '\nexecutable="/bin/sh"\nargv='
                            + json.dumps(argv) + '\ncontext_fields=["download_id","url","private"]\n'
                            + 'allowed_results=["message"]\nallow_private=' + str(allowed).lower()
                            + '\n[[actions]]\nid=' + json.dumps('userscript.' + name + '.download')
                            + '\nsubject="download"\nverb=' + json.dumps('inspect-' + name)
                            + '\nlabel="Inspect download"\nrequired_fields=["download_id"]\n')
                path = script_root / (name + '.toml')
                path.write_text(manifest)
                path.chmod(0o600)
            events = Events(endpoint)
            def public_query(params):
                with connect(endpoint) as stream:
                    value = rpc(stream, 'downloads.query', params)
                facts['queries'].append({'params': params, 'response': value})
                return value

            def action(name, identifier, **fields):
                with connect(endpoint) as stream:
                    return rpc(stream, 'action.execute', {'action': 'userscript.' + name + '.download',
                                                         'arguments': {'download_id': identifier}, **fields})

            def script_payload(name, identifier, source, private_profile):
                output = script_outputs[name]
                def read():
                    try:
                        return json.loads(output.read_text())
                    except (FileNotFoundError, json.JSONDecodeError):
                        return None
                value = wait_for(read, bool)
                assert value['context']['download_id'] == identifier, value
                assert value['context']['url'] == f'http://127.0.0.1:{server.server_port}/' + source, value
                assert value['context']['private'] is private_profile, value
                assert value['invocation_id'].startswith('op-'), value
                facts.setdefault('script_payloads', []).append(value)
                return value

            def catalog(private_profile):
                with connect(endpoint) as stream:
                    response = rpc(stream, 'actions.query', {})
                rows = [row for row in response.get('result', {}).get('actions', [])
                        if row['id'] in ['userscript.download_allowed.download', 'userscript.download_denied.download']]
                facts.setdefault('script_catalog', []).append(rows)
                assert len(rows) == 2, response
                expected = {'userscript.download_allowed.download': 'available',
                            'userscript.download_denied.download': 'unavailable' if private_profile else 'available'}
                assert all(row['availability']['state'] == expected[row['id']] for row in rows), rows
                return rows

            def release_and_verify(name, folder):
                transfer = server.transfers[name]
                assert transfer['started'].wait(10), name
                transfer['release'].set()
                path = folder / (name + '.bin')
                wait_for(lambda: path.is_file() and path.stat().st_size == TOTAL, bool)
                assert path.read_bytes() == b'x' * TOTAL, path
                assert transfer['done'].wait(3), name
                assert 'error' not in transfer, transfer.get('error')
                facts['checks'].append(name + ':actual complete owned payload')

            def normal_transfer(name, inspect_held=False):
                before = {event['payload']['download_id'] for event in events.values}
                cli('command', '--window', facts['primary_window'], '--',
                    f'open http://127.0.0.1:{server.server_port}/{name}')
                def started():
                    return [event for event in events.values if event['payload']['download_id'] not in before
                            and event['payload'].get('bytes_received', 0) >= FIRST]
                observed = wait_for(started, bool)
                identifier = observed[0]['payload']['download_id']
                if inspect_held:
                    snapshot = public_query({})
                    facts['normal_held_live_query'] = snapshot
                    result = snapshot.get('result', {})
                    rows = [row for row in result.get('downloads', []) if row['id'] == identifier]
                    assert len(rows) == 1, snapshot
                    assert rows[0]['bytes_received'] >= FIRST and rows[0]['active'], rows
                    assert not rows[0]['file_ready'] and rows[0]['state'] == 'in-progress', rows
                    assert result['history_status'] in ('saving', 'loading', 'unavailable'), result
                    catalog(False)
                    script = action('download_allowed', identifier)
                    facts['normal_held_script_action'] = script
                    assert 'error' not in script, script
                    payload = script_payload('download_allowed', identifier, name, False)
                    assert payload['invocation_id'] == script['result']['operation_id'], (payload, script)
                    script_outputs['download_allowed'].unlink()
                    for context, code in [
                        ('invalid', 'E_INVALID_ARGUMENT'),
                        (None, 'E_INVALID_ARGUMENT'),
                        ({'extra': True}, 'E_INVALID_ARGUMENT'),
                        ({'source': 'ui'}, 'E_INVALID_ARGUMENT'),
                        ({'source': 'keyboard'}, 'E_INVALID_ARGUMENT'),
                        ({'window': 5}, 'E_INVALID_ARGUMENT'),
                        ({'window': 'windowid-18446744073709551615'}, 'E_STALE_TARGET'),
                        ({'profile': 'unopened-profile'}, 'E_DENIED'),
                        ({'context': 'unopened-context'}, 'E_INVALID_ARGUMENT'),
                    ]:
                        rejected = action('download_allowed', identifier, context=context)
                        assert rejected.get('error', {}).get('code') == code, (context, rejected)
                        facts.setdefault('rejected_action_contexts', []).append({'context': context, 'response': rejected})
                        assert not script_outputs['download_allowed'].exists()
                    scoped = action('download_allowed', identifier,
                                    context={'window': facts['primary_window'], 'profile': 'default', 'source': 'cli'})
                    assert 'error' not in scoped, scoped
                    payload = script_payload('download_allowed', identifier, name, False)
                    assert payload['invocation_id'] == scoped['result']['operation_id'], (payload, scoped)
                release_and_verify(name, normal)
                wait_for(lambda: any(event['payload']['download_id'] == identifier
                                     and event['payload'].get('state') == 'completed' for event in events.values), bool)
                return identifier

            wait_for(lambda: public_query({}), lambda value: value.get('result', {}).get('ready') is True)
            databases = list((base / 'browser').rglob('browser.sqlite'))
            assert len(databases) == 1, databases
            history_lock = sqlite3.connect(databases[0], timeout=3, isolation_level=None)
            history_lock.execute('BEGIN IMMEDIATE')
            facts['history_writer_held'] = True
            first = normal_transfer('normal-before', inspect_held=True)
            completed_query = public_query({})
            facts['normal_held_completed_query'] = completed_query
            rows = [row for row in completed_query.get('result', {}).get('downloads', []) if row['id'] == first]
            assert len(rows) == 1 and rows[0]['file_ready'] and rows[0]['state'] == 'completed', completed_query
            assert rows[0]['bytes_received'] == TOTAL and not rows[0]['active'], rows
            history_lock.execute('ROLLBACK')
            history_lock.close()
            history_lock = None
            assert cli('query', 'active-tab', '--format', 'json').get('tabs'), 'Normal owner control missing'
            public_query({})
            cli('command', 'window-new --private')
            def private_ready():
                value = cli('query', 'active-tab', '--format', 'json')
                return value.get('tabs') == []
            wait_for(private_ready, bool)
            cli('command', 'set --temp downloads.directory=' + json.dumps(str(private)))
            cli('command', 'set --temp downloads.ask_destination=false')
            folder = cli('query', 'config', 'downloads.directory', '--format', 'json')['value']
            assert folder == {'path': str(private)}, folder
            assert cli('query', 'config', 'downloads.ask_destination', '--format', 'json')['value'] is False
            facts['private_folder_preflight'] = folder
            cli('command', f'open http://127.0.0.1:{server.server_port}/private-transfer')
            release_and_verify('private-transfer', private)
            filtered = public_query({})
            assert filtered.get('result', {}).get('downloads') == [], filtered
            explicit = public_query({'include_private': True})
            if not enabled:
                assert explicit.get('error', {}).get('code') == 'E_INVALID_ARGUMENT', explicit
            else:
                result = explicit.get('result', {})
                rows = result.get('downloads', [])
                assert len(rows) == 1 and result['private'] is True, explicit
                assert rows[0]['destination'] == str(private / 'private-transfer.bin'), rows
                assert rows[0]['bytes_received'] == TOTAL and rows[0]['file_ready'], rows
                assert not rows[0]['active'] and rows[0]['state'] == 'completed', rows
                identifier = rows[0]['id']
                facts['explicit_private_id'] = identifier
                catalog(True)
                script_outputs['download_allowed'].unlink(missing_ok=True)
                foreign = action('download_allowed', first)
                assert foreign.get('error', {}).get('code') == 'E_STALE_TARGET', foreign
                assert not script_outputs['download_allowed'].exists()
                # The private window is focused. An explicit normal owner must
                # still execute with its normal session, never the focused one.
                scoped = action('download_allowed', first, context={'window': facts['primary_window']})
                assert 'error' not in scoped, scoped
                payload = script_payload('download_allowed', first, 'normal-before', False)
                assert payload['invocation_id'] == scoped['result']['operation_id'], (payload, scoped)
                script_outputs['download_allowed'].unlink()
                wrong_owner = action('download_allowed', identifier, context={'window': facts['primary_window']})
                assert wrong_owner.get('error', {}).get('code') == 'E_STALE_TARGET', wrong_owner
                assert not script_outputs['download_allowed'].exists()
                facts['explicit_normal_owner_while_private_focused'] = scoped
                script = action('download_allowed', identifier)
                assert 'error' not in script, script
                payload = script_payload('download_allowed', identifier, 'private-transfer', True)
                assert payload['invocation_id'] == script['result']['operation_id'], (payload, script)
                denied = action('download_denied', identifier)
                assert denied.get('error', {}).get('code') == 'E_DENIED', denied
                assert "isn't enabled for private or temporary profiles" in denied['error']['message'], denied
                assert not script_outputs['download_denied'].exists()
                facts['private_script_denied'] = denied
                # The command form must preserve the same typed policy failure.
                denied_command = subprocess.run([*prefix, 'command', 'action download inspect-download_denied ' + identifier],
                                                env=env, capture_output=True, text=True, timeout=12)
                assert denied_command.returncode != 0 and 'E_DENIED:' in denied_command.stderr, denied_command.stderr
                assert not script_outputs['download_denied'].exists()
                facts['private_script_command_denied'] = denied_command.stderr
            # A second normal completed notification is the ordered positive
            # barrier: all private completion work precedes this event on the
            # same authenticated subscriber, which is drained continuously.
            second = normal_transfer('normal-after')
            observed_ids = {event['payload']['download_id'] for event in events.values}
            assert observed_ids == {first, second}, (observed_ids, first, second, events.values)
            assert first != second
            assert not events.errors, events.errors
            facts['checks'].append('private transfer excluded between two delivered normal completion barriers')
            facts['private_event_barrier_events'] = events.values.copy()

            def owned_rpc(method, params):
                with connect(endpoint) as stream:
                    return rpc(stream, method, params)

            def operation_state(identifier):
                response = owned_rpc('operations.query', {'window': facts['primary_window'], 'operation_id': identifier})
                rows = response.get('result', {}).get('operations', [])
                assert len(rows) == 1, response
                return rows[0]['status']

            def source_snapshot():
                response = owned_rpc('tabs.query', {'window': facts['primary_window']})
                return [{key: row[key] for key in ('id', 'window_id', 'url', 'selected')}
                        for row in response.get('result', {}).get('tabs', [])]

            def request_download(name):
                return owned_rpc('command.execute', {'command': 'download',
                    'arguments': {'input': f'http://127.0.0.1:{server.server_port}/' + name},
                    'context': {'window': facts['primary_window']}})

            source_before = source_snapshot()
            assert source_before
            focused_before = owned_rpc('tabs.query', {'active_only': True}).get('result', {}).get('tabs')
            files_before = set(normal.iterdir())
            cancelled = request_download('request-cancelled')
            assert 'error' not in cancelled, cancelled
            cancel_operation = cancelled['result']['operation_id']
            assert server.transfers['request-cancelled']['started'].wait(10), 'Before-headers producer barrier missing'
            busy = request_download('request-complete')
            assert busy.get('error', {}).get('code') == 'E_BUSY', busy
            cancelled_result = owned_rpc('operations.cancel', {'operation_id': cancel_operation})
            assert 'error' not in cancelled_result, cancelled_result
            assert operation_state(cancel_operation) == 'cancelled'
            facts['cancelled_request_operation'] = cancelled_result
            facts['busy_download_request'] = busy
            assert server.transfers['request-cancelled']['done'].wait(5), 'Cancelled request left its TCP connection open'
            assert server.transfers['request-cancelled'].get('disconnected') is True
            facts['checks'].append('public operation cancellation aborts the actual connection before headers')
            assert set(normal.iterdir()) == files_before
            assert source_snapshot() == source_before
            completed = request_download('request-complete')
            assert 'error' not in completed, completed
            completed_operation = completed['result']['operation_id']
            assert completed_operation != cancel_operation
            wait_for(lambda: operation_state(completed_operation), lambda status: status == 'completed: download offered')
            assert server.transfers['request-complete']['started'].wait(10)
            # Admission acknowledges the offer. File completion still requires
            # the remaining payload and its download.changed event.
            assert not server.transfers['request-complete']['done'].is_set()
            server.transfers['request-complete']['release'].set()
            def new_completed():
                return [event for event in events.values if event['payload']['download_id'] not in (first, second)
                        and event['payload'].get('state') == 'completed']
            final = wait_for(new_completed, bool)
            third = final[0]['payload']['download_id']
            assert {event['payload']['download_id'] for event in events.values} == {first, second, third}, events.values
            added = set(normal.iterdir()) - files_before
            assert len(added) == 1, added
            path = added.pop()
            assert path.read_bytes() == b'x' * TOTAL
            assert source_snapshot() == source_before
            assert owned_rpc('tabs.query', {'active_only': True}).get('result', {}).get('tabs') == focused_before
            assert operation_state(cancel_operation) == 'cancelled', 'Late callbacks revived a cancelled request'
            facts['completed_download_request'] = completed
            facts['checks'].append('download-offer operation acknowledgement precedes complete payload without changing source tabs or focused owner')
            facts['events'] = events.values.copy()
            facts['status'] = 'passed'
        except BaseException as error:
            facts['status'] = 'failed'
            facts['error'] = repr(error)
            log.flush()
            print(log_path.read_text()[-6000:], flush=True)
            raise
        finally:
            if history_lock:
                history_lock.execute('ROLLBACK')
                history_lock.close()
            for transfer in server.transfers.values():
                transfer['release'].set()
            if events:
                facts['events'] = events.values.copy()
                events.close()
            if app.poll() is None:
                os.killpg(app.pid, signal.SIGTERM)
                try:
                    app.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(app.pid, signal.SIGKILL)
                    app.wait(timeout=5)
            facts['owned_process_exit'] = app.returncode
            server.shutdown()
            server.server_close()
            (artifacts / 'observations.json').write_text(json.dumps(facts, indent=2) + '\n')
    return facts


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--executable', required=True, type=Path)
    parser.add_argument('--artifacts', required=True, type=Path)
    args = parser.parse_args()
    args.artifacts.mkdir(parents=True, exist_ok=False)
    binary = args.executable.resolve()
    digest = hashlib.sha256()
    with binary.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(chunk)
    digest = digest.hexdigest()
    status = {'status': 'running', 'binary': str(binary), 'sha256': digest, 'cases': []}
    try:
        with tempfile.TemporaryDirectory(prefix='fd-') as temporary:
            for enabled in (False, True):
                tag = str(enabled).lower()
                status['cases'].append(run_case(binary, Path(temporary) / tag, args.artifacts / tag,
                                               enabled))
        status['status'] = 'passed'
    except BaseException as error:
        status['status'] = 'failed'
        status['error'] = repr(error)
        raise
    finally:
        (args.artifacts / 'status.json').write_text(json.dumps(status, indent=2) + '\n')
    print({'status': status['status']}, flush=True)


if __name__ == '__main__':
    main()

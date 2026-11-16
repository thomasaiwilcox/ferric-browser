#!/usr/bin/env python3
"""Qualify bounded responses, source history and replay through production IPC."""
import argparse
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import time

from run_settings_handoff_ipc_probe import Client, digest


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--executable', type=Path, required=True)
    parser.add_argument('--record', type=Path, required=True)
    args = parser.parse_args()
    binary = args.executable.resolve()
    record = {'status': 'failed', 'executable': str(binary), 'sha256': digest(binary), 'checks': []}
    try:
        with tempfile.TemporaryDirectory(prefix='ferric-large-inspection-') as temporary:
            root = Path(temporary)
            config = root / 'config.toml'
            (root / 'included.toml').write_text("[[site_rules]]\nid='retired'\n"
                "pattern='https://example.test/*'\n[site_rules.set]\n\"content.zoom\"=1.5\n")
            argv = ['editor'] + ['x' * 4096] * 254 + ['{file}']
            template = 'https://example.test/' + 'q' * (32 * 1024) + '?q={query}'
            config.write_text("schema_version=3\ninclude=['included.toml']\nsite_rules=[]\n"
                              "[navigation]\ndefault_search='bulk'\n[search_engines]\nbulk="
                              + json.dumps(template) + "\n[tools]\neditor="
                              + json.dumps(argv) + '\n[hints.selectors.links]\nmode="replace"\nselectors=["#initial-css"]\n')
            record['configuration_bytes'] = config.stat().st_size
            env = os.environ.copy()
            env.pop('LD_PRELOAD', None)
            env.update(QT_QPA_PLATFORM='offscreen', QT_QPA_PLATFORMTHEME='', QT_QUICK_BACKEND='software',
                       QTWEBENGINE_CHROMIUM_FLAGS='--disable-gpu', FERRIC_BROWSER_ALLOW_NON_WAYLAND='1')
            with tempfile.TemporaryFile(mode='w+') as log:
                app = subprocess.Popen([str(binary), '--basedir', str(root), '--instance', 'large-inspection',
                    '--config', str(config), '--software-rendering', '--userscripts-off', 'open', 'rb://blank'],
                    env=env, stdout=log, stderr=subprocess.STDOUT)
                client = None
                try:
                    deadline = time.monotonic() + 30
                    while True:
                        assert app.poll() is None, 'fixture browser exited'
                        sockets = list(root.glob('runtime/instances/*/browser.sock'))
                        if sockets:
                            assert len(sockets) == 1
                            client = Client(sockets[0])
                            break
                        assert time.monotonic() < deadline, 'socket startup timeout'
                        time.sleep(.05)
                    while not client.rpc('windows.query', {}, allow_busy=True).get('live_registry'):
                        assert time.monotonic() < deadline, 'profile startup timeout'
                        time.sleep(.05)

                    def send(request):
                        packet = json.dumps(request).encode()
                        client.socket.sendall(struct.pack('>I', len(packet)) + packet)
                        response = client.receive()
                        assert response['id'] == request['id'], response
                        return response

                    for method, params in [
                        ('config.get', {'key': 'tools.editor'}),
                        ('config.get', {'key': 'tools.editor', 'explain': True}),
                        ('command.execute', {'command': 'get', 'arguments': {'key': 'tools.editor'},
                                             'context': {'source': 'cli'}}),
                    ]:
                        request = {'id': 'large-' + str(len(record['checks'])), 'method': method, 'params': params}
                        response = send(request)
                        error = response.get('error', {})
                        assert response.get('result') is None, 'partial large value leaked'
                        assert error.get('code') == 'E_ENGINE', response
                        assert error.get('details', {}).get('reason') == 'response_too_large', response
                        assert error['details']['limit_bytes'] == 1024 * 1024, response
                        assert error['details']['preserved'] == 'request effects have not been rolled back', response
                        assert len(json.dumps(response).encode()) < 4096
                        replay = send(request)
                        assert replay == response, ('same request was not replayed from its exact cached response', response, replay)
                        record['checks'].append({'method': method, 'explain': params.get('explain', False),
                                                 'bounded_error_and_exact_replay': True})
                    small = client.rpc('config.get', {'key': 'content.zoom'})
                    assert small['value'] == 1.0, small
                    assert 'explanation' not in small, small
                    record['checks'].append('same connection continues to deliver complete small settings')
                    detailed = client.rpc('config.get', {'key': 'content.zoom',
                        'url': 'https://example.test/retired', 'explain': True})
                    assert detailed['value'] == 1.0 and detailed['desired_value'] == 1.0, detailed
                    selected = detailed['explanation']['selection']
                    assert selected['survivors'] == [] and selected['shadowed'] == [], selected
                    assert selected['authored_array']['document_id'] == 1, selected
                    assert len(selected['replaced_arrays']) == 1, selected
                    retired = selected['replaced_arrays'][0]
                    assert retired['reason'] == 'array-replaced', retired
                    assert retired['array']['document_id'] == 0 and retired['replaced_by']['document_id'] == 1, retired
                    assert 'rule' not in retired['replaced_by'], retired
                    assert len(retired['rules']) == 1, retired
                    assert retired['rules'][0]['raw']['id'] == 'retired', retired
                    assert retired['rules'][0]['raw']['set']['content.zoom'] == 1.5, retired
                    assert retired['rules'][0]['origin']['rule']['id'] == 'retired', retired
                    assert str(root) not in json.dumps(detailed), 'source filesystem path leaked'
                    record['checks'].append('opt-in explanation retains cleared include rules without matching them or exposing source paths')
                    initial_css = client.rpc('config.get', {'key': 'hints.selectors.links'})
                    assert initial_css['value']['selectors'] == ['#initial-css'], initial_css
                    for boundary in ('validation', 'desired_validation'):
                        assert initial_css[boundary]['engine_css'] == 'passed', initial_css
                        assert initial_css[boundary]['live_document'] == 'unknown', initial_css
                    absent_css = client.rpc('config.get', {'key': 'hints.selectors.buttons'})
                    assert absent_css['value'] is None, absent_css
                    assert absent_css['validation']['engine_css'] == 'unknown', absent_css
                    assert absent_css['desired_validation']['engine_css'] == 'unknown', absent_css
                    assert small['validation']['engine_css'] == 'unknown', small
                    record['checks'].append('actual startup parser callback certifies only nonempty selector leaves; ordinary/absent and live-document stages remain unknown')
                    before_operations = {row['operation_id'] for row in client.rpc('operations.query', {})['operations']}
                    subscribe_id = 'subscribe-mixed'
                    packet = json.dumps({'id': subscribe_id, 'method': 'events.subscribe',
                        'params': {'event_types': ['operation.completed']}}).encode()
                    client.socket.sendall(struct.pack('>I', len(packet)) + packet)
                    notifications = {}
                    while True:
                        packet = client.receive()
                        if packet.get('id') == subscribe_id:
                            assert 'error' not in packet, packet
                            break
                        assert packet.get('event_type') == 'operation.completed', packet
                        notifications.setdefault(packet['payload']['operation_id'], []).append(packet)
                    before_notifications = set(notifications)
                    commands = set()
                    pending_ids = set()
                    for index in range(16):
                        request_id = 'mixed-' + str(index)
                        if index % 2 == 0:
                            method = 'command.execute'
                            params = {'command': 'get', 'arguments': {'key': 'search_engines.bulk'},
                                      'context': {'source': 'cli'}}
                            commands.add(request_id)
                        else:
                            method = 'config.get'
                            params = {'key': 'search_engines.bulk'}
                        packet = json.dumps({'id': request_id, 'method': method, 'params': params}).encode()
                        client.socket.sendall(struct.pack('>I', len(packet)) + packet)
                        pending_ids.add(request_id)
                    replies = {}
                    deadline = time.monotonic() + 30
                    while True:
                        assert time.monotonic() < deadline, 'mixed frame delivery timeout'
                        packet = client.receive()
                        if 'id' in packet:
                            request_id = packet['id']
                            assert request_id in pending_ids and request_id not in replies, packet
                            assert 'error' not in packet, packet
                            assert packet['result']['value'] == template, 'response body interleaved or truncated'
                            assert packet['result']['desired_value'] == template, 'desired body interleaved or truncated'
                            replies[request_id] = packet['result']
                        else:
                            assert packet.get('event_type') == 'operation.completed', packet
                            operation = packet['payload']['operation_id']
                            notifications.setdefault(operation, []).append(packet)
                        if len(replies) == len(pending_ids):
                            operation_ids = {replies[request_id]['operation_id'] for request_id in commands}
                            assert len(operation_ids) == len(commands) == 8, replies
                            assert not operation_ids.intersection(before_operations | before_notifications), replies
                            if operation_ids.issubset(notifications):
                                break
                    # Include another complete writer deadline and the normal
                    # publication interval, rather than ignoring queued duplicates.
                    for packet in client.drain(2.25):
                        notifications.setdefault(packet['payload']['operation_id'], []).append(packet)
                    for operation in operation_ids:
                        assert len(notifications[operation]) == 1, notifications[operation]
                        assert notifications[operation][0]['payload']['status'] == 'completed', notifications[operation]
                    record['checks'].append('16 pipelined large replies and 8 operation events retain intact framing on one connection')
                    record['status'] = 'passed'
                finally:
                    if client:
                        client.close()
                    app.terminate()
                    try:
                        app.wait(timeout=8)
                    except subprocess.TimeoutExpired:
                        app.kill()
                        app.wait(timeout=5)
                    if record['status'] != 'passed':
                        log.seek(0)
                        record['browser_log_tail'] = log.read()[-6000:]
    finally:
        args.record.write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps(record), flush=True)


if __name__ == '__main__':
    main()

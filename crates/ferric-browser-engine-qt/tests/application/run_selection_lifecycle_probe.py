#!/usr/bin/env python3
"""Owned IPC regression for selection handoff, cancellation and replacement."""
import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import threading
import time

PAGE = b'''<!doctype html><p id=s tabindex=0>Selected fixture text</p><script>
function select(){const e=document.getElementById('s');e.focus();const r=document.createRange();r.selectNodeContents(e);getSelection().removeAllRanges();getSelection().addRange(r)}
addEventListener('load',()=>{select();document.title='Selection ready';const selectionTimer=setInterval(select,100);const poll=setInterval(()=>fetch('/control').then(r=>r.text()).then(v=>{if(v==='freeze'){clearInterval(selectionTimer);fetch('/frozen');return}if(v!=='hold')return;clearInterval(poll);const x=new XMLHttpRequest();x.open('GET','/hold',false);x.send();fetch('/drained');document.title='Barrier drained'}),50)})</script>'''


def run_case(executable, case):
    freeze, frozen, armed, entered, released, drained = (threading.Event() for _ in range(6))
    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            body = PAGE
            if self.path == '/control': body = b'hold' if armed.is_set() else b'freeze' if freeze.is_set() else b'wait'
            if self.path == '/frozen':
                frozen.set()
                body = b'acknowledged'
            if self.path == '/drained':
                drained.set()
                body = b'acknowledged'
            if self.path == '/hold':
                entered.set()
                if not released.wait(10):
                    self.send_error(504)
                    return
                body = b'released'
            self.send_response(200)
            self.send_header('Content-Type', 'text/html; charset=utf-8')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        def log_message(self, *unused):
            pass
    server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        with tempfile.TemporaryDirectory(prefix='ferric-selection-lifecycle-') as temporary:
            root = Path(temporary)
            output = root / 'result.txt'
            config = root / 'config.toml'
            argv = ['-c', 'printf \'%s\' "$1" > "$2"', 'fixture', '{selection}', str(output)]
            config.write_text('schema_version=3\n[action_targets.fixture]\nexecutable="/bin/sh"\nargv=' + json.dumps(argv) + '\nsubject_types=["selection"]\ndetach=false\nallow_private=false\n')
            env = os.environ.copy()
            env.update(QT_QPA_PLATFORM='offscreen', QT_QPA_PLATFORMTHEME='', QT_QUICK_BACKEND='software', QTWEBENGINE_CHROMIUM_FLAGS='--disable-gpu', FERRIC_BROWSER_ALLOW_NON_WAYLAND='1', XDG_CONFIG_HOME=str(root/'config'), XDG_CACHE_HOME=str(root/'cache'), XDG_DATA_HOME=str(root/'data'))
            prefix = [str(executable.resolve()), '--basedir', str(root/'browser'), '--instance', 'selection-lifecycle', '--config', str(config)]
            connection = None
            serial = 0
            with tempfile.TemporaryFile(mode='w+') as log:
                app = subprocess.Popen([*prefix, '--software-rendering', '--userscripts-off', 'open', f'http://127.0.0.1:{server.server_port}/'], env=env, stdout=log, stderr=log)
                def receive(length):
                    data = b''
                    while len(data) < length:
                        block = connection.recv(length-len(data))
                        assert block, 'IPC disconnected'
                        data += block
                    return data
                def rpc(method, params):
                    nonlocal serial
                    serial += 1
                    packet = json.dumps(dict(id=str(serial), method=method, params=params)).encode()
                    connection.sendall(struct.pack('>I', len(packet))+packet)
                    length = struct.unpack('>I', receive(4))[0]
                    assert length <= 1024*1024
                    response = json.loads(receive(length))
                    assert response['id'] == str(serial), response
                    if method == 'tabs.query' and response.get('error', {}).get('code') == 'E_BUSY': return {}
                    assert 'error' not in response, response
                    return response['result']
                def command(name, arguments):
                    return rpc('command.execute', dict(command=name, arguments=arguments, context=dict(source='cli')))
                def wait_for(probe, accepted, timeout=10):
                    deadline = time.monotonic()+timeout
                    while True:
                        assert app.poll() is None, 'browser exited'
                        value = probe()
                        if accepted(value): return value
                        assert time.monotonic() < deadline, value
                        time.sleep(.02)
                def operations():
                    return rpc('operations.query', {})['operations']
                try:
                    deadline = time.monotonic()+20
                    while connection is None:
                        paths = list((root/'browser/runtime/instances').glob('*/browser.sock'))
                        if paths:
                            connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                            connection.settimeout(12)
                            connection.connect(str(paths[0]))
                            break
                        assert app.poll() is None and time.monotonic() < deadline
                        time.sleep(.02)
                    rpc('hello', dict(protocol_major=3, protocol_minor=0, client='selection-lifecycle-probe'))
                    wait_for(lambda: rpc('tabs.query', {'active_only': True}), lambda value: any(row.get('title') == 'Selection ready' for row in value.get('tabs', [])))
                    command('mode-enter', {'mode': 'insert'})
                    # Stop the page's startup focus assistance, then establish
                    # readiness through a real successful selection action. No
                    # selection mutation can race the held-reply scenarios.
                    freeze.set()
                    assert frozen.wait(3), 'selection fixture did not settle'
                    deadline = time.monotonic()+4
                    while True:
                        output.unlink(missing_ok=True)
                        warmup_id = command('send', {'target':'fixture','selection':True,'send_subject':'selection'})['operation_id']
                        rows = wait_for(operations, lambda rows: any(row['operation_id']==warmup_id and (row['status']=='completed' or row['status'].startswith('failed')) for row in rows), timeout=3)
                        outcome = next(row for row in rows if row['operation_id']==warmup_id)
                        if outcome['status']=='completed':
                            assert output.read_text()=='Selected fixture text'
                            output.unlink()
                            break
                        assert time.monotonic() < deadline, ('selection source did not become ready', outcome)
                    if case == 'identity':
                        before = {row['operation_id'] for row in operations()}
                        result = command('spawn', {'argv': ['/bin/sh', *argv]})
                        operation_id = result['operation_id']
                        rows = wait_for(operations, lambda rows: output.exists() and any(row['status']=='completed' for row in rows if row['operation_id'] not in before))
                        created = [row for row in rows if row['operation_id'] not in before]
                        assert len(created)==1 and created[0]['operation_id']==operation_id and created[0]['status']=='completed', ('spawn selection split the operation', result, created)
                        assert output.read_text()=='Selected fixture text'
                    else:
                        armed.set()
                        assert entered.wait(3), 'renderer barrier did not enter'
                        first = command('send', {'target':'fixture','selection':True,'send_subject':'selection'})
                        first_id = first['operation_id']
                        assert next(row for row in operations() if row['operation_id']==first_id)['status']=='running'
                        if case == 'cancel':
                            rpc('operations.cancel', {'operation_id':first_id})
                            second_id = None
                        else:
                            second_id = command('send', {'target':'fixture','selection':True,'send_subject':'selection'})['operation_id']
                            assert second_id != first_id
                            assert next(row for row in operations() if row['operation_id']==first_id)['status']=='cancelled', 'replaced selection stayed pending'
                        released.set()
                        assert drained.wait(3), 'renderer did not acknowledge barrier release'
                        if second_id:
                            wait_for(operations, lambda rows: any(row['operation_id']==second_id and row['status']=='completed' for row in rows))
                            assert output.read_text()=='Selected fixture text'
                        else:
                            deadline = time.monotonic()+2.2
                            while time.monotonic() < deadline:
                                assert not output.exists(), 'cancelled selection launched a child'
                                assert next(row for row in operations() if row['operation_id']==first_id)['status']=='cancelled', ('late reply revived a retired operation', operations())
                                time.sleep(.02)
                        assert next(row for row in operations() if row['operation_id']==first_id)['status']=='cancelled', ('late reply revived a retired operation', operations())
                    print(case, 'passed', flush=True)
                except BaseException:
                    log.seek(0)
                    print(log.read(), flush=True)
                    raise
                finally:
                    released.set()
                    if connection: connection.close()
                    app.terminate()
                    try: app.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        app.kill(); app.wait(timeout=5)
    finally:
        released.set()
        server.shutdown()
        server.server_close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--executable', type=Path, required=True)
    parser.add_argument('--case', choices=('all', 'identity', 'cancel', 'replace'), default='all')
    args = parser.parse_args()
    for case in ('identity', 'cancel', 'replace') if args.case == 'all' else (args.case,):
        run_case(args.executable, case)


if __name__ == '__main__':
    main()

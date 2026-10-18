#!/usr/bin/env python3
"""Qualify selection-action operation identity through the public CLI/IPC."""
import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import subprocess
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[4]
PAGE = b"""<!doctype html><p id=selection tabindex=0>Selected fixture text</p>
<script>addEventListener('load',()=>{const e=document.getElementById('selection');
e.focus();const r=document.createRange();r.selectNodeContents(e);
getSelection().removeAllRanges();getSelection().addRange(r);document.title='Selection ready';const focusTimer=setInterval(()=>{e.focus();getSelection().removeAllRanges();getSelection().addRange(r)},100);const settleTimer=setInterval(()=>fetch('/selection-control').then(r=>r.text()).then(value=>{if(value!=='freeze')return;clearInterval(focusTimer);clearInterval(settleTimer);fetch('/selection-frozen')}),50)})</script>"""


class PageHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        body = PAGE
        if self.path == '/selection-control':
            body = b'freeze' if self.server.freeze.is_set() else b'wait'
        elif self.path == '/selection-frozen':
            self.server.frozen.set()
            body = b'acknowledged'
        self.send_response(200)
        self.send_header('Content-Type', 'text/html; charset=utf-8')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--executable', type=Path, default=ROOT / 'target/release/ferric-browser')
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='ferric-action-ipc-') as temporary:
        root = Path(temporary)
        env = os.environ.copy()
        env.update(QT_QPA_PLATFORM='offscreen', QT_QPA_PLATFORMTHEME='', QT_QUICK_BACKEND='software',
                   QTWEBENGINE_CHROMIUM_FLAGS='--disable-gpu', FERRIC_BROWSER_ALLOW_NON_WAYLAND='1',
                   XDG_CONFIG_HOME=str(root / 'config'), XDG_CACHE_HOME=str(root / 'cache'),
                   XDG_DATA_HOME=str(root / 'data'))
        config = root / 'config.toml'
        text = 'schema_version=3\n'
        for name, detached in (('tracked', False), ('detached', True)):
            argv = ['-c', 'printf \'%s\' "$1" > "$2"; printf \'%s\' "$$" > "$3"',
                    'fixture', '{selection}', str(root / (name + '.txt')), str(root / (name + '.pid'))]
            text += (f'[action_targets.{name}]\nexecutable="/bin/sh"\nargv={json.dumps(argv)}\n'
                     f'subject_types=["selection"]\ndetach={str(detached).lower()}\nallow_private=false\n')
        config.write_text(text)
        prefix = [str(args.executable.resolve()), '--basedir', str(root / 'browser'),
                  '--instance', 'action-feedback', '--config', str(config)]
        server = ThreadingHTTPServer(('127.0.0.1', 0), PageHandler)
        server.freeze, server.frozen = threading.Event(), threading.Event()
        threading.Thread(target=server.serve_forever, daemon=True).start()
        url = f'http://127.0.0.1:{server.server_port}/selection'
        with tempfile.TemporaryFile(mode='w+') as log:
            app = subprocess.Popen([*prefix, '--software-rendering', '--userscripts-off', 'open', url],
                                   env=env, stdout=log, stderr=log)
            def invoke(*arguments):
                result = subprocess.run([*prefix, *arguments], env=env, capture_output=True,
                                        text=True, timeout=12)
                assert result.returncode == 0, (arguments, result.stdout, result.stderr)
                if arguments[0] == 'query':
                    value = json.loads(result.stdout)
                    return value.get('result', value)
                return result.stdout
            try:
                deadline = time.monotonic() + 20
                while True:
                    assert app.poll() is None, 'browser exited during bootstrap'
                    try:
                        tabs = invoke('query', 'active-tab', '--format', 'json').get('tabs', [])
                        if tabs and tabs[0].get('url') == url and tabs[0].get('title') == 'Selection ready':
                            break
                    except (AssertionError, json.JSONDecodeError):
                        pass
                    assert time.monotonic() < deadline, 'browser did not load selection fixture'
                    time.sleep(0.05)
                invoke('command', 'mode-enter insert')
                server.freeze.set()
                assert server.frozen.wait(3), 'selection fixture did not settle'
                # Readiness is an actual successful selection action. The page
                # then remains unchanged throughout the measured operations.
                ready_deadline = time.monotonic() + 4
                while True:
                    before = {row['operation_id'] for row in invoke('query', 'operations', '--format', 'json')['operations']}
                    invoke('command', 'send tracked --selection')
                    deadline = time.monotonic() + 3
                    while True:
                        rows = [row for row in invoke('query', 'operations', '--format', 'json')['operations']
                                if row['operation_id'] not in before]
                        if rows and all(row['status'] == 'completed' or row['status'].startswith('failed') for row in rows):
                            break
                        assert time.monotonic() < deadline, ('selection readiness did not finish', rows)
                        time.sleep(0.02)
                    if len(rows) == 1 and rows[0]['status'] == 'completed':
                        assert (root / 'tracked.txt').read_text() == 'Selected fixture text'
                        break
                    assert time.monotonic() < ready_deadline, ('selection source did not become ready', rows)
                for name in ('tracked', 'detached'):
                    (root / (name + '.txt')).unlink(missing_ok=True)
                    (root / (name + '.pid')).unlink(missing_ok=True)
                    before = {row['operation_id'] for row in invoke('query', 'operations', '--format', 'json')['operations']}
                    invoke('command', f'send {name} --selection')
                    deadline = time.monotonic() + 5
                    while True:
                        assert app.poll() is None, 'browser exited during action'
                        rows = [row for row in invoke('query', 'operations', '--format', 'json')['operations']
                                if row['operation_id'] not in before]
                        assert len(rows) <= 1, ('selection handoff allocated another operation', rows)
                        if rows and rows[0]['status'] in ('completed', 'detached'):
                            assert rows[0]['status'] == ('completed' if name == 'tracked' else 'detached'), rows
                            break
                        assert time.monotonic() < deadline, ('selection action did not finish', rows)
                        time.sleep(0.05)
                    deadline = time.monotonic() + 5
                    while True:
                        try:
                            payload = (root / (name + '.txt')).read_text()
                            pid_text = (root / (name + '.pid')).read_text()
                            if payload == 'Selected fixture text' and pid_text.isdigit():
                                pid = int(pid_text)
                                break
                        except FileNotFoundError:
                            pass
                        assert time.monotonic() < deadline, ('fixture child did not write its result', name)
                        time.sleep(0.02)
                    assert pid > 1 and pid != app.pid
                    deadline = time.monotonic() + 3
                    while Path(f'/proc/{pid}').exists():
                        assert time.monotonic() < deadline, ('completed fixture child was not reaped', name, pid)
                        time.sleep(0.02)
                    print(name, 'selection retained one operation through extraction and launch; child reaped', flush=True)
            except BaseException:
                log.seek(0)
                print(log.read(), flush=True)
                raise
            finally:
                app.terminate()
                try:
                    app.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    app.kill()
                    app.wait(timeout=5)
                server.shutdown()
                server.server_close()


if __name__ == '__main__':
    main()

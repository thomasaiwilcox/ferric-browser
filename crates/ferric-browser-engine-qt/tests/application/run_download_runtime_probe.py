#!/usr/bin/env python3
"""Verify safe download rejection under an actual runtime-version mismatch."""
import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import threading
import time

from run_download_ipc_probe import connect, rpc


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == '/target':
            self.server.targets += 1
        if self.path == '/barrier' and self.server.release.is_set():
            self.server.observed.set()
        body = b"<!doctype html><title>Owned runtime source</title><p>Unchanged source</p><script>setInterval(()=>fetch('/barrier'),50)</script>"
        self.send_response(200)
        self.send_header('Content-Type', 'text/html')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        try:
            self.wfile.write(body)
        except (BrokenPipeError, ConnectionResetError):
            pass

    def log_message(self, *args):
        pass


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--executable', type=Path, required=True)
    parser.add_argument('--interposer', type=Path, required=True)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='ferric-runtime-guard-') as temporary:
        root = Path(temporary)
        env = os.environ.copy()
        for key in ('XDG_CONFIG_HOME', 'XDG_CACHE_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME', 'XDG_RUNTIME_DIR'):
            folder = root / key
            folder.mkdir(mode=0o700)
            env[key] = str(folder)
        env.update(QT_QPA_PLATFORM='offscreen', QT_QPA_PLATFORMTHEME='', QT_QUICK_BACKEND='software',
                   QTWEBENGINE_CHROMIUM_FLAGS='--disable-gpu', FERRIC_BROWSER_ALLOW_NON_WAYLAND='1',
                   LD_PRELOAD=str(args.interposer.resolve(strict=True)))
        output = root / 'downloads'
        output.mkdir()
        config = root / 'config.toml'
        config.write_text('schema_version=3\n[downloads]\nask_destination=false\ndirectory=' + json.dumps(str(output)) + '\n')
        prefix = [str(args.executable.resolve(strict=True)), '--basedir', str(root / 'browser'),
                  '--instance', 'runtime-guard', '--config', str(config)]
        server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        server.targets = 0
        server.release = threading.Event()
        server.observed = threading.Event()
        threading.Thread(target=server.serve_forever, daemon=True).start()
        source = f'http://127.0.0.1:{server.server_port}/source'
        with tempfile.TemporaryFile(mode='w+') as log:
            app = subprocess.Popen([*prefix, '--software-rendering', '--userscripts-off', 'open', source],
                                   env=env, stdout=log, stderr=log, start_new_session=True)
            def wait_for(probe, accepted, seconds=20):
                deadline = time.monotonic() + seconds
                while True:
                    assert app.poll() is None, 'Owned browser exited'
                    value = probe()
                    if accepted(value):
                        return value
                    assert time.monotonic() < deadline, value
                    time.sleep(0.05)
            try:
                def endpoint_ready():
                    values = list((root / 'browser').glob('runtime/instances/*/browser.sock'))
                    return values[0] if len(values) == 1 else None
                endpoint = wait_for(endpoint_ready, bool)
                def query(method, params):
                    with connect(endpoint) as stream:
                        return rpc(stream, method, params)
                rows = wait_for(lambda: query('tabs.query', {}).get('result', {}).get('tabs', []),
                                lambda rows: len(rows) == 1 and rows[0].get('url') == source and rows[0].get('title') == 'Owned runtime source')
                before = [{key: row[key] for key in ('id', 'window_id', 'url', 'selected')} for row in rows]
                owner = rows[0]['window_id']
                response = query('command.execute', {'command': 'download',
                    'arguments': {'input': f'http://127.0.0.1:{server.server_port}/target'},
                    'context': {'window': owner}})
                assert 'error' not in response, response
                operation = response['result']['operation_id']
                def terminal():
                    rows = query('operations.query', {'window': owner, 'operation_id': operation}).get('result', {}).get('operations', [])
                    return rows[0]['status'] if len(rows) == 1 else None
                status = wait_for(terminal, lambda value: value and value.startswith('failed'))
                server.release.set()
                assert server.observed.wait(5), 'Ordinary website did not reach the causal barrier'
                assert server.targets == 0, 'Unsupported download contacted the target'
                rows = query('tabs.query', {}).get('result', {}).get('tabs', [])
                after = [{key: row[key] for key in ('id', 'window_id', 'url', 'selected')} for row in rows]
                assert after == before, (before, after)
                assert list(output.iterdir()) == []
                downloads = query('downloads.query', {})
                assert downloads.get('result', {}).get('downloads') == [], downloads
                print(json.dumps({'status': 'passed', 'operation': status, 'target_requests': 0,
                                  'source_preserved': True, 'ordinary_page_barrier': True}), flush=True)
            except BaseException:
                log.seek(0)
                print(log.read(), flush=True)
                raise
            finally:
                if app.poll() is None:
                    os.killpg(app.pid, signal.SIGTERM)
                    try:
                        app.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        os.killpg(app.pid, signal.SIGKILL)
                        app.wait(timeout=5)
                server.shutdown()
                server.server_close()


if __name__ == '__main__':
    main()

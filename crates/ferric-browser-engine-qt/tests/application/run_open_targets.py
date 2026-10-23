#!/usr/bin/env python3
"""Exercise public CLI forwarding on an owned browser and disposable profile."""
import argparse
import hashlib
import http.server
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import threading
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--browser', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    browser = args.browser.resolve()
    digest = hashlib.sha256(browser.read_bytes()).hexdigest()
    args.output.mkdir(parents=True, exist_ok=True)
    facts = {'browser_sha256': digest, 'steps': [], 'success': False}
    requests = []
    with tempfile.TemporaryDirectory(prefix='ferric-open-') as temporary:
        root = Path(temporary)
        env = dict(os.environ, QT_QPA_PLATFORM='offscreen', QT_QUICK_BACKEND='software',
                   QT_QPA_PLATFORMTHEME='', QTWEBENGINE_CHROMIUM_FLAGS='--disable-gpu')
        for key in ('XDG_RUNTIME_DIR', 'XDG_CONFIG_HOME', 'XDG_CACHE_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME'):
            path = root / key
            path.mkdir(mode=0o700)
            env[key] = str(path)

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                requests.append(self.path)
                body = b'<!doctype html><title>Owned open target</title><input id="draft">'
                self.send_response(200)
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *_):
                pass

        server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        url = f'http://127.0.0.1:{server.server_port}'
        config_directory = root / 'user-config'
        config_directory.mkdir(parents=True)
        (config_directory / 'config.toml').write_text('schema_version=3\n')
        (config_directory / 'contexts.toml').write_text(
            'schema_version=3\n[[contexts]]\nname="routed"\nlabel="Routed"\nprofile="default"\n')
        prefix = [str(browser), '--basedir', str(root / 'profile'), '--config',
                  str(config_directory / 'config.toml'), '--instance', 'owned-open-targets']

        def cli(*arguments):
            result = subprocess.run(prefix + list(arguments), env=env, text=True,
                                    capture_output=True, timeout=20)
            assert result.returncode == 0, (arguments, result.returncode, result.stdout, result.stderr)
            return result.stdout

        def wait_tabs(count, selected_url):
            deadline = time.monotonic() + 30
            latest = None
            while time.monotonic() < deadline:
                response = subprocess.run(prefix + ['query', 'tabs', '--format', 'json'],
                                          env=env, text=True, capture_output=True, timeout=10)
                if response.returncode == 0:
                    latest = json.loads(response.stdout)['tabs']
                    if (len(latest) == count and any(tab.get('selected') and tab.get('url') == selected_url
                                                    for tab in latest)):
                        return latest
                assert child.poll() is None, f'Owned browser exited: {child.returncode}'
                time.sleep(0.05)
            raise AssertionError(f'Expected {count} tabs with selected URL {selected_url}; got {latest}')

        def record(name, tabs):
            facts['steps'].append({'name': name, 'tabs': tabs})
            return tabs

        with (args.output / 'browser.log').open('w') as log:
            child = subprocess.Popen(prefix + ['--software-rendering', '--userscripts-off', 'open', url + '/source'],
                                     env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            facts['owned_pid'] = child.pid
            try:
                initial = record('startup', wait_tabs(1, url + '/source'))
                source_id = initial[0]['id']
                cli('open', '--target', 'tab', url + '/new-tab')
                tabs = record('foreground-tab', wait_tabs(2, url + '/new-tab'))
                assert any(tab['id'] == source_id and tab['url'] == url + '/source' for tab in tabs)
                selected_id = next(tab['id'] for tab in tabs if tab['selected'])
                cli('open', '--target', 'current', url + '/current')
                tabs = record('explicit-current', wait_tabs(2, url + '/current'))
                assert next(tab['id'] for tab in tabs if tab['selected']) == selected_id
                cli('open', url + '/default-current')
                tabs = record('default-current', wait_tabs(2, url + '/default-current'))
                assert next(tab['id'] for tab in tabs if tab['selected']) == selected_id
                cli('open', '--target', 'tab-bg', url + '/background')
                tabs = record('background-tab', wait_tabs(3, url + '/default-current'))
                assert any(tab['url'] == url + '/background' and not tab['selected'] for tab in tabs)
                cli('open', '--target', 'tab', '--profile', 'default', '--context', 'routed', url + '/routed')
                tabs = record('explicit-profile-context', wait_tabs(4, url + '/routed'))
                assert next(tab for tab in tabs if tab['selected'])['context_name'] == 'routed'
                assert any(tab['id'] == source_id and tab['url'] == url + '/source' for tab in tabs)
                cli('open', '--target', 'tab', '--profile', 'default', url + '/additional-a', url + '/additional-b')
                tabs = record('additional-inputs', wait_tabs(6, url + '/additional-b'))
                assert {url + '/additional-a', url + '/additional-b'} <= {tab['url'] for tab in tabs}
                cli('open', '--profile', 'default', '--context', 'routed', url + '/default-multi-a', url + '/default-multi-b')
                tabs = record('additional-inputs-default-target', wait_tabs(7, url + '/default-multi-b'))
                assert next(tab for tab in tabs if tab['selected'])['context_name'] == 'routed'
                assert {url + '/default-multi-a', url + '/default-multi-b'} <= {tab['url'] for tab in tabs}
                cli('command', '--', 'open --target current ' + url + '/command-current')
                tabs = record('command-current', wait_tabs(7, url + '/command-current'))
                cli('command', '--', 'open --target tab ' + url + '/command-tab')
                tabs = record('command-tab', wait_tabs(8, url + '/command-tab'))
                deadline = time.monotonic() + 10
                while '/command-tab' not in requests and time.monotonic() < deadline:
                    time.sleep(0.05)
                assert '/command-tab' in requests
                assert hashlib.sha256(browser.read_bytes()).hexdigest() == digest
                facts['success'] = True
            finally:
                if child.poll() is None:
                    os.killpg(child.pid, signal.SIGKILL)
                child.wait(timeout=10)
                facts['reaped'] = True
                server.shutdown()
                server.server_close()
                thread.join(timeout=5)
                facts['requests'] = requests
                (args.output / 'facts.json').write_text(json.dumps(facts, indent=2) + '\n')
    print(json.dumps({'success': facts['success'], 'steps': len(facts['steps']), 'browser_sha256': digest}))


if __name__ == '__main__':
    main()

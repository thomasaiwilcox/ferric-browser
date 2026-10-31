#!/usr/bin/env python3
"""Natural production Quit and final durable aggregate in an owned compositor."""
import argparse
import http.server
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import threading
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--browser', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    token = os.environ['FERRIC_DISPOSABLE_BUS_OWNER']
    assert re.fullmatch('[0-9a-f]{32}', token)
    assert Path(os.environ['XDG_RUNTIME_DIR']).name == 'fn-' + token[:6]
    instance = os.environ['HYPRLAND_INSTANCE_SIGNATURE']
    args.output.mkdir(parents=True, exist_ok=False)
    facts = {'passed': False}
    owner = None
    with tempfile.TemporaryDirectory(prefix='ferric-owned-quit-') as temporary:
        root = Path(temporary)
        env = dict(os.environ, QT_QPA_PLATFORM='wayland', QT_QUICK_BACKEND='software', QT_QPA_PLATFORMTHEME='')
        for key in ('XDG_CONFIG_HOME', 'XDG_CACHE_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME'):
            folder = root / key
            folder.mkdir(mode=0o700)
            env[key] = str(folder)
        config = root / 'config.toml'
        config.write_text("schema_version=3\n[tabs]\npresentation='windows'\n")
        prefix = [str(args.browser.resolve()), '--basedir', str(root / 'profile'), '--config', str(config), '--instance', 'owned-quit']

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                body = ('<!doctype html><title>Owned quit ' + self.path + '</title><p>Disposable page</p>').encode()
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

        def wait(probe):
            deadline = time.monotonic() + 25
            while time.monotonic() < deadline:
                assert owner.poll() is None, ('browser exited early', owner.returncode)
                value = probe()
                if value:
                    return value
                time.sleep(0.05)
            raise AssertionError('owned Quit state timed out')

        def cli(*arguments):
            result = subprocess.run(prefix + list(arguments), env=env, text=True, capture_output=True, timeout=15)
            if result.returncode != 0:
                try:
                    error = json.loads(result.stderr).get('error', {})
                except json.JSONDecodeError:
                    error = {}
                if error.get('code') == 'E_BUSY':
                    return None
            assert result.returncode == 0, (arguments, result.stderr)
            return json.loads(result.stdout) if result.stdout.lstrip().startswith(('{', '[')) else result.stdout

        def tabs():
            return (cli('query', 'tabs', '--format', 'json') or {}).get('tabs', [])

        try:
            owner = subprocess.Popen(prefix + ['--software-rendering', '--userscripts-off', 'open', base + '/root'], env=env,
                                     stdin=subprocess.DEVNULL, stdout=log, stderr=log, start_new_session=True)
            wait(lambda: list((root / 'profile').rglob('browser.sock')))
            first = wait(lambda: rows[0] if len(rows := tabs()) == 1 and rows[0]['loading'] == 'complete' else None)
            for expected, (target, path) in enumerate([('tab', '/peer'), ('tab-bg', '/background')], start=2):
                cli('open', '--target', target, base + path)
                wait(lambda: len(rows := tabs()) == expected and all(row['loading'] == 'complete' for row in rows))
            rows = wait(lambda: rows if len(rows := tabs()) == 3 and all(row['loading'] == 'complete' for row in rows) else None)
            peer = next(row for row in rows if row['url'] == base + '/peer')
            cli('command', '--window', peer['window_id'], '--', 'tab-mute ;; tab-pin')
            cli('command', '--window', first['window_id'], '--', 'quit')
            time.sleep(0.35)
            result = subprocess.run(['/usr/bin/hyprctl', '--instance', instance, 'clients', '-j'], env=env,
                                    text=True, capture_output=True, timeout=5)
            assert result.returncode == 0, result.stderr
            clients = [row for row in json.loads(result.stdout) if row['pid'] == owner.pid and 'Owned quit /root' in row['title']]
            assert len(clients) == 1, clients
            address = clients[0]['address']
            assert re.fullmatch('0x[0-9a-fA-F]+', address)
            result = subprocess.run(['/usr/bin/hyprctl', '--instance', instance, 'eval',
                                     f'hl.dispatch(hl.dsp.focus({{window="address:{address}"}}))'], env=env,
                                    text=True, capture_output=True, timeout=5)
            assert result.returncode == 0 and 'error' not in result.stdout.lower(), (result.stdout, result.stderr)
            result = subprocess.run(['/usr/bin/hyprctl', '--instance', instance, 'activewindow', '-j'], env=env,
                                    text=True, capture_output=True, timeout=5)
            assert result.returncode == 0 and json.loads(result.stdout)['address'] == address, result.stdout
            # Supply a real virtual keyboard even on a keyboard-less headless
            # seat. The checked Wayland display belongs only to this fixture.
            subprocess.run(['/usr/bin/wtype', '-s', '200', '-k', 'y', '-s', '200'], env=env, check=True, timeout=5)
            try:
                assert owner.wait(timeout=20) == 0
            except subprocess.TimeoutExpired:
                for subject in ('windows', 'tabs', 'operations'):
                    result = subprocess.run(prefix + ['query', subject, '--format', 'json'], env=env,
                                            text=True, capture_output=True, timeout=5)
                    (args.output / (subject + '-timeout.txt')).write_text(result.stdout + result.stderr)
                if grim := shutil.which('grim'):
                    subprocess.run([grim, str(args.output / 'owned-timeout.png')], env=env,
                                   capture_output=True, timeout=5, check=False)
                raise
            checkpoints = list((root / 'profile').rglob('current*.json'))
            assert len(checkpoints) == 1, checkpoints
            snapshot = json.loads(checkpoints[0].read_text())
            saved = [tab for window in snapshot['windows'] for tab in window['tabs']]
            assert len(saved) == 3, snapshot
            assert {tab['safe_restore_url'] for tab in saved} == {base + path for path in ('/root', '/peer', '/background')}, saved
            peer = next(tab for tab in saved if tab['safe_restore_url'] == base + '/peer')
            assert peer['pinned'] and peer['muted'], peer
            facts.update(passed=True, page_count=3, background_retained=True, peer_metadata_retained=True, natural_exit=True)
        finally:
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

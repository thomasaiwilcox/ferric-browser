#!/usr/bin/env python3
"""Exercise real BrowserUi IPC using a disposable standalone browser instance."""
import argparse
from concurrent.futures import ThreadPoolExecutor
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import subprocess
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[4]


class PageHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        body = b'<!doctype html><title>IPC fixture</title><button>Action</button>'
        self.send_response(200)
        self.send_header('Content-Type', 'text/html')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


@contextmanager
def page_fixture():
    with ThreadingHTTPServer(('127.0.0.1', 0), PageHandler) as server:
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            yield f'http://127.0.0.1:{server.server_port}/fixture.html'
        finally:
            server.shutdown()
            thread.join(timeout=3)
            assert not thread.is_alive(), 'Fixture server did not stop'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--executable', type=Path, default=ROOT / 'target/release/ferric-browser')
    args = parser.parse_args()
    env = os.environ.copy()
    env.update(QT_QPA_PLATFORM='offscreen', QT_QPA_PLATFORMTHEME='', QT_QUICK_BACKEND='software',
               QTWEBENGINE_CHROMIUM_FLAGS='--disable-gpu', FERRIC_BROWSER_ALLOW_NON_WAYLAND='1')
    with page_fixture() as page_url, tempfile.TemporaryDirectory(prefix='ferric-application-ipc-') as temporary:
        prefix = [str(args.executable), '--basedir', temporary, '--instance', 'completion']
        with tempfile.TemporaryFile(mode='w+') as log:
            app = subprocess.Popen([*prefix, '--software-rendering', '--userscripts-off', 'open', page_url],
                                   env=env, stdout=log, stderr=log)
            def invoke(*arguments, accepted=True, error_code=None):
                result = subprocess.run([*prefix, *arguments], env=env, capture_output=True,
                                        text=True, timeout=30 if arguments[0] == 'diagnostics' else 12)
                if accepted:
                    assert result.returncode == 0, (arguments, result.stdout, result.stderr)
                    if arguments[0] == 'query':
                        value = json.loads(result.stdout)
                        return value.get('result', value)
                    return result.stdout
                assert result.returncode != 0, (arguments, result.stdout)
                if error_code:
                    if '--format' in arguments:
                        failure = json.loads(result.stderr)['error']
                        assert failure['code'] == error_code, failure
                        assert failure['correlation_id'] == failure['details']['correlation_id'], failure
                    else:
                        assert result.stderr.startswith('ferric-browser: ' + error_code + ':'), result.stderr
                print('remote rejection', arguments, result.stderr.strip(), flush=True)
                return result.stderr
            try:
                deadline = time.monotonic() + 20
                while True:
                    assert app.poll() is None, 'browser exited during bootstrap'
                    try:
                        before = invoke('query', 'active-tab', '--format', 'json')
                        if before.get('tabs', [{}])[0].get('url') == page_url:
                            break
                        if time.monotonic() >= deadline:
                            raise AssertionError(('bootstrap active-tab', before))
                        time.sleep(0.1)
                    except (AssertionError, json.JSONDecodeError):
                        if time.monotonic() >= deadline:
                            raise
                        time.sleep(0.1)
                # Both callers must receive a complete live report, not the
                # asynchronous collector's internal pending placeholder.
                with ThreadPoolExecutor(max_workers=2) as readers:
                    reports = [readers.submit(invoke, 'diagnostics', '--format', 'json') for _ in range(2)]
                    invoke('query', 'active-tab', '--format', 'json')
                    for future in reports:
                        report = json.loads(future.result())
                        assert report['collection']['status'] == 'complete', report.get('collection')
                        assert report['build']['qt_webengine']['status'] == 'available', report['build']
                        assert 'request_resolutions' in report, 'standalone fallback was used'
                print('concurrent diagnostic clients received complete live reports', flush=True)
                for command in ('hint --target pointer all', 'target-search controls',
                                'open https://example.invalid/ ;; hint --target pointer all'):
                    invoke('command', command, accepted=False)
                after = invoke('query', 'active-tab', '--format', 'json')
                assert before['tabs'][0]['url'] == after['tabs'][0]['url'], (before, after)
                print('remote chain rejection preserved active URL', after['tabs'][0]['url'], flush=True)
                # Page routing needs a real page policy context. Each admitted
                # configuration request must publish before the next request.
                for key, literal, expected in [('input.shortcut_policy', 'mixed', 'mixed'),
                                               ('input.page_keys', "['f']", ['f'])]:
                    invoke('command', f'set --temp "{key}={literal}"')
                    deadline = time.monotonic() + 5
                    while True:
                        setting = invoke('query', 'config', key, '--format', 'json')
                        if setting['value'] == expected:
                            break
                        assert time.monotonic() < deadline, setting
                        time.sleep(.05)
                explanation = invoke('query', 'binding-explain', 'f', '--mode', 'normal', '--format', 'json')
                print('binding explanation', json.dumps(explanation, sort_keys=True), flush=True)
                assert explanation['mixed_routing']['owner'] == 'page', explanation
                reserved = invoke('query', 'binding-explain', ':', '--mode', 'normal', '--format', 'json')
                assert reserved['mixed_routing']['owner'] == 'browser', reserved
                for panel in ('settings', 'profiles', 'sessions'):
                    rejected = invoke('command', panel + ' unexpected', accepted=False,
                                      error_code='E_INVALID_ARGUMENT')
                    assert 'does not accept arguments' in rejected, rejected
                    invoke('command', '--window', 'not-a-window',
                           '--', panel, accepted=False, error_code='E_INVALID_ARGUMENT')
                    invoke('command', '--window', 'windowid-18446744073709551615',
                           '--', panel, accepted=False, error_code='E_STALE_TARGET')
                    invoke('command', panel)
                rejected = invoke('query', 'tabs', '--window', 'not-a-window', '--format', 'json',
                                  accepted=False, error_code='E_INVALID_ARGUMENT')
                assert 'window' in json.loads(rejected)['error']['message'], rejected
                rejected = invoke('command', 'open https://should-not-open.invalid/ ;; settings unexpected',
                                  accepted=False, error_code='E_INVALID_ARGUMENT')
                assert 'earlier requests were accepted' not in rejected, rejected
                assert 'earlier commands were accepted' not in rejected, rejected
                invoke('command', 'get downloads.ask_destination')
                rejected = invoke('command', 'get no.such.setting',
                                  accepted=False, error_code='E_NOT_FOUND')
                assert 'Unknown setting: no.such.setting.' in rejected and ':settings' in rejected, rejected
                assert 'earlier requests were accepted' not in rejected, rejected
                assert 'earlier commands were accepted' not in rejected, rejected
                rejected = invoke('command', 'get downloads.ask_destination --url file:///tmp/not-a-site',
                                  accepted=False, error_code='E_INVALID_ARGUMENT')
                assert 'Use an HTTP or HTTPS page address with --url.' in rejected, rejected
                assert 'config.get' not in rejected, rejected
                invoke('command', '--window', 'windowid-18446744073709551615',
                       '--', 'get downloads.ask_destination', accepted=False, error_code='E_STALE_TARGET')
                retained_operations = {row['operation_id'] for row in
                                       invoke('query', 'operations', '--format', 'json')['operations']}
                invoke('command',
                       'set --temp downloads.ask_destination=false ;; get no.such.setting ;; '
                       'set --temp downloads.ask_destination=true ;; open https://should-not-open.invalid/')
                # Deferred configuration admits the chain first, then reports
                # its typed terminal failure through the operation ledger.
                deadline = time.monotonic() + 5
                while True:
                    operations = [row for row in invoke('query', 'operations', '--format', 'json')['operations']
                                  if row['operation_id'] not in retained_operations]
                    assert len(operations) == 1, operations
                    failed = operations[0]
                    if failed['status'].startswith('failed'):
                        break
                    assert not failed['status'].startswith('completed'), failed
                    assert time.monotonic() < deadline, failed
                    time.sleep(.05)
                assert failed['error']['code'] == 'E_NOT_FOUND', failed
                rejected = failed['error']['message']
                assert 'Unknown setting: no.such.setting.' in rejected and ':settings' in rejected, failed
                retained_setting = invoke('query', 'config', 'downloads.ask_destination', '--format', 'json')
                assert retained_setting['value'] is False, retained_setting
                assert invoke('query', 'active-tab', '--format', 'json')['tabs'][0]['url'] == before['tabs'][0]['url']
                print('typed missing-setting failure stopped the chain and retained the earlier setting', flush=True)
                # Command-authored selectors must reach route validation; an
                # unknown owner must never silently navigate the default one.
                invoke('command', 'open --profile no-such-profile https://should-not-open.invalid/',
                       accepted=False, error_code='E_DENIED')
                invoke('command', 'open --context no-such-context https://should-not-open.invalid/',
                       accepted=False, error_code='E_NOT_FOUND')
                # Admission is asynchronous. A rejected destination must still
                # finish the operation instead of remaining accepted forever.
                retained = {row['operation_id'] for row in
                            invoke('query', 'operations', '--format', 'json')['operations']}
                invoke('command', 'tab-give missing-window')
                deadline = time.monotonic() + 3
                while True:
                    transfer = [row for row in invoke('query', 'operations', '--format', 'json')['operations']
                                if row['operation_id'] not in retained]
                    assert len(transfer) == 1, transfer
                    if transfer[0]['status'].startswith('failed'):
                        break
                    assert time.monotonic() < deadline, ('rejected transfer remained nonterminal', transfer)
                    time.sleep(0.05)
                print('rejected live tab transfer reached terminal failure', flush=True)
                after_panels = invoke('query', 'active-tab', '--format', 'json')
                assert after_panels['tabs'][0]['url'] == before['tabs'][0]['url'], after_panels
                print('native manager commands accepted over IPC without navigating', flush=True)
                print('T010 IPC provenance and T022 real BrowserUi binding explanation passed', flush=True)
            finally:
                app.terminate()
                try:
                    app.wait(timeout=8)
                except subprocess.TimeoutExpired:
                    app.kill()
                    app.wait(timeout=5)
                log.seek(0)
                print(log.read())


if __name__ == '__main__':
    main()

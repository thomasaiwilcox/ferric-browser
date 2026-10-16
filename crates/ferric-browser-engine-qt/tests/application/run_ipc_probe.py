#!/usr/bin/env python3
"""Exercise real BrowserUi IPC using a disposable standalone browser instance."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[4]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--executable', type=Path, default=ROOT / 'target/release/ferric-browser')
    args = parser.parse_args()
    env = os.environ.copy()
    env.update(QT_QPA_PLATFORM='offscreen', QT_QPA_PLATFORMTHEME='', QT_QUICK_BACKEND='software',
               QTWEBENGINE_CHROMIUM_FLAGS='--disable-gpu', FERRIC_BROWSER_ALLOW_NON_WAYLAND='1')
    with tempfile.TemporaryDirectory(prefix='ferric-application-ipc-') as temporary:
        prefix = [str(args.executable), '--basedir', temporary, '--instance', 'completion']
        with tempfile.TemporaryFile(mode='w+') as log:
            app = subprocess.Popen([*prefix, '--software-rendering', '--userscripts-off', 'open', 'rb://blank'],
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
                        if before.get('tabs', [{}])[0].get('url') == 'rb://blank':
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
                invoke('command', 'set --temp input.shortcut_policy=mixed')
                invoke('command', "set --temp \"input.page_keys=['f']\"")
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
                rejected = invoke('command', 'reload ;; get no.such.setting ;; open https://should-not-open.invalid/',
                                  accepted=False, error_code='E_ENGINE')
                assert 'earlier requests were accepted' in rejected, rejected
                # Command-authored selectors must reach route validation; an
                # unknown owner must never silently navigate the default one.
                invoke('command', 'open --profile no-such-profile https://should-not-open.invalid/',
                       accepted=False, error_code='E_ENGINE')
                invoke('command', 'open --context no-such-context https://should-not-open.invalid/',
                       accepted=False, error_code='E_ENGINE')
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

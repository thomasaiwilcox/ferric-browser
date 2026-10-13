#!/usr/bin/env python3
"""Exercise real BrowserUi IPC using a disposable standalone browser instance."""
import argparse
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
            def invoke(*arguments, accepted=True):
                result = subprocess.run([*prefix, *arguments], env=env, capture_output=True,
                                        text=True, timeout=12)
                if accepted:
                    assert result.returncode == 0, (arguments, result.stdout, result.stderr)
                    if arguments[0] == 'query':
                        value = json.loads(result.stdout)
                        return value.get('result', value)
                    return result.stdout
                assert result.returncode != 0, (arguments, result.stdout)
                print('remote rejection', arguments, result.stderr.strip(), flush=True)
                return None
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

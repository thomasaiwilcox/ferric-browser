#!/usr/bin/env python3
"""Verify repeat and macro admission through an isolated production CLI instance."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
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
    with tempfile.TemporaryDirectory(prefix='ferric-repeat-') as temporary:
        root = Path(temporary)
        env = dict(os.environ, QT_QPA_PLATFORM='offscreen', QT_QUICK_BACKEND='software',
                   QT_QPA_PLATFORMTHEME='', QTWEBENGINE_CHROMIUM_FLAGS='--disable-gpu')
        for key in ('XDG_RUNTIME_DIR', 'XDG_CONFIG_HOME', 'XDG_CACHE_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME'):
            directory = root / key
            directory.mkdir(mode=0o700)
            env[key] = str(directory)
        config = root / 'config'
        config.mkdir()
        (config / 'config.toml').write_text('schema_version=3\n')
        (config / 'contexts.toml').write_text(
            'schema_version=3\n[[contexts]]\nname="routed"\nlabel="Routed"\nprofile="default"\n')
        prefix = [str(browser), '--basedir', str(root / 'profile'), '--config',
                  str(config / 'config.toml'), '--instance', 'owned-repeat']

        def cli(*arguments, status=0):
            result = subprocess.run(prefix + list(arguments), env=env, capture_output=True,
                                    text=True, timeout=20)
            assert result.returncode == status, (arguments, result.returncode, result.stdout, result.stderr)
            return result

        def tabs(count, selected_url):
            deadline = time.monotonic() + 25
            latest = None
            while time.monotonic() < deadline:
                result = subprocess.run(prefix + ['query', 'tabs', '--format', 'json'], env=env,
                                        capture_output=True, text=True, timeout=10)
                if result.returncode == 0:
                    latest = json.loads(result.stdout)['tabs']
                    if len(latest) == count and any(row['selected'] and row['url'] == selected_url for row in latest):
                        return latest
                assert owner.poll() is None, owner.returncode
                time.sleep(.05)
            raise AssertionError((count, selected_url, latest))

        def record(name, count, selected_url):
            rows = tabs(count, selected_url)
            facts['steps'].append({'name': name, 'tabs': rows})
            return rows

        with (args.output / 'browser.log').open('w') as log:
            owner = subprocess.Popen(prefix + ['--software-rendering', '--userscripts-off', 'open', 'rb://start'],
                                     env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            facts['owned_pid'] = owner.pid
            try:
                record('startup', 1, 'rb://start')
                cli('command', '--', 'macro-record a')
                cli('command', '--', 'tab-open rb://blank')
                record('accepted-tab-open', 2, 'rb://blank')
                denied = cli('open', '--profile', 'wrong', '--target', 'tab', 'rb://start', status=4)
                assert 'E_DENIED' in denied.stderr
                record('rejected-profile', 2, 'rb://blank')
                cli('command', '--', 'repeat')
                record('repeat-after-rejected-profile', 3, 'rb://blank')
                cli('open', '--target', 'tab', 'http://[invalid', status=1)
                record('rejected-url', 3, 'rb://blank')
                cli('command', '--', 'macro-stop')
                cli('command', '--', 'macro-play a')
                record('macro-excludes-rejected-commands', 5, 'rb://blank')
                cli('command', '--', 'macro-record b')
                cli('open', '--target', 'tab-bg', 'rb://start')
                rows = record('background-open', 6, 'rb://blank')
                selected = next(row['id'] for row in rows if row['selected'])
                cli('command', '--', 'repeat --count 2')
                rows = record('background-repeat-count', 8, 'rb://blank')
                assert next(row['id'] for row in rows if row['selected']) == selected
                cli('command', '--', 'macro-stop')
                cli('command', '--', 'macro-play b')
                record('background-macro', 11, 'rb://blank')
                cli('open', '--profile', 'default', '--context', 'routed', '--target', 'tab', 'rb://start')
                record('context-open', 12, 'rb://start')
                cli('command', '--', 'repeat')
                rows = record('context-repeat', 13, 'rb://start')
                assert next(row for row in rows if row['selected'])['context_name'] == 'routed'
                # Accepted asynchronous view commands must remain eligible too.
                cli('command', '--', 'macro-record c')
                cli('command', '--', 'zoom in')
                cli('command', '--', 'zoom invalid', status=2)
                cli('command', '--', 'macro-stop')
                cli('command', '--', 'macro-play c')
                record('accepted-view-command', 13, 'rb://start')
                assert hashlib.sha256(browser.read_bytes()).hexdigest() == digest
                facts['success'] = True
            finally:
                if owner.poll() is None:
                    os.killpg(owner.pid, signal.SIGKILL)
                owner.wait(timeout=10)
                facts['reaped'] = True
                (args.output / 'facts.json').write_text(json.dumps(facts, indent=2) + '\n')
    print(json.dumps({'success': facts['success'], 'steps': len(facts['steps']), 'browser_sha256': digest}))


if __name__ == '__main__':
    main()

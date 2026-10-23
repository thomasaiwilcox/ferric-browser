#!/usr/bin/env python3
"""Qualify closed pipes and genuine output failures on the public CLI."""
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
    args.output.mkdir(parents=True, exist_ok=True)
    facts = {'browser_sha256': hashlib.sha256(browser.read_bytes()).hexdigest(),
             'success': False, 'cases': []}
    with tempfile.TemporaryDirectory(prefix='fcli-out-') as temporary:
        root = Path(temporary)
        env = dict(os.environ, QT_QPA_PLATFORM='offscreen', QT_QUICK_BACKEND='software',
                   QT_QPA_PLATFORMTHEME='', QTWEBENGINE_CHROMIUM_FLAGS='--disable-gpu')
        for key in ('XDG_RUNTIME_DIR', 'XDG_CONFIG_HOME', 'XDG_CACHE_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME'):
            directory = root / key
            directory.mkdir(mode=0o700)
            env[key] = str(directory)
        # Default-browser output uses an owned executable; no desktop settings change.
        bin_directory = root / 'bin'
        bin_directory.mkdir()
        xdg_settings = bin_directory / 'xdg-settings'
        xdg_settings.write_text('#!/bin/sh\n[ "$1" = get ] || exit 9\nprintf "%s\\n" "io.github.ferricbrowser.FerricBrowser.desktop"\n')
        xdg_settings.chmod(0o700)
        env['PATH'] = str(bin_directory) + os.pathsep + env.get('PATH', '')
        config = root / 'valid.toml'
        config.write_text('schema_version=3\n')
        invalid_config = root / 'invalid.toml'
        invalid_config.write_text('schema_version = [not valid TOML\n')
        child = None

        def invoke(name, argv, output='capture', error_output='capture', expected=0):
            descriptors = []

            def destination(mode):
                if mode == 'capture':
                    return subprocess.PIPE
                if mode == 'closed':
                    reader, writer = os.pipe()
                    os.close(reader)  # No reader exists before exec, so EPIPE is deterministic.
                    descriptors.append(writer)
                    return writer
                assert mode == 'full'
                descriptor = os.open('/dev/full', os.O_WRONLY)
                descriptors.append(descriptor)
                return descriptor

            try:
                result = subprocess.run(argv, env=env, stdin=subprocess.DEVNULL,
                                        stdout=destination(output), stderr=destination(error_output), timeout=30)
            finally:
                for descriptor in descriptors:
                    os.close(descriptor)
            stderr = result.stderr or b''
            facts['cases'].append({'name': name, 'status': result.returncode,
                                   'stdout_bytes': len(result.stdout or b''), 'stderr_bytes': len(stderr)})
            assert result.returncode == expected, (name, result.returncode, stderr.decode(errors='replace'))
            assert b'panicked' not in stderr and b'failed printing' not in stderr, (name, stderr)
            if expected == 0:
                assert not stderr, (name, stderr)
            return result

        prefix = [str(browser), '--basedir', str(root / 'live'), '--instance', 'owned-cli-output']

        def wait_tabs(count, selected=None):
            deadline = time.monotonic() + 30
            latest = None
            while time.monotonic() < deadline:
                response = subprocess.run(prefix + ['query', 'tabs', '--format', 'json'], env=env,
                                          capture_output=True, timeout=10)
                if response.returncode == 0:
                    latest = json.loads(response.stdout)['tabs']
                    if len(latest) == count and (selected is None or any(
                            tab['id'] == selected and tab['selected'] for tab in latest)):
                        return latest
                assert child.poll() is None, ('Owned browser exited', child.returncode)
                time.sleep(0.05)
            raise AssertionError(('Timed out waiting for tabs', count, selected, latest))

        try:
            offline = [
                ('help', ['--help']),
                ('version', ['--version']),
                ('built-in-config', ['config', 'check']),
                ('user-config', ['--config', str(config), 'config', 'check']),
                ('offline-diagnostics', ['diagnostics', '--format', 'json']),
                ('default-browser-status', ['default-browser', 'status']),
            ]
            for name, arguments in offline:
                normal = invoke(name + '-normal', [str(browser), *arguments])
                assert normal.stdout.endswith(b'\n')
                if name == 'help':
                    assert b'Registry command catalog:' in normal.stdout
                    (args.output / 'help.txt').write_bytes(normal.stdout)
                if name in ('offline-diagnostics', 'default-browser-status'):
                    assert isinstance(json.loads(normal.stdout), dict)
                invoke(name + '-closed-stdout', [str(browser), *arguments], output='closed')
                failed = invoke(name + '-full-stdout', [str(browser), *arguments], output='full', expected=1)
                assert b'E_IO' in failed.stderr
                if '--format' in arguments:
                    assert json.loads(failed.stderr)['error']['code'] == 'E_IO'
            # Reading a small prefix and then closing is the ordinary `... | head` journey.
            help_child = subprocess.Popen([str(browser), '--help'], env=env, stdout=subprocess.PIPE,
                                          stderr=subprocess.PIPE, stdin=subprocess.DEVNULL)
            try:
                first_line = help_child.stdout.readline()
                help_child.stdout.close()
                stderr = help_child.stderr.read()
                status = help_child.wait(timeout=10)
                facts['cases'].append({'name': 'help-reader-closes-after-first-line', 'status': status,
                                       'read_bytes': len(first_line), 'stderr_bytes': len(stderr)})
                assert first_line.startswith(b'Ferric Browser') and status == 0 and not stderr
            finally:
                if help_child.poll() is None:
                    help_child.kill()
                help_child.wait(timeout=10)
                help_child.stderr.close()
            invoke('closed-stderr-text-error', [str(browser), '--unknown-option'], error_output='closed', expected=2)
            invoke('closed-stderr-json-error', [str(browser), '--unknown-option', '--format', 'json'],
                   error_output='closed', expected=2)
            # Logging stderr is best effort while successful stdout remains available.
            logged = invoke('closed-stderr-logging', [str(browser), '--log-level', 'debug', 'config', 'check'],
                            error_output='closed')
            assert b'configuration structure valid' in logged.stdout
            invalid = invoke('closed-stdout-invalid-config', [str(browser), '--config', str(invalid_config),
                             'config', 'check'], output='closed', expected=2)
            assert b'E_CONFIG' in invalid.stderr
            missing = invoke('closed-stdout-no-instance', [str(browser), '--instance', 'missing-output-fixture',
                             'query', 'tabs', '--format', 'json'], output='closed', expected=3)
            assert json.loads(missing.stderr)['error']['code'] == 'E_NO_INSTANCE'
            with (args.output / 'browser.log').open('w') as log:
                child = subprocess.Popen(prefix + ['--software-rendering', '--userscripts-off', 'open', 'rb://start'],
                                         env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
                facts['owned_pid'] = child.pid
                tabs = wait_tabs(1)
                first_id = tabs[0]['id']
                invoke('live-query-closed-stdout', prefix + ['query', 'tabs', '--format', 'json'], output='closed')
                invoke('live-diagnostics-closed-stdout', prefix + ['diagnostics', '--format', 'json'], output='closed')
                invoke('live-command-closed-stdout', prefix + ['command', '--', 'tab-open rb://blank'], output='closed')
                wait_tabs(2)
                invoke('live-open-closed-stdout', prefix + ['open', '--target', 'tab', 'rb://start'], output='closed')
                wait_tabs(3)
                invoke('live-focus-closed-stdout', prefix, output='closed')
                wait_tabs(3)
                invoke('live-activate-closed-stdout', prefix + ['activate', 'tab', first_id], output='closed')
                wait_tabs(3, first_id)
                failed = invoke('live-query-full-stdout', prefix + ['query', 'tabs', '--format', 'json'],
                                output='full', expected=1)
                assert json.loads(failed.stderr)['error']['code'] == 'E_IO'
                invoke('live-command-full-stdout', prefix + ['command', '--', 'tab-open rb://blank'],
                       output='full', expected=1)
                # A failed receipt must not roll back or retry the already accepted command.
                facts['final_tabs'] = wait_tabs(4)
                assert any(tab['id'] == first_id for tab in facts['final_tabs'])
                assert child.poll() is None
            assert hashlib.sha256(browser.read_bytes()).hexdigest() == facts['browser_sha256']
            facts['success'] = True
        finally:
            if child is not None:
                if child.poll() is None:
                    os.killpg(child.pid, signal.SIGKILL)
                child.wait(timeout=10)
                facts['reaped'] = True
            (args.output / 'facts.json').write_text(json.dumps(facts, indent=2) + '\n')
    print(json.dumps({'success': facts['success'], 'cases': len(facts['cases']),
                      'browser_sha256': facts['browser_sha256']}))


if __name__ == '__main__':
    main()

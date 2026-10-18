#!/usr/bin/env python3
"""Qualify context deletion and retained membership through public CLI/IPC."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[4]
AUTHORED = """schema_version=3
[[contexts]]
name='work'
label='Saved work'
profile='default'
default_target='reuse'
[[routes]]
id='work-route'
pattern='https://example.test/*'
context='work'
behavior='prompt'
entry_points=['explicit-open']
"""


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--executable', type=Path, default=ROOT / 'target/release/ferric-browser')
    parser.add_argument('--quit-plugin-path', type=Path, default=ROOT / 'target/application-tests/plugins')
    args = parser.parse_args()
    plugin_path = args.quit_plugin_path.resolve()
    assert (plugin_path / 'generic').is_dir(), 'Build the native application tests before this CLI fixture'
    # Keep Unix socket paths short; every filesystem root is disposable.
    with tempfile.TemporaryDirectory(prefix='fctx-') as temporary:
        root = Path(temporary)
        (root / '.ferric-owned-fixture').write_text('Owned context-deletion test only.\n')
        config, contexts = root / 'config.toml', root / 'contexts.toml'
        config.write_text('schema_version=3\n[ui]\nfont_size_pt=12\n')
        contexts.write_text(AUTHORED)
        authored = contexts.read_bytes()
        env = os.environ.copy()
        env.update(QT_QPA_PLATFORM='offscreen', QT_QPA_PLATFORMTHEME='', QT_QUICK_BACKEND='software',
                   QTWEBENGINE_CHROMIUM_FLAGS='--disable-gpu', FERRIC_BROWSER_ALLOW_NON_WAYLAND='1')
        for key, folder in [('XDG_CONFIG_HOME', 'config'), ('XDG_CACHE_HOME', 'cache'),
                            ('XDG_DATA_HOME', 'data'), ('XDG_RUNTIME_DIR', 'runtime')]:
            path = root / folder
            path.mkdir(mode=0o700)
            env[key] = str(path)
        env['QT_PLUGIN_PATH'] = str(plugin_path) + (os.pathsep + env['QT_PLUGIN_PATH'] if env.get('QT_PLUGIN_PATH') else '')
        env['QT_QPA_GENERIC_PLUGINS'] = 'ferricownedquitfixture'
        env['FERRIC_OWNED_QUIT_ROOT'] = str(root)
        prefix = [str(args.executable.resolve()), '--basedir', str(root / 'browser'),
                  '--instance', 'context-delete', '--config', str(config)]
        before = None
        for process in range(2):
            (root / 'quit-confirmed.json').unlink(missing_ok=True)
            (root / 'confirm-quit').unlink(missing_ok=True)
            with tempfile.TemporaryFile(mode='w+') as log:
                app = subprocess.Popen([*prefix, '--software-rendering', '--userscripts-off', 'open', 'rb://blank'],
                                       env=env, stdout=log, stderr=log)

                def invoke(*arguments, rejected=False):
                    result = subprocess.run([*prefix, *arguments], env=env, capture_output=True,
                                            text=True, timeout=12)
                    if rejected:
                        assert result.returncode != 0, (arguments, result.stdout)
                        assert result.stderr.startswith('ferric-browser: E_CONFIG:'), result.stderr
                        assert 'contexts.toml' in result.stderr and ':config-reload' in result.stderr, result.stderr
                        return result.stderr
                    assert result.returncode == 0, (arguments, result.stdout, result.stderr)
                    if arguments[0] == 'query':
                        value = json.loads(result.stdout)
                        return value.get('result', value)
                    return result.stdout

                def wait_for(probe, accepted, timeout=20):
                    deadline = time.monotonic() + timeout
                    while True:
                        assert app.poll() is None, 'browser exited during context fixture'
                        value = probe()
                        if accepted(value):
                            return value
                        assert time.monotonic() < deadline, value
                        time.sleep(.05)

                def catalog():
                    return invoke('query', 'contexts', '--members', '--format', 'json')['contexts']

                def ready():
                    try:
                        return catalog()
                    except (AssertionError, json.JSONDecodeError):
                        assert app.poll() is None, 'browser exited during bootstrap'
                        return None

                try:
                    rows = wait_for(ready, lambda value: bool(value))
                    if process == 0:
                        invoke('command', 'context-enter work')
                        invoke('command', 'context-save')
                        rows = wait_for(catalog, lambda value: bool(value[0]['members']))
                        before = rows[0]
                    else:
                        assert rows == [before], ('restart changed saved context', rows, before)
                    invoke('command', 'context-delete work --confirm', rejected=True)
                    assert catalog() == [before], 'rejected deletion changed identity or saved membership'
                    assert contexts.read_bytes() == authored, 'rejected deletion changed authored definitions or routes'
                    if process == 0:
                        invoke('command', 'context-create manual --profile default')
                        wait_for(catalog, lambda value: any(row['name'] == 'manual' for row in value))
                        invoke('command', 'context-delete manual --confirm')
                        assert catalog() == [before], 'manual deletion removed the wrong context'
                        config.write_text('schema_version=3\n[ui]\nfont_size_pt=18\n')
                    else:
                        contexts.write_text('schema_version=3\n')
                        config.write_text('schema_version=3\n[ui]\nfont_size_pt=20\n')
                    invoke('command', 'config-reload')
                    wait_for(lambda: invoke('query', 'config', 'ui.font_size_pt', '--format', 'json'),
                             lambda value: value['value'] == (18 if process == 0 else 20))
                    assert catalog() == ([before] if process == 0 else []), 'reload used the wrong context authority'
                    if process == 0:
                        assert contexts.read_bytes() == authored
                    invoke('command', 'quit')
                    (root / 'confirm-quit').write_text("Confirm this fixture's browser quit.\n")
                    assert app.wait(timeout=12) == 0, 'browser did not shut down cleanly'
                    assert json.loads((root / 'quit-confirmed.json').read_text())['choice'] == 'Quit browser'
                    print('Context deletion process', process + 1, 'passed', flush=True)
                except BaseException:
                    for name in ['quit-helper-state.json', 'quit-confirmed.json', 'quit-observed-state.json']:
                        if (root / name).exists():
                            print(name, (root / name).read_text(), flush=True)
                    log.seek(0)
                    print(log.read(), flush=True)
                    raise
                finally:
                    if app.poll() is None:
                        app.terminate()
                        try:
                            app.wait(timeout=8)
                        except subprocess.TimeoutExpired:
                            app.kill()
                            app.wait(timeout=5)
        print('Configured context identity, saved tabs and routes survived rejection, reload and restart; manual deletion and source removal worked', flush=True)


if __name__ == '__main__':
    main()

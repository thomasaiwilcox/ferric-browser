#!/usr/bin/env python3
"""Public CLI selection of live and saved contexts, with owned browser processes."""
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
    parser.add_argument('--expect-old-failure', action='store_true')
    args = parser.parse_args()
    browser = args.browser.resolve()
    args.output.mkdir(parents=True, exist_ok=True)
    facts = {'browser_sha256': hashlib.sha256(browser.read_bytes()).hexdigest(),
             'baseline': args.expect_old_failure, 'success': False, 'cases': [],
             'owned_pids': [], 'reaped_pids': []}
    requests = []
    owner = None
    logs = []
    with tempfile.TemporaryDirectory(prefix='fctx-') as temporary:
        root = Path(temporary)
        env = dict(os.environ, QT_QPA_PLATFORM='offscreen', QT_QUICK_BACKEND='software',
                   QT_QPA_PLATFORMTHEME='', QTWEBENGINE_CHROMIUM_FLAGS='--disable-gpu')
        for key in ('XDG_RUNTIME_DIR', 'XDG_CONFIG_HOME', 'XDG_CACHE_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME'):
            directory = root / key
            directory.mkdir(mode=0o700)
            env[key] = str(directory)
        config_directory = root / 'user-config'
        config_directory.mkdir()
        config = config_directory / 'config.toml'
        config.write_text('schema_version=3\n')
        definitions = config_directory / 'contexts.toml'
        definitions.write_text('schema_version=3\n[[contexts]]\nname="defined"\nlabel="Defined"\nprofile="default"\n')
        base = root / 'profile'
        registry_path = base / 'state' / 'contexts.json'
        prefix = [str(browser), '--basedir', str(base), '--config', str(config), '--instance', 'owned-context-selection']

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                requests.append(self.path)
                body = b'<!doctype html><title>Owned context selection</title><input id="draft">'
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

        def cli(*arguments, expected=0):
            result = subprocess.run(prefix + list(arguments), env=env, text=True,
                                    capture_output=True, timeout=20)
            assert result.returncode == expected, (arguments, result.returncode, result.stdout, result.stderr)
            return result

        def query(kind):
            return json.loads(cli('query', kind, '--format', 'json').stdout)

        def wait(check, description):
            deadline = time.monotonic() + 30
            latest = None
            while time.monotonic() < deadline:
                latest = check()
                if latest:
                    return latest
                assert owner is None or owner.poll() is None, ('Owned browser exited', owner.returncode)
                time.sleep(0.05)
            raise AssertionError(('Timed out', description, latest))

        def tabs_at(path, count=None):
            def check():
                result = subprocess.run(prefix + ['query', 'tabs', '--format', 'json'], env=env,
                                        text=True, capture_output=True, timeout=10)
                if result.returncode:
                    return None
                tabs = json.loads(result.stdout)['tabs']
                if count is not None and len(tabs) != count:
                    return None
                return tabs if any(tab['selected'] and tab['url'] == url + path and tab['loading'] == 'complete'
                                   for tab in tabs) else None
            return wait(check, path)

        def background_ready():
            tabs = query('tabs')['tabs']
            return tabs if len(tabs) == 3 and any(tab['url'] == url + '/background'
                and tab['loading'] == 'complete' for tab in tabs) else None

        def saved_members(name, address):
            document = json.loads(registry_path.read_text())
            context = next(context for context in document['contexts'] if context['name'] == name)
            return context if any(tab.get('safe_restore_url') == url + address
                for member in context['members'] for tab in member['tab_descriptors']) else None

        def start(phase, *arguments):
            nonlocal owner
            log = (args.output / (phase + '-browser.log')).open('w')
            logs.append(log)
            owner = subprocess.Popen(prefix + ['--software-rendering', '--userscripts-off', *arguments],
                                     env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            facts['owned_pids'].append(owner.pid)

        def stop():
            nonlocal owner
            if owner is not None:
                if owner.poll() is None:
                    os.killpg(owner.pid, signal.SIGKILL)
                owner.wait(timeout=10)
                facts['reaped_pids'].append(owner.pid)
                owner = None

        def record(name, **values):
            facts['cases'].append({'name': name, **values})

        def catalog_context(name):
            return next((context for context in query('contexts')['contexts'] if context['name'] == name), None)

        def signature(tabs):
            return [(tab['id'], tab['url'], tab['selected'], tab['context_name'], tab['profile_name'], tab['window_id'])
                    for tab in tabs]

        def reject_warm(name, code, status, *arguments):
            before = signature(query('tabs')['tabs'])
            result = cli(*arguments, expected=status)
            assert code in result.stderr, (name, result.stderr)
            assert signature(query('tabs')['tabs']) == before, name
            record(name, status=status, code=code, tabs_preserved=True)

        def reject_cold(name, code, status, *arguments):
            before = registry_path.read_bytes()
            child = subprocess.Popen(prefix + list(arguments), env=env, text=True,
                                     stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
            try:
                output, error = child.communicate(timeout=15)
                assert child.returncode == status and code in error, (name, child.returncode, output, error)
            finally:
                if child.poll() is None:
                    os.killpg(child.pid, signal.SIGKILL)
                child.wait(timeout=10)
            assert registry_path.read_bytes() == before, name
            record(name, status=status, code=code, registry_unchanged=True)

        try:
            start('warm', 'open', url + '/source')
            tabs = tabs_at('/source', 1)
            source_id = tabs[0]['id']
            wait(lambda: catalog_context('defined'), 'authored context import')
            cli('command', '--', 'context-create runtime --profile default')
            runtime = wait(lambda: catalog_context('runtime'), 'created context publication')
            facts['runtime_context'] = runtime
            assert runtime['name'] not in definitions.read_text()
            record('runtime-context-published', context=runtime, tabs=tabs)
            if args.expect_old_failure:
                result = cli('open', '--context', 'runtime', '--target', 'tab', url + '/warm', expected=2)
                assert 'E_CONFIG' in result.stderr
                assert signature(query('tabs')['tabs']) == signature(tabs)
                record('warm-selection-rejected', status=2, stderr=result.stderr)
                stop()
                reject_cold('cold-selection-rejected', 'E_CONFIG', 2, 'open', '--context', 'runtime', url + '/cold')
            else:
                reject_warm('unknown-live-context', 'E_NOT_FOUND', 3, 'open', '--context', 'missing', url + '/reject')
                reject_warm('conflicting-live-profile', 'E_INVALID_ARGUMENT', 2, 'open', '--profile', 'wrong',
                            '--context', 'runtime', url + '/reject')
                reject_warm('foreign-tab-profile', 'E_DENIED', 4, 'open', '--profile', 'wrong', url + '/reject')
                cli('open', '--context', 'runtime', '--target', 'tab', url + '/warm')
                tabs = tabs_at('/warm', 2)
                current = next(tab for tab in tabs if tab['selected'])
                assert current['context_id'] == runtime['id']
                assert any(tab['id'] == source_id and tab['url'] == url + '/source' for tab in tabs)
                record('warm-created-context-tab', tabs=tabs)
                cli('open', '--context', 'runtime', '--target', 'tab-bg', url + '/background')
                tabs = wait(background_ready, 'background tab')
                assert next(tab for tab in tabs if tab['id'] == current['id'])['selected']
                assert any(tab['url'] == url + '/background' and not tab['selected'] for tab in tabs)
                record('warm-created-context-background', tabs=tabs)
                cli('open', '--profile', 'default', '--context', 'runtime', '--target', 'current', url + '/current')
                tabs = tabs_at('/current', 3)
                assert next(tab for tab in tabs if tab['selected'])['id'] == current['id']
                record('warm-explicit-profile-current', tabs=tabs)
                cli('open', '--context', 'runtime', url + '/multi-a', url + '/multi-b')
                tabs = tabs_at('/multi-b', 4)
                assert {url + '/multi-a', url + '/multi-b'} <= {tab['url'] for tab in tabs}
                assert all(tab['context_id'] == runtime['id'] for tab in tabs)
                record('warm-context-additional-inputs', tabs=tabs)
                cli('command', '--', 'context-save runtime')
                saved = wait(lambda: saved_members('runtime', '/multi-b'), 'saved runtime membership')
                record('saved-runtime-members', context=saved)
                cli('command', '--', 'profile-create work')
                profiles_path = base / 'state' / 'profiles.json'
                created = wait(lambda: next((profile for profile in json.loads(profiles_path.read_text())
                               if profile['name'] == 'work'), None), 'CLI-created work profile')
                record('cli-created-work-profile', profile=created)
                cli('open', '--target', 'window', '--profile', 'work', url + '/seed-work')
                seeded = tabs_at('/seed-work')
                record('created-work-profile-window', tabs=seeded)
                cli('command', '--', 'context-create workctx --profile work')
                work = wait(lambda: catalog_context('workctx'), 'created work context')
                facts['work_context'] = work
                reject_warm('foreign-context-tab', 'E_DENIED', 4, 'open', '--context', 'runtime', url + '/reject')
                cli('open', '--context', 'workctx', '--target', 'window', url + '/work-window')
                tabs = tabs_at('/work-window')
                row = next(tab for tab in tabs if tab['selected'] and tab['url'] == url + '/work-window')
                assert row['context_id'] == work['id'] and row['profile_name'] == 'work'
                record('warm-context-window-profile', tabs=tabs, windows=query('windows'))
                cli('command', '--', 'context-save workctx')
                saved = wait(lambda: saved_members('workctx', '/work-window'), 'saved work membership')
                record('saved-work-members', context=saved)
                stop()
                definitions.write_text('schema_version=3\n')
                reject_cold('removed-authored-context', 'E_NOT_FOUND', 3, 'open', '--context', 'defined', url + '/reject')
                reject_cold('unknown-cold-context', 'E_NOT_FOUND', 3, 'open', '--context', 'missing', url + '/reject')
                reject_cold('conflicting-cold-profile', 'E_INVALID_ARGUMENT', 2, 'open', '--profile', 'default',
                            '--context', 'workctx', url + '/reject')
                start('cold-work', 'open', '--context', 'workctx', url + '/cold-work')
                tabs = tabs_at('/cold-work')
                selected = next(tab for tab in tabs if tab['selected'] and tab['url'] == url + '/cold-work')
                assert selected['context_id'] == work['id'] and selected['profile_name'] == 'work'
                assert catalog_context('workctx')['id'] == work['id']
                document = json.loads(registry_path.read_text())
                assert 'workctx' not in document['configured_names'] and 'runtime' not in document['configured_names']
                assert not any(context['name'] == 'defined' for context in document['contexts'])
                record('cold-created-context-profile', tabs=tabs, registry=document)
                stop()
                config_option = prefix.index('--config')
                del prefix[config_option:config_option + 2]
                start('cold-default', 'open', '--profile', 'default', '--context', 'runtime', url + '/cold-default')
                tabs = tabs_at('/cold-default')
                selected = next(tab for tab in tabs if tab['selected'] and tab['url'] == url + '/cold-default')
                assert selected['context_id'] == runtime['id'] and selected['profile_name'] == 'Default'
                record('cold-explicit-profile-context', tabs=tabs, configuration_source='built-in')
                stop()
                original = registry_path.read_bytes()
                registry_path.write_bytes(b'{invalid context catalog')
                reject_cold('corrupt-cold-context-catalog', 'E_STORAGE', 1, 'open', '--context', 'runtime', url + '/reject')
                registry_path.write_bytes(original)
            assert hashlib.sha256(browser.read_bytes()).hexdigest() == facts['browser_sha256']
            facts['success'] = True
        finally:
            stop()
            for log in logs:
                log.close()
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)
            facts['requests'] = requests
            (args.output / 'facts.json').write_text(json.dumps(facts, indent=2) + '\n')
    print(json.dumps({'success': facts['success'], 'baseline': facts['baseline'], 'cases': len(facts['cases']),
                      'browser_sha256': facts['browser_sha256']}))


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Exercise typed draft/edit/apply replies against an isolated production browser."""
import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import threading
import tomllib

from run_settings_handoff_ipc_probe import Client, digest


class PreviewFixture(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_GET(self):
        if self.path == '/never':
            self.server.activations += 1
        body = b'<!doctype html><a id="preview" href="/never">Navigation</a>'
        self.send_response(200)
        self.send_header('Content-Type', 'text/html; charset=utf-8')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def run(binary):
    record = {'purpose': 'P11 production IPC draft creation, immutable apply and activation',
              'binary_sha256': digest(binary), 'status': 'failed'}
    with tempfile.TemporaryDirectory(prefix='ferric-policy-ipc-') as temporary, tempfile.TemporaryFile(mode='w+') as log:
        root = Path(temporary)
        config = root / 'fixtures/config/fixture.toml'
        config.parent.mkdir(parents=True)
        config.write_text("""schema_version=3
[[site_rules]]
id='diagnostics'
pattern='https://diagnostics.test/*'
priority=0
set={'content.zoom'=1.25}
[site_rules.interaction]
enabled=true
hint_page_keys=['p']
[[site_rules.interaction.anchors]]
id='main'
code='m'
families=['links']
target_selector='#missing'
[[site_rules.interaction.remaps]]
id='player'
from='p'
to='k'
contexts=['hint-page-key','passthrough']
[[site_rules.interaction.remaps]]
id='back-test'
from='b'
to='Back'
contexts=['passthrough']
""")
        env = os.environ.copy()
        for key in list(env):
            if key.startswith('FERRIC_SETTINGS_SYNC_'):
                env.pop(key)
        env.update(QT_QPA_PLATFORM='offscreen', QT_QPA_PLATFORMTHEME='', QT_QUICK_BACKEND='software',
                   QTWEBENGINE_CHROMIUM_FLAGS='--disable-gpu', FERRIC_BROWSER_ALLOW_NON_WAYLAND='1')
        app = subprocess.Popen([str(binary), '--basedir', str(root), '--instance', 'policy-drafts',
                                '--config', str(config), '--software-rendering', '--userscripts-off',
                                'open', 'rb://blank'], env=env, stdout=log, stderr=subprocess.STDOUT)
        client = None
        preview_server = None

        def wait(probe, accept, seconds=20):
            deadline = time.monotonic() + seconds
            while True:
                assert app.poll() is None, 'owned fixture browser exited'
                result = probe()
                if accept(result):
                    return result
                assert time.monotonic() < deadline, result
                time.sleep(.025)

        try:
            sockets = wait(lambda: list(root.glob('runtime/instances/*/browser.sock')), bool)
            assert len(sockets) == 1, sockets
            client = Client(sockets[0])
            windows = wait(lambda: client.rpc('windows.query', {}, allow_busy=True),
                           lambda result: len(result.get('live_registry', [])) == 1)
            owner = windows['live_registry'][0]['id']
            wait(lambda: client.rpc('tabs.query', {'window': owner}, allow_busy=True),
                 lambda result: len(result.get('tabs', [])) == 1 and result['tabs'][0]['loading'] == 'complete')

            def command(name, arguments, expected_error=None):
                return client.rpc('command.execute', {'command': name, 'arguments': arguments,
                                  'context': {'window': owner, 'source': 'cli'}}, expected_error=expected_error)

            def finish_transfer(reply, expected='completed'):
                operation = reply['operation_id']
                rows = wait(lambda: client.rpc('operations.query', {'operation_id': operation})['operations'],
                            lambda rows: len(rows) == 1 and rows[0]['status'] in ('completed', 'failed', 'cancelled'))
                assert rows[0]['status'] == expected, rows
                return rows[0]

            draft = command('site-policy-edit', {'source': 'generated', 'id': 'ipc-policy', 'new': True,
                             'pattern': 'https://example.test/*', 'enabled': True})
            assert draft.get('id') and draft['source'] == 'generated', draft
            revision = int(draft['revision'])
            assert draft['dirty'] and draft['rules'][0]['original'] is None, draft
            runtime = root / 'state/runtime-overrides.toml'
            assert not runtime.exists(), 'editing a draft saved configuration'
            applied = command('site-policy-apply', {'draft': draft['id'], 'revision': revision, 'activate': True})
            operation = applied['operation_id']
            rows = wait(lambda: client.rpc('operations.query', {'operation_id': operation})['operations'],
                        lambda rows: len(rows) == 1 and rows[0]['status'] in ('completed', 'failed'))
            assert rows[0]['status'] == 'completed' and 'activated' in rows[0]['detail'], rows
            assert rows[0]['window_id'] == owner and not rows[0]['cancellable'], rows
            saved = tomllib.loads(runtime.read_text())
            rule = next(rule for rule in saved['site_rules'] if rule['id'] == 'ipc-policy')
            assert rule['interaction']['enabled'] is True, rule
            opened = command('site-policy-edit', {'source': 'generated', 'id': 'ipc-policy'})
            assert opened.get('id') and not opened['dirty'], opened
            before = runtime.read_bytes()
            rebased = command('site-policy-edit', {'draft': opened['id'], 'revision': int(opened['revision']), 'rebase': True})
            assert rebased['id'] == opened['id'] and int(rebased['revision']) > int(opened['revision']), rebased
            assert rebased['rebased_from'] and runtime.read_bytes() == before, rebased
            listed = command('site-policy-list', {'url': 'https://example.test/'})
            assert listed['desired'][0]['source'] == 'generated', listed
            assert listed['active'][0]['id'] == 'ipc-policy', listed
            assert listed['active'][0]['matched'] is True, listed
            assert listed['saved']['generated'][0]['id'] == 'ipc-policy', listed
            assert listed['saved_error'] is None, listed
            assert listed['active_revision'] == listed['configuration_revision'], listed
            nonmatching = command('site-policy-list', {'url': 'https://elsewhere.test/'})
            assert nonmatching['desired'] == [] and nonmatching['active'] == [], nonmatching
            all_rules = command('site-policy-list', {'all': True})
            assert all_rules['url'] is None and any(row['id'] == 'ipc-policy' for row in all_rules['desired']), all_rules
            explained = command('site-policy-explain', {'url': 'https://diagnostics.test/', 'code': 'm'})
            assert explained['activation_permitted'] is False, explained
            assert explained['live_codes']['status'] == 'unavailable', explained
            detail = explained['active']['explanation']
            book = next(book for book in detail['codebooks'] if book['family'] == 'links')
            assert book['reservations'] == [{'kind': 'anchor', 'id': 'main', 'slot': 0, 'rank': 1, 'code': 'm'}], book
            assert any(row['capacity'] > 0 for row in book['capacity_by_suffix_width']), book
            field = next(field for field in detail['fields'] if field['path'] == 'anchors')
            assert field['origin']['source']['kind'] == 'authored' and field['origin']['rule']['id'] == 'diagnostics', field
            assert explained['active']['validation']['live_document'] == 'unknown', explained
            key = command('site-policy-explain', {'url': 'https://diagnostics.test/', 'key': 'p'})
            contexts = key['active']['key']['contexts']
            hint = next(row for row in contexts if row['context'] == 'hint-page-key')
            assert hint['source'] == 'p' and hint['destination'] == 'k' and hint['source_admission'] == 'configured', hint
            assert hint['qualification']['status'] == 'qualified', hint
            assert key['live_key']['status'] == 'unavailable', key
            back = command('site-policy-explain', {'url': 'https://diagnostics.test/', 'key': 'b'})
            back_context = next(row for row in back['active']['key']['contexts'] if row['context'] == 'passthrough')
            assert back_context['destination'] == 'Back' and back_context['qualification']['status'] == 'unqualified', back_context
            assert back_context['qualification']['reason'], back_context
            raw = command('site-policy-explain', {'source': 'authored:0', 'id': 'diagnostics'})
            assert raw['rule']['desired']['rule']['interaction']['anchors'][0]['code'] == 'm', raw
            assert raw['context_error'] and raw['activation_permitted'] is False, raw
            missing = command('site-policy-explain', {'source': 'authored:0', 'id': 'absent'}, 'E_NOT_FOUND')
            assert 'selected source' in missing['message'], missing
            command('site-policy-explain', {'source': 'authored:999', 'id': 'diagnostics'}, 'E_NOT_FOUND')
            command('site-policy-explain', {'key': 'p'}, 'E_INVALID_ARGUMENT')
            for code, exclusions in [('p', [{'character': 'p', 'reason': 'page-source'}]), ('k', [])]:
                codes = command('site-policy-explain', {'url': 'https://diagnostics.test/', 'code': code})
                link_book = next(book for book in codes['active']['explanation']['codebooks'] if book['family'] == 'links')
                assert link_book['requested_code']['excluded_characters'] == exclusions, link_book
            exports = root / 'exports'
            exports.mkdir()
            shared = exports / 'policy.toml'
            export = command('site-policy-export', {'path': str(shared), 'source': 'authored:0', 'id': 'diagnostics'})
            assert export['status'] == 'pending-export' and export['activation_permitted'] is False, export
            assert not finish_transfer(export)['cancellable'], export
            document = tomllib.loads(shared.read_text())
            assert document['format'] == 'ferric-site-interaction' and document['format_version'] == 1, document
            assert document['rules'][0]['id'] == 'diagnostics' and 'set' not in document['rules'][0], document
            assert 'semantic-anchors-v1' in document['required_capabilities'], document
            assert shared.stat().st_mode & 0o777 == 0o600, oct(shared.stat().st_mode)
            exported_bytes = shared.read_bytes()
            duplicate = command('site-policy-export', {'path': str(shared), 'source': 'authored:0', 'id': 'diagnostics'})
            assert finish_transfer(duplicate, 'failed')['error']['code'] == 'E_IO', duplicate
            assert shared.read_bytes() == exported_bytes
            finish_transfer(command('site-policy-export', {'path': str(shared), 'source': 'authored:0', 'id': 'diagnostics', 'overwrite': True}))
            sentinel = exports / 'sentinel'
            sentinel.write_text('untouched')
            link = exports / 'link.toml'
            link.symlink_to(sentinel)
            rejected = command('site-policy-export', {'path': str(link), 'source': 'authored:0', 'id': 'diagnostics', 'overwrite': True})
            assert finish_transfer(rejected, 'failed')['error']['code'] == 'E_IO'
            assert link.is_symlink() and sentinel.read_text() == 'untouched'
            config_bytes = config.read_bytes()
            for protected in [config, runtime]:
                rejected = command('site-policy-export', {'path': str(protected), 'source': 'authored:0', 'id': 'diagnostics', 'overwrite': True})
                assert finish_transfer(rejected, 'failed')['error']['code'] == 'E_IO'
            assert config.read_bytes() == config_bytes and runtime.read_bytes() == before
            collision = command('site-policy-import', {'path': str(shared)})
            assert collision['status'] == 'pending-import' and collision['draft'], collision
            assert finish_transfer(collision, 'failed')['error']['code'] == 'E_CONFIG'
            imported = command('site-policy-import', {'path': str(shared), 'temporary': True, 'replace_existing': True})
            finish_transfer(imported)
            reviewed = command('site-policy-edit', {'draft': imported['draft'], 'revision': 1, 'rebase': True})
            assert reviewed['source'] == 'temporary' and reviewed['rules'][0]['copied_from']['set']['content.zoom'] == 1.25, reviewed
            assert reviewed['rules'][0]['proposed']['set']['content.zoom'] == 1.25, reviewed
            draft_path = exports / 'draft.toml'
            saved_draft = command('site-policy-export', {'path': str(draft_path), 'draft': imported['draft']})
            assert saved_draft['draft'] == imported['draft'] and int(saved_draft['revision']) == int(reviewed['revision']), saved_draft
            assert saved_draft['persistence_warning'], saved_draft
            finish_transfer(saved_draft)
            assert 'set' not in tomllib.loads(draft_path.read_text())['rules'][0]
            bad = exports / 'bad.toml'
            bad.write_text(shared.read_text().replace('priority = 0', "priority = 0\nset = {'content.zoom'=3}"))
            assert bad.read_bytes() != shared.read_bytes(), bad.read_text()
            rejected = command('site-policy-import', {'path': str(bad), 'replace_existing': True})
            assert finish_transfer(rejected, 'failed')['error']['code'] == 'E_CONFIG'
            rejected = command('site-policy-import', {'path': str(link), 'replace_existing': True})
            assert finish_transfer(rejected, 'failed')['error']['code'] == 'E_IO'
            watched_export = config.parent / 'shared-policy.toml'
            finish_transfer(command('site-policy-export', {'path': str(watched_export), 'source': 'authored:0', 'id': 'diagnostics'}))
            for _ in range(40):
                time.sleep(.05)
                unchanged = command('site-policy-list', {'url': 'https://example.test/'})
                assert unchanged['configuration_revision'] == listed['configuration_revision'], unchanged
                assert unchanged['active_revision'] == listed['active_revision'], unchanged
            discarded = command('site-policy-discard', {'draft': opened['id']})
            assert discarded['draft'] == opened['id'] and discarded['discarded'] is True, discarded
            assert runtime.read_bytes() == before, 'listing/discard wrote configuration'
            config.write_text(config.read_text().replace("set={'content.zoom'=1.25}", "set={'content.zoom'=1.5}"))
            reloaded = wait(lambda: command('site-policy-list', {'url': 'https://example.test/'}),
                            lambda value: value['active_revision'] > listed['active_revision'])
            assert reloaded['configuration_revision'] > listed['configuration_revision'], reloaded
            preview_server = ThreadingHTTPServer(('127.0.0.1', 0), PreviewFixture)
            preview_server.activations = 0
            threading.Thread(target=preview_server.serve_forever, daemon=True).start()
            preview_url = f'http://127.0.0.1:{preview_server.server_port}/'
            command('open', {'input': preview_url})
            preview_tabs = wait(lambda: client.rpc('tabs.query', {'window': owner}),
                                lambda value: value['tabs'][0]['loading'] == 'complete' and value['tabs'][0]['url'] == preview_url)
            preview_file = exports / 'native-preview.toml'
            preview_file.write_text(f'''format='ferric-site-interaction'
format_version=1
[[rules]]
operation='upsert-interaction'
id='ipc-preview'
pattern='{preview_url}*'
priority=0
[rules.interaction]
enabled=true
[rules.interaction.labels]
auto_follow='full-match'
fallback_prefix='x'
[[rules.interaction.anchors]]
id='navigation'
code='na'
families=['all','links']
target_selector='#preview'
''')
            preview_import = command('site-policy-import', {'path': str(preview_file), 'temporary': True})
            finish_transfer(preview_import)
            static = command('site-policy-preview', {'draft': preview_import['draft']})
            assert static['kind'] == 'static-only' and static['valid'] and not static['activation_permitted'], static
            preview_before = runtime.read_bytes()
            ticket = command('site-policy-preview', {'draft': preview_import['draft'], 'tab': preview_tabs['tabs'][0]['id']})
            operation = finish_transfer(ticket)
            live = operation['result']
            assert live['kind'] == 'live' and live['revision'] == static['revision'], live
            assert live['activation_permitted'] is False and live['observations']['complete'], live
            invocations = live['observations']['invocations']
            assert len(invocations) == 11, invocations
            links = next(row['result'] for row in invocations if row['family'] == 'links')
            assert links['reservations'][0]['code'] == 'na' and links['reservations'][0]['state'] == 'Live', links
            assert all('result' not in row for row in client.rpc('operations.query', {})['operations'])
            assert preview_server.activations == 0 and runtime.read_bytes() == preview_before
            preview_after = command('site-policy-list', {'url': preview_url})
            assert preview_after['active_revision'] == reloaded['active_revision'], preview_after
            record.update(status='passed', draft_id=draft['id'], operation=rows[0],
                          checks=['structured draft ID returned to IPC caller', 'draft creation did not write',
                                  'same original operation completes after activation', 'actual generated file contains reviewed rule',
                                  'explicit rebase returns new revision without writing',
                                  'matching, nonmatching and all-rules lists return structured saved/active state',
                                  'discard returns a structured result without writing configuration',
                                  'explain returns winning sources, checked reservations and static capacity without live proof',
                                  'key explain separates qualified ASCII mapping, excluded native destination and live availability',
                                  'exact authored source explain works without an HTTP page or a draft',
                                  'unknown source/ID and missing HTTP context return precise typed errors',
                                  'code exclusions use page source keys, not remap destinations',
                                  'constrained export omits local set, uses private permissions and requires explicit overwrite',
                                  'symlink and managed configuration destinations are refused without touching their targets',
                                  'import returns a pending draft identity and preserves local settings in an isolated temporary draft',
                                  'draft export binds its captured revision and warns that a temporary export persists',
                                  'malformed and symlink imports return typed failures without configuration writes',
                                  'export inside a watched parent does not activate configuration; actual source edits still reload',
                                  'static preview and nonactivating native live preview expose immutable receipts through authorized exact-operation queries',
                                  'ordinary operation lists omit preview bodies; live preview neither activates a link nor writes/activates configuration'])
        except Exception:
            log.seek(0)
            record['browser_log_tail'] = log.read()[-6000:]
            print(json.dumps(record, indent=2), file=sys.stderr)
            raise
        finally:
            if preview_server is not None:
                preview_server.shutdown()
                preview_server.server_close()
            if client is not None:
                client.close()
            app.terminate()
            try:
                app.wait(timeout=5)
            except subprocess.TimeoutExpired:
                app.kill()
                app.wait(timeout=5)
    return record


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--executable', required=True, type=Path)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    result = run(args.executable.resolve())
    if args.output:
        args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))

#!/usr/bin/env python3
"""Exercise saved Journey identities with isolated profiles and loopback pages."""
import argparse
from contextlib import closing
import json
import os
from pathlib import Path
import signal
import sqlite3
import subprocess
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

from run_download_ipc_probe import connect, rpc


def run_case(executable, artifacts, mode):
    artifacts.mkdir(parents=True, exist_ok=False)
    release = threading.Event()
    committed = threading.Event()
    facts = {'requests': [], 'mode': mode, 'artifacts': str(artifacts)}

    class Handler(BaseHTTPRequestHandler):

        def log_message(self, *args):
            pass

        def do_GET(self):
            facts['requests'].append(self.path)
            if self.path == '/committed':
                committed.set()
            title = (f"Same visit {facts['requests'].count('/same')}"
                     if self.path == '/same' else 'Owned visit')
            body = f'<!doctype html><title>{title}</title>Owned visit'.encode()
            tail = b''
            if self.path == '/stream' and facts['requests'].count('/stream') == 1:
                body = (b'<!doctype html><title>Streaming visit</title>'
                        b'<script>fetch("/committed",{cache:"no-store"})</script>'
                        b'Streaming visit' + b' ' * 8192)
                tail = b' ' * 8192
            self.send_response(200)
            self.send_header('Content-Type', 'text/html')
            self.send_header('Cache-Control', 'no-store')
            self.send_header('Content-Length', str(len(body) + len(tail)))
            self.end_headers()
            self.wfile.write(body)
            self.wfile.flush()
            if tail:
                if release.wait(90):
                    try:
                        self.wfile.write(tail)
                        facts['response_tail_sent'] = True
                    except (BrokenPipeError, ConnectionResetError):
                        pass

    with tempfile.TemporaryDirectory(prefix='fj-') as temporary:
        root = Path(temporary)
        env = os.environ.copy()
        env.update(QT_QPA_PLATFORM='offscreen', QT_QUICK_BACKEND='software',
                   QT_QPA_PLATFORMTHEME='', QTWEBENGINE_CHROMIUM_FLAGS='--disable-gpu',
                   FERRIC_BROWSER_ALLOW_NON_WAYLAND='1')
        for name in ['XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME',
                     'XDG_CACHE_HOME', 'XDG_RUNTIME_DIR']:
            folder = root / name
            folder.mkdir(mode=0o700)
            env[name] = str(folder)
        config = root / 'config.toml'
        config.write_text('schema_version=3\n')
        prefix = [str(executable.resolve(strict=True)), '--basedir', str(root / 'browser'),
                  '--instance', 'streaming-journey-probe', '--config', str(config)]
        server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        server.daemon_threads = True
        threading.Thread(target=server.serve_forever, daemon=True).start()
        base = f'http://127.0.0.1:{server.server_port}'

        def command(*args):
            result = subprocess.run([*prefix, *args], env=env, capture_output=True,
                                    text=True, timeout=10)
            assert result.returncode == 0, (args, result.stdout, result.stderr)
            return result.stdout

        endpoint = []

        def tab():
            if not endpoint:
                endpoint.extend((root / 'browser').glob('runtime/instances/*/browser.sock'))
            assert len(endpoint) == 1, endpoint
            with connect(endpoint[0]) as stream:
                value = rpc(stream, 'tabs.query', {'active_only': True})
            assert 'error' not in value, value
            return value['result']['tabs'][0]

        def wait_tab(url, title=None):
            end = time.monotonic() + 15
            while time.monotonic() < end:
                value = tab()
                if (value['url'] == url and value['loading'] == 'complete'
                        and (title is None or value['title'] == title)):
                    return value
                time.sleep(0.1)
            raise AssertionError(value)

        journey_database = []

        def journey_rows(query, parameters=()):
            for path in journey_database or (root / 'browser').rglob('*'):
                if not path.is_file() or path.name.endswith(('-wal', '-shm')):
                    continue
                with path.open('rb') as source:
                    if source.read(16) != b'SQLite format 3\x00':
                        continue
                try:
                    with closing(sqlite3.connect(f'file:{path}?mode=ro', uri=True)) as db:
                        if not journey_database and not db.execute(
                                "SELECT 1 FROM sqlite_master WHERE name='journey_current'").fetchone():
                            continue
                        if not journey_database:
                            journey_database.append(path)
                        return db.execute(query, parameters).fetchall()
                except sqlite3.OperationalError:
                    if journey_database:
                        raise
            return []

        source_tab = []

        def current_rows():
            return journey_rows(
                'SELECT c.tab_id,n.url,n.id FROM journey_current c '
                'JOIN journey_nodes n ON n.id=c.node_id WHERE c.tab_id=?', (source_tab[0],))

        def storage_barrier():
            # Its later write uses the same FIFO worker as the source pointer.
            barrier_url = base + '/storage-barrier'
            command('open', '--target', 'tab-bg', barrier_url)
            end = time.monotonic() + 15
            while not journey_rows('SELECT id FROM journey_nodes WHERE url=?', (barrier_url,)):
                assert browser.poll() is None and time.monotonic() < end, \
                    'Saved background visit barrier missing'
                time.sleep(0.05)
            facts['storage_barrier'] = journey_rows(
                'SELECT id,tab_id FROM journey_nodes WHERE url=?', (barrier_url,))
            assert tab()['id'] == source_tab[0], 'Background barrier changed the active tab'

        writer = None
        with (artifacts / 'browser.log').open('w') as log:
            browser = subprocess.Popen(
                [*prefix, '--software-rendering', '--userscripts-off', 'open', base + '/first'],
                env=env, stdout=log, stderr=log, start_new_session=True)
            try:
                end = time.monotonic() + 25
                while not any((path == '/first' for path in facts['requests'])):
                    assert browser.poll() is None and time.monotonic() < end
                    time.sleep(0.1)
                source_tab.append(wait_tab(base + '/first')['id'])
                command('open', base + '/form')
                facts['completed_before'] = wait_tab(base + '/form')
                end = time.monotonic() + 10
                while not any((row[1] == base + '/form' for row in current_rows())):
                    assert time.monotonic() < end, current_rows()
                    time.sleep(0.1)
                facts['journey_before'] = current_rows()
                if mode == 'pending':
                    assert len(journey_database) == 1, 'Journey database not discovered'
                    writer = sqlite3.connect(journey_database[0], isolation_level=None, timeout=1)
                    writer.execute('BEGIN IMMEDIATE')
                    started = time.monotonic()
                    command('open', base + '/same')
                    wait_tab(base + '/same')
                    # Make the URL-based fallback select the newer visit
                    # deterministically, even with second-resolution storage.
                    first_second = int(time.time())
                    while int(time.time()) == first_second:
                        assert time.monotonic() - started < 3
                        time.sleep(0.01)
                    command('open', base + '/bridge')
                    wait_tab(base + '/bridge')
                    command('open', base + '/same')
                    wait_tab(base + '/same')
                    command('command', '--', 'back --count 2')
                    facts['after'] = wait_tab(base + '/same', title='Same visit 3')
                    facts['writer_hold_seconds'] = time.monotonic() - started
                    writer.execute('ROLLBACK')
                    writer.close()
                    writer = None
                    assert facts['writer_hold_seconds'] < 4.5, facts
                    storage_barrier()
                    facts['same_visits'] = journey_rows(
                        'SELECT id,title,committed_at FROM journey_nodes '
                        'WHERE tab_id=? AND url=? ORDER BY committed_at,id',
                        (source_tab[0], base + '/same'))
                    facts['journey_after'] = current_rows()
                    facts['all_nodes'] = journey_rows(
                        'SELECT id,tab_id,url,title,committed_at FROM journey_nodes '
                        'ORDER BY committed_at,id')
                    assert len(facts['same_visits']) == 2, facts
                    assert facts['same_visits'][0][2] < facts['same_visits'][1][2], facts
                    facts['matches_original_visit'] = (
                        facts['journey_after'][0][2] == facts['same_visits'][0][0])
                    assert facts['matches_original_visit'], facts
                else:
                    command('open', base + '/stream')
                    assert committed.wait(10)
                    facts['streaming'] = tab()
                    assert facts['streaming']['loading'] == 'provisional'
                    if mode == 'unrecorded':
                        command('open', base + '/after')
                        wait_tab(base + '/after')
                        end = time.monotonic() + 10
                        while not any((row[1] == base + '/after' for row in current_rows())):
                            assert time.monotonic() < end, current_rows()
                            time.sleep(0.1)
                        facts['journey_before_unrecorded_back'] = current_rows()
                        facts['stream_nodes_before_back'] = journey_rows(
                            'SELECT id,url FROM journey_nodes WHERE url=?', (base + '/stream',))
                        assert facts['stream_nodes_before_back'] == []
                    facts['tail_withheld_before_back'] = not facts.get('response_tail_sent', False)
                    assert facts['tail_withheld_before_back']
                    command('command', '--', 'back')
                    facts['after'] = wait_tab(base + ('/stream' if mode == 'unrecorded' else '/form'))
                    storage_barrier()
                    facts['journey_after'] = current_rows()
                    facts['matches_visible_page'] = any(
                        row[1] == facts['after']['url'] for row in facts['journey_after'])
                    facts['matches_original_visit'] = facts['journey_after'] == facts['journey_before']
                    if mode == 'retained':
                        assert facts['matches_visible_page'] and facts['matches_original_visit'], facts
                    else:
                        facts['stream_nodes_after_back'] = journey_rows(
                            'SELECT id,url FROM journey_nodes WHERE url=?', (base + '/stream',))
                        assert facts['matches_visible_page'] and len(facts['stream_nodes_after_back']) == 1, facts
            except BaseException as error:
                facts['error'] = repr(error)
                raise
            finally:
                if writer is not None:
                    writer.execute('ROLLBACK')
                    writer.close()
                release.set()
                try:
                    os.killpg(browser.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                try:
                    browser.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(browser.pid, signal.SIGKILL)
                    browser.wait(timeout=5)
                server.shutdown()
                (artifacts / 'observation.json').write_text(json.dumps(facts, indent=2) + '\n')
    print(json.dumps(facts, indent=2))
    return facts


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--executable', type=Path, required=True)
    parser.add_argument('--artifacts', type=Path, required=True)
    parser.add_argument('--case', choices=['all', 'retained', 'unrecorded', 'pending'],
                        default='all')
    args = parser.parse_args()
    root = args.artifacts.resolve()
    root.mkdir(parents=True, exist_ok=True)
    run = Path(tempfile.mkdtemp(prefix='run-', dir=root))
    cases = ['retained', 'unrecorded', 'pending'] if args.case == 'all' else [args.case]
    for case in cases:
        run_case(args.executable, run / case, case)
    print(json.dumps({'status': 'complete', 'cases': cases, 'artifacts': str(run)}))
    return 0


def terminate(signum, frame):
    raise SystemExit(128 + signum)


if __name__ == '__main__':
    signal.signal(signal.SIGTERM, terminate)
    raise SystemExit(main())

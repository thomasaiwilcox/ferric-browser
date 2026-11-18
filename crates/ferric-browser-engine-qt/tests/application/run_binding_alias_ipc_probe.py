#!/usr/bin/env python3
"""Check canonical binding queries on a disposable production browser."""
import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import threading


class Fixture(BaseHTTPRequestHandler):
    def do_GET(self):
        body = b'<!doctype html><title>Binding aliases</title><button>Fixture</button>'
        self.send_response(200)
        self.send_header('Content-Type', 'text/html; charset=utf-8')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--executable", type=Path, required=True)
    args = parser.parse_args()
    env = dict(os.environ, QT_QPA_PLATFORM="offscreen", QT_QPA_PLATFORMTHEME="",
               QT_QUICK_BACKEND="software", QTWEBENGINE_CHROMIUM_FLAGS="--disable-gpu",
               FERRIC_BROWSER_ALLOW_NON_WAYLAND="1")
    with tempfile.TemporaryDirectory(prefix="ferric-binding-alias-ipc-") as temporary:
        prefix = [str(args.executable.resolve()), "--basedir", temporary, "--instance", "binding-alias"]
        with tempfile.TemporaryFile(mode="w+") as log:
            server = ThreadingHTTPServer(('127.0.0.1', 0), Fixture)
            threading.Thread(target=server.serve_forever, daemon=True).start()
            fixture_url = f'http://127.0.0.1:{server.server_port}/fixture'
            app = subprocess.Popen(prefix + ["--software-rendering", "--userscripts-off", "open", fixture_url],
                                   env=env, stdout=log, stderr=log)

            def invoke(*arguments):
                assert app.poll() is None, "owned browser exited"
                previous = None
                if arguments[0] == "command":
                    previous = {row['operation_id'] for row in invoke('query', 'operations', '--format', 'json')['operations']}
                result = subprocess.run(prefix + list(arguments), env=env,
                                        capture_output=True, text=True, timeout=15)
                assert result.returncode == 0, (arguments, result.stdout, result.stderr)
                if arguments[0] == "query":
                    value = json.loads(result.stdout)
                    return value.get("result", value)
                if previous is not None:
                    rows = wait_for(lambda: invoke('query', 'operations', '--format', 'json')['operations'],
                                    lambda rows: any(row['operation_id'] not in previous and row['status'] in ('completed', 'failed', 'cancelled') for row in rows),
                                    ('terminal command receipt', arguments))
                    new = [row for row in rows if row['operation_id'] not in previous]
                    assert len(new) == 1 and new[0]['status'] == 'completed', (arguments, new)
                return result.stdout

            def wait_for(query, predicate, description):
                deadline = time.monotonic() + 20
                last = None
                while time.monotonic() < deadline:
                    try:
                        last = query()
                        if predicate(last):
                            return last
                    except (AssertionError, json.JSONDecodeError) as error:
                        last = str(error)
                    time.sleep(0.05)
                raise AssertionError((description, last))

            def explain(keys):
                return invoke("query", "binding-explain", keys, "--mode", "normal", "--format", "json")

            def bindings():
                return invoke("query", "bindings", "--mode", "normal", "--format", "json")

            def published(keys, command):
                return wait_for(lambda: explain(keys),
                                lambda value: (value.get("winning_binding") or {}).get("command") == command,
                                ("published binding", keys, command))

            def user_entry(keys, command):
                rows = bindings()["bindings"]
                row = next(row for row in rows if row["keys"] == keys and row["command"] == command)
                assert row["source"] == "user", row
                assert any(layer["layer"] == "runtime" and layer["value"] == command
                           for layer in row["provenance"]), row

            try:
                # Do not launch a CLI caller against a not-yet-initialized
                # storage root. The owned browser creates its socket afterward.
                wait_for(lambda: list(Path(temporary).glob('runtime/instances/*/browser.sock')),
                         lambda sockets: app.poll() is None and len(sockets) == 1,
                         'owned browser socket')
                wait_for(lambda: invoke("query", "active-tab", "--format", "json"),
                         lambda value: value.get("tabs", [{}])[0].get("url") == fixture_url
                         and value['tabs'][0].get('loading') == 'complete',
                         "owned browser bootstrap")
                wait_for(lambda: explain('+'),
                         lambda value: value['mixed_routing']['status'] == 'ready',
                         'local HTTP policy ready')
                for keys in ("+", "Plus"):
                    value = explain(keys)
                    assert value["resolution"] == "exact", value
                    assert value["winning_binding"]["command"] == "zoom in", value
                    assert value["winning_binding"]["source"] == "built-in", value
                print("default plus explanation agrees with canonical input", flush=True)

                invoke("command", "bind Plus,g zoom out")
                wait_for(lambda: explain("Plus"), lambda value: value["resolution"] == "exact-prefix",
                         "canonical prefix ambiguity")
                assert "g" in explain("+")["continuations"]
                assert any(set(conflict["commands"]) == {"zoom in", "zoom out"}
                           for conflict in bindings()["conflicts"])
                user_entry(["Plus", "g"], "zoom out")

                invoke("command", "set --temp input.shortcut_policy=mixed")
                invoke("command", "set --temp \"input.page_keys=['Plus']\"")
                routed = wait_for(lambda: explain("+"),
                                  lambda value: value["mixed_routing"]["owner"] == "page", "Mixed Plus route")
                suppressed = routed["mixed_routing"]["browser_bindings_suppressed_by_page_mapping"]
                assert any(row["keychain"] == "+" and row["command"] == "zoom in" for row in suppressed), routed
                invoke("command", "set --temp input.shortcut_policy=browser-first")
                invoke("command", "unbind Plus,g")
                wait_for(lambda: explain("+"), lambda value: value["resolution"] == "exact", "removed continuation")
                print("prefix conflicts and page suppression use canonical identity", flush=True)

                invoke("command", "bind + zoom out")
                value = published("Plus", "zoom out")
                assert value["winning_binding"]["source"] == "user", value
                user_entry(["Plus"], "zoom out")
                invoke("command", "bind g,g zoom reset")
                value = published("3gg", "zoom reset")
                assert value["count"]["value"] == 3, value
                assert value["winning_binding"]["source"] == "user", value
                assert any(layer["layer"] == "runtime" and layer["value"] == "zoom reset"
                           for layer in value["winning_binding"]["provenance"]), value
                user_entry(["g", "g"], "zoom reset")
                invoke("command", "bind Ctrl+v,g zoom 1.25")
                published("Ctrl+v,g", "zoom 1.25")
                user_entry(["Ctrl+v", "g"], "zoom 1.25")
                print("alias, counted and modified-chain provenance matches active bindings", flush=True)

                invoke("command", "unbind Plus")
                wait_for(lambda: explain("+"), lambda value: value["resolution"] == "unmatched", "alias unbind")
                print("binding alias IPC probe passed", flush=True)
                return 0
            finally:
                if app.poll() is None:
                    app.terminate()
                    try:
                        app.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        app.kill()
                        app.wait(timeout=5)
                server.shutdown()
                server.server_close()
                log.seek(0)
                print(log.read()[-4000:], flush=True)


if __name__ == "__main__":
    raise SystemExit(main())

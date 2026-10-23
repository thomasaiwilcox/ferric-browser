"""Recover real killed browser processes using owned HTTP, storage and Qt input."""
import collections
import hashlib
import http.server
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import threading
import time


def scenario(build, browser, startup):
    with tempfile.TemporaryDirectory(prefix="frsr-") as temporary:
        root = Path(temporary)
        base = root / "profile"
        base.mkdir()
        env = dict(os.environ, QT_QPA_PLATFORM="offscreen", QT_QUICK_BACKEND="software",
                   QT_QPA_PLATFORMTHEME="", QTWEBENGINE_CHROMIUM_FLAGS="--disable-gpu")
        for key, folder in [("XDG_RUNTIME_DIR", "runtime"), ("XDG_CONFIG_HOME", "config"),
                            ("XDG_CACHE_HOME", "cache"), ("XDG_DATA_HOME", "data"),
                            ("XDG_STATE_HOME", "state")]:
            path = root / folder
            path.mkdir(mode=0o700)
            env[key] = str(path)
        processes, logs = [], []
        requests = collections.Counter()

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                requests[self.path] += 1
                body = ("<!doctype html><title>" + self.path + "</title><input id='draft'>").encode()
                self.send_response(200)
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *_):
                pass

        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        origin = f"http://127.0.0.1:{server.server_port}"
        old_a, old_b = origin + "/old-a?keep=yes", origin + "/old-b"
        new_a, new_b = origin + "/new-a", origin + "/new-b"
        prefix = [str(browser), "--basedir", str(base), "--instance", "recovery-test"]

        def wait(check, label, timeout=30):
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                for error in root.glob("*/error"):
                    raise AssertionError(error.read_text())
                value = check()
                if value:
                    return value
                time.sleep(0.05)
            raise AssertionError("Timed out: " + label)

        def cli(*args, check=True):
            result = subprocess.run(prefix + list(args), env=env, capture_output=True, text=True, timeout=10)
            if check:
                assert result.returncode == 0, (args, result.stdout, result.stderr)
            return result

        def observed(directory):
            # The native helper publishes a small file on another thread of
            # execution. A partially observed write is not a failed browser.
            try:
                return json.loads((directory / "after.json").read_text())
            except (FileNotFoundError, json.JSONDecodeError):
                return None

        def tabs():
            result = cli("query", "tabs", "--format", "json", check=False)
            return json.loads(result.stdout)["tabs"] if result.returncode == 0 else []

        def launch(phase, url, recover=False):
            directory = root / phase
            directory.mkdir()
            current = dict(env)
            if recover:
                (directory / ".owned-recovery-fixture").write_text("Owned startup recovery test only.\n")
                current.update(QT_PLUGIN_PATH=str(build / "plugins"),
                               QT_QPA_GENERIC_PLUGINS="ferricownedstartuprecovery",
                               FERRIC_OWNED_STARTUP_RECOVERY_ROOT=str(directory))
            output = (directory / "browser.log").open("w")
            logs.append(output)
            child = subprocess.Popen(prefix + ["--software-rendering", "--userscripts-off", "open", url],
                                     env=current, stdout=output, stderr=subprocess.STDOUT, start_new_session=True)
            processes.append(child)
            return child, directory

        def kill(child):
            if child.poll() is None:
                os.killpg(child.pid, signal.SIGKILL)
            child.wait(timeout=5)

        def addresses(snapshot):
            return [tab["safe_restore_url"] for window in snapshot["windows"] for tab in window["tabs"]]

        def checkpoint_urls(expected, require_lineage=False):
            for path in base.rglob("current*.json"):
                try:
                    snapshot = json.loads(path.read_text())
                except (OSError, ValueError):
                    continue
                if sorted(addresses(snapshot)) == sorted(expected) and (not require_lineage or snapshot["recovered_session_ids"]):
                    return snapshot
            return None

        try:
            warm, _ = launch("warm", old_a)
            wait(lambda: any(tab["url"] == old_a for tab in tabs()), "warm first tab")
            cli("command", "--", "tab-open", old_b)
            wait(lambda: len(tabs()) == 2 and {tab["url"] for tab in tabs()} == {old_a, old_b}, "warm tabs")
            wait(lambda: checkpoint_urls([old_a, old_b]), "warm durable checkpoint")
            kill(warm)
            cold, directory = launch("cold", "rb://start" if startup == "start" else new_a, True)
            wait(lambda: len(tabs()) == 1, "cold first tab")
            if startup == "link":
                wait(lambda: tabs()[0]["url"] == new_a, "incoming link")
                cli("command", "--", "tab-open", new_b)
                wait(lambda: len(tabs()) == 2 and {tab["url"] for tab in tabs()} == {new_a, new_b}, "new work")
            current_ids = {tab["id"]: tab["url"] for tab in tabs()} if startup == "link" else {}
            expected = [old_a, old_b] + ([new_a, new_b] if startup == "link" else [])
            (directory / "recover").write_text("Recover this owned browser.\n")
            after = wait(lambda: observed(directory), "native recovery")
            assert sorted(tab["modelUrl"] for tab in after) == sorted(expected), after
            for tab_id, url in current_ids.items():
                assert any(tab["id"] == tab_id and tab["viewUrl"] == url for tab in after), after
            assert requests["/old-a?keep=yes"] == 1, requests
            assert not next(tab for tab in after if tab["modelUrl"] == old_a)["loaded"], after
            if startup == "link":
                assert requests["/new-a"] == 1 and requests["/new-b"] == 1, requests
            cli("command", "--", "session-save", "after-recovery")
            named = wait(lambda: next(base.rglob("after-recovery.json"), None), "named recovery save")
            saved = json.loads(named.read_text())
            assert sorted(addresses(saved)) == sorted(expected), saved
            assert saved["schema_version"] == 4 and saved["recovered_session_ids"], saved
            # Wait for the merged current generation before another forced
            # exit. Its ancestry must suppress the retained older files.
            wait(lambda: checkpoint_urls(expected, True), "merged durable checkpoint")
            kill(cold)
            again, directory = launch("again", "rb://start", True)
            wait(lambda: len(tabs()) == 1, "second restart")
            (directory / "recover").write_text("Recover this owned browser.\n")
            after_again = wait(lambda: observed(directory), "second recovery")
            assert sorted(tab["modelUrl"] for tab in after_again) == sorted(expected), after_again
            assert requests["/old-a?keep=yes"] == 1, requests
            print(json.dumps({"case": startup, "restored": len(expected), "restarts": 2,
                              "lazy_old_a_requests": requests["/old-a?keep=yes"], "passed": True}), flush=True)
            kill(again)
        finally:
            for child in processes:
                kill(child)
            server.shutdown()
            for output in logs:
                output.close()
            if sys.exc_info()[0]:
                for log in root.glob("*/browser.log"):
                    print(str(log), log.read_text()[-12000:], file=sys.stderr)


def main():
    build, browser = (Path(value).resolve() for value in sys.argv[1:3])
    print(json.dumps({"browser_sha256": hashlib.sha256(browser.read_bytes()).hexdigest()}), flush=True)
    for startup in ("start", "link"):
        scenario(build, browser, startup)


if __name__ == "__main__":
    main()

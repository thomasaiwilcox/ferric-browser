#!/usr/bin/env python3
"""Verify external Switcher privacy through the real local IPC endpoint."""
import argparse
import json
import os
from pathlib import Path
import signal
import socket
import struct
import subprocess
import tempfile
import time
import uuid

ROOT = Path(__file__).resolve().parents[4]


def receive(stream, count, deadline):
    data = bytearray()
    while len(data) < count:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError("Owned IPC response timed out")
        stream.settimeout(remaining)
        chunk = stream.recv(count - len(data))
        if not chunk:
            raise RuntimeError("Owned IPC endpoint closed before replying")
        data.extend(chunk)
    return data


def frame(stream, method, params):
    request = {"id": "owned-" + uuid.uuid4().hex, "method": method, "params": params}
    payload = json.dumps(request).encode()
    stream.settimeout(8)
    stream.sendall(struct.pack(">I", len(payload)) + payload)
    deadline = time.monotonic() + 8
    length = struct.unpack(">I", receive(stream, 4, deadline))[0]
    assert 0 < length <= 1024 * 1024, "Invalid owned IPC frame size"
    return json.loads(receive(stream, length, deadline))


def request(endpoint, method, params):
    with socket.socket(socket.AF_UNIX) as stream:
        stream.settimeout(8)
        stream.connect(str(endpoint))
        hello = frame(stream, "hello", {"protocol_major": 2, "protocol_minor": 0,
                                       "client": "ferric-owned-switcher-test"})
        assert "error" not in hello, hello
        return frame(stream, method, params)


def wait_for(probe, accepted, timeout=25):
    deadline = time.monotonic() + timeout
    while True:
        value = probe()
        if accepted(value):
            return value
        assert time.monotonic() < deadline, value
        time.sleep(.05)


def run_case(executable, base, enabled, observations):
    base.mkdir()
    (base / ".ferric-owned-fixture").write_text("Owned Switcher IPC privacy test only.\n")
    config = base / "browser.toml"
    config.write_text("schema_version=3\n[ui]\nfont_size_pt=12\n[ipc]\nprivate_queries="
                      + ("true" if enabled else "false") + "\n")
    env = os.environ.copy()
    env.update(QT_QPA_PLATFORM="offscreen", QT_QPA_PLATFORMTHEME="",
               QT_QUICK_BACKEND="software", QTWEBENGINE_CHROMIUM_FLAGS="--disable-gpu",
               FERRIC_BROWSER_ALLOW_NON_WAYLAND="1")
    for key, folder in [("XDG_CONFIG_HOME", "config"), ("XDG_CACHE_HOME", "cache"),
                        ("XDG_DATA_HOME", "data"), ("XDG_RUNTIME_DIR", "runtime")]:
        path = base / folder
        path.mkdir(mode=0o700)
        env[key] = str(path)
    prefix = [str(executable), "--basedir", str(base / "browser"),
              "--instance", "switcher-privacy", "--config", str(config)]
    facts = {"private_queries": enabled}
    observations.append(facts)
    with tempfile.TemporaryFile(mode="w+") as log:
        app = subprocess.Popen([*prefix, "--software-rendering", "--userscripts-off",
                                "open", "rb://blank"], env=env, stdout=log, stderr=log,
                               start_new_session=True)
        try:
            def ordinary():
                assert app.poll() is None, "Owned browser exited during startup"
                endpoints = [path for path in base.rglob("*") if path.is_socket()]
                if len(endpoints) != 1:
                    return None
                response = request(endpoints[0], "switcher.query", {"scope": "tabs"})
                facts["ordinary_control"] = response
                if response.get("result", {}).get("results"):
                    return endpoints[0]
                return None

            endpoint = wait_for(ordinary, lambda value: value is not None)
            child = subprocess.run([*prefix, "command", "window-new --private"], env=env,
                                   capture_output=True, text=True, timeout=12)
            assert child.returncode == 0, (child.stdout, child.stderr)

            def private_ready():
                assert app.poll() is None, "Owned browser exited while opening its private window"
                profiles = request(endpoint, "profiles.query", {}).get("result", {}).get("profiles", [])
                private = [profile for profile in profiles if profile.get("private") is True]
                # The ordinary window still has its one blank tab. A successful
                # empty live-tab query, after private admission, proves the
                # implicit owner is now the private window, with its rows filtered.
                filtered = request(endpoint, "switcher.query", {"scope": "tabs"})
                facts["private_owner_filtered_control"] = filtered
                if private and "error" not in filtered and filtered["result"]["results"] == []:
                    facts["private_profile_control"] = private
                    return private[0]["name"]
                return None

            private_scope = wait_for(private_ready, lambda value: value is not None)
            facts["queries"] = []
            for scope in [None, private_scope, "retired-owned-session"]:
                params = {"scope": "tabs", "include_private": True}
                if scope is not None:
                    params["private_scope"] = scope
                response = request(endpoint, "switcher.query", params)
                facts["queries"].append({"params": params, "response": response})
                if not enabled:
                    assert response.get("error", {}).get("code") == "E_DENIED", response
                elif scope == "retired-owned-session":
                    assert response.get("error", {}).get("code") == "E_STALE_TARGET", response
                else:
                    rows = response.get("result", {}).get("results", [])
                    assert rows and all(row["privacy"] == "private" for row in rows), response
            malformed = request(endpoint, "switcher.query", {"scope": "tabs", "include_private": "yes"})
            assert malformed.get("error", {}).get("code") == "E_INVALID_ARGUMENT", malformed
            facts["malformed_control"] = malformed
            facts["passed"] = True
        except BaseException:
            log.seek(0)
            facts["browser_log_tail"] = log.read()[-3000:]
            raise
        finally:
            if app.poll() is None:
                os.killpg(app.pid, signal.SIGTERM)
                try:
                    app.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(app.pid, signal.SIGKILL)
                    app.wait(timeout=5)
            facts["owned_process_exit"] = app.returncode


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--executable", type=Path, default=ROOT / "target/release/ferric-browser")
    args = parser.parse_args()
    observations = []
    try:
        with tempfile.TemporaryDirectory(prefix="fspriv-") as temporary:
            root = Path(temporary)
            for enabled in [False, True]:
                run_case(args.executable.resolve(), root / ("enabled" if enabled else "disabled"),
                         enabled, observations)
        print("External Switcher privacy: disabled and explicitly enabled cases passed")
    finally:
        artifacts = os.environ.get("FERRIC_TEST_ARTIFACT_DIR")
        if artifacts:
            destination = Path(artifacts) / "switcher-private-ipc.json"
            destination.write_text(json.dumps(observations, indent=2) + "\n")


if __name__ == "__main__":
    main()

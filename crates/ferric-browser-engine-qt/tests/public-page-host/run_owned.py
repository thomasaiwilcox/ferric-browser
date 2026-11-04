#!/usr/bin/env python3
"""Run native host tests with the existing owned compositor's seat focus."""

import argparse
from contextlib import ExitStack
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compositor", type=Path, required=True)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        parser.error("A test command is required")
    if os.environ.get("FERRIC_PORTAL_TEST_BUS") != "isolated":
        parser.error("Use dbus-run-session with notification-test-bus.conf")

    fixture = Path(__file__).resolve().parents[1] / "portal/run.py"
    spec = importlib.util.spec_from_file_location("owned_portal_fixture", fixture)
    runner = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(runner)
    with tempfile.TemporaryDirectory(prefix="ferric-public-host-") as temporary, ExitStack() as stack:
        env = dict(os.environ, QT_QPA_PLATFORMTHEME="", QT_QUICK_BACKEND="software",
                   QTWEBENGINE_CHROMIUM_FLAGS="--disable-gpu",
                   QT_WAYLAND_DISABLE_WINDOWDECORATION="1",
                   WAYLAND_DISPLAY="ferric-public-host",
                   FERRIC_PORTAL_FIXTURE_LIFETIME_MS="1800000")
        for variable, folder in [
            ("XDG_RUNTIME_DIR", "runtime"), ("XDG_CONFIG_HOME", "config"),
            ("XDG_CACHE_HOME", "cache"), ("XDG_DATA_HOME", "data"),
            ("XDG_STATE_HOME", "state"),
        ]:
            path = Path(temporary) / folder
            path.mkdir(mode=0o700)
            env[variable] = str(path)
        server_env = dict(env, QT_QPA_PLATFORM="offscreen",
                          FERRIC_PARENT_DISPLAY=env["WAYLAND_DISPLAY"])
        runner.start(stack, [str(args.compositor.resolve())], server_env)
        env["QT_QPA_PLATFORM"] = "wayland"
        return subprocess.run(command, env=env).returncode


if __name__ == "__main__":
    raise SystemExit(main())

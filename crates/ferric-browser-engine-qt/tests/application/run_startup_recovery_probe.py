#!/usr/bin/env python3
"""Check public startup recovery errors without touching existing browser data."""

import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile


XDG_ROOTS = (
    "XDG_CONFIG_HOME", "XDG_CACHE_HOME", "XDG_DATA_HOME",
    "XDG_STATE_HOME", "XDG_RUNTIME_DIR",
)
CASES = (
    "custom-unmarked", "custom-incompatible", "xdg-unmarked", "xdg-incompatible",
    "long-custom-startup", "long-custom-client", "long-xdg-startup", "long-xdg-client",
)


def run_case(browser, root, name, facts):
    base = root / name
    base.mkdir()
    env = os.environ.copy()
    env.update(
        QT_QPA_PLATFORM="offscreen", QT_QPA_PLATFORMTHEME="",
        QT_QUICK_BACKEND="software", QTWEBENGINE_CHROMIUM_FLAGS="--disable-gpu",
        FERRIC_BROWSER_ALLOW_NON_WAYLAND="1",
    )
    for key in XDG_ROOTS:
        folder = base / key
        folder.mkdir(mode=0o700)
        env[key] = str(folder)

    sentinels = {}

    def sentinel(path, content):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
        sentinels[path] = content

    custom = name.startswith(("custom-", "long-custom-"))
    long_address = name.startswith("long-")
    selected = base / "selected"
    if name.startswith("long-custom-"):
        selected = selected / ("a" * 64) / ("b" * 64)
    if name.startswith("long-xdg-"):
        runtime = base / ("r" * 64) / ("s" * 64)
        runtime.mkdir(parents=True, mode=0o700)
        env["XDG_RUNTIME_DIR"] = str(runtime)

    data = selected / "data" if custom else Path(env["XDG_DATA_HOME"]) / "ferric-browser"
    config = selected / "config" if custom else Path(env["XDG_CONFIG_HOME"]) / "ferric-browser"
    state = selected / "state" if custom else Path(env["XDG_STATE_HOME"]) / "ferric-browser"
    if custom:
        for key in ("XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_STATE_HOME"):
            sentinel(Path(env[key]) / "ferric-browser" / "unrelated-owned-sentinel",
                     ("unrelated " + key).encode())
    if not long_address:
        sentinel(data / "keep-selected-data", b"selected data")
        sentinel(config / "keep-selected-config", b"selected config")
        if "incompatible" in name:
            sentinel(state / ".ferric-root-schema", b"999999\n")

    client = not long_address or name.endswith("client")
    for structured in ((False, True) if client else (False,)):
        command = [str(browser)]
        if custom:
            command.extend(("--basedir", str(selected)))
        command.extend(("query", "tabs") if client else ("open", "about:blank"))
        if structured:
            command.extend(("--format", "json"))
        result = subprocess.run(command, env=env, text=True, capture_output=True, timeout=12)
        public = result.stderr
        code = "E_CONFIG" if long_address else "E_STORAGE"
        expected_exit = 2 if long_address else 1
        assert result.stdout == "", (name, result.stdout)
        assert result.returncode == expected_exit, (name, result.returncode, public)
        if structured:
            error = json.loads(public)["error"]
            assert error["code"] == code, (name, error)
            assert error["status"] == result.returncode, (name, error)
            assert error["correlation_id"] == error["details"]["correlation_id"], (name, error)
            next_action = error["next_action"]
        else:
            assert code in public, (name, public)
            next_action = public
        if long_address:
            assert "shorter --basedir" in next_action, (name, public)
            assert "XDG_RUNTIME_DIR" in next_action, (name, public)
            assert "storage health" not in public, (name, public)
        else:
            assert "unknown or incompatible" in public, (name, public)
            assert ("--basedir" if custom else "--temp-basedir") in next_action, (name, public)
            assert "reset-data" not in public, (name, public)
        assert str(selected) not in public and str(base) not in public, (name, public)
        for path, content in sentinels.items():
            assert path.read_bytes() == content, (name, path)
        facts["cases"].append({
            "name": name, "structured": structured, "command_tail": command[1:],
            "exit": result.returncode, "stderr": public, "preserved_files": len(sentinels),
        })


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--artifacts", type=Path, required=True)
    args = parser.parse_args()
    browser = args.executable.resolve(strict=True)
    out = args.artifacts.resolve()
    out.mkdir(parents=True, exist_ok=False)
    facts = {
        "status": "running",
        "browser": {"path": str(browser), "sha256": hashlib.sha256(browser.read_bytes()).hexdigest()},
        "cases": [],
        "scope": "Actual CLI processes with disposable selected and unrelated XDG roots; sentinel preservation and public recovery guidance.",
    }
    try:
        with tempfile.TemporaryDirectory(prefix="ferric-startup-") as temporary:
            for name in CASES:
                run_case(browser, Path(temporary), name, facts)
        facts["status"] = "passed"
    except BaseException as error:
        facts.update(status="failed", error=repr(error))
        raise
    finally:
        facts["updated_at"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        (out / "observations.json").write_text(json.dumps(facts, indent=2) + "\n")
    print(json.dumps({"status": facts["status"], "cases": len(facts["cases"])}))


if __name__ == "__main__":
    main()

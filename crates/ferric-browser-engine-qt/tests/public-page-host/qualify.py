#!/usr/bin/env python3
"""Run component, disposable-clipboard and real Hyprland input scopes."""

import argparse
import copy
import json
import os
from pathlib import Path
import subprocess
import sys
import xml.etree.ElementTree as ET


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", type=Path, required=True)
    parser.add_argument("--compositor", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    args.build = args.build.resolve()
    args.report = args.report.resolve()
    args.report.parent.mkdir(parents=True, exist_ok=True)
    run_dir = args.report.with_suffix(".runs")
    run_dir.mkdir(exist_ok=True)
    here = Path(__file__).resolve().parent
    discovery = json.loads(subprocess.check_output(
        ["ctest", "--test-dir", str(args.build), "--show-only=json-v1"], text=True))
    expected = {test["name"] for test in discovery["tests"]}
    native = "public_page_host_native_input"
    clipboard = "public_page_captured_commands"
    if not {native, clipboard} <= expected:
        raise RuntimeError("Missing native input or compositor clipboard test")
    env = dict(os.environ, QT_QPA_PLATFORMTHEME="", QT_QUICK_BACKEND="software",
               QTWEBENGINE_CHROMIUM_FLAGS="--disable-gpu")
    component_env = dict(env, QT_QPA_PLATFORM="offscreen")
    native_env = dict(env, QT_QPA_PLATFORM="wayland",
                      FERRIC_TEST_HYPRLAND_RUNTIME_DIR=os.environ.get("XDG_RUNTIME_DIR", ""))
    owned = ["dbus-run-session", "--config-file=" + str(here.parent / "application/notification-test-bus.conf"),
             "--", "env", "FERRIC_PORTAL_TEST_BUS=isolated", sys.executable,
             str(here / "run_owned.py"), "--compositor", str(args.compositor.resolve()), "--"]
    scopes = [
        ("components", component_env, [], ["-E", f"^({native}|{clipboard})$"]),
        ("clipboard", env, owned, ["-R", f"^{clipboard}$"]),
        ("hyprland-input", native_env, [], ["-R", f"^{native}$"]),
    ]
    cases, attempts, code = {}, [], 0
    for name, environment, prefix, selection in scopes:
        report = run_dir / (name + ".xml")
        command = [*prefix, "ctest", "--test-dir", str(args.build), "--output-on-failure",
                   "--output-junit", str(report), *selection]
        result = subprocess.run(command, env=environment)
        code = code or result.returncode
        attempts.append({"scope": name, "command": command, "returncode": result.returncode,
                         "platform": environment.get("QT_QPA_PLATFORM", "owned Wayland"),
                         "report": str(report)})
        for case in ET.parse(report).getroot().iter("testcase"):
            name = case.get("name")
            if name in cases:
                raise RuntimeError("Duplicate test across qualification scopes")
            cases[name] = copy.deepcopy(case)
    if set(cases) != expected:
        raise RuntimeError("Qualification scopes did not cover the complete CTest inventory")
    suite = ET.Element("testsuite", name="public-page-host qualification scopes",
                       tests=str(len(cases)),
                       failures=str(sum(c.find("failure") is not None for c in cases.values())),
                       errors=str(sum(c.find("error") is not None for c in cases.values())),
                       skipped=str(sum(c.find("skipped") is not None for c in cases.values())))
    for case in cases.values():
        suite.append(case)
    ET.ElementTree(suite).write(args.report, encoding="utf-8", xml_declaration=True)
    args.report.with_suffix(".scopes.json").write_text(json.dumps(attempts, indent=2) + "\n")
    return code


if __name__ == "__main__":
    raise SystemExit(main())

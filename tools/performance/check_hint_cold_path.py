#!/usr/bin/env python3
"""Compare the live collector with immutable Git baseline in qualified WebEngine.

No controller, GPU qualification or new dependency build is required. The default
profile establishes DOM semantics and renderer collection cost, not native paint.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
BASELINE = "30e9daf34079186dd5775d5f9c18b26c40176344"
SCRIPTS = "crates/ferric-browser-engine-qt/qml/scripts/"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qml-tests", type=Path, default=Path(os.environ.get("FERRIC_QML_TESTS", ROOT.parent / "setup-build/qml/qml_tests")))
    parser.add_argument("--input", type=Path, default=ROOT / "crates/ferric-browser-engine-qt/tests/qml-perf/tst_hint_cold_path.qml", help="Qt Quick test file or directory")
    parser.add_argument("--artifact-dir", type=Path, help="existing fixture data directory for other test suites")
    parser.add_argument("selectors", nargs="*", help="optional QtTest selectors")
    args = parser.parse_args()
    executable = args.qml_tests.absolute()
    if not executable.is_file():
        parser.error("supply the existing qualified Qt 6.11.2 qml_tests executable")
    env = dict(os.environ)
    # Reuse saved SDK paths only; no controller state or setup operation.
    saved = ROOT.parent / "environment.json"
    if saved.is_file():
        env.update({k: str(v) for k, v in json.loads(saved.read_text()).get("exports", {}).items() if v is not None})
    env.update(QT_QPA_PLATFORM="offscreen", QT_QUICK_BACKEND="software", QSG_RHI_BACKEND="opengl",
               QML_XHR_ALLOW_FILE_READ="1", QTWEBENGINE_CHROMIUM_FLAGS="--disable-gpu")
    env.pop("QTWEBENGINE_DISABLE_SANDBOX", None)
    with tempfile.TemporaryDirectory(prefix="ferric-hint-cold-") as folder:
        for name in ("PageInteractionScripts.js", "BrowserScripts.js"):
            raw = subprocess.check_output(["git", "show", BASELINE + ":" + SCRIPTS + name], cwd=ROOT)
            (Path(folder) / name).write_bytes(raw)
        env["FERRIC_TEST_ARTIFACT_DIR"] = str(args.artifact_dir.absolute()) if args.artifact_dir else folder
        return subprocess.run([str(ROOT / "tools/performance/with_qt611.sh"), str(executable), "-input",
                               str(args.input.absolute()),
                               *args.selectors], cwd=ROOT, env=env).returncode


if __name__ == "__main__":
    raise SystemExit(main())

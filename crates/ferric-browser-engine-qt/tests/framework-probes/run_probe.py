#!/usr/bin/env python3
"""Build and run the opt-in, local-only S22 WebEngine research fixture."""

from __future__ import annotations

import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from threading import Thread


ROOT = Path(__file__).resolve().parents[4]
PROBES = Path(__file__).resolve().parent
ENGINE = ROOT / "crates/ferric-browser-engine-qt"
PORT = 8765


class QuietHandler(SimpleHTTPRequestHandler):
    def log_message(self, _format: str, *_args: object) -> None:
        return


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def run(command: list[str], *, cwd: Path, env: dict[str, str] | None = None) -> None:
    print("$ " + " ".join(command), flush=True)
    subprocess.run(command, cwd=cwd, env=env, check=True)


def main() -> int:
    run([
        "cmake",
        "-S",
        str(ENGINE / "tests/qml-runner"),
        "-B",
        str(ROOT / "target/qml-tests"),
    ], cwd=ROOT)
    run(["cmake", "--build", str(ROOT / "target/qml-tests"), "--parallel", "2"], cwd=ROOT)

    with tempfile.TemporaryDirectory(prefix="ferric-s22-probes-") as temporary:
        work = Path(temporary)
        webroot = work / "www"
        source = work / "src"
        vendor = webroot / "vendor"
        webroot.mkdir()
        source.mkdir()
        vendor.mkdir()
        shutil.copy2(PROBES / "package.json", work / "package.json")
        shutil.copy2(PROBES / "package-lock.json", work / "package-lock.json")
        shutil.copy2(PROBES / "framework-probe.js", source / "framework-probe.js")
        (webroot / "framework-probes.html").symlink_to(
            ENGINE / "tests/fixtures/framework-probes.html"
        )

        run(["npm", "ci", "--ignore-scripts", "--no-audit", "--no-fund"], cwd=work)
        esbuild = work / "node_modules/.bin/esbuild"
        run([
            str(esbuild),
            str(source / "framework-probe.js"),
            "--bundle",
            "--format=esm",
            "--platform=browser",
            "--outfile=" + str(webroot / "framework-probe.js"),
        ], cwd=work)

        packages = work / "node_modules"
        (vendor / "ace.js").symlink_to(packages / "ace-builds/src-min/ace.js")
        (vendor / "quill.js").symlink_to(packages / "quill/dist/quill.js")
        (vendor / "quill.snow.css").symlink_to(packages / "quill/dist/quill.snow.css")
        (vendor / "monaco").symlink_to(packages / "monaco-editor/min", target_is_directory=True)
        lock = PROBES / "package-lock.json"
        print("S22 npm lock sha256=" + sha256(lock), flush=True)
        print("S22 generated browser bundle sha256=" + sha256(webroot / "framework-probe.js"), flush=True)
        print("S22 fixture origin=http://127.0.0.1:%d (no CDN requests from WebEngine)" % PORT, flush=True)

        handler = lambda *args, **kwargs: QuietHandler(*args, directory=str(webroot), **kwargs)
        server = ThreadingHTTPServer(("127.0.0.1", PORT), handler)
        thread = Thread(target=server.serve_forever, daemon=True)
        thread.start()
        env = os.environ.copy()
        env.update({
            "QT_QPA_PLATFORM": "offscreen",
            "QT_QPA_PLATFORMTHEME": "",
            "QT_QUICK_BACKEND": "software",
            "NO_PROXY": "localhost,127.0.0.1",
            "no_proxy": "localhost,127.0.0.1",
        })
        try:
            run([
                str(ROOT / "target/qml-tests/qml_tests"),
                "-input",
                str(ENGINE / "tests/framework-probes"),
            ], cwd=ROOT, env=env)
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=2)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

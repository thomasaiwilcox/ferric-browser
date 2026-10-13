#!/usr/bin/env python3
"""Run the opt-in Qt public child-frame provider research fixture."""

from __future__ import annotations

import os
from pathlib import Path
import shlex
import subprocess
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from threading import Thread


ROOT = Path(__file__).resolve().parents[4]
ENGINE = Path(__file__).resolve().parents[2]
PROBE = Path(__file__).resolve().parent
FIXTURES = ENGINE / "tests/fixtures/webapp-interaction"
PORT = 8766
REQUIRED_QT = (6, 11, 2)


class QuietHandler(SimpleHTTPRequestHandler):
    def log_message(self, _format: str, *_args: object) -> None:
        return


def version_tuple(value: str) -> tuple[int, ...]:
    return tuple(int(part) for part in value.split(".")[:3])


def run(command: list[str], *, cwd: Path, env: dict[str, str] | None = None,
        input_text: str | None = None) -> subprocess.CompletedProcess[str]:
    print("$ " + " ".join(command), flush=True)
    result = subprocess.run(command, cwd=cwd, env=env, input=input_text,
                            text=True, check=True, capture_output=True)
    if result.stdout:
        print(result.stdout, end="", flush=True)
    if result.stderr:
        print(result.stderr, end="", flush=True)
    return result


def main() -> int:
    versions = run(["pkg-config", "--modversion", "Qt6Core", "Qt6WebEngineCore"],
                   cwd=ROOT).stdout.split()
    if len(versions) != 2 or any(version_tuple(item) < REQUIRED_QT for item in versions):
        raise RuntimeError("S23 QML frame-tree probe requires the declared Qt >= 6.11.2 target")
    print("S23 public QML frame-tree target versions=" + " / ".join(versions), flush=True)

    cflags = run(["pkg-config", "--cflags", "Qt6WebEngineCore"], cwd=ROOT).stdout
    snippet = r'''#include <QWebEngineFrame>
#include <QWebEnginePage>
#include <QWebEngineScript>
#include <QVariant>
#include <type_traits>
static_assert(QT_VERSION >= QT_VERSION_CHECK(6, 8, 0));
void public_frame_api(QWebEnginePage &page) {
    QWebEngineFrame root = page.mainFrame();
    const auto children = root.children();
    if (!children.isEmpty()) {
        QWebEngineFrame child = children.constFirst();
        (void)child.isValid();
        (void)child.url();
        (void)child.htmlName();
        (void)child.size();
        child.runJavaScript(QStringLiteral("true"),
            quint32(QWebEngineScript::ApplicationWorld),
            [](const QVariant &) {});
        (void)(child == children.constFirst());
    }
}
'''
    compile_command = ["g++", "-std=c++20", "-Werror", "-fsyntax-only", "-x", "c++", "-"]
    compile_command.extend(shlex.split(cflags))
    run(compile_command, cwd=ROOT, input_text=snippet)
    print("S23 public C++ QWebEngineFrame API compile passed", flush=True)

    run(["cmake", "-S", str(ENGINE / "tests/qml-runner"),
         "-B", str(ROOT / "target/qml-tests")], cwd=ROOT)
    run(["cmake", "--build", str(ROOT / "target/qml-tests"), "--parallel", "2"], cwd=ROOT)

    handler = lambda *args, **kwargs: QuietHandler(*args, directory=str(FIXTURES), **kwargs)
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
        run([str(ROOT / "target/qml-tests/qml_tests"), "-input", str(PROBE)],
            cwd=ROOT, env=env)
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=2)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

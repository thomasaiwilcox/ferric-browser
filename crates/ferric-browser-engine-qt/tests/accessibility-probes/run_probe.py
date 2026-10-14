#!/usr/bin/env python3
"""Opt-in Linux AT-SPI probe for public Qt Quick and Qt WebEngine semantics."""

from __future__ import annotations

import ast
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile
from threading import Thread
import time

ROOT = Path(__file__).resolve().parents[4]
PROBES = Path(__file__).resolve().parent
ENGINE = ROOT / "crates/ferric-browser-engine-qt"
FIXTURES = ENGINE / "tests/fixtures/webapp-interaction"


def run(command: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
    print("$ " + shlex.join(command), flush=True)
    check = bool(kwargs.pop("check", True))
    result = subprocess.run(command, text=True, check=False, capture_output=True, **kwargs)
    if result.stdout:
        print(result.stdout, end="", flush=True)
    if result.stderr:
        print(result.stderr, end="", file=sys.stderr, flush=True)
    if check and result.returncode:
        raise subprocess.CalledProcessError(result.returncode, command, result.stdout, result.stderr)
    return result


def build_probe() -> tuple[Path, Path]:
    cmake_build = ROOT / "target/qml-tests"
    if not (cmake_build / "qml_tests").exists():
        run(["cmake", "-S", str(ENGINE / "tests/qml-runner"), "-B", str(cmake_build)], cwd=ROOT)
    run(["cmake", "--build", str(cmake_build), "--parallel", "2"], cwd=ROOT)
    helper = cmake_build / "s24-atspi-probe"
    run([
        "cc", *subprocess.check_output(["pkg-config", "--cflags", "atspi-2"], text=True).split(),
        str(PROBES / "atspi_probe.c"), "-o", str(helper),
        *subprocess.check_output(["pkg-config", "--libs", "atspi-2"], text=True).split(),
        *subprocess.check_output(["pkg-config", "--libs-only-l", "gobject-2.0"], text=True).split(),
    ], cwd=ROOT)
    return cmake_build, helper


def run_isolated() -> int:
    cmake_build = ROOT / "target/qml-tests"
    helper = cmake_build / "s24-atspi-probe"

    env = os.environ.copy()
    if not env.get("WAYLAND_DISPLAY") or not env.get("XDG_RUNTIME_DIR"):
        raise RuntimeError("native Wayland probe requires WAYLAND_DISPLAY and XDG_RUNTIME_DIR")

    # This branch runs under a private session D-Bus and a temporary XDG runtime
    # directory. The bus address check ensures it cannot reach the user AT-SPI bus.
    status = {}
    for property_name in ("IsEnabled", "ScreenReaderEnabled"):
        value = run(["gdbus", "call", "--session", "--dest", "org.a11y.Bus",
                     "--object-path", "/org/a11y/bus", "--method",
                     "org.freedesktop.DBus.Properties.Get", "org.a11y.Status", property_name],
                    cwd=ROOT)
        status[property_name] = value.stdout.strip().startswith("(<true>,)")
    if status != {"IsEnabled": False, "ScreenReaderEnabled": False}:
        raise RuntimeError("refusing to run unless desktop accessibility flags remain false: " + str(status))

    address_result = run(["gdbus", "call", "--session", "--dest", "org.a11y.Bus",
                          "--object-path", "/org/a11y/bus", "--method", "org.a11y.Bus.GetAddress"],
                         cwd=ROOT)
    address = ast.literal_eval(address_result.stdout.strip())[0]
    if not address.startswith("unix:") or env["XDG_RUNTIME_DIR"] not in address:
        raise RuntimeError("AT-SPI returned an unsupported bus address: " + address)
    for property_name in ("IsEnabled", "ScreenReaderEnabled"):
        run(["gdbus", "call", "--session", "--dest", "org.a11y.Bus",
             "--object-path", "/org/a11y/bus", "--method",
             "org.freedesktop.DBus.Properties.Set", "org.a11y.Status",
             property_name, "<true>"], cwd=ROOT)
    env["AT_SPI_BUS_ADDRESS"] = address
    registry = subprocess.Popen([
        "/usr/lib/at-spi2-registryd", "--dbus-name", "org.a11y.atspi.Registry",
    ], cwd=ROOT, env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    time.sleep(1)
    if registry.poll() is not None:
        output, _ = registry.communicate()
        raise RuntimeError("isolated AT-SPI registry failed to start: " + output)
    env.update({
        "QT_QPA_PLATFORM": "wayland",
        "QT_QPA_PLATFORMTHEME": "",
        "QT_LINUX_ACCESSIBILITY_ALWAYS_ON": "1",
        "QTWEBENGINE_ENABLE_LINUX_ACCESSIBILITY": "1",
        "QT_QUICK_BACKEND": "software",
        "NO_PROXY": "localhost,127.0.0.1",
        "no_proxy": "localhost,127.0.0.1",
    })
    class QuietHandler(SimpleHTTPRequestHandler):
        def __init__(self, *args: object, **kwargs: object) -> None:
            super().__init__(*args, directory=str(FIXTURES), **kwargs)

        def log_message(self, _format: str, *_args: object) -> None:
            return

    server = ThreadingHTTPServer(("127.0.0.1", 8766), QuietHandler)
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    app = subprocess.Popen([
        str(cmake_build / "qml_tests"), "-input", str(PROBES),
    ], cwd=ROOT, env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    try:
        time.sleep(5)
        client = run([str(helper)], cwd=ROOT, env=env, check=False)
        remaining, _ = app.communicate(timeout=20)
        print(remaining, end="", flush=True)
        if app.returncode:
            raise subprocess.CalledProcessError(app.returncode, app.args)
        if client.returncode:
            raise subprocess.CalledProcessError(client.returncode, [str(helper)])
        expected = (
            "provider targets qml=1",
            "qml press accepted=1",
            "qml stale target defunct=1 visible=0 still_enumerated=0 stale_press_accepted=0",
        )
        if not all(item in client.stdout for item in expected):
            raise RuntimeError("public QML provider/action/invalidation probe did not meet its assertions")
        return 0
    finally:
        if app.poll() is None:
            app.terminate()
            try:
                app.communicate(timeout=5)
            except subprocess.TimeoutExpired:
                app.kill()
                app.communicate()
        if registry.poll() is None:
            registry.terminate()
            try:
                registry.communicate(timeout=3)
            except subprocess.TimeoutExpired:
                registry.kill()
                registry.communicate()
        server.shutdown()
        server.server_close()
        thread.join(timeout=2)


def main() -> int:
    if "--isolated" in sys.argv:
        return run_isolated()

    build_probe()
    env = os.environ.copy()
    original_runtime = env.get("XDG_RUNTIME_DIR")
    wayland = env.get("WAYLAND_DISPLAY")
    if not original_runtime or not wayland:
        raise RuntimeError("native Wayland probe requires WAYLAND_DISPLAY and XDG_RUNTIME_DIR")
    wayland_socket = str(Path(original_runtime) / wayland) if not wayland.startswith("/") else wayland
    with tempfile.TemporaryDirectory(prefix="ferric-s24-atspi-", ignore_cleanup_errors=True) as temporary:
        root = Path(temporary)
        runtime = root / "runtime"
        runtime.mkdir(mode=0o700)
        data_home = root / "data"
        services = data_home / "dbus-1" / "services"
        services.mkdir(parents=True)
        source = Path("/usr/share/dbus-1/services/org.a11y.Bus.service").read_text()
        private_service = "\n".join(
            line for line in source.splitlines() if not line.startswith("SystemdService=")
        ) + "\n"
        (services / "org.a11y.Bus.service").write_text(private_service)
        env.update({
            "XDG_RUNTIME_DIR": str(runtime),
            "XDG_DATA_HOME": str(data_home),
            "XDG_DATA_DIRS": env.get("XDG_DATA_DIRS", "/usr/local/share:/usr/share"),
            "XDG_CACHE_HOME": str(root / "cache"),
            "XDG_CONFIG_HOME": str(root / "config"),
            "GSETTINGS_BACKEND": "memory",
            "WAYLAND_DISPLAY": wayland_socket,
        })
        command = ["dbus-run-session", "--", sys.executable, str(Path(__file__).resolve()), "--isolated"]
        print("$ " + shlex.join(command), flush=True)
        result = subprocess.run(command, cwd=ROOT, env=env, check=False)
        if result.returncode == 0:
            for command in (
                ["gsettings", "get", "org.gnome.desktop.interface", "toolkit-accessibility"],
                ["gsettings", "get", "org.gnome.desktop.a11y.applications", "screen-reader-enabled"],
            ):
                after = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, check=True)
                if after.stdout.strip() != "false":
                    raise RuntimeError("isolated probe altered a user accessibility preference")
        for attempt in range(6):
            try:
                shutil.rmtree(root)
                break
            except OSError:
                time.sleep(0.25 * (attempt + 1))
        return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())

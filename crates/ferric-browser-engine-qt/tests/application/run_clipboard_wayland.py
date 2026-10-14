#!/usr/bin/env python3
"""Private Wayland data-device qualification; never touches the host clipboard."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[4]


def inject_native(action):
    sequences = {
        "copy": ["-M", "ctrl", "-k", "c", "-m", "ctrl"],
        "paste": ["-M", "ctrl", "-k", "v", "-m", "ctrl"],
        "yank": ["yy"], "command": ["o"],
    }
    if action not in sequences or os.environ.get("FERRIC_CLIPBOARD_ISOLATED") != "1":
        raise SystemExit("Only private clipboard fixture gestures may be injected")
    wanted = int(os.environ["FERRIC_CLIPBOARD_COMPOSITOR_PID"])
    env = os.environ.copy()
    env.update(XDG_RUNTIME_DIR=env["FERRIC_CLIPBOARD_HOST_RUNTIME"],
               WAYLAND_DISPLAY=env["FERRIC_CLIPBOARD_HOST_DISPLAY"])
    if env.get("HYPRLAND_INSTANCE_SIGNATURE") and shutil.which("hyprctl"):
        result = subprocess.run(["hyprctl", "activewindow", "-j"], env=env,
                                capture_output=True, text=True, check=True, timeout=3)
        owned = json.loads(result.stdout).get("pid") == wanted
    elif env.get("SWAYSOCK") and shutil.which("swaymsg"):
        result = subprocess.run(["swaymsg", "-t", "get_tree", "-r"], env=env,
                                capture_output=True, text=True, check=True, timeout=3)
        def focused(node):
            return (node.get("focused") and node.get("pid") == wanted) or any(
                focused(child) for child in node.get("nodes", []) + node.get("floating_nodes", []))
        owned = focused(json.loads(result.stdout))
    else:
        raise SystemExit("Native input qualification unavailable: cannot verify parent window ownership")
    if not owned:
        raise SystemExit("Private Weston window lost focus; refusing to inject input")
    subprocess.run(["wtype", *sequences[action]], env=env, check=True, timeout=5)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--inject", choices=["copy", "paste", "yank", "command"])
    args = parser.parse_args()
    if args.inject:
        inject_native(args.inject)
        return

    for tool in ("weston", "wl-copy", "wl-paste", "wtype"):
        if not shutil.which(tool):
            raise SystemExit(f"Wayland clipboard qualification unavailable: missing {tool}")
    host_display = os.environ.get("WAYLAND_DISPLAY", "")
    host_runtime = os.environ.get("XDG_RUNTIME_DIR", "")
    if not host_display or not host_runtime:
        raise SystemExit("Wayland clipboard qualification unavailable: no parent compositor/seat")
    parent_socket = Path(host_display)
    if not parent_socket.is_absolute():
        parent_socket = Path(host_runtime) / parent_socket
    with tempfile.TemporaryDirectory(prefix="ferric-clipboard-wayland-") as runtime:
        display = "ferric-clipboard-private"
        compositor_env = os.environ.copy()
        compositor_env.update(XDG_RUNTIME_DIR=runtime, WAYLAND_DISPLAY=str(parent_socket))
        with tempfile.TemporaryFile(mode="w+") as log:
            compositor = subprocess.Popen(
                ["weston", "--backend=wayland-backend.so", f"--socket={display}",
                 "--idle-time=0", "--no-config", "--width=1280", "--height=900"],
                env=compositor_env, stdout=log, stderr=log,
            )
            try:
                deadline = time.monotonic() + 8
                socket = Path(runtime) / display
                while not socket.exists():
                    if compositor.poll() is not None or time.monotonic() >= deadline:
                        raise RuntimeError("private compositor unavailable; native clipboard remains unqualified")
                    time.sleep(0.05)
                env = os.environ.copy()
                env.update(XDG_RUNTIME_DIR=runtime, WAYLAND_DISPLAY=display,
                           QT_QPA_PLATFORM="wayland", QT_QPA_PLATFORMTHEME="",
                           QT_QUICK_BACKEND="software", QTWEBENGINE_CHROMIUM_FLAGS="--disable-gpu",
                           FERRIC_CLIPBOARD_ISOLATED="1",
                           FERRIC_CLIPBOARD_COMPOSITOR_PID=str(compositor.pid),
                           FERRIC_CLIPBOARD_HOST_RUNTIME=host_runtime,
                           FERRIC_CLIPBOARD_HOST_DISPLAY=str(parent_socket),
                           FERRIC_CLIPBOARD_INJECTOR=str(Path(__file__).resolve()))
                subprocess.run([str(ROOT / "target/application-tests/application_tests"),
                                "clipboardWaylandTransport"], env=env, check=True, timeout=60)
                print("Private Wayland clipboard transfers passed with compositor-delivered input; "
                      "physical keyboard input is a separate qualification.")
            finally:
                compositor.terminate()
                try:
                    compositor.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    compositor.kill()
                    compositor.wait(timeout=5)
                if compositor.returncode not in (0, -15):
                    log.seek(0)
                    print(log.read())


if __name__ == "__main__":
    main()

"""Run the P01 native-input probe on an isolated owned Wayland compositor.

Invoke through dbus-run-session with FERRIC_PORTAL_TEST_BUS=isolated, passing
the qualified portal_compositor executable, then native_insert and its args.
"""

from contextlib import ExitStack
from pathlib import Path
import os
import subprocess
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "portal"))
from run import start


def main():
    if os.environ.get("FERRIC_PORTAL_TEST_BUS") != "isolated":
        raise RuntimeError("Native Wayland probe requires an isolated session bus")
    compositor = str(Path(sys.argv[1]).resolve())
    with tempfile.TemporaryDirectory(prefix="ferric-p01-wayland-") as temporary, ExitStack() as stack:
        env = os.environ.copy()
        for variable, folder in [("XDG_RUNTIME_DIR", "runtime"), ("XDG_CONFIG_HOME", "config"),
                                 ("XDG_DATA_HOME", "data"), ("XDG_CACHE_HOME", "cache"),
                                 ("XDG_STATE_HOME", "state")]:
            path = Path(temporary) / folder
            path.mkdir(mode=0o700)
            env[variable] = str(path)
        env.update(WAYLAND_DISPLAY="ferric-p01-native", QT_QPA_PLATFORM="wayland",
                   QT_QPA_PLATFORMTHEME="", QT_QUICK_BACKEND="software",
                   QT_WAYLAND_DISABLE_WINDOWDECORATION="1", FERRIC_PORTAL_FIXTURE_LIFETIME_MS="180000")
        start(stack, [compositor], dict(env, QT_QPA_PLATFORM="offscreen",
                                      FERRIC_PARENT_DISPLAY=env["WAYLAND_DISPLAY"]))
        return subprocess.run(sys.argv[2:], env=env, timeout=150).returncode


if __name__ == "__main__":
    raise SystemExit(main())

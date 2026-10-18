#!/usr/bin/env python3
"""Hold actual file sync in an owned browser and keep an external release watchdog."""
import argparse
import os
from pathlib import Path
import subprocess
import tempfile
import threading
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--interposer", type=Path, required=True)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="ferric-save-probe-") as temporary:
        root = Path(temporary)
        env = os.environ.copy()
        for key in ("XDG_CONFIG_HOME", "XDG_CACHE_HOME", "XDG_DATA_HOME", "XDG_STATE_HOME", "XDG_RUNTIME_DIR"):
            folder = root / key
            folder.mkdir(mode=0o700)
            env[key] = str(folder)
        barrier = root / "barrier"
        barrier.mkdir(mode=0o700)
        env.update(
            FERRIC_TEST_SAVE_BARRIER_ROOT=str(barrier),
            LD_PRELOAD=str(args.interposer.resolve(strict=True)),
            QT_QPA_PLATFORM="offscreen", QT_QPA_PLATFORMTHEME="", QT_QUICK_BACKEND="software",
            QTWEBENGINE_CHROMIUM_FLAGS="--disable-gpu",
        )
        stopped = threading.Event()
        watchdog_released = []

        def watchdog():
            observed = None
            deadline = None
            while not stopped.wait(0.02):
                try:
                    stamp = (barrier / "entered").stat().st_mtime_ns
                except FileNotFoundError:
                    observed = deadline = None
                    continue
                if stamp != observed:
                    observed, deadline = stamp, time.monotonic() + 10
                if time.monotonic() >= deadline and not (barrier / "release").exists():
                    watchdog_released.append(stamp)
                    (barrier / "release").touch(mode=0o600)

        thread = threading.Thread(target=watchdog, daemon=True)
        thread.start()
        child = subprocess.Popen([str(args.executable.resolve(strict=True)), "downloadFinalization"], env=env)
        try:
            code = child.wait(timeout=90)
            assert not watchdog_released, "GUI failed to release its owned file-sync barrier in time"
            assert code == 0, f"File finalization regression exited {code}"
        finally:
            (barrier / "release").touch(mode=0o600)
            if child.poll() is None:
                child.terminate()
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait(timeout=5)
            stopped.set()
            thread.join(timeout=1)


if __name__ == "__main__":
    main()

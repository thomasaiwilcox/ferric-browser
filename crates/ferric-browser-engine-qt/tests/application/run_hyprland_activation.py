#!/usr/bin/env python3
"""Give only the initial owned Qt seat probe focus, then observe the test."""

import argparse
import json
import os
from pathlib import Path
import re
import select
import signal
import socket
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        parser.error("A native test command is required")
    runtime = os.environ.get("XDG_RUNTIME_DIR", "")
    signature = os.environ.get("HYPRLAND_INSTANCE_SIGNATURE", "")
    if not runtime or not signature:
        parser.error("Use the inherited live Hyprland session")
    events = socket.socket(socket.AF_UNIX)
    events.connect(str(Path(runtime) / "hypr" / signature / ".socket2.sock"))
    with events:
        def interrupted(_signum, _frame):
            raise InterruptedError("Native fixture interrupted")
        signal.signal(signal.SIGTERM, interrupted)
        child = subprocess.Popen(command)
        try:
            deadline = time.monotonic() + 5
            admitted = False
            pending = b""
            while child.poll() is None and time.monotonic() < deadline:
                if not select.select([events], [], [], .2)[0]:
                    continue
                data = events.recv(65536)
                if not data:
                    raise RuntimeError("Hyprland event connection closed")
                pending += data
                lines = pending.split(b"\n")
                pending = lines.pop()
                if not any(line.startswith(b"openwindow>>") for line in lines):
                    continue
                clients = json.loads(subprocess.check_output(
                    ["hyprctl", "clients", "-j"], text=True, timeout=2))
                owned = [window for window in clients
                         if window.get("pid") == child.pid and window.get("mapped")
                         and window.get("title") == f"ferric-native-input-probe-{child.pid}"]
                if not owned:
                    continue
                if len(owned) != 1 or not re.fullmatch(r"0x[0-9a-fA-F]+", owned[0]["address"]):
                    raise RuntimeError("Ambiguous owned probe identity")
                if child.poll() is not None:
                    break
                address = owned[0]["address"]
                expression = (
                    "return (function() local found={} for _,w in ipairs(hl.get_windows()) do "
                    f"if tostring(w.address)=='{address}' and w.pid=={child.pid} "
                    f"and w.title=='ferric-native-input-probe-{child.pid}' "
                    "then found[#found+1]=w end end "
                    "if #found~=1 then return 'reject-identity' end "
                    "local result=hl.dispatch(hl.dsp.focus({window=found[1]})) "
                    "return result and result.ok and 'requested' or 'rejected' end)()"
                )
                result = subprocess.check_output(["hyprctl", "repl", expression],
                                                 text=True, timeout=2)
                if result.strip().splitlines()[-1:] != ["requested"]:
                    raise RuntimeError("Owned probe focus rejected: " + result)
                print(json.dumps({"setup": "owned-initial-seat-probe", "pid": child.pid,
                                  "address": owned[0]["address"]}), flush=True)
                admitted = True
                break
            # No focus requests follow admission. Every browser activation,
            # role recreation and focus assertion remains the test's work.
            print(json.dumps({"initial_probe_focus_requested": admitted}), flush=True)
            return child.wait()
        finally:
            signal.signal(signal.SIGTERM, signal.SIG_IGN)
            if child.poll() is None:
                child.terminate()
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait()


if __name__ == "__main__":
    raise SystemExit(main())

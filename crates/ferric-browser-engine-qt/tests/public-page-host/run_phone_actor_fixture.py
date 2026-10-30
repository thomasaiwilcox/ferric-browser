#!/usr/bin/python3
"""Run the production actor against an owned, fixed-sibling synthetic helper."""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import time


def same_process(row):
    try:
        status = Path(f"/proc/{row['pid']}/stat").read_text()
        return status.rsplit(")", 1)[1].split()[19] == row["start"]
    except FileNotFoundError:
        return False


def owned_helper(row, helper):
    if not same_process(row):
        return False
    process = Path(f"/proc/{row['pid']}")
    # Receipts alone cannot authorize signalling a reused/unrelated PID.
    return process.stat().st_uid == os.getuid() and os.fsencode(helper) in (process / "cmdline").read_bytes().split(b"\0")


def read_receipts(receipt):
    rows, valid = [], True
    try:
        lines = receipt.read_text().splitlines() if receipt.exists() else []
    except (OSError, UnicodeError):
        return rows, False
    for line in lines:
        try:
            row = json.loads(line)
            if (type(row["pid"]) is not int or row["pid"] <= 1
                    or not isinstance(row["start"], str) or not row["start"].isdigit()
                    or row["kind"] not in ("started", "request")):
                raise ValueError("invalid ownership receipt")
            rows.append(row)
        except (ValueError, KeyError, TypeError):
            valid = False
    return rows, valid


def wait_gone(rows, seconds):
    def live(row):
        if not same_process(row):
            return False
        try:
            # A timed-out browser can leave this subreaper an adopted child.
            # waitpid cannot reap an unrelated process, and uses exact receipts.
            os.waitpid(row["pid"], os.WNOHANG)
        except ChildProcessError:
            pass
        return same_process(row)
    deadline = time.monotonic() + seconds
    while any(live(row) for row in rows) and time.monotonic() < deadline:
        time.sleep(0.05)
    return not any(live(row) for row in rows)


def interrupted(signum, _frame):
    raise InterruptedError(f"fixture interrupted by signal {signum}")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("--scratch-root", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=180)
    args = parser.parse_args()
    # Linux-only fixture ownership: adopted helpers remain reapable even if
    # their browser must be killed. This affects only this runner's descendants.
    if ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0) != 0:
        raise OSError(ctypes.get_errno(), "cannot own orphaned fixture helpers")
    layout = Path(tempfile.mkdtemp(prefix="phone-actor-", dir=args.scratch_root))
    receipt = layout / "helper.jsonl"
    helper = layout / "ferric-phone-auth-helper"
    process, result, natural_reaping, cleanup_complete = None, 1, False, False
    previous_term = signal.signal(signal.SIGTERM, interrupted)
    try:
        binary = layout / args.binary.name
        try:
            os.link(args.binary, binary)
        except OSError:
            shutil.copy2(args.binary, binary)
        shutil.copy2(Path(__file__).with_name("phone_actor_helper.py"), helper)
        helper.chmod(0o700)
        environment = os.environ.copy()
        environment["FERRIC_PHONE_ACTOR_FIXTURE"] = "1"
        environment["FERRIC_PHONE_ACTOR_RECEIPT"] = str(receipt)
        # Only this disposable process models direct networking. Its helper has
        # no sockets/Bluetooth, and no user policy or installed path is changed.
        for name in ("http_proxy", "https_proxy", "all_proxy", "HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"):
            environment.pop(name, None)
        process = subprocess.Popen([str(binary)], env=environment)
        result = process.wait(timeout=args.timeout)
    except (Exception, KeyboardInterrupt) as error:
        print(json.dumps({"fixture_error": type(error).__name__}), flush=True)
    finally:
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        cleanup_int = signal.signal(signal.SIGINT, signal.SIG_IGN)
        # Always stop the owned browser before reading its final helper receipts,
        # including on timeout, interruption or malformed output.
        forced_parent_cleanup = bool(process and process.poll() is None)
        if forced_parent_cleanup:
            process.terminate()
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=3)
        rows, receipts_valid = read_receipts(receipt)
        natural_reaping = wait_gone(rows, 5)
        forced_helper_cleanup = not natural_reaping
        for termination in (signal.SIGTERM, signal.SIGKILL):
            if wait_gone(rows, 0):
                break
            for row in {row["pid"]: row for row in rows}.values():
                try:
                    if owned_helper(row, helper):
                        os.kill(row["pid"], termination)
                except (FileNotFoundError, ProcessLookupError):
                    pass
            wait_gone(rows, 2)
        cleanup_complete = wait_gone(rows, 0) and receipts_valid
        print(json.dumps({"synthetic_helper_sha256": hashlib.sha256(helper.read_bytes()).hexdigest() if helper.exists() else None,
                          "test_exit": result, "helpers_reaped": natural_reaping,
                          "forced_parent_cleanup": forced_parent_cleanup,
                          "forced_helper_cleanup": forced_helper_cleanup,
                          "cleanup_complete": cleanup_complete, "receipts_valid": receipts_valid,
                          "helper_requests": sum(row["kind"] == "request" for row in rows)}), flush=True)
        if cleanup_complete:
            shutil.rmtree(layout)
        else:
            print(json.dumps({"retained_ownership_evidence": str(layout)}), flush=True)
        print(json.dumps({"fixture_layout_removed": not layout.exists()}), flush=True)
        signal.signal(signal.SIGTERM, previous_term)
        signal.signal(signal.SIGINT, cleanup_int)
    return result if result else 0 if natural_reaping and cleanup_complete and not forced_parent_cleanup else 1


if __name__ == "__main__":
    raise SystemExit(main())

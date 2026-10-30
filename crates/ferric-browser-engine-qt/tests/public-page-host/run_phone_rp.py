#!/usr/bin/env python3
"""Owned RP/profile preparation; interactive credentials require an operator."""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "everyday/rp"))
from server import VerifierFixture


def children():
    path = Path(f"/proc/self/task/{os.getpid()}/children")
    return [int(pid) for pid in path.read_text().split()]


def reap_children():
    for pid in children():
        try:
            os.waitpid(pid, os.WNOHANG)
        except ChildProcessError:
            pass
    return children()


def interrupted(signum, _frame):
    raise InterruptedError(f"RP fixture interrupted by signal {signum}")


def signal_owned_helper(pid, helper, termination):
    try:
        proc = Path(f"/proc/{pid}")
        fields = (proc / "stat").read_text().rsplit(")", 1)[1].split()
        if (int(fields[1]) == os.getpid() and proc.stat().st_uid == os.getuid()
                and os.fsencode(helper) in (proc / "cmdline").read_bytes().split(b"\0")
                and (proc / "stat").read_text().rsplit(")", 1)[1].split()[19] == fields[19]):
            os.kill(pid, termination)
            return True
    except (FileNotFoundError, ProcessLookupError):
        pass
    return False


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--browser", type=Path, required=True)
    parser.add_argument("--helper", type=Path, required=True)
    parser.add_argument("--scratch-root", type=Path, required=True)
    parser.add_argument("--interactive", action="store_true")
    args = parser.parse_args()
    if ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0) != 0:
        raise OSError(ctypes.get_errno(), "cannot own fixture descendants")
    previous_term = signal.signal(signal.SIGTERM, interrupted)
    layout = Path(tempfile.mkdtemp(prefix="phone-rp-", dir=args.scratch_root))
    binary = layout / "bin" / args.browser.name
    helper = layout / "bin/ferric-phone-auth-helper"
    process, fixture, result, forced = None, None, 1, False
    try:
        binary.parent.mkdir()
        try:
            os.link(args.browser.resolve(strict=True), binary)
        except OSError:
            shutil.copy2(args.browser.resolve(strict=True), binary)
        shutil.copy2(args.helper.resolve(strict=True), helper)
        data = layout / "share/ferric-browser/public_suffix_list.dat"
        data.parent.mkdir(parents=True)
        repository = Path(__file__).resolve().parents[4]
        shutil.copy2(repository / "helpers/ferric-phone-auth/data/public_suffix_list.dat", data)
        certificate, private_key = layout / "rp-cert.pem", layout / "rp-key.pem"
        subprocess.run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
                        "-subj", "/CN=passkeys.ferric.test", "-addext", "subjectAltName=DNS:passkeys.ferric.test",
                        "-keyout", str(private_key), "-out", str(certificate)],
                       stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, timeout=15, check=True)
        private_key.chmod(0o600)
        certificate.chmod(0o600)
        environment = os.environ.copy()
        original_flags = environment.get("QTWEBENGINE_CHROMIUM_FLAGS", "")
        # Append only the scoped RP mapping. Inherit proxy policy unchanged;
        # explicit native direct startup rejects conflicts before any profile.
        environment["QTWEBENGINE_CHROMIUM_FLAGS"] = original_flags + ' --host-resolver-rules="MAP passkeys.ferric.test 127.0.0.1"'
        fixture = VerifierFixture(layout, bind="127.0.0.1", port=8443,
            rp_id="passkeys.ferric.test", origin="https://passkeys.ferric.test:8443",
            certificate=certificate, private_key=private_key)
        fixture.start()
        command = [str(binary), "--interactive" if args.interactive else "--check",
                   str(certificate), str(layout), "--direct-network"]
        process = subprocess.Popen(command, env=environment)
        result = process.wait(timeout=900 if args.interactive else 45)
    except (Exception, KeyboardInterrupt) as error:
        result = 1
        print(json.dumps({"rp_fixture_error": type(error).__name__}), flush=True)
    finally:
        cleanup_errors = {}
        cancelled = False

        def cancel_cleanup(_signum, _frame):
            nonlocal cancelled
            cancelled = True

        def attempt(stage, action):
            try:
                return action()
            except (Exception, KeyboardInterrupt) as error:
                cleanup_errors[stage] = type(error).__name__
                return None

        signal.signal(signal.SIGTERM, cancel_cleanup)
        previous_int = signal.signal(signal.SIGINT, cancel_cleanup)
        # Supervise the browser before stopping the RP, including when browser
        # wait/start fails. An RP shutdown error cannot skip child cleanup.
        if process and attempt("browser_poll", process.poll) is None:
            forced = True
            attempt("browser_terminate", process.terminate)
            attempt("browser_wait", lambda: process.wait(timeout=3))
            if attempt("browser_poll", process.poll) is None:
                attempt("browser_kill", process.kill)
                attempt("browser_kill_wait", lambda: process.wait(timeout=3))
        if fixture:
            attempt("rp_stop", fixture.close)
        deadline = time.monotonic() + 3
        while attempt("child_audit", reap_children) and time.monotonic() < deadline:
            time.sleep(0.05)
        # A real helper starts its own session. Audit kernel-adopted children;
        # signal only this layout's fixed helper, never a process-name match.
        for termination in (signal.SIGTERM, signal.SIGKILL):
            for pid in attempt("child_audit", reap_children) or []:
                if attempt("helper_cleanup", lambda: signal_owned_helper(pid, helper, termination)):
                    forced = True
            deadline = time.monotonic() + 2
            while attempt("child_audit", reap_children) and time.monotonic() < deadline:
                time.sleep(0.05)
        remaining = attempt("child_audit", reap_children)
        registrations = attempt("registration_metrics", lambda: fixture.httpd.app.verified_registrations) if fixture else 0
        assertions = attempt("assertion_metrics", lambda: fixture.httpd.app.verified_assertions) if fixture else 0
        registrations, assertions = registrations or 0, assertions or 0
        verified = registrations > 0 and assertions > 0
        helper_hash = attempt("helper_hash", lambda: hashlib.sha256(helper.read_bytes()).hexdigest()) if helper.exists() else None
        parent_stopped = not process or attempt("browser_poll", process.poll) is not None
        if remaining == [] and parent_stopped and not cleanup_errors:
            attempt("layout_remove", lambda: shutil.rmtree(layout))
        if (forced or cancelled or cleanup_errors or remaining != [] or not parent_stopped
                or (args.interactive and not verified) or (not args.interactive and (registrations or assertions))):
            result = 1
        print(json.dumps({"browser_exit": process.returncode if process else None,
            "fixture_exit": result, "interactive": args.interactive, "interrupted": cancelled,
            "cleanup_errors": cleanup_errors, "helper_sha256": helper_hash,
            "rp_verified_registrations": registrations, "rp_verified_assertions": assertions,
            "rp_verified_create_get": verified, "physical_qualified": False,
            "forced_cleanup": forced, "remaining_owned_pids": remaining}), flush=True)
        if layout.exists():
            print(json.dumps({"retained_ownership_evidence": str(layout)}), flush=True)
        print(json.dumps({"rp_layout_removed": not layout.exists()}), flush=True)
        signal.signal(signal.SIGTERM, previous_term)
        signal.signal(signal.SIGINT, previous_int)
        if cancelled or cleanup_errors:
            result = 1
    return result


if __name__ == "__main__":
    raise SystemExit(main())

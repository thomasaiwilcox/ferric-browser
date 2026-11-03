#!/usr/bin/python3
"""Ownership regressions for the synthetic helper runner, without Qt or keys."""
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time
import unittest


class RunnerOwnershipTests(unittest.TestCase):
    def run_fixture(self, mode):
        with tempfile.TemporaryDirectory(prefix="actor-runner-test-") as scratch:
            root = Path(scratch)
            browser = root / "owned-browser"
            browser.write_text('''#!/usr/bin/python3
import json, os, pathlib, subprocess, time
child = subprocess.Popen([str(pathlib.Path(__file__).with_name("ferric-phone-auth-helper"))],
                         stdin=subprocess.PIPE, stdout=subprocess.DEVNULL)
mode = os.environ["OWNED_RUNNER_TEST_MODE"]
request = {"v":1,"id":"1","op":"get","origin":"https://localhost:443",
           "network":{"mode":"direct"},"options":{"challenge":"CA" if mode == "normal" else "Bg"}}
child.stdin.write((json.dumps(request)+"\\n").encode()); child.stdin.flush()
if mode == "normal":
    raise SystemExit(child.wait(timeout=5))
if mode == "malformed":
    time.sleep(0.3)
    with open(os.environ["FERRIC_PHONE_ACTOR_RECEIPT"], "a") as receipt:
        receipt.write("invalid ownership receipt\\n")
time.sleep(60)
''')
            browser.chmod(0o700)
            environment = dict(os.environ, OWNED_RUNNER_TEST_MODE=mode)
            command = ["python3", str(Path(__file__).with_name("run_phone_actor_fixture.py")),
                       str(browser), "--scratch-root", scratch, "--timeout", "1" if mode != "interrupt" else "30"]
            process = subprocess.Popen(command, env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            try:
                if mode == "interrupt":
                    deadline = time.monotonic() + 5
                    while not any(path.read_text().count('"kind":"request"') for path in root.glob("phone-actor-*/helper.jsonl")):
                        self.assertIsNone(process.poll())
                        self.assertLess(time.monotonic(), deadline)
                        time.sleep(0.02)
                    process.send_signal(signal.SIGTERM)
                stdout, stderr = process.communicate(timeout=20)
            finally:
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=3)
            self.assertEqual(stderr, "")
            rows = [json.loads(line) for line in stdout.splitlines()]
            receipt = next(row for row in rows if "test_exit" in row)
            if mode == "normal":
                self.assertEqual(process.returncode, 0)
                self.assertTrue(receipt["helpers_reaped"])
                self.assertFalse(receipt["forced_parent_cleanup"])
                self.assertFalse(receipt["forced_helper_cleanup"])
            else:
                self.assertNotEqual(process.returncode, 0)
                self.assertFalse(receipt["helpers_reaped"])
                self.assertTrue(receipt["forced_parent_cleanup"])
                self.assertTrue(receipt["forced_helper_cleanup"])
            if mode == "malformed":
                self.assertFalse(receipt["receipts_valid"])
                self.assertFalse(rows[-1]["fixture_layout_removed"])
                self.assertTrue(any("retained_ownership_evidence" in row for row in rows))
            else:
                self.assertTrue(receipt["cleanup_complete"])
                self.assertTrue(rows[-1]["fixture_layout_removed"])

    def test_normal_exit(self):
        self.run_fixture("normal")

    def test_timeout(self):
        self.run_fixture("timeout")

    def test_interruption(self):
        self.run_fixture("interrupt")

    def test_malformed_receipt_preserves_evidence(self):
        self.run_fixture("malformed")


if __name__ == "__main__":
    unittest.main()

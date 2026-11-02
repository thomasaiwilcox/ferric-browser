"""Runner teardown outcomes with an owned no-op browser and no authenticator."""
import contextlib
import io
import json
import os
from pathlib import Path
import signal
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "public-page-host"))
import run_phone_rp


class RunnerTeardownTests(unittest.TestCase):
    def exercise(self, mode):
        class FakeRp:
            def __init__(self, *_args, **_kwargs):
                self.httpd = types.SimpleNamespace(app=types.SimpleNamespace(
                    verified_registrations=0, verified_assertions=0,
                    verified_discoverable_assertions=0))

            def start(self):
                return self

            def close(self):
                if mode == "interrupt":
                    os.kill(os.getpid(), signal.SIGTERM)
                if mode == "close-error":
                    raise OSError("owned fixture shutdown failure")

        with tempfile.TemporaryDirectory(prefix="rp-runner-test-") as temporary:
            root = Path(temporary)
            browser, helper = root / "browser", root / "helper"
            for binary in (browser, helper):
                binary.write_text("#!/usr/bin/python3\nraise SystemExit(0)\n")
                binary.chmod(0o700)
            output = io.StringIO()
            arguments = ["run_phone_rp.py", "--browser", str(browser), "--helper", str(helper), "--scratch-root", temporary]
            with patch.object(run_phone_rp, "VerifierFixture", FakeRp), patch.object(sys, "argv", arguments), contextlib.redirect_stdout(output):
                result = run_phone_rp.main()
            rows = [json.loads(line) for line in output.getvalue().splitlines()]
            receipt = next(row for row in rows if "browser_exit" in row)
            self.assertEqual(receipt["browser_exit"], 0)
            self.assertEqual(receipt["remaining_owned_pids"], [])
            if mode == "normal":
                self.assertEqual(result, 0)
                self.assertTrue(rows[-1]["rp_layout_removed"])
            else:
                self.assertNotEqual(result, 0)
                self.assertNotEqual(receipt["fixture_exit"], 0)
            if mode == "interrupt":
                self.assertTrue(receipt["interrupted"])
                self.assertTrue(rows[-1]["rp_layout_removed"])
            if mode == "close-error":
                self.assertEqual(receipt["cleanup_errors"]["rp_stop"], "OSError")
                self.assertFalse(rows[-1]["rp_layout_removed"])
                self.assertTrue(any("retained_ownership_evidence" in row for row in rows))

    def test_normal_browser_and_rp_teardown(self):
        self.exercise("normal")

    def test_interrupted_rp_teardown_after_successful_browser_is_failure(self):
        self.exercise("interrupt")

    def test_rp_close_error_cannot_skip_audit_or_erase_evidence(self):
        self.exercise("close-error")


if __name__ == "__main__":
    unittest.main()

"""Successor gates reject dependency introduction and stale/fake evidence."""
import argparse
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("kde_free", Path(__file__).with_name("kde_free.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class QualificationTests(unittest.TestCase):
    def root(self, root):
        for name in ["crates", "packaging", ".github/ci", "tools/performance"]:
            (root / name).mkdir(parents=True)
        for name in ["Cargo.toml", "packaging/PKGBUILD", "packaging/dependencies.toml"]:
            (root / name).write_text("")

    def test_new_framework_discovery_is_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            self.root(root)
            (root / "crates/CMakeLists.txt").write_text("find_package(KF6CoreAddons REQUIRED)\n")
            with patch.object(module, "ROOT", root), self.assertRaisesRegex(ValueError, "explicit user approval"):
                module.policy()

    def test_kde_runtime_package_declaration_is_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            self.root(root)
            (root / "packaging/PKGBUILD").write_text("depends=('kio')\n")
            with patch.object(module, "ROOT", root), self.assertRaisesRegex(ValueError, "explicit user approval"):
                module.policy()

    def test_historical_receipts_and_negative_runtime_assertions_are_allowed(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            self.root(root)
            historical = root / "docs/implementation/old"
            historical.mkdir(parents=True)
            (historical / "receipt.json").write_text('{"KF6WindowSystem": "6.28.0"}')
            (root / "crates/test.cpp").write_text('QVERIFY(!maps.contains("libKF6"));\n')
            with patch.object(module, "ROOT", root):
                self.assertEqual(module.policy()["policy"], "passed")

    def test_changed_evidence_is_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "artifact"
            path.write_bytes(b"original")
            ref = module.reference(path)
            path.write_bytes(b"replacement")
            with self.assertRaisesRegex(ValueError, "changed evidence"):
                module.checked(ref)

    def test_failed_commands_remain_failed_receipts(self):
        with tempfile.TemporaryDirectory() as folder:
            args = argparse.Namespace(output=Path(folder) / "failure.json", artifact=None, junit=None,
                                      command=[sys.executable, "-c", "raise SystemExit(7)"])
            with patch.object(module, "snapshot", return_value={"sha256": "fixture"}):
                self.assertEqual(module.record(args), 7)
            receipt = json.loads(args.output.read_text())
            self.assertEqual(receipt["returncode"], 7)
            module.checked(receipt["log"])

    def test_failed_build_keeps_receipt_when_artifact_is_missing(self):
        with tempfile.TemporaryDirectory() as folder:
            args = argparse.Namespace(output=Path(folder) / "failure.json", artifact=Path(folder) / "absent",
                                      junit=None, command=[sys.executable, "-c", "raise SystemExit(9)"])
            with patch.object(module, "snapshot", return_value={"sha256": "fixture"}):
                self.assertEqual(module.record(args), 9)
            receipt = json.loads(args.output.read_text())
            self.assertEqual(receipt["returncode"], 9)
            self.assertIn("capture_errors", receipt)

    def test_historical_schema_cannot_claim_v2_qualification(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "receipt.json"
            path.write_text(json.dumps({"schema": 1, "KF6WindowSystem": "passed"}))
            with patch.object(module, "snapshot", return_value={}), self.assertRaises(ValueError):
                module.verify(path)

    def test_raw_performance_regression_cannot_be_hidden_by_summary(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            artifact = root / "product"
            artifact.write_bytes(b"frozen product")
            ref = module.reference(artifact)
            source = {"sha256": "fixture"}
            rows = []
            for pair in range(5):
                for label in ["baseline", "candidate"]:
                    path = root / f"{pair}-{label}.json"
                    path.write_text(json.dumps({"source": source, "artifact": ref, "probe": ref,
                        "label": label, "pair": pair, "workload": "startup-1", "warmups": 10,
                        "samples": 100, "values": [{"milliseconds": 1 if label == "baseline" else 999}] * 100,
                        "warmup_values": [1] * 10, "processes": [{"pid": i + 1} for i in range(110)]}))
                    rows.append(module.reference(path))
            workloads = dict.fromkeys(["startup-1", "startup-20", "creation", "recreation", "idle-1", "idle-20"])
            workloads["startup-1"] = {"raw_receipts": rows,
                "metrics": {"milliseconds": {"baseline": 1, "candidate": 1}}}
            performance = {"schema": module.SCHEMA, "candidate_source": source,
                "baseline_artifact": ref, "candidate_artifact": ref, "probe": ref,
                "pairs": 5, "warmups": 10, "samples": 100, "pilot": False, "workloads": workloads}
            with self.assertRaisesRegex(ValueError, "Summary does not match"):
                module.verify_performance(performance, source, ref)


if __name__ == "__main__":
    unittest.main()

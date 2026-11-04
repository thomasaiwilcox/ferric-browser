"""Successor gates reject dependency introduction and stale/fake evidence."""
import argparse
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET
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

    def qt_report(self, message="'request()' returned FALSE. ()", passed=28):
        suite = ET.Element("testsuite")
        case = ET.SubElement(suite, "testcase", name="existing_component")
        ET.SubElement(case, "failure", message="Failed")
        ET.SubElement(case, "system-out").text = (
            "FAIL!  : ExistingTests::policy() " + message + "\n"
            "   Loc: [/control/source/crates/example/test.cpp(42)]\n"
            f"Totals: {passed} passed, 1 failed, 0 skipped, 0 blacklisted, 1ms\n")
        return suite

    def test_baseline_comparison_preserves_actual_failure(self):
        self.assertEqual(module.compare_baseline(self.qt_report(), self.qt_report()),
                         ["existing_component"])

    def test_baseline_cannot_exempt_a_new_assertion_failure(self):
        with self.assertRaisesRegex(ValueError, "New assertion failure"):
            module.compare_baseline(self.qt_report("'new_generation()' returned FALSE. ()"),
                                    self.qt_report())

    def test_baseline_cannot_hide_reduced_test_coverage(self):
        with self.assertRaisesRegex(ValueError, "reduced Qt test coverage"):
            module.compare_baseline(self.qt_report(passed=27), self.qt_report())

    def test_baseline_cannot_hide_a_new_skip(self):
        report = self.qt_report()
        ET.SubElement(report.find("testcase"), "skipped")
        with self.assertRaisesRegex(ValueError, "Errors or skips"):
            module.compare_baseline(report, self.qt_report())

    def idle_comparison(self, root, candidate_cpu):
        artifact = root / "product"
        artifact.write_bytes(b"policy fixture, not a qualification artifact")
        ref, source = module.reference(artifact), {"sha256": "fixture"}
        workloads = {}
        for name in ["startup-1", "startup-20", "creation", "recreation", "idle-1", "idle-20"]:
            idle = name.startswith("idle")
            rows = []
            for pair in range(5):
                for label in ["baseline", "candidate"]:
                    values = [{"tree_cpu_percent": candidate_cpu if label == "candidate" else 2,
                               "gui_wakeups_per_second": 10}] if idle else [
                        {"milliseconds": 1, "operation": "role" if index % 2 else "surface"}
                        for index in range(100)]
                    path = root / f"{name}-{pair}-{label}.json"
                    path.write_text(json.dumps({"source": source, "artifact": ref, "probe": ref,
                        "label": label, "pair": pair, "workload": name, "warmups": 10,
                        "samples": 100, "values": values, "started_ns": 1000 + pair,
                        "warmup_values": [] if idle else [{"milliseconds": 1}] * 10,
                        "processes": [{"pid": i + 1} for i in range(110 if name.startswith("startup") else 1)]}))
                    rows.append(module.reference(path))
            entry = {"raw_receipts": rows, "metrics": {"milliseconds": {"baseline": 1, "candidate": 1}}}
            if idle:
                repetitions, cpus, wakeups = [], [], []
                for index in range(5):
                    cpu, awake = 2 + index / 10, 10 + index
                    path = root / f"{name}-repeat-{index}.json"
                    path.write_text(json.dumps({"source": source, "artifact": ref, "probe": ref,
                        "label": "baseline", "pair": f"repeat-{index}", "workload": name,
                        "baseline_repetition": True, "processes": [{"pid": index + 1}],
                        "finished_ns": index + 10,
                        "values": [{"tree_cpu_percent": cpu, "gui_wakeups_per_second": awake}]}))
                    repetitions.append(module.reference(path))
                    cpus.append(cpu)
                    wakeups.append(awake)
                entry["metrics"] = {
                    field: {"baseline": 2 if field == "tree_cpu_percent" else 10,
                            "candidate": candidate_cpu if field == "tree_cpu_percent" else 10,
                            "baseline_repetitions": samples, "baseline_raw_receipts": repetitions}
                    for field, samples in [("tree_cpu_percent", cpus), ("gui_wakeups_per_second", wakeups)]}
            workloads[name] = entry
        return {"schema": module.SCHEMA, "candidate_source": source,
                "baseline_artifact": ref, "candidate_artifact": ref, "probe": ref,
                "pairs": 5, "warmups": 10, "samples": 100, "pilot": False,
                "workloads": workloads}, source, ref

    def test_idle_decrease_below_baseline_variability_is_allowed(self):
        with tempfile.TemporaryDirectory() as folder:
            module.verify_performance(*self.idle_comparison(Path(folder), 1.5))

    def test_idle_increase_above_baseline_variability_requires_resolution(self):
        with tempfile.TemporaryDirectory() as folder:
            with self.assertRaisesRegex(ValueError, "Unresolved idle variability/regression"):
                module.verify_performance(*self.idle_comparison(Path(folder), 2.5))

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

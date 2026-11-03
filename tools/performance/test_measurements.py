"""Measurement-contract regressions; no browser or timed assertions."""
import importlib.util
import tempfile
import unittest
from pathlib import Path

import benchmark_matcher

REPO = Path(__file__).resolve().parents[2]
COMPARE = REPO / 'docs/implementation/keyboard_interaction-v03/tools/compare_performance.py'
spec = importlib.util.spec_from_file_location('qml_compare', COMPARE)
qml_compare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(qml_compare)


class MeasurementTests(unittest.TestCase):
    def test_nearest_rank_and_paired_retention(self):
        self.assertEqual(benchmark_matcher.p95(list(range(1, 101))), 95)
        runs = [{'baseline': [{'query': 'a', 'count': 5000, 'samples_ms': [5.0] * 100}],
                 'candidate': [{'query': 'a', 'count': 5000, 'samples_ms': [2.0] * 100}]}
                for _ in range(5)]
        result = benchmark_matcher.compare(runs)[0]['p95']
        self.assertTrue(result['passes_gain_gate'])
        self.assertFalse(result['regression'])
        self.assertEqual(result['paired_mean_delta_95ci_ms'], [-3.0, -3.0])

    def test_historical_log_remains_readable(self):
        log = REPO / 'docs/implementation/keyboard_interaction-v03/evidence/S25-qml-perf.log'
        self.assertEqual(len(qml_compare.read_rows(log)), 9)

    def test_missing_scenarios_fail_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / 'empty.log'
            log.write_text('No measurements\n')
            with self.assertRaises(AssertionError):
                qml_compare.read_rows(log)


if __name__ == '__main__':
    unittest.main()

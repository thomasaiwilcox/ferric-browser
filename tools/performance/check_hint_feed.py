#!/usr/bin/env python3
"""Paired visible-target and timing checks against a committed collector."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess

ROOT = Path(__file__).resolve().parents[2]
SOURCE = 'crates/ferric-browser-engine-qt/qml/scripts/PageInteractionScripts.js'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', default='c91d660')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--qml-tests', type=Path, default=ROOT / 'target/site-policy-baseline/qml/qml_tests')
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    control = subprocess.check_output(['git', 'show', args.baseline + ':' + SOURCE], cwd=ROOT)
    candidate = (ROOT / SOURCE).read_bytes()
    (out / 'control.js').write_bytes(control)
    (out / 'candidate.js').write_bytes(candidate)
    env = dict(os.environ, FERRIC_TEST_ARTIFACT_DIR=str(out), QML_XHR_ALLOW_FILE_READ='1',
               QT_QPA_PLATFORM='offscreen', QT_QUICK_BACKEND='software', QTWEBENGINE_CHROMIUM_FLAGS='--disable-gpu')
    command = [str(ROOT / 'tools/performance/with_qt611.sh'), str(args.qml_tests), '-input',
               str(ROOT / 'crates/ferric-browser-engine-qt/tests/qml-perf/tst_hint_feed.qml'), '-o', str(out / 'test.log') + ',txt']
    result = subprocess.run(command, cwd=ROOT, env=env)
    record = {'baseline': args.baseline, 'control_sha256': hashlib.sha256(control).hexdigest(),
              'candidate_sha256': hashlib.sha256(candidate).hexdigest(), 'exit_code': result.returncode,
              'command': command, 'measurements': []}
    for line in (out / 'test.log').read_text().splitlines():
        if 'FERRIC_FEED_JSON ' in line:
            record['measurements'] = json.loads(line.split('FERRIC_FEED_JSON ', 1)[1])
        if 'FERRIC_FEED_COLD_JSON ' in line:
            record['fresh_pages'] = json.loads(line.split('FERRIC_FEED_COLD_JSON ', 1)[1])
    record['timings'] = []
    for row in record['measurements']:
        if 'control' not in row:
            continue
        summary = {'y': row['y']}
        for key in ('control', 'candidate'):
            samples = sorted(row[key])
            summary[key] = {'median_ms': statistics.median(samples), 'p95_ms': samples[18], 'samples': len(samples)}
        record['timings'].append(summary)
    (out / 'result.json').write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps({'exit_code': result.returncode, 'timings': record['timings']}, indent=2))
    return result.returncode


if __name__ == '__main__':
    raise SystemExit(main())

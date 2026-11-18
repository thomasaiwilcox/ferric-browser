#!/usr/bin/env python3
"""Measure strict collector boundaries and a non-actionable proposal diagnostic."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'crates/ferric-browser-engine-qt/tests/qml-perf/tst_hint_progressive_feasibility.qml'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--qml-tests', type=Path, default=ROOT / 'target/qml-tests/qml_tests')
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, QT_QPA_PLATFORM='offscreen', QT_QUICK_BACKEND='software',
               QTWEBENGINE_CHROMIUM_FLAGS='--disable-gpu')
    command = [str(ROOT / 'tools/performance/with_qt611.sh'), str(args.qml_tests.resolve()),
               '-input', str(FIXTURE), '-o', str(out / 'fixture.log') + ',txt']
    result = subprocess.run(command, cwd=ROOT, env=env)
    fixtures = []
    for line in (out / 'fixture.log').read_text().splitlines():
        marker = 'PROGRESSIVE_FEASIBILITY '
        if marker not in line:
            continue
        value = json.loads(line.split(marker, 1)[1])
        rows = value['rows']
        value['summary'] = {
            key: statistics.median(row[key] for row in rows)
            for key in ('cheap_ms', 'ordinary_ms', 'policy_ms')
        }
        value['summary'].update(
            cheap_count=len(rows[0]['cheap_ids']), strict_count=len(rows[0]['strict_ids']),
            stable_ids=all(row['cheap_ids'] == rows[0]['cheap_ids']
                           and row['strict_ids'] == rows[0]['strict_ids'] for row in rows))
        fixtures.append(value)
    sources = [FIXTURE, ROOT / 'crates/ferric-browser-engine-qt/qml/scripts/PageInteractionScripts.js',
               ROOT / 'crates/ferric-browser-engine-qt/qml/scripts/InteractionRootRuntime.js',
               ROOT / 'crates/ferric-browser-engine-qt/qml/scripts/InteractionWitnessRuntime.js']
    record = dict(
        commit=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
        command=command, exit_code=result.returncode,
        scope='Renderer bodies only; no native seals, allocation, input, transport or presentation. '
              'The cheap observation always precedes strict collection and is not an optimized implementation.',
        sources={str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sources},
        fixtures=fixtures)
    (out / 'result.json').write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps({row['fixture']: row['summary'] for row in fixtures}, indent=2))
    if not result.returncode and len(fixtures) != 4:
        raise RuntimeError('Missing fixture results')
    return result.returncode


if __name__ == '__main__':
    raise SystemExit(main())

#!/usr/bin/env python3
"""Compare raw C17 samples without changing performance budgets."""
import argparse
import json
import math
from pathlib import Path
import statistics


def read_rows(path, overlay_only=False):
    result = {}
    decoder = json.JSONDecoder()
    for line in Path(path).read_text().splitlines():
        for marker, kind in [('FERRIC_PERFORMANCE_JSON ', 'collector'),
                             ('FERRIC_OVERLAY_PERFORMANCE_JSON ', 'overlay')]:
            if marker in line:
                row, _ = decoder.raw_decode(line.split(marker, 1)[1])
                assert row['warmups'] == 10, (path, row['scenario'])
                samples = row['samples'] if kind == 'collector' else row['time_to_visible_ms']
                assert len(samples) == 100, (path, row['scenario'], len(samples))
                key = (kind, row['scenario'])
                assert key not in result, ('duplicate scenario', path, key)
                result[key] = row
    if overlay_only:
        result = {key: row for key, row in result.items() if key[0] == 'overlay'}
    assert len(result) == (3 if overlay_only else 9), ('missing scenarios', path, list(result))
    return result


def p95(values):
    assert all(math.isfinite(x) and x >= 0 for x in values)
    return sorted(values)[math.ceil(len(values) * .95) - 1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('baseline')
    parser.add_argument('current')
    parser.add_argument('--output', required=True)
    parser.add_argument('--overlay-only', action='store_true')
    args = parser.parse_args()
    baseline, current = read_rows(args.baseline, args.overlay_only), read_rows(args.current, args.overlay_only)
    assert baseline.keys() == current.keys()
    comparisons = []
    for key, old in baseline.items():
        new = current[key]
        assert (old['count'], old['zoom'], old['viewport']) == (new['count'], new['zoom'], new['viewport'])
        if key[0] == 'collector':
            assert old['user_agent'] == new['user_agent']
            stages = [name for name in old['samples'][0] if name.endswith('_ms')]
            sources = [(name, [x[name] for x in old['samples'] if x[name] is not None],
                        [x[name] for x in new['samples'] if x[name] is not None]) for name in stages]
            sources.extend(('collector.' + name,
                            [x['collector'][name] for x in old['samples']],
                            [x['collector'][name] for x in new['samples']])
                           for name in old['samples'][0]['collector'] if name.endswith('_ms'))
        else:
            sources = [(name, old[name], new[name]) for name in ('time_to_visible_ms', 'layout_compute_ms')]
        for stage, a, b in sources:
            if not a and not b:
                continue
            assert len(a) == len(b) == 100
            before, after = p95(a), p95(b)
            delta = after - before
            percentage = delta / before * 100 if before else None
            common_callback = key[0] == 'collector' and old['count'] == 500 and stage == 'browser_round_trip_ms'
            comparisons.append(dict(kind=key[0], scenario=key[1], stage=stage,
                                    baseline_p50_ms=statistics.median(a), current_p50_ms=statistics.median(b),
                                    baseline_p95_ms=before, current_p95_ms=after,
                                    delta_ms=delta, delta_percent=percentage,
                                    investigation_required=common_callback and delta > 10 and percentage > 10,
                                    above_overlay_ambition=key[0] == 'overlay' and stage == 'time_to_visible_ms' and after > 200))
    Path(args.output).write_text(json.dumps(dict(method='nearest-rank p95; 10 warmups and 100 samples; same viewport and user agent',
                                               baseline=args.baseline, current=args.current,
                                               comparisons=comparisons), indent=2) + '\n')
    for row in comparisons:
        if row['stage'] in ('browser_round_trip_ms', 'time_to_visible_ms'):
            print(row['scenario'], row['stage'], round(row['baseline_p95_ms'], 2), '->', round(row['current_p95_ms'], 2),
                  'investigate' if row['investigation_required'] else '', 'above ambition' if row['above_overlay_ambition'] else '')


if __name__ == '__main__':
    main()

# Hint collector component measurements

The collector reuses geometry, ancestor clipping, computed styles and
scrollability facts within one synchronous collection. It also avoids unused
scrollability queries for candidates whose family does not need them. All
per-call caches are released on normal return, early return and failure.
Selection and action validation continue to read the live page.

[Raw samples and source hashes](cold-path-results.json) record the 8 November
2026 comparison with immutable baseline
`30e9daf34079186dd5775d5f9c18b26c40176344`. The measured script and test files are
unchanged in this branch; the record retains its original working-branch parent
and historical validation references.

| Visible buttons | Original median | Cached median |
|---|---:|---:|
| 100 | 20.90 ms | 19.45 ms |
| 500 | 61.45 ms | 55.20 ms |
| 1,000 | 118.00 ms | 105.05 ms |
| 5,000 | 1,290.25 ms | 1,195.70 ms |

The 1,000-hidden-button case increased from 22.25 to 23.45 ms. The half-offscreen
case decreased from 384.35 to 374.80 ms. All observations, including regressions,
are retained in the JSON.

These are offscreen/software WebEngine collector timings in one process, with
four warmups and twenty alternating observations per variant and cell. They do
not measure keyboard-to-paint latency, hardware graphics or energy use.

Run `python3 tools/performance/check_hint_cold_path.py` with the existing qualified
Qt 6.11.2 `qml_tests` binary, or supply `--qml-tests /absolute/path`. The helper
exports baseline scripts from Git into temporary storage. The independent
fixtures cover mixed controls, clipping, shadows and frames, live mutations and
inspection/candidate limits; no implementation-pack controller is required.

The differential/performance suite is opt-in under `tests/qml-perf`, outside
default functional discovery.

[Early-input behavior and validation](early-input.md) describe the companion fix
for keys typed before the first Rust hint session exists.

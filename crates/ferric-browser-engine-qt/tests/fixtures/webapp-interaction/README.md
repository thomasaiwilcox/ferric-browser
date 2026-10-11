# Unified interaction fixtures

These self-contained synthetic fixtures are authored for Ferric under its
GPL-3.0-or-later licence. They do not vendor a named framework, require live
network access, or contain real account data. The password string in the
privacy regression is fixed synthetic data. Effects are observed using DOM
data attributes, not a page-supplied privileged bridge.

`tests/qml/tst_webapp_interaction.qml` exercises production collection,
fresh validation and final focus/click in ApplicationWorld. The native suite
adds original Qt text entry after the same production shadow-focus pipeline.
The opt-in benchmark in `tests/qml-perf/` measures production renderer script
timings; it does not stand in for full browser input or live-site proof.

Canonical test identities, original baseline red logs, raw measurements and
remaining coverage gaps are in
`docs/implementation/keyboard_interaction-v03/`. Do not weaken the positive
assertions because they fail before S02; that is the reproduced baseline.

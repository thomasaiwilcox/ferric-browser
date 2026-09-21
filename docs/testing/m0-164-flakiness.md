# M0-164 flakiness evidence

Date: 2026-09-18  
Status: in-progress

The repeat entry point runs the Qt-independent browser-core and
browser-storage suites three times in fresh Cargo test processes. It uses the
normal test scheduler, so reducer ordering, SQLite worker handoffs, bounded
queues, and transactional storage tests are exercised repeatedly without
turning the check into an unbounded stress job.

The entry point fails on the first unsuccessful run and prints the run number.
The full workspace check remains the broader deterministic gate.

## Validation

- cargo xtask test repeat
- cargo xtask check --locked --offline

Native Qt/Wayland repetition, IME/accessibility input, portal cancellation,
renderer teardown, filesystem-full behavior, and long-duration soak testing
still require a qualified desktop or release environment.

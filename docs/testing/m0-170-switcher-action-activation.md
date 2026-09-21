# M0-170 switcher action activation evidence

Status: in progress, implementation slice recorded 2026-09-18.

The universal switcher now accepts an `actions` scope and includes registry
actions that declare switcher support and require no explicit argument. These
rows use `kind = "action"`, stable action IDs, bounded match fields, and the
single `execute` activation. Activation re-resolves the shared action registry,
rejects stale or argument-bearing rows, rechecks capability/privacy availability
at the execution boundary, sets `CommandSource::Switcher`, and routes execution
through the existing typed IPC/action executor.

Configured external URL and tab targets are also represented as available
`kind = "action"` rows and revalidated against the current profile and shared
capability/privacy availability checks at activation. Link and selection targets remain out of the argument-free scope
because they require a live contextual subject; they continue through their
hint/context-menu surfaces rather than receiving an inferred subject.

Live tab rows carry their captured generation through the QML bridge and typed
`switcher.activate` IPC path. Rust compares that token with the current tab
before focusing or activating it; a missing tab, mismatched generation, or
generation supplied for a non-tab result is rejected without falling back to a
same-looking replacement. The pure generation validator is covered for the
no-token, matching, stale, and non-tab cases in the Qt regression suite.

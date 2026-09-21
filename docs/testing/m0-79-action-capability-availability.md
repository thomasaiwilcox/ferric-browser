# M0-79 action capability and availability metadata

Date: 2026-09-16  
Status: in-progress

## Scope

Action discovery now describes the capabilities that an action may need, and
the native and typed IPC surfaces expose a bounded availability object. The
shared registry remains the source of stable action IDs, subjects, verbs,
arguments, examples, and executor commands.

Built-in capability declarations currently cover:

- configured external action targets;
- a live document for selection search;
- durable download indexing;
- durable profile storage for history, bookmark, and quickmark actions;
- the durable context registry; and
- the command registry for command help and execution.

The live-document capability is revalidated against the current captured tab,
document identity, live existence state, and healthy renderer state. A core
state object by itself is not enough to advertise selection search as ready.

`actions.query` and native `action-list` both report the registry's typed
arguments, allowed invocation sources, sensitivity, current state, and a
non-sensitive reason such as
`ready`, `no-configured-target-for-subject`, or
`durable-storage-unavailable`. External target rows include the same bounded
availability envelope, with link and selection rows derived independently from
the target's declared subject set; private/ephemeral profiles also mark
targets with `allow_private = false` unavailable. Native `action-list` includes
the same runtime result for built-in actions; configured external target rows
retain their target-specific discovery data.

## Enforcement

Before a typed `action.execute` request is dispatched, Rust resolves the stable
action ID and rechecks its current capability state. Browser-owned UI action
activation performs the same check before entering the existing typed command
path. This keeps discovery advisory while ensuring that a capability can be
revoked between listing and execution. Rejected actions use the existing
structured action error/audit path and do not expose URLs, titles, or stored
entry contents.

Every built-in availability object marks
`requires_subject_revalidation: true`, since a capability check does not replace
the executor's exact tab, window, selection, download, profile, or privacy
validation.

## Verification

- `cargo test -p browser-core -p browser-engine-qt --locked --offline`
- `cargo xtask check --locked`
- `cargo build -p rustbrowser --locked`
- `cargo fmt --all -- --check`

The focused regression test verifies explicit capability metadata and the
subject-revalidation marker, including the healthy-live-document boundary.
Remaining work is broader predicate coverage for
all capability-sensitive actions, confirmation parity across every surface,
localization/provenance metadata, and live Wayland/fixture qualification.

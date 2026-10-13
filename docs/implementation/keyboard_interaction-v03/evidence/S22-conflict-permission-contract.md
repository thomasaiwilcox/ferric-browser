# S22 conflict and permission contract

This is a research boundary for any future framework-specific adapter. It
does not grant Ferric a production content-read or content-write capability.

## Independent grants

Focus targets an exact receiver or editor instance and does not expose its
document. Extraction is a separate, explicit, bounded read request. Apply is a
separate, explicit confirmation for one proposed change to one captured
editor/document revision. Framework commands such as undo, formatting, or
selection movement are separate actions and require their own user intent.
No remembered focus, extraction, or apply grant transfers to a replaced editor
or a later navigation.

The existing C07 proxy rule remains only a focus rule. A proxy association
cannot grant extraction or apply permission.

## Required captured identity

An adapter must retain a short-lived capture containing:

- the owning top-level page/document epoch and frame identity;
- the exact framework editor instance, model/session/document object, and live
  focus target;
- the adapter's document-change revision, plus a framework version token when
  available (for example Monaco's model version ID);
- the selected range or mapped bookmark, expected selection state, and a
  digest or bounded description of the proposed operation;
- the specific consent scope and its expiry.

## Apply transaction

Immediately before apply, revalidate the owning page/document epoch, exact
frame, editor and document/model/session object, revision/version, schema,
range, selection mapping, and consent. If any check changed, return a bounded
`stale-target`, `wrong-owner`, `wrong-frame`, `config-revision-changed`, or
`unsupported-capability` result without calling the editing API. Do not
re-discover a replacement editor, rebase silently, retry an uncertain call,
or roll back a collaborative document.

An accepted operation must use the framework's documented transaction/edit
API as one bounded change. It must preserve supported formatting and selection
semantics, enter the expected framework undo history, and either apply in full
or produce no document mutation. A whole-document setter is not a substitute
for a local transaction. The adapter must report the observed result without
claiming atomicity against page scripts, plugins, or collaboration clients
that can mutate outside its call boundary.

## Privacy and errors

Page-derived text, JSON, Deltas, HTML, and model values remain private and
ephemeral. Do not place them in hints, logs, diagnostics, browser history,
telemetry, or persisted command history. Diagnostics may show only a bounded
framework identifier, operation category, revision-conflict category, and
result status. Consent denial and conflicts are no-ops; no content is included
in the error.

## Research result

The S22 fixture demonstrates these checks with disposable local sample content.
Its boolean consent flag and counters are not application security controls.
Production remains no-go until each supported framework has an owned adapter,
versioned target contract, real consent UI, framework-specific regression
tests, and native keyboard/IME qualification.

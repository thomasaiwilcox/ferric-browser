# S07 implementation outline and evidence map

## Bounded model

Add a pure `ferric-browser-core` model for `browser-first` and `mixed` without
adding configuration fields, settings-registry entries, generated documentation,
or runtime routing. The public core types let S08 consume the same validated
policy contract synchronously; until S08, no user-visible setting is accepted
or advertised.

Each key is the existing logical token produced by
`KeyboardPresentation.logicalNormalKeyText`. Preserve its spelling, case,
modifier names, and layout dependence. Accept only the exact token vocabulary
already emitted by that function or one printable Unicode grapheme that fits
its existing four UTF-16-unit text bound. Do not infer physical scan codes or
map `AltGr` to `Ctrl+Alt`.

Resolve global, profile, site, then runtime layers. A supplied `page_keys`
array replaces the inherited set, including when the supplied array is empty;
an omitted field inherits. Validate every supplied layer and fail atomically on
unknown policy/key syntax, reserved controls, sequences, duplicate canonical
tokens, or more than 64 entries. Retain source provenance for policy and list
values, including an explicitly empty list.

The pure first-key decision reports page/browser ownership and its reason.
Non-Normal modes and reserved controls remain browser-owned. An already-active
browser count or keychain owns its continuation before a Mixed mapping is
considered. The existing binding resolver remains responsible for the actual
binding result; mismatches and timeouts consume/reset its buffered prefix and
must never reclassify or replay that prefix to the page. A policy revision
cancels the incomplete resolver state before the next gesture; it cannot
reclassify a gesture already in progress.

## Tests and evidence

| Test | Assertion | Evidence class |
| --- | --- | --- |
| T021 | Exact existing token vocabulary, one-grapheme text, case/layout and AltGr distinction, limits, reserved controls, duplicate detection, ordered replace/inherit layers, and atomic errors. | Pure unit |
| T023 | An idle mapped `g` or digit goes to the page; active browser counts/chains own continuations; mismatch and timeout do not replay a prefix or suffix; a later fresh gesture is evaluated normally. | Pure unit |
| T055 | No user-visible registry/config/docs are added in S07; workspace checks keep generated contracts and documentation in parity. | Pure unit |

Planned source changes are limited to a new `crates/ferric-browser-core/src/mixed_policy.rs`, its declaration/re-exports in `lib.rs`, the core crate's direct dependency on its already-locked Unicode grapheme helper, a QML-token parity test in `crates/ferric-browser-engine-qt/src/tests/presentation_boundaries.rs`, and this evidence note. The model is separate from `input.rs` because it classifies page-versus-browser ownership before the existing binding resolver and does not modify binding execution or native event routing.

Run focused core tests, `cargo xtask check --locked --offline`,
`cargo xtask test engine`, formatter, strict workspace Clippy, and the pack
validator. Record actual final commit hashes and logs in `evidence/S07.json`.

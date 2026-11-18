# FerricBrowser fuzz corpus

These seeds exercise the parser and validation boundaries described by
TEST-006. Keep minimized, non-sensitive reproductions here; never add real
credentials, cookies, account data, or private URLs.

The default Qt-free targets cover public parser, validation, routing, durable-session, and
hint-label boundaries that are safe to exercise without starting Qt or a
display server:

- `command_parser` parses interactive, CLI, and IPC command chains.
- `ipc_frames` exercises framed requests and response serialization.
- `url_and_clean_link` validates URLs and applies built-in link cleaning.
- `switcher_matching` tokenizes and ranks bounded switcher fields.
- `site_rule` parses and validates one site rule.
- `interaction_policy` parses strict TOML/JSON Interaction data, root paths,
  selectors and canonical key tokens; validated policies round-trip and compile
  bounded predicates with one shared work counter. It has no publication or
  native-input entry point. Corpus smoke does not claim coverage-guided fuzzing.
- `contexts_routes` parses `contexts.toml` data and evaluates bounded route
  matching.
- `session_snapshot` parses imported session JSON and evaluates its restore
  plan.
- `action_target` parses one external action-target definition.
- `hints` assigns labels to bounded candidate sets, including the hard cap.
- `bindings` exercises the default binding trie and resolver transitions.
- `userscript_results` validates bounded, manifest-gated script output.
- `state_sequences` drives bounded random reducer events and checks ownership
  invariants after every event.

Run a target with cargo-fuzz, for example:

```text
cargo fuzz run command_parser fuzz/corpus/command_parser
cargo fuzz run ipc_frames fuzz/corpus/ipc_frames
```

The targets intentionally feed arbitrary bytes to bounded public APIs. Invalid
UTF-8 is retained as a valid input class for framing and is skipped only by
text-only parsers.

The optional `interaction_proof` target uses the production engine annotation
decoder and requires the qualified Qt 6.11.2 SDK/libraries. It is enabled only
by the `interaction-proof` fuzz feature, so the default13-target smoke retains
its Qt-free dependency graph. The engine's `fuzzing` feature exposes only this
test driver; normal application builds omit it. Fixed browser requests and
ordinary candidates are captured independently of fuzzed annotations. Seeds
cover top, collection, focused-frame, shadow/slot and unavailable outcomes,
plus duplicate fields, stale revisions, forbidden commands and bad geometry.
These are structural facts and never live DOM/broker or action authority.

Run the bounded optional smoke in the existing qualified SDK environment:

```text
cargo xtask test fuzz-interaction-proof
```

It builds the optional release target and performs128runs, with a maximum input
length of2097153bytes to permit one byte beyond the annotation cap. This stable
libFuzzer corpus smoke does not claim coverage-guided campaign qualification.
The separate fuzz lock retains exact root-lock identities/checksums for shared
production packages; enabling the feature introduces no new production version
resolutions. Engine property tests also exercise incomplete prefixes, retained
snapshots, shared-budget exhaustion and the real prepared configuration
publication service (including test-only injected compilation panic recovery).

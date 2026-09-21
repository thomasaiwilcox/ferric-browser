# RustBrowser fuzz corpus

These seeds exercise the parser and validation boundaries described by
TEST-006. Keep minimized, non-sensitive reproductions here; never add real
credentials, cookies, account data, or private URLs.

The targets cover public parser, validation, routing, durable-session, and
hint-label boundaries that are safe to exercise without starting Qt or a
display server:

- `command_parser` parses interactive, CLI, and IPC command chains.
- `ipc_frames` exercises framed requests and response serialization.
- `url_and_clean_link` validates URLs and applies built-in link cleaning.
- `switcher_matching` tokenizes and ranks bounded switcher fields.
- `site_rule` parses and validates one site rule.
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

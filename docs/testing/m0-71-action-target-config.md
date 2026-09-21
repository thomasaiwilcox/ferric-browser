# M0-71 — Typed external action target configuration

## Scope

This slice establishes the validated configuration boundary for
`action_targets.<name>`. Each target has a conservative slug, one or more
implemented subject types (`url`, `link`, `selection`, or `tab`), a fixed
executable, a bounded direct argv vector,
and explicit `detach`/`allow_private` flags. The configuration is data-only;
it does not perform process execution or shell expansion while loading.

## Implemented behavior

- `ferric-browser-config::Config` deserializes `action_targets` with unknown fields
  rejected and a maximum of 64 entries.
- Names, subject types, executable values, argv count/size, duplicate subject
  types, and control characters are validated before a configuration becomes
  current.
- Only complete `{url}`, `{title}`, and `{selection}` placeholders are
  accepted; placeholders must be supported by the declared subject set.
- The Qt action registry exposes configured URL-, link-, selection-, and
  tab-capable targets
  through `actions.query` and native `action-list` discovery without exposing
  the executable or raw argv. Each dynamic descriptor includes the standard
  bounded `availability` envelope and marks subject revalidation as required;
  link and selection availability is computed independently, and targets that
  disallow private mode are reported unavailable while a private or ephemeral
  profile is active. Selection descriptors use the matching
  `external.<name>.selection.send` identity and example.
- `browser.url.send`, `browser.link.send`, `browser.selection.send`, and
  `browser.tab.send` map the shared action/command surfaces to the typed `send`
  executor. Execution checks target existence, declared subject support,
  HTTP(S) URL policy for URL/link/tab subjects, transient-profile policy, and
  uses a direct argv spawn. Selection sends use the existing browser-owned,
  target-validated extraction boundary. `detach = true` drops the child
  lifetime after a successful spawn; attached targets reuse the bounded browser
  operation state.
- Link hints can select a configured target with
  `hint --target external:<name> links`; the target name is a bounded slug,
  rapid mode is rejected, and execution reuses the same target-aware send
  policy. Missing or disallowed targets fail closed after fresh hint
  validation.

## Qualification

Automated evidence:

```text
cargo test -p ferric-browser-config -p ferric-browser-core -p ferric-browser-engine-qt --locked --offline
cargo fmt --all -- --check
```

The tests cover valid typed target data, duplicate/unsupported subjects,
non-complete placeholders, action parsing, typed IPC mapping, and the
external hint target grammar. The execution path enforces private-mode
gating. A live external-tool fixture, switcher activation parity, richer V1
subject coverage, confirmation parity, and process-fault qualification
remain.

# M1-08 configuration scope evidence

Date: 2026-09-16  
Status: in-progress

## Task card

- Requirement: CONFIG-007.
- Files: `crates/ferric-browser-config/src/lib.rs` and
  `crates/ferric-browser-engine-qt/src/lib.rs`.
- Observable result: the validated site-rule key set is centralized, and
  `config.get --url` rejects a profile/global-only setting instead of making
  it appear per-site.

## Evidence

- Per-site rules are limited to `input.entry_mode`, the supported content
  settings, and `content.zoom`; the same capability predicate is used by
  schema validation and the Qt `config.get` response.
- `config.get` now reports `supported_scopes` and a `site_scope` capability
  object, including the reason profile/global values cannot be changed on tab
  focus without leaking behavior across tabs.
- A URL-qualified query for an unsupported key is rejected after the key is
  resolved. Global proxy is not represented as a per-profile setting and is
  never inferred from the site-rule mechanism.
- The settings manager continues to edit the active global/profile runtime
  layer; site-scoped values remain explicit validated site rules.

## Limitations

Generated interactive site-rule editing is now available through
`set --pattern PATTERN KEY=VALUE` and `unset --pattern PATTERN KEY`. It uses
the atomic runtime override document, rejects unsupported site keys, and gives
generated rules highest precedence while keeping authored configuration files
unchanged. Typed default-profile definitions are now loaded from
`profiles.toml`; the profile manager can open profile-aware secondary windows,
while the complete V1 setting registry metadata, global proxy qualification,
and richer profile/context management remain future slices.

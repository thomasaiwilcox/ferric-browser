# M0-137 site-pattern semantics

Site rules now use a parsed, bounded CONFIG-006 matcher rather than matching
the complete URL as an undifferentiated string. Supported patterns have:

- `http`, `https`, or `*` schemes;
- exact hosts, `*`, or leading `*.` label wildcards (`*.example.test` also
  matches `example.test`, but never `badexample.test`);
- optional exact ports, including bracketed IPv6 authorities; and
- path globs matched after query strings and fragments are removed.

Host matching is case-insensitive and omitted ports normalize to the scheme's
default. Invalid schemes, userinfo, ambiguous IPv6 syntax, malformed ports,
and unsafe wildcard placement are rejected for both authored and generated
runtime rules.

Verification:

```text
cargo fmt --all -- --check
cargo test -p ferric-browser-config --locked --offline
cargo xtask check --locked
```

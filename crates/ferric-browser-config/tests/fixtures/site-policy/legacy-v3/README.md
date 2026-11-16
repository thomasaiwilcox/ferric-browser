# Pre-Interaction v3 serialization goldens

`config.toml`, `defaults.toml` and `runtime.toml` were generated from the original
configuration crate at `f2c5d6deebd97eb861084503f25613a6a819949d`, using its
unchanged Cargo.lock, Rust 1.98.0, `--locked --offline`, and TOML 0.9.12.
`input.toml` is the authored input. These are fresh regression fixtures from
that baseline, not rewritten historical qualification receipts.

The isolated baseline source and generator are retained under
`target/site-policy-baseline/legacy-source/`; exact invocation and output are
in `target/site-policy-baseline/legacy-golden-build.log`. The generator writes:

- `toml::to_string(&toml::from_str::<Config>(input))` after validation;
- `toml::to_string(&Config::default())`;
- `toml::to_string(&RuntimeOverrides::default())` after setting global
  `hints.min_chars=2` and site `content.zoom=1.25` at `https://example.test/*`.

Do not regenerate expected bytes with the new schema merely to make a failure
pass. An intentional serialization change needs an explicit compatibility review.

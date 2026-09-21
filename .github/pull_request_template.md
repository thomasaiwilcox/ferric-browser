## Summary

## Behavior and compatibility

- [ ] User-visible changes are documented.
- [ ] New state mutation enters through the reducer.
- [ ] New commands, actions, bindings, or settings use their typed registry.
- [ ] Presentation changes use typed models or document an approved diagnostic JSON exception.

## Verification

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --locked -- -D warnings`
- [ ] `cargo test --workspace --locked`
- [ ] `cargo xtask check`

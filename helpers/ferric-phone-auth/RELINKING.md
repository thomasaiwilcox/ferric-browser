# Rebuilding the phone-auth helper

The installed `phone-auth-source.tar.gz` contains this helper workspace, its
locked dependency graph, the pinned libwebauthn source and Ferric's documented
transport patch. Original LGPL licensing and notices are under `vendor/`.
Ferric's helper code is GPL-3.0-or-later, as declared in Cargo.toml; the complete
project license is installed alongside the library notices.

To rebuild, extract the archive into a directory you control. Install Rust
1.88.0 or a compatible newer compiler, a C toolchain, pkg-config and D-Bus
development files. From the extracted `ferric-phone-auth` directory:

```sh
cargo +1.88.0 build --release --locked
```

The output is `target/release/ferric-phone-auth`. To modify or relink
libwebauthn, edit `vendor/libwebauthn` and repeat that command. The local Cargo
patch selects that source. No signing key or service is needed to build or run
the helper. Dependency archives and exact Git revisions are recorded in
Cargo.lock. Network access is needed to obtain dependencies not already cached.

For a disposable local installation, place the rebuilt helper under
`PREFIX/bin/ferric-phone-auth-helper`, beside a Ferric browser executable, and
copy `data/public_suffix_list.dat` to
`PREFIX/share/ferric-browser/public_suffix_list.dat`. Do not overwrite a live
browser installation while testing. The parent locates its helper beside its
own executable, without searching PATH or accepting a page-supplied executable.

`tools/check_vendor.py` intentionally reports your library changes until its
patch manifest is refreshed; it is a provenance check, not a restriction on
rebuilding modified code. The production provider remains unavailable until
the browser's capability and policy requirements are met.

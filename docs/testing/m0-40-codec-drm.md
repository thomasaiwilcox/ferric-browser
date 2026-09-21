# M0-40 codec and DRM capability report

Date: 2026-09-16  
Status: in-progress

## Task card

- Requirement: MEDIA-004.
- Files: `packaging/media-capabilities.toml` and
  `crates/ferric-browser-engine-qt/src/diagnostics.rs`.
- Observable result: the diagnostics report includes a versioned media
  manifest with candidate codec MIME strings, package-input provenance, DRM
  entries, and an explicit local-playback qualification status.

The manifest deliberately keeps every codec and DRM entry `not-tested` until
the installed QtWebEngine package decodes a local fixture. FFmpeg, OpenH264,
or other package dependencies are recorded as inputs only; they do not become
support claims. Widevine remains separately qualified and no CDM is downloaded
or bundled by this project.

## Verification

```text
cargo fmt --all -- --check
cargo test -p ferric-browser-engine-qt --locked
cargo xtask check
ferric-browser diagnostics --format json
```

The manifest/report boundary is automated. Actual H.264/VP8/VP9/AV1/Opus
decode fixtures, package-specific hardware decode, and lawful DRM testing
remain manual qualification work.

Diagnostics tests also verify that the embedded manifest remains schema-versioned
and `not-tested`; package-input declarations never become codec or DRM support
claims without a separate playback qualification result.

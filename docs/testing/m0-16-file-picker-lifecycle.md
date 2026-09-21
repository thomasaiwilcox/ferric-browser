# File-picker lifecycle evidence

Date: 2026-09-16  
Status: in-progress

## Scope

Primary and secondary WebEngine file requests now use the public Qt Quick
`FileDialog` for open, multiple-open, and save requests, and `FolderDialog`
for directory-upload requests. Accepted results are converted to local paths
only; remote or empty selections reject the WebEngine request. MIME filters
remain bounded and are preserved where the browser has a known mapping.

Each window owns its pending WebEngine request and native dialog. Navigation,
view destruction, window teardown, and replacement by another dialog reject the
request and clear the native dialog before any late result can be delivered to
a new document. This keeps the dialog lifecycle separate from website
permission and capture prompts, including popup requests adopted from an
opener.

## Verification

```text
cargo build -p ferric-browser --locked --offline
cargo fmt --all -- --check
cargo xtask check
QT_QPA_PLATFORM=wayland timeout 8s target/debug/ferric-browser --temp-basedir
```

The native Wayland smoke stayed alive for the timeout without QML loading
errors. Interactive upload, Unicode-folder, cancellation, and portal-backend
qualification remain live desktop evidence work.

# M0-160 UI-thread ownership evidence

Status: in progress; deterministic workspace checks pass, native thread-affinity
qualification remains pending.

The Qt adapter keeps `BrowserUi` and its QML-facing properties as the GUI-owned
state boundary. QML `Timer` callbacks call the adapter's polling methods on the
Qt event loop; those methods only consume bounded worker/IPC result queues and
apply state to QObject properties. SQLite access, config file watching, IPC
acceptance, userscript/process execution, and other potentially blocking work
are performed by dedicated or bounded worker threads. The adapter has no
`block_on` path.

The crash/FFI boundary is complementary: both workspace profiles use
`panic = "abort"`, so a Rust failure cannot unwind through a generated Qt
`noexcept` entry point.

## Verification

```text
cargo xtask check --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
```

`cargo xtask check` now rejects blocking executor markers in the Qt adapter and
QML sources. A disposable native Wayland run
is still required to qualify actual Qt thread affinity, QObject destruction,
and worker shutdown ordering on the supported runtime.

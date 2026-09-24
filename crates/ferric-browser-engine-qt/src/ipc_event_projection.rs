//! Qt transport policy for IPC methods.
//!
//! Public event names and payloads are projected by ferric-browser-runtime at
//! the reducer boundary. This adapter module retains only transport admission
//! policy that depends on the local IPC server lifecycle.

#[must_use]
pub(super) fn is_mutating_ipc_method(method: &str) -> bool {
    matches!(
        method,
        "command.execute" | "action.execute" | "switcher.activate" | "window.focus"
    )
}

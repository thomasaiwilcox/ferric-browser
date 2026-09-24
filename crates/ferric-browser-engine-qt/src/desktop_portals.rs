//! Typed presentation of desktop-portal probe results.

use crate::{SingleFlightWorker, SubmitError, WorkerPoll, diagnostics};
use ferric_browser_config::PortalMode;
use serde_json::{Value, json};

pub(super) struct PortalProbeWorker {
    inner: SingleFlightWorker<(), Value>,
}

impl PortalProbeWorker {
    pub(super) fn spawn() -> Result<Self, String> {
        let inner = SingleFlightWorker::spawn("ferric-browser-portal-probe", |()| {
            diagnostics::portal_probe()
        })
        .map_err(|error| error.to_string())?;
        Ok(Self { inner })
    }

    pub(super) fn request(&mut self) -> Result<(), String> {
        match self.inner.submit(()) {
            Ok(()) | Err(SubmitError::Busy) => Ok(()),
            Err(SubmitError::QueueFull) => {
                Err("portal probe queue unavailable: sending on a full channel".into())
            }
            Err(SubmitError::Stopped) => Err("portal probe stopped".into()),
        }
    }

    pub(super) fn poll(&mut self) -> Option<Value> {
        match self.inner.poll() {
            WorkerPoll::Pending => None,
            WorkerPoll::Ready(result) => Some(result),
            WorkerPoll::Stopped => Some(json!({
                "service": {
                    "status": "unavailable",
                    "value": "org.freedesktop.portal.Desktop",
                    "reason": "portal probe worker stopped",
                    "provenance": "runtime-worker"
                }
            })),
        }
    }

    pub(super) fn is_pending(&self) -> bool {
        self.inner.is_pending()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PortalCapabilities {
    service: &'static str,
    file_chooser: &'static str,
    screen_cast: &'static str,
    open_uri: &'static str,
    notifications: &'static str,
}

impl PortalCapabilities {
    pub(super) fn pending() -> Self {
        Self::with_service("pending")
    }

    pub(super) fn unavailable() -> Self {
        Self::with_service("unavailable")
    }

    pub(super) fn capability_status(self, capability: &str) -> &'static str {
        if self.service != "available" {
            return self.service;
        }
        match capability {
            "file_chooser" => self.file_chooser,
            "screen_cast" => self.screen_cast,
            "open_uri" => self.open_uri,
            "notifications" => self.notifications,
            _ => "unavailable",
        }
    }

    fn with_service(service: &'static str) -> Self {
        Self {
            service,
            file_chooser: service,
            screen_cast: service,
            open_uri: service,
            notifications: service,
        }
    }
}

pub(super) fn project(value: &Value) -> PortalCapabilities {
    let service = status_at(value.get("service"));
    if service != "available" {
        return PortalCapabilities::with_service(service);
    }
    let interfaces = value.get("interfaces");
    PortalCapabilities {
        service,
        file_chooser: status_at(interfaces.and_then(|value| value.get("file_chooser"))),
        screen_cast: status_at(interfaces.and_then(|value| value.get("screen_cast"))),
        open_uri: status_at(interfaces.and_then(|value| value.get("open_uri"))),
        notifications: status_at(interfaces.and_then(|value| value.get("notifications"))),
    }
}

pub(super) fn mode_name(mode: &PortalMode) -> &'static str {
    match mode {
        PortalMode::Auto => "auto",
        PortalMode::Required => "required",
    }
}

fn status_at(value: Option<&Value>) -> &'static str {
    match value
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
    {
        Some("available") => "available",
        Some("unavailable") => "unavailable",
        Some("pending") => "pending",
        Some("not-probed") => "not-probed",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::{PortalCapabilities, mode_name, project};
    use ferric_browser_config::PortalMode;
    use serde_json::json;

    #[test]
    fn exposes_only_known_capabilities_after_the_service_is_available() {
        let capabilities = project(&json!({
            "service": {"status": "available"},
            "interfaces": {
                "file_chooser": {"status": "available"},
                "screen_cast": {"status": "unavailable"},
                "open_uri": {"status": "other"},
                "notifications": {"status": "available"}
            }
        }));
        assert_eq!(capabilities.capability_status("file_chooser"), "available");
        assert_eq!(capabilities.capability_status("screen_cast"), "unavailable");
        assert_eq!(capabilities.capability_status("open_uri"), "unknown");
        assert_eq!(capabilities.capability_status("invalid"), "unavailable");

        assert_eq!(
            PortalCapabilities::pending().capability_status("file_chooser"),
            "pending"
        );
        assert_eq!(mode_name(&PortalMode::Required), "required");
    }
}

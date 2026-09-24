//! Typed presentation values for bounded desktop-preference probe results.
//!
//! Probe workers return diagnostic JSON because that is useful in diagnostics,
//! but presentation does not need to decode that envelope. These projections
//! deliberately expose only the values that chrome can render.

use crate::{SingleFlightWorker, SubmitError, WorkerPoll, diagnostics};
use serde_json::{Value, json};

pub(super) struct ReducedMotionProbeWorker {
    inner: SingleFlightWorker<(), Value>,
    requested: bool,
}

impl ReducedMotionProbeWorker {
    pub(super) fn spawn() -> Result<Self, String> {
        let inner = SingleFlightWorker::spawn("ferric-browser-reduced-motion-probe", |()| {
            diagnostics::reduced_motion_probe()
        })
        .map_err(|error| error.to_string())?;
        Ok(Self {
            inner,
            requested: false,
        })
    }

    pub(super) fn request_once(&mut self) -> Result<(), String> {
        if self.requested {
            return Ok(());
        }
        match self.inner.submit(()) {
            Ok(()) | Err(SubmitError::Busy) => {
                self.requested = true;
                Ok(())
            }
            Err(SubmitError::QueueFull) => {
                Err("reduced-motion probe queue unavailable: sending on a full channel".into())
            }
            Err(SubmitError::Stopped) => Err("reduced-motion probe stopped".into()),
        }
    }

    pub(super) fn poll(&mut self) -> Option<Value> {
        match self.inner.poll() {
            WorkerPoll::Pending => None,
            WorkerPoll::Ready(result) => Some(result),
            WorkerPoll::Stopped => Some(json!({
                "status": "unavailable",
                "value": null,
                "reason": "reduced-motion probe worker stopped",
                "provenance": "runtime-worker"
            })),
        }
    }

    pub(super) fn is_pending(&self) -> bool {
        self.inner.is_pending()
    }
}

pub(super) struct SystemFontScaleProbeWorker {
    inner: SingleFlightWorker<(), Value>,
    requested: bool,
}

impl SystemFontScaleProbeWorker {
    pub(super) fn spawn() -> Result<Self, String> {
        let inner = SingleFlightWorker::spawn("ferric-browser-font-scale-probe", |()| {
            diagnostics::system_font_scale_probe()
        })
        .map_err(|error| error.to_string())?;
        Ok(Self {
            inner,
            requested: false,
        })
    }

    pub(super) fn request_once(&mut self) -> Result<(), String> {
        if self.requested {
            return Ok(());
        }
        match self.inner.submit(()) {
            Ok(()) | Err(SubmitError::Busy) => {
                self.requested = true;
                Ok(())
            }
            Err(SubmitError::QueueFull) => Err("font-scale probe queue unavailable".into()),
            Err(SubmitError::Stopped) => Err("font-scale probe stopped".into()),
        }
    }

    pub(super) fn poll(&mut self) -> Option<Value> {
        match self.inner.poll() {
            WorkerPoll::Pending => None,
            WorkerPoll::Ready(result) => Some(result),
            WorkerPoll::Stopped => Some(json!({
                "status": "unavailable",
                "value": null,
                "reason": "font-scale probe worker stopped",
                "provenance": "runtime-worker"
            })),
        }
    }

    pub(super) fn is_pending(&self) -> bool {
        self.inner.is_pending()
    }
}

pub(super) struct ReducedMotionPresentation {
    pub(super) status: &'static str,
    pub(super) enabled: bool,
}

pub(super) struct FontScalePresentation {
    pub(super) status: &'static str,
    pub(super) scale: f64,
}

pub(super) fn reduced_motion(value: &Value) -> ReducedMotionPresentation {
    let status = known_status(value);
    ReducedMotionPresentation {
        status,
        enabled: status == "available"
            && value.get("value").and_then(Value::as_bool).unwrap_or(false),
    }
}

pub(super) fn font_scale(value: &Value) -> FontScalePresentation {
    let status = known_status(value);
    let scale = value
        .get("value")
        .and_then(Value::as_f64)
        .filter(|scale| (0.5..=3.0).contains(scale))
        .filter(|_| status == "available")
        .unwrap_or(1.0);
    FontScalePresentation { status, scale }
}

fn known_status(value: &Value) -> &'static str {
    match value.get("status").and_then(Value::as_str) {
        Some("available") => "available",
        Some("unavailable") => "unavailable",
        Some("pending") => "pending",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::{font_scale, reduced_motion};
    use serde_json::json;

    #[test]
    fn projections_keep_only_known_statuses_and_valid_values() {
        let motion = reduced_motion(&json!({"status":"available", "value":true}));
        assert_eq!(motion.status, "available");
        assert!(motion.enabled);

        let font = font_scale(&json!({"status":"available", "value":1.25}));
        assert_eq!(font.status, "available");
        assert_eq!(font.scale, 1.25);

        let invalid = font_scale(&json!({"status":"other", "value":999.0}));
        assert_eq!(invalid.status, "unknown");
        assert_eq!(invalid.scale, 1.0);
    }
}

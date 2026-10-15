use super::{
    CxxQtType, Pin, QString, QStringList, Value, diagnostics, diagnostics_presentation, qobject,
};

impl qobject::BrowserUi {
    // The QObject receiver is required by Qt's scalar invokable contract.
    #[allow(clippy::unused_self)]
    pub(super) fn engine_update_notice(&self) -> QStringList {
        let runtime = serde_json::json!({"value": super::qt_webengine_version()});
        let policy = diagnostics::engine_update_policy(&runtime);
        let (status, notice) =
            diagnostics_presentation::engine_notice(policy["status"].as_str().unwrap_or_default());
        [status, notice].into_iter().map(QString::from).collect()
    }

    pub(super) fn diagnostics_json(mut self: Pin<&mut Self>) -> QString {
        let result = self
            .as_ref()
            .ipc_diagnostics(&Value::Null)
            .and_then(|value| {
                let serialized = serde_json::to_string_pretty(&value)
                    .map_err(|_| "The report couldn't be formatted".to_owned())?;
                Ok((value, serialized))
            });
        let available = result.is_ok();
        let summary = result.as_ref().map_or_else(
            |_| vec!["The report couldn't be prepared. Refresh to try again.".to_owned()],
            |(value, _)| {
                diagnostics_presentation::summary(
                    value,
                    self.as_ref().active_profile_is_transient(),
                )
            },
        );
        let serialized = match result {
            Ok((_, serialized)) => serialized,
            Err(error) => serde_json::json!({"error": error}).to_string(),
        };
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.diagnostics_preview_payload = available.then(|| serialized.clone());
            this.diagnostics_preview_ready = available;
        }
        self.as_mut()
            .set_diagnostics_summary(summary.iter().map(QString::from).collect());
        self.as_mut().set_diagnostics_available(available);
        QString::from(super::presentation_text::report_preview(
            &serialized,
            48 * 1024,
        ))
    }
}

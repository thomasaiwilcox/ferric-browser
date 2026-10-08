//! Apply one validated versioned policy update to the Qt-facing projections.

use super::{Pin, QString, Value, publish_ipc_event, qobject};
use cxx_qt::CxxQtType;

impl qobject::BrowserUi {
    /// Synchronizes the one explicit versioned policy change after the
    /// application has validated and committed it. It updates only the
    /// in-memory temporary override layer; authored configuration files and
    /// permission/trust stores are untouched.
    pub(super) fn accept_versioned_policy_configuration(
        mut self: Pin<&mut Self>,
        snapshot: ferric_browser_application::ConfigurationSnapshot,
    ) -> Result<(), String> {
        let javascript = snapshot.effective.content.javascript;
        let active_config = serde_json::to_value(&snapshot.effective)
            .map_err(|_| "validated policy configuration could not be serialized".to_owned())?;
        let config_json = serde_json::to_string(&active_config)
            .map_err(|_| "validated policy configuration could not be encoded".to_owned())?;
        let mut temporary = self.as_ref().rust().temporary_overrides.clone();
        temporary
            .set_setting("content.javascript", toml::Value::Boolean(javascript))
            .map_err(|_| "validated policy override could not be synchronized".to_owned())?;
        let pending_summary = snapshot
            .pending_changes
            .iter()
            .map(|change| {
                serde_json::json!({
                    "key": change.key,
                    "apply_time": change.apply_time,
                    "current": change.current,
                    "pending": change.pending,
                })
            })
            .collect::<Vec<_>>();
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.config = active_config.clone();
            this.temporary_overrides = temporary;
            this.learning_mode = active_config
                .get("discovery")
                .and_then(Value::as_object)
                .and_then(|discovery| discovery.get("learning_mode"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
        }
        self.as_mut().update_chrome_preferences(&snapshot.effective);
        self.as_mut()
            .update_feature_preferences(&snapshot.effective);
        self.as_mut().update_hint_preferences(&snapshot.effective);
        self.as_mut().update_settings_presentation(&active_config);
        self.as_mut().update_theme_palette(&active_config);
        self.as_mut().set_config_json(QString::from(config_json));
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        publish_ipc_event(
            &mut this.ipc_sequence,
            "config.changed",
            serde_json::json!({
                "revision": snapshot.revision,
                "pending": pending_summary,
                "source": "versioned-allowlisted-policy",
            }),
        );
        Ok(())
    }
}

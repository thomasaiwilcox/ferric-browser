//! Policy for browser-owned asynchronous operation records.
//!
//! The Qt bridge reports operation state and audit events but does not decide
//! their public classification or accept untrusted operation identifiers.

use crate::input_validation::is_bounded_untrusted_text;
use serde_json::{Value, json};
use uuid::Uuid;

#[must_use]
pub(super) fn action_operation_id(result: Option<&Value>, prefix: &str) -> String {
    result
        .and_then(|value| value.get("operation_id"))
        .and_then(Value::as_str)
        .filter(|value| is_bounded_untrusted_text(value))
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{prefix}-{}", Uuid::new_v4()))
}

#[must_use]
pub(super) fn action_audit_record(
    action_id: &str,
    operation_id: &str,
    outcome: &str,
    category: Option<&str>,
) -> Value {
    json!({
        "action_id": action_id,
        "operation_id": operation_id,
        "outcome": outcome,
        "category": category,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_ids_accept_only_bounded_untrusted_values() {
        let known = json!({"operation_id": "operation-7"});
        assert_eq!(
            action_operation_id(Some(&known), "ui-action"),
            "operation-7"
        );

        let invalid = json!({"operation_id": concat!("operation", "\0")});
        let generated = action_operation_id(Some(&invalid), "ui-action");
        assert!(generated.starts_with("ui-action-"));
    }

    #[test]
    fn audit_record_has_only_the_public_ledger_fields() {
        let record = action_audit_record("browser.tab.reload", "op-7", "failed", Some("stale"));
        assert_eq!(record.as_object().expect("record").len(), 4);
        assert_eq!(record["action_id"], "browser.tab.reload");
        assert_eq!(record["operation_id"], "op-7");
        assert_eq!(record["outcome"], "failed");
        assert_eq!(record["category"], "stale");
    }
}

//! Fixed-width native blocking-evidence rows for the Rust IPC boundary.
//!
//! The request interceptor owns the raw `QVariantMap` values produced while
//! inspecting `WebEngine` requests. It projects those maps into these bounded
//! rows before QML forwards them to the Rust bridge. This avoids using QML as
//! a JSON serializer for browser policy data.

use super::{CxxQtType, Pin, QString, QStringList, Value, qobject};

const EVIDENCE_FIELD_COUNT: usize = 13;
const MAX_DECISIONS: usize = 100;
const MAX_FIELD_BYTES: usize = 1024;
const MAX_LIVE_COUNT: f64 = 1_000_000_000.0;

#[derive(Clone, Debug, Default)]
pub(super) struct BlockingEvidence {
    explanation: Option<BlockingDecision>,
    decisions: Vec<BlockingDecision>,
}

#[derive(Clone, Debug, Default)]
struct BlockingDecision {
    resource_host: String,
    first_party_host: String,
    initiator_host: String,
    resource_type: String,
    navigation_type: String,
    is_top_level: bool,
    is_subframe: bool,
    matched_rule: String,
    list_id: String,
    decision: String,
    reason: String,
    exception_rule: String,
    exception_list_id: String,
}

impl BlockingEvidence {
    fn from_rows(explanation: &QStringList, decisions: &QStringList) -> Option<Self> {
        let explanation_len = usize::try_from(explanation.len()).ok()?;
        let decisions_len = usize::try_from(decisions.len()).ok()?;
        let explanation = if explanation.is_empty() {
            None
        } else if explanation_len == EVIDENCE_FIELD_COUNT {
            Some(row_at(explanation, 0, false)?)
        } else {
            return None;
        };
        if decisions_len % EVIDENCE_FIELD_COUNT != 0 {
            return None;
        }
        let decision_count = decisions_len / EVIDENCE_FIELD_COUNT;
        if decision_count > MAX_DECISIONS {
            return None;
        }
        let decisions = (0..decision_count)
            .map(|index| row_at(decisions, index * EVIDENCE_FIELD_COUNT, true))
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            explanation,
            decisions,
        })
    }

    pub(super) fn json_values(&self) -> (Value, Value) {
        let explanation = self
            .explanation
            .as_ref()
            .map_or_else(|| serde_json::json!({}), BlockingDecision::json);
        let decisions = Value::Array(self.decisions.iter().map(BlockingDecision::json).collect());
        (explanation, decisions)
    }
}

impl BlockingDecision {
    fn json(&self) -> Value {
        serde_json::json!({
            "resource_host": self.resource_host,
            "first_party_host": self.first_party_host,
            "initiator_host": self.initiator_host,
            "resource_type": self.resource_type,
            "navigation_type": self.navigation_type,
            "is_top_level": self.is_top_level,
            "is_subframe": self.is_subframe,
            "matched_rule": self.matched_rule,
            "list_id": self.list_id,
            "decision": self.decision,
            "reason": self.reason,
            "exception_rule": self.exception_rule,
            "exception_list_id": self.exception_list_id,
        })
    }
}

fn row_at(values: &QStringList, offset: usize, require_decision: bool) -> Option<BlockingDecision> {
    if usize::try_from(values.len()).ok()? < offset + EVIDENCE_FIELD_COUNT {
        return None;
    }
    let fields = values
        .iter()
        .skip(offset)
        .take(EVIDENCE_FIELD_COUNT)
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if fields.iter().any(|field| !valid_field(field)) {
        return None;
    }
    let is_top_level = parse_bool(&fields[5])?;
    let is_subframe = parse_bool(&fields[6])?;
    if fields[0].is_empty() || fields[7].is_empty() || (require_decision && fields[9].is_empty()) {
        return None;
    }
    Some(BlockingDecision {
        resource_host: fields[0].clone(),
        first_party_host: fields[1].clone(),
        initiator_host: fields[2].clone(),
        resource_type: fields[3].clone(),
        navigation_type: fields[4].clone(),
        is_top_level,
        is_subframe,
        matched_rule: fields[7].clone(),
        list_id: fields[8].clone(),
        decision: fields[9].clone(),
        reason: fields[10].clone(),
        exception_rule: fields[11].clone(),
        exception_list_id: fields[12].clone(),
    })
}

fn valid_field(value: &str) -> bool {
    value.len() <= MAX_FIELD_BYTES && !value.chars().any(char::is_control)
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

impl qobject::BrowserUi {
    pub(super) fn set_blocking_active_evidence(
        mut self: Pin<&mut Self>,
        explanation: &QStringList,
        decisions: &QStringList,
    ) -> bool {
        let Some(evidence) = BlockingEvidence::from_rows(explanation, decisions) else {
            self.as_mut()
                .set_status_text(QString::from("Blocking evidence was invalid"));
            return false;
        };
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .blocking_active_evidence = evidence;
        true
    }

    pub(super) fn clear_blocking_active_evidence(mut self: Pin<&mut Self>) {
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .blocking_active_evidence = BlockingEvidence::default();
    }

    pub(super) fn publish_blocking_live_counts(
        mut self: Pin<&mut Self>,
        blocked: f64,
        unknown_context: f64,
        active_site: f64,
    ) -> bool {
        let [Some(blocked), Some(unknown_context), Some(active_site)] =
            [blocked, unknown_context, active_site].map(live_count)
        else {
            return false;
        };
        self.as_mut().set_blocking_blocked_count(blocked);
        self.as_mut()
            .set_blocking_unknown_context_count(unknown_context);
        self.as_mut().set_blocking_active_site_count(active_site);
        true
    }
}

fn live_count(value: f64) -> Option<i64> {
    if !value.is_finite() || value < 0.0 || value > MAX_LIVE_COUNT {
        return None;
    }
    Some(value.floor() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(decision: &str) -> QStringList {
        [
            "cdn.example",
            "example",
            "example",
            "script",
            "other",
            "false",
            "false",
            "example.com",
            "builtin",
            decision,
            "matched host blocking rule",
            "",
            "",
        ]
        .into_iter()
        .map(QString::from)
        .collect()
    }

    #[test]
    fn evidence_requires_complete_bounded_rows() {
        let explanation = QStringList::default();
        let decisions = row("blocked");
        let evidence = BlockingEvidence::from_rows(&explanation, &decisions).expect("valid rows");
        let (_, values) = evidence.json_values();
        assert_eq!(values[0]["resource_host"], "cdn.example");
        assert_eq!(values[0]["is_subframe"], false);
        assert!(BlockingEvidence::from_rows(&explanation, &QStringList::default()).is_some());
        assert!(
            BlockingEvidence::from_rows(
                &explanation,
                &decisions.iter().take(EVIDENCE_FIELD_COUNT - 1).collect()
            )
            .is_none()
        );
    }

    #[test]
    fn live_counts_reject_non_finite_negative_and_unbounded_values() {
        assert_eq!(live_count(4.9), Some(4));
        assert!(live_count(f64::NAN).is_none());
        assert!(live_count(-1.0).is_none());
        assert!(live_count(MAX_LIVE_COUNT + 1.0).is_none());
    }
}

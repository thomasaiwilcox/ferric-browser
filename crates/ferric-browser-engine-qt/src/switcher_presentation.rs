//! Typed switcher-result projection for the QML multi-window aggregator.
//!
//! IPC retains its JSON response envelope, while this module turns the bounded
//! result rows into parallel Qt lists at the presentation boundary. QML never
//! parses that envelope or infers missing fields.

use super::{Pin, QString, Value, qobject};

#[derive(Default)]
struct SwitcherPresentationRows {
    kinds: Vec<QString>,
    ids: Vec<QString>,
    generations: Vec<QString>,
    labels: Vec<QString>,
    secondaries: Vec<QString>,
    profiles: Vec<QString>,
    workspaces: Vec<QString>,
    actions: Vec<QString>,
    ranks: Vec<QString>,
    recencies: Vec<QString>,
}

impl qobject::BrowserUi {
    pub fn publish_switcher_rows(mut self: Pin<&mut Self>, result: &Value) -> Result<(), String> {
        let rows = SwitcherPresentationRows::from_result(result)?;
        self.as_mut()
            .set_switcher_result_kinds(rows.kinds.into_iter().collect());
        self.as_mut()
            .set_switcher_result_ids(rows.ids.into_iter().collect());
        self.as_mut()
            .set_switcher_result_generations(rows.generations.into_iter().collect());
        self.as_mut()
            .set_switcher_result_labels(rows.labels.into_iter().collect());
        self.as_mut()
            .set_switcher_result_secondaries(rows.secondaries.into_iter().collect());
        self.as_mut()
            .set_switcher_result_profiles(rows.profiles.into_iter().collect());
        self.as_mut()
            .set_switcher_result_workspaces(rows.workspaces.into_iter().collect());
        self.as_mut()
            .set_switcher_result_actions(rows.actions.into_iter().collect());
        self.as_mut()
            .set_switcher_result_ranks(rows.ranks.into_iter().collect());
        self.as_mut()
            .set_switcher_result_recencies(rows.recencies.into_iter().collect());
        Ok(())
    }
}

impl SwitcherPresentationRows {
    fn from_result(result: &Value) -> Result<Self, String> {
        let values = result
            .get("results")
            .and_then(Value::as_array)
            .ok_or_else(|| "switcher response has no result list".to_owned())?;
        let mut rows = Self::default();

        for value in values {
            let row = value
                .as_object()
                .ok_or_else(|| "switcher result is not an object".to_owned())?;
            rows.kinds
                .push(QString::from(required_string(row, "kind")?));
            rows.ids.push(QString::from(required_string(row, "id")?));
            rows.generations
                .push(QString::from(optional_number(row, "generation")?));
            rows.labels
                .push(QString::from(required_string(row, "label")?));
            rows.secondaries
                .push(QString::from(required_string(row, "secondary")?));
            rows.profiles
                .push(QString::from(optional_string(row, "profile")?));
            rows.workspaces
                .push(QString::from(optional_string(row, "workspace")?));
            rows.actions.push(QString::from(actions(row)?));
            rows.ranks
                .push(QString::from(required_number(row, "rank")?));
            rows.recencies
                .push(QString::from(required_number(row, "recency")?));
        }
        Ok(rows)
    }
}

fn required_string(row: &serde_json::Map<String, Value>, field: &str) -> Result<String, String> {
    row.get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("switcher result {field} must be a string"))
}

fn optional_string(row: &serde_json::Map<String, Value>, field: &str) -> Result<String, String> {
    match row.get(field) {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(value)) => Ok(value.clone()),
        Some(_) => Err(format!("switcher result {field} must be a string or null")),
    }
}

fn required_number(row: &serde_json::Map<String, Value>, field: &str) -> Result<String, String> {
    row.get(field)
        .and_then(Value::as_i64)
        .map(|value| value.to_string())
        .ok_or_else(|| format!("switcher result {field} must be an integer"))
}

fn optional_number(row: &serde_json::Map<String, Value>, field: &str) -> Result<String, String> {
    match row.get(field) {
        None | Some(Value::Null) => Ok(String::new()),
        Some(value) => value
            .as_u64()
            .map(|number| number.to_string())
            .ok_or_else(|| format!("switcher result {field} must be an unsigned integer or null")),
    }
}

fn actions(row: &serde_json::Map<String, Value>) -> Result<String, String> {
    let values = row
        .get("actions")
        .and_then(Value::as_array)
        .ok_or_else(|| "switcher result actions must be an array".to_owned())?;
    let mut actions = Vec::with_capacity(values.len());
    for value in values {
        let action = value
            .as_str()
            .ok_or_else(|| "switcher result action must be a string".to_owned())?;
        if action.contains('\t') {
            return Err("switcher result action contains an invalid separator".into());
        }
        actions.push(action);
    }
    Ok(actions.join("\t"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_requires_the_complete_typed_row_contract() {
        let rows = SwitcherPresentationRows::from_result(&serde_json::json!({
            "results": [{
                "kind": "tab", "id": "tab-1", "generation": 4,
                "label": "Example", "secondary": "https://example.test/",
                "profile": "default", "workspace": null,
                "actions": ["focus", "open"], "rank": 12, "recency": 7
            }]
        }))
        .expect("complete row");
        assert_eq!(rows.kinds.len(), 1);
        assert_eq!(rows.generations[0].to_string(), "4");
        assert_eq!(rows.actions[0].to_string(), "focus\topen");
        assert!(
            SwitcherPresentationRows::from_result(&serde_json::json!({
                "results": [{"kind": "tab"}]
            }))
            .is_err()
        );
    }
}

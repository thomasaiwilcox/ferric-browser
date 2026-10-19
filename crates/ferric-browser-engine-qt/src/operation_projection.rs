//! Bounded operation retention and IPC projection.

use crate::{
    BrowserUiRust,
    browser_ui_query_permissions::private_queries_enabled,
    ipc_params::{query_bool_param, query_object, query_optional_string},
};
use serde_json::{Value, json};

pub(super) fn remember_operation_stderr(rust: &mut BrowserUiRust, id: &str, stderr: String) {
    if let Some(application) = rust.state.as_mut() {
        application.operations_mut().remember_stderr(id, stderr);
    }
}

pub(super) fn operations_query_value(
    rust: &BrowserUiRust,
    params: &Value,
) -> Result<Value, String> {
    page_operations(operation_rows(rust, params)?, params, rust.ipc_sequence)
}

pub(super) fn operations_collection_value(
    rust: &BrowserUiRust,
    params: &Value,
) -> Result<Value, String> {
    Ok(json!({"operations": operation_rows(rust, params)?}))
}

fn operation_rows(rust: &BrowserUiRust, params: &Value) -> Result<Vec<Value>, String> {
    let object = query_object(
        params,
        "operations.query",
        &[
            "operation_id",
            "include_stderr",
            "include_private",
            "window",
            "limit",
            "cursor",
        ],
    )?;
    if let Some(window) = query_optional_string(object, "operations.query", "window")?
        && !matches!(window, "active" | "last-focused")
        && ferric_browser_core::WindowId::from_display(window)
            .is_none_or(|id| Some(id) != rust.window)
    {
        return Err("The requested browser window is no longer available.".into());
    }
    let requested = query_optional_string(object, "operations.query", "operation_id")?;
    if requested.is_some_and(str::is_empty) {
        return Err("operations.query operation_id must be nonempty".into());
    }
    let include_stderr = query_bool_param(object, "operations.query", "include_stderr", false)?;
    let include_private = query_bool_param(object, "operations.query", "include_private", false)?;
    if include_private && !private_queries_enabled(rust) {
        return Err("Private operation queries are disabled by configuration.".into());
    }
    let transient = rust.state.as_ref().is_none_or(|state| {
        rust.window
            .and_then(|window| state.windows().get(&window))
            .and_then(|window| state.profiles().get(&window.profile))
            .is_none_or(|profile| profile.privacy.is_transient())
    });
    let window = rust.window.map_or_else(String::new, |id| id.to_string());
    let rows = rust
        .operation_records()
        .filter(|_| !transient || include_private)
        .filter(|record| requested.is_none_or(|requested| requested == record.id))
        .map(|record| {
            let mut value = crate::browser_ui_operations::operation_value(record, include_stderr);
            value["window_id"] = Value::String(window.clone());
            value
        })
        .collect();
    Ok(rows)
}

pub(super) fn page_operations(
    mut rows: Vec<Value>,
    params: &Value,
    sequence: u64,
) -> Result<Value, String> {
    let limit = match params.get("limit") {
        None => 50,
        Some(value) => value
            .as_u64()
            .filter(|limit| (1..=100).contains(limit))
            .ok_or_else(|| "operations.query limit must be 1 to 100".to_owned())?
            as usize,
    };
    let cursor = params
        .get("cursor")
        .map(|value| {
            value
                .as_str()
                .filter(|cursor| cursor.len() <= 128)
                .ok_or_else(|| "invalid operation cursor".to_owned())
        })
        .transpose()?;
    let after = cursor
        .map(|cursor| {
            let (owner, sequence) = cursor
                .split_once('/')
                .ok_or_else(|| "invalid operation cursor".to_owned())?;
            let sequence = sequence
                .parse::<u64>()
                .map_err(|_| "invalid operation cursor".to_owned())?;
            Ok::<_, String>((owner.to_owned(), sequence))
        })
        .transpose()?;
    let key = |row: &Value| {
        (
            row["window_id"].as_str().unwrap_or_default().to_owned(),
            row["sequence"].as_u64().unwrap_or(0),
        )
    };
    rows.sort_by_key(&key);
    let mut values = Vec::new();
    let mut bytes = 0;
    let mut more = false;
    for row in rows {
        if after.as_ref().is_some_and(|after| key(&row) <= *after) {
            continue;
        }
        let row_bytes = serde_json::to_vec(&row)
            .map_err(|error| error.to_string())?
            .len();
        if values.len() == limit || bytes + row_bytes > 500 * 1024 {
            more = true;
            break;
        }
        bytes += row_bytes;
        values.push(row);
    }
    let next_cursor = if more {
        values.last().map(|row| {
            let (owner, sequence) = key(row);
            format!("{owner}/{sequence}")
        })
    } else {
        None
    };
    Ok(json!({"sequence": sequence, "operations": values, "next_cursor": next_cursor}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferric_browser_application::BrowserApplication;
    use ferric_browser_core::PrivacyKind;

    #[test]
    fn private_and_temporary_operation_details_require_configured_explicit_opt_in() {
        for privacy in [PrivacyKind::Private, PrivacyKind::Ephemeral] {
            let mut rust = BrowserUiRust::default();
            let (application, window, _) = BrowserApplication::bootstrap(
                ferric_browser_config::Config::default(),
                privacy,
                "Private fixture",
            )
            .expect("owner");
            rust.state = Some(application);
            rust.window = Some(window);
            rust.config = json!({"ipc":{"private_queries":false}});
            rust.admit_operation("private-operation".into()).unwrap();
            rust.set_operation(
                "private-operation".into(),
                crate::OperationUpdate::failed(crate::FailureKind::Engine, "private detail"),
            );
            remember_operation_stderr(
                &mut rust,
                "private-operation",
                "private stderr fixture".into(),
            );
            for params in [
                json!({}),
                json!({"include_stderr":true}),
                json!({"operation_id":"private-operation"}),
            ] {
                assert_eq!(
                    operations_query_value(&rust, &params).unwrap()["operations"],
                    json!([])
                );
            }
            assert!(operations_query_value(&rust, &json!({"include_private":true})).is_err());
            rust.config["ipc"]["private_queries"] = json!(true);
            assert_eq!(
                operations_query_value(&rust, &json!({})).unwrap()["operations"],
                json!([])
            );
            let visible = operations_query_value(&rust, &json!({"include_private":true})).unwrap();
            assert_eq!(
                visible["operations"][0]["operation_id"],
                "private-operation"
            );
            assert!(visible["operations"][0].get("stderr").is_none());
            let details = operations_query_value(
                &rust,
                &json!({"include_private":true,"include_stderr":true,"window":window.to_string()}),
            )
            .unwrap();
            assert_eq!(details["operations"][0]["stderr"], "private stderr fixture");
            assert!(
                operations_query_value(&rust, &json!({"window":"windowid-18446744073709551615"}))
                    .is_err()
            );
            assert!(operations_query_value(&rust, &json!({"include_private":"yes"})).is_err());
        }
    }
    #[test]
    fn pagination_respects_order_cursor_and_encoded_byte_budget() {
        let rows = (0..200).rev().map(|sequence| json!({"window_id":"windowid-1",
            "sequence":sequence, "operation_id":format!("op-{sequence}"), "detail":"x".repeat(4096)})).collect::<Vec<_>>();
        let first = page_operations(rows.clone(), &json!({"limit":100}), 9).unwrap();
        assert_eq!(first["operations"][0]["sequence"], 0);
        assert!(serde_json::to_vec(&first).unwrap().len() < 512 * 1024);
        let second =
            page_operations(rows, &json!({"limit":100,"cursor":first["next_cursor"]}), 9).unwrap();
        assert_eq!(second["operations"][0]["sequence"], 100);
        assert!(second["next_cursor"].is_null());
        assert!(page_operations(vec![], &json!({"limit":0}), 0).is_err());
        assert!(page_operations(vec![], &json!({"cursor":"bad"}), 0).is_err());
    }
}

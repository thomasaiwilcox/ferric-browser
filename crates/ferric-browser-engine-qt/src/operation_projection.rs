//! Bounded operation retention and IPC projection.

use crate::{
    BrowserUiRust,
    browser_ui_query_permissions::private_queries_enabled,
    ipc_params::{query_bool_param, query_object, query_optional_string},
};
use serde_json::{Value, json};

pub(super) fn remember_operation_stderr(
    rust: &mut BrowserUiRust,
    operation_id: &str,
    stderr: String,
) {
    const MAX_RETAINED_OPERATION_STDERR: usize = 256;
    if !rust.operation_stderr.contains_key(operation_id)
        && rust.operation_stderr.len() >= MAX_RETAINED_OPERATION_STDERR
        && let Some(oldest) = rust.operation_stderr.keys().next().cloned()
    {
        rust.operation_stderr.remove(&oldest);
    }
    rust.operation_stderr
        .insert(operation_id.to_owned(), stderr);
}

pub(super) fn operations_query_value(
    rust: &BrowserUiRust,
    params: &Value,
) -> Result<Value, String> {
    let object = query_object(
        params,
        "operations.query",
        &[
            "operation_id",
            "include_stderr",
            "include_private",
            "window",
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
    let operations = rust
        .operation_states
        .iter()
        .filter(|_| !transient || include_private)
        .filter(|(id, _)| requested.is_none_or(|requested| requested == id.as_str()))
        .map(|(id, status)| {
            let mut operation = json!({"operation_id": id, "status": status});
            if include_stderr {
                operation["stderr"] = rust
                    .operation_stderr
                    .get(id)
                    .cloned()
                    .map_or(Value::Null, Value::String);
            }
            operation
        })
        .collect::<Vec<_>>();
    Ok(json!({"sequence": rust.ipc_sequence, "operations": operations}))
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
            rust.operation_states
                .insert("private-operation".into(), "failed (private detail)".into());
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
}

//! Bounded operation retention and IPC projection.

use crate::{
    BrowserUiRust,
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
        &["operation_id", "include_stderr"],
    )?;
    let requested = query_optional_string(object, "operations.query", "operation_id")?;
    if requested.is_some_and(str::is_empty) {
        return Err("operations.query operation_id must be nonempty".into());
    }
    let include_stderr = query_bool_param(object, "operations.query", "include_stderr", false)?;
    let operations = rust
        .operation_states
        .iter()
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

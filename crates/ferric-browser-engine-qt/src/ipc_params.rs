//! Shared validation for the JSON-RPC parameter envelope.
//!
//! The Qt adapter uses these helpers only to validate the transport shape;
//! command decoding and browser policy remain in their dedicated boundaries.

use serde_json::{Map, Value};

pub(super) fn query_object<'a>(
    params: &'a Value,
    method: &str,
    allowed: &[&str],
) -> Result<&'a Map<String, Value>, String> {
    let object = params
        .as_object()
        .ok_or_else(|| format!("{method} params must be an object"))?;
    if object
        .keys()
        .any(|key| !allowed.iter().any(|allowed| *allowed == key))
    {
        return Err(format!("{method} contains an unknown field"));
    }
    Ok(object)
}

pub(super) fn query_bool_param(
    object: &Map<String, Value>,
    method: &str,
    name: &str,
    default: bool,
) -> Result<bool, String> {
    object
        .get(name)
        .map(|value| {
            value
                .as_bool()
                .ok_or_else(|| format!("{method} {name} must be a boolean"))
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}

pub(super) fn query_optional_string<'a>(
    object: &'a Map<String, Value>,
    method: &str,
    name: &str,
) -> Result<Option<&'a str>, String> {
    object
        .get(name)
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| format!("{method} {name} must be a string"))
        })
        .transpose()
}

pub(super) fn query_limit(
    object: &Map<String, Value>,
    method: &str,
    default: usize,
) -> Result<usize, String> {
    object
        .get("limit")
        .map(|value| {
            value
                .as_u64()
                .filter(|limit| (1..=1_000).contains(limit))
                .map(|limit| limit as usize)
                .ok_or_else(|| format!("{method} limit must be an integer from 1 to 1000"))
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}

pub(super) fn query_offset(object: &Map<String, Value>, method: &str) -> Result<usize, String> {
    object
        .get("offset")
        .map(|value| {
            value
                .as_u64()
                .filter(|offset| *offset <= 100_000)
                .map(|offset| offset as usize)
                .ok_or_else(|| format!("{method} offset must be an integer from 0 to 100000"))
        })
        .transpose()
        .map(|value| value.unwrap_or(0))
}

pub(super) fn query_empty_object_or_null(params: &Value, method: &str) -> Result<(), String> {
    if params.is_null() {
        return Ok(());
    }
    let object = query_object(params, method, &[])?;
    if !object.is_empty() {
        return Err(format!("{method} does not accept parameters"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_fields_and_invalid_types() {
        let parameters = serde_json::json!({"include_private": "yes"});
        let object =
            query_object(&parameters, "tabs.query", &["include_private"]).expect("known field");
        assert!(query_bool_param(object, "tabs.query", "include_private", false).is_err());
        assert!(query_object(&serde_json::json!({"unexpected": true}), "tabs.query", &[]).is_err());
    }

    #[test]
    fn bounds_pagination_parameters() {
        let parameters = serde_json::json!({"limit": 1001, "offset": 100_001});
        let object = query_object(&parameters, "switcher.query", &["limit", "offset"])
            .expect("known fields");
        assert!(query_limit(object, "switcher.query", 50).is_err());
        assert!(query_offset(object, "switcher.query").is_err());
    }
}

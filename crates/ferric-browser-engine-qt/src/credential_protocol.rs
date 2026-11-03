//! Bounded codec for the private version-one helper stream.
//!
//! This module deliberately has no browser or ceremony authority. A decoded
//! frame is data for the private transport only; it cannot advance an actor.

use serde::{Deserialize, Deserializer, Serialize, de};
use std::io::{self, Write};

pub(crate) const MAX_TERMINAL_FRAME: usize = 1024 * 1024;
pub(crate) const MAX_PROGRESS_FRAME: usize = 32 * 1024;
pub(crate) const MAX_QUEUED_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const MAX_QUEUED_FRAMES: usize = 128;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ProtocolError {
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "request encoder is retained for the later P05b2 caller"
        )
    )]
    InvalidRequest,
    InvalidFrame,
    TooLarge,
    QueueFull,
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::InvalidRequest => "invalid request",
            Self::InvalidFrame => "invalid frame",
            Self::TooLarge => "frame exceeds its limit",
            Self::QueueFull => "transport queue is full",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ProtocolError {}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "request encoder is retained for the later P05b2 caller"
    )
)]
pub(crate) struct HelperRequest<'a> {
    pub v: u8,
    pub id: &'a str,
    pub op: Operation,
    pub origin: &'a str,
    pub options: &'a serde_json::Value,
    pub network: NetworkPolicy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Operation {
    Create,
    Get,
}

#[derive(Default, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "request encoder is retained for the later P05b2 caller"
    )
)]
pub(crate) struct NetworkPolicy {
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<&'static str>,
}

impl NetworkPolicy {
    // Only the private actor, after native direct-route admission, uses this.
    pub(crate) fn direct() -> Self {
        Self {
            mode: Some("direct"),
        }
    }
}

/// Strict JSON value that rejects duplicate object members at every depth.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub(crate) enum Json {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<Json>),
    Object(std::collections::BTreeMap<String, Json>),
}

impl<'de> Deserialize<'de> for Json {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> de::Visitor<'de> for V {
            type Value = Json;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("strict JSON")
            }
            fn visit_unit<E: de::Error>(self) -> Result<Json, E> {
                Ok(Json::Null)
            }
            fn visit_none<E: de::Error>(self) -> Result<Json, E> {
                Ok(Json::Null)
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Json, E> {
                Ok(Json::Bool(v))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Json, E> {
                Ok(Json::Number(v.into()))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Json, E> {
                Ok(Json::Number(v.into()))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Json, E> {
                serde_json::Number::from_f64(v)
                    .map(Json::Number)
                    .ok_or_else(|| E::custom("non-finite number"))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Json, E> {
                Ok(Json::String(v.to_owned()))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Json, E> {
                Ok(Json::String(v))
            }
            fn visit_seq<A: de::SeqAccess<'de>>(self, mut a: A) -> Result<Json, A::Error> {
                let mut out = Vec::new();
                while let Some(v) = a.next_element()? {
                    out.push(v);
                }
                Ok(Json::Array(out))
            }
            fn visit_map<A: de::MapAccess<'de>>(self, mut a: A) -> Result<Json, A::Error> {
                let mut out = std::collections::BTreeMap::new();
                while let Some((k, v)) = a.next_entry::<String, Json>()? {
                    if out.insert(k, v).is_some() {
                        return Err(de::Error::custom("duplicate member"));
                    }
                }
                Ok(Json::Object(out))
            }
        }
        d.deserialize_any(V)
    }
}

fn within_depth(bytes: &[u8]) -> bool {
    let (mut depth, mut quoted, mut escape) = (0usize, false, false);
    for &b in bytes {
        if quoted {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                quoted = false;
            }
        } else if b == b'"' {
            quoted = true;
        } else if b == b'{' || b == b'[' {
            depth += 1;
            if depth > 32 {
                return false;
            }
        } else if b == b'}' || b == b']' {
            depth = depth.saturating_sub(1);
        }
    }
    !quoted && depth == 0
}

#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "request encoder is retained for the later P05b2 caller"
    )
)]
struct BoundedWriter {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for BoundedWriter {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        let end = self
            .bytes
            .len()
            .checked_add(input.len())
            .ok_or_else(|| io::Error::other("limit"))?;
        if end > self.limit {
            return Err(io::Error::other("limit"));
        }
        self.bytes.extend_from_slice(input);
        Ok(input.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "request encoder is retained for the later P05b2 caller"
    )
)]
pub(crate) fn encode_request(request: &HelperRequest<'_>) -> Result<Vec<u8>, ProtocolError> {
    if request.v != 1
        || request.id.is_empty()
        || request.id.len() > 128
        || !request.id.is_ascii()
        || request.id.bytes().any(|b| b.is_ascii_control())
        || request.origin.is_empty()
        || request.origin.len() > 4096
    {
        return Err(ProtocolError::InvalidRequest);
    }
    if !value_depth_ok(request.options, 31) {
        return Err(ProtocolError::InvalidRequest);
    }
    let mut writer = BoundedWriter {
        bytes: Vec::new(),
        limit: MAX_TERMINAL_FRAME,
    };
    serde_json::to_writer(&mut writer, request).map_err(|_| ProtocolError::TooLarge)?;
    writer
        .write_all(b"\n")
        .map_err(|_| ProtocolError::TooLarge)?;
    Ok(writer.bytes)
}

#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "request encoder is retained for the later P05b2 caller"
    )
)]
fn value_depth_ok(value: &serde_json::Value, maximum: usize) -> bool {
    fn walk(value: &serde_json::Value, depth: usize, maximum: usize) -> bool {
        match value {
            serde_json::Value::Array(items) => {
                depth < maximum && items.iter().all(|item| walk(item, depth + 1, maximum))
            }
            serde_json::Value::Object(fields) => {
                depth < maximum && fields.values().all(|item| walk(item, depth + 1, maximum))
            }
            _ => true,
        }
    }
    walk(value, 0, maximum)
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum HelperFrame {
    Qr {
        id: String,
        width: u16,
        modules: Vec<u8>,
    },
    Status {
        id: String,
        status: String,
    },
    Result {
        id: String,
        body: Json,
    },
    Error {
        id: String,
        code: String,
    },
}

#[derive(Default)]
pub(crate) struct QueueBudget {
    frames: usize,
    bytes: usize,
}

impl QueueBudget {
    pub(crate) fn reserve(&mut self, bytes: usize) -> Result<(), ProtocolError> {
        let frames = self.frames.checked_add(1).ok_or(ProtocolError::QueueFull)?;
        let total = self
            .bytes
            .checked_add(bytes)
            .ok_or(ProtocolError::QueueFull)?;
        if frames > MAX_QUEUED_FRAMES || total > MAX_QUEUED_BYTES {
            return Err(ProtocolError::QueueFull);
        }
        self.frames = frames;
        self.bytes = total;
        Ok(())
    }

    pub(crate) fn release(&mut self, bytes: usize) {
        self.frames = self.frames.saturating_sub(1);
        self.bytes = self.bytes.saturating_sub(bytes);
    }

    pub(crate) fn pending_bytes(&self) -> usize {
        self.bytes
    }
}

fn classify_kind(bytes: &[u8]) -> Result<String, ProtocolError> {
    struct KindVisitor;
    impl<'de> de::Visitor<'de> for KindVisitor {
        type Value = String;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a v1 helper object")
        }
        fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<String, A::Error> {
            let mut kind = None;
            while let Some(key) = map.next_key::<String>()? {
                if key == "kind" {
                    if kind.is_some() {
                        return Err(de::Error::custom("duplicate kind"));
                    }
                    kind = Some(map.next_value::<String>()?);
                } else {
                    map.next_value::<de::IgnoredAny>()?;
                }
            }
            kind.ok_or_else(|| de::Error::missing_field("kind"))
        }
    }
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let kind = deserializer
        .deserialize_map(KindVisitor)
        .map_err(|_| ProtocolError::InvalidFrame)?;
    deserializer
        .end()
        .map_err(|_| ProtocolError::InvalidFrame)?;
    Ok(kind)
}

fn object(value: Option<Json>) -> Result<std::collections::BTreeMap<String, Json>, ProtocolError> {
    match value {
        Some(Json::Object(fields)) => Ok(fields),
        _ => Err(ProtocolError::InvalidFrame),
    }
}

fn take_string(
    fields: &mut std::collections::BTreeMap<String, Json>,
    key: &str,
) -> Result<String, ProtocolError> {
    match fields.remove(key) {
        Some(Json::String(value)) => Ok(value),
        _ => Err(ProtocolError::InvalidFrame),
    }
}

fn exact_fields(
    fields: &std::collections::BTreeMap<String, Json>,
    required: &[&str],
    optional: &[&str],
) -> bool {
    required.iter().all(|key| fields.contains_key(*key))
        && fields
            .keys()
            .all(|key| required.contains(&key.as_str()) || optional.contains(&key.as_str()))
}

fn credential_outer_shape(value: &Json) -> bool {
    let Json::Object(fields) = value else {
        return false;
    };
    credential_outer_fields(fields)
}

fn credential_outer_fields(fields: &std::collections::BTreeMap<String, Json>) -> bool {
    let required = ["id", "rawId", "response", "clientExtensionResults", "type"];
    if !exact_fields(fields, &required, &["authenticatorAttachment"]) {
        return false;
    }
    if !matches!(fields.get("id"), Some(Json::String(_)))
        || !matches!(fields.get("rawId"), Some(Json::String(_)))
        || !matches!(fields.get("type"), Some(Json::String(t)) if t == "public-key")
        || !matches!(fields.get("response"), Some(Json::Object(_)))
        || !matches!(fields.get("clientExtensionResults"), Some(Json::Object(_)))
    {
        return false;
    }
    if fields
        .get("authenticatorAttachment")
        .is_some_and(|v| !matches!(v, Json::String(_)))
    {
        return false;
    }
    true
}

fn validate_result(body: &std::collections::BTreeMap<String, Json>, operation: Operation) -> bool {
    match operation {
        Operation::Create => credential_outer_fields(body),
        Operation::Get => {
            if !exact_fields(body, &["assertions"], &["accounts"]) {
                return false;
            }
            let Some(Json::Array(assertions)) = body.get("assertions") else {
                return false;
            };
            !assertions.is_empty() && assertions.len() <= 16 && assertions.iter().all(credential_outer_shape)
                && body.get("accounts").is_none_or(|accounts| {
                    matches!(accounts, Json::Array(labels) if labels.len() == assertions.len()
                        && labels.iter().all(|label| matches!(label, Json::Null)
                            || matches!(label, Json::String(text) if !text.is_empty() && text.len() <= 512
                                && !text.chars().any(|c| c.is_control() || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')))))
                })
        }
    }
}

fn admission_error(code: &str) -> bool {
    matches!(
        code,
        "invalid_envelope"
            | "invalid_origin"
            | "unavailable"
            | "unsupported_options"
            | "invalid_request"
            | "request_too_large"
            | "transport_unavailable"
            | "security_error"
    )
}

fn operation_error(code: &str) -> bool {
    matches!(
        code,
        "authenticator_failed" | "serialization_failed" | "response_too_large" | "output_failed"
    )
}

pub(crate) fn decode_frame(
    bytes: &[u8],
    expected_id: &str,
    operation: Operation,
) -> Result<HelperFrame, ProtocolError> {
    if bytes.is_empty() || bytes.len() > MAX_TERMINAL_FRAME || !within_depth(bytes) {
        return Err(if bytes.len() > MAX_TERMINAL_FRAME {
            ProtocolError::TooLarge
        } else {
            ProtocolError::InvalidFrame
        });
    }
    let kind = classify_kind(bytes)?;
    if matches!(kind.as_str(), "status" | "qr") && bytes.len() > MAX_PROGRESS_FRAME {
        return Err(ProtocolError::TooLarge);
    }
    let mut de = serde_json::Deserializer::from_slice(bytes);
    let Json::Object(mut root) =
        Json::deserialize(&mut de).map_err(|_| ProtocolError::InvalidFrame)?
    else {
        return Err(ProtocolError::InvalidFrame);
    };
    de.end().map_err(|_| ProtocolError::InvalidFrame)?;
    if root.len() != 4 || root.remove("v") != Some(Json::Number(1u64.into())) {
        return Err(ProtocolError::InvalidFrame);
    }
    let id = match root.remove("id") {
        Some(Json::String(id)) if id == expected_id => id,
        _ => return Err(ProtocolError::InvalidFrame),
    };
    if root.remove("kind") != Some(Json::String(kind.clone())) {
        return Err(ProtocolError::InvalidFrame);
    }
    let mut body = object(root.remove("body"))?;
    match kind.as_str() {
        "status" => {
            if body.len() != 1 {
                return Err(ProtocolError::InvalidFrame);
            }
            let status = take_string(&mut body, "status")?;
            if !matches!(status.as_str(), "submitted" | "completed") {
                return Err(ProtocolError::InvalidFrame);
            }
            Ok(HelperFrame::Status { id, status })
        }
        "qr" => {
            if body.len() != 2 {
                return Err(ProtocolError::InvalidFrame);
            }
            let width = match body.remove("width") {
                Some(Json::Number(n)) => {
                    n.as_u64()
                        .filter(|w| (21..=177).contains(w) && (w - 21) % 4 == 0)
                        .ok_or(ProtocolError::InvalidFrame)? as u16
                }
                _ => return Err(ProtocolError::InvalidFrame),
            };
            let modules = match body.remove("modules") {
                Some(Json::Array(values))
                    if values.len() == usize::from(width) * usize::from(width) =>
                {
                    values
                        .into_iter()
                        .map(|v| match v {
                            Json::Number(n) if n.as_u64().is_some_and(|x| x <= 1) => {
                                Ok(n.as_u64().unwrap_or_default() as u8)
                            }
                            _ => Err(ProtocolError::InvalidFrame),
                        })
                        .collect::<Result<Vec<_>, _>>()?
                }
                _ => return Err(ProtocolError::InvalidFrame),
            };
            Ok(HelperFrame::Qr { id, width, modules })
        }
        "result" => {
            if validate_result(&body, operation) {
                Ok(HelperFrame::Result {
                    id,
                    body: Json::Object(body),
                })
            } else {
                Err(ProtocolError::InvalidFrame)
            }
        }
        "error" => {
            let code = take_string(&mut body, "code")?;
            let implemented = body.remove("implemented");
            if implemented == Some(Json::Bool(false)) && body.is_empty() && admission_error(&code) {
                return Ok(HelperFrame::Error { id, code });
            }
            if implemented.is_none()
                && body.is_empty()
                && (operation_error(&code)
                    || (code == "credential_excluded" && operation == Operation::Create))
            {
                Ok(HelperFrame::Error { id, code })
            } else {
                Err(ProtocolError::InvalidFrame)
            }
        }
        _ => Err(ProtocolError::InvalidFrame),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(kind: &str, body: serde_json::Value) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"v":1,"id":"r1","kind":kind,"body":body})).unwrap()
    }

    fn padded(mut frame: Vec<u8>, size: usize) -> Vec<u8> {
        frame.pop();
        frame.resize(size - 1, b' ');
        frame.push(b'}');
        frame
    }

    fn credential() -> serde_json::Value {
        serde_json::json!({
            "id":"Y3JlZA", "rawId":"Y3JlZA", "response":{},
            "clientExtensionResults":{}, "type":"public-key"
        })
    }

    #[test]
    fn strict_helper_protocol_frames() {
        let op_error = envelope("error", serde_json::json!({"code":"authenticator_failed"}));
        assert!(decode_frame(&op_error, "r1", Operation::Create).is_ok());
        let admission = envelope(
            "error",
            serde_json::json!({"code":"unavailable","implemented":false}),
        );
        assert!(decode_frame(&admission, "r1", Operation::Get).is_ok());
        for code in ["authenticator_failed", "credential_excluded"] {
            for implemented in [
                serde_json::json!(false),
                serde_json::json!(true),
                serde_json::Value::Null,
            ] {
                let annotated = envelope(
                    "error",
                    serde_json::json!({"code":code,"implemented":implemented}),
                );
                assert!(decode_frame(&annotated, "r1", Operation::Create).is_err());
            }
        }
        for bad in [
            envelope("error", serde_json::json!({"code":"no_input"})),
            envelope("error", serde_json::json!({"code":"invalid_request"})),
            envelope("error", serde_json::json!({"code":"timeout"})),
            envelope("error", serde_json::json!({"code":"timeout","implemented":false})),
            br#"{"v":1,"id":"r1","kind":"error","body":{"code":"invalid_envelope","implemented":false,"extra":0}}"#.to_vec(),
        ] { assert!(decode_frame(&bad, "r1", Operation::Create).is_err()); }

        let create = envelope("result", credential());
        assert!(matches!(
            decode_frame(&create, "r1", Operation::Create),
            Ok(HelperFrame::Result { .. })
        ));
        assert!(decode_frame(&create, "r1", Operation::Get).is_err());
        let get = envelope("result", serde_json::json!({"assertions":[credential()]}));
        assert!(matches!(
            decode_frame(&get, "r1", Operation::Get),
            Ok(HelperFrame::Result { .. })
        ));
        assert!(decode_frame(&get, "r1", Operation::Create).is_err());
        let bad_create = envelope(
            "result",
            serde_json::json!({"id":"x","rawId":"x","response":{},"clientExtensionResults":{},"type":"password"}),
        );
        assert!(decode_frame(&bad_create, "r1", Operation::Create).is_err());

        let status = envelope("status", serde_json::json!({"status":"submitted"}));
        assert!(matches!(
            decode_frame(&status, "r1", Operation::Create),
            Ok(HelperFrame::Status { .. })
        ));
        for status in ["waiting", "complete", "submitted "] {
            let bad = envelope("status", serde_json::json!({"status":status}));
            assert!(decode_frame(&bad, "r1", Operation::Create).is_err());
        }
        assert!(decode_frame(&status, "other", Operation::Create).is_err());
        assert!(
            decode_frame(
                br#"{"v":1,"kind":"status","body":{"status":"submitted"}}"#,
                "r1",
                Operation::Create
            )
            .is_err()
        );
        assert!(
            decode_frame(
                br#"{"v":1,"id":null,"kind":"status","body":{"status":"submitted"}}"#,
                "r1",
                Operation::Create
            )
            .is_err()
        );

        let qr = envelope("qr", serde_json::json!({"width":21,"modules":vec![0;441]}));
        assert!(matches!(
            decode_frame(&qr, "r1", Operation::Create),
            Ok(HelperFrame::Qr { width: 21, .. })
        ));
        for width in [1u64, 22, 177] {
            let count = (width * width).min(441) as usize;
            let invalid = envelope(
                "qr",
                serde_json::json!({"width":width,"modules":vec![0;count]}),
            );
            assert!(decode_frame(&invalid, "r1", Operation::Create).is_err());
        }
        let oversized_qr = padded(qr.clone(), MAX_PROGRESS_FRAME + 1);
        assert_eq!(
            decode_frame(&oversized_qr, "r1", Operation::Create),
            Err(ProtocolError::TooLarge)
        );
        let oversized_status = padded(status.clone(), MAX_PROGRESS_FRAME + 1);
        assert_eq!(
            decode_frame(&oversized_status, "r1", Operation::Create),
            Err(ProtocolError::TooLarge)
        );
        let terminal = padded(op_error, MAX_PROGRESS_FRAME + 1000);
        assert!(decode_frame(&terminal, "r1", Operation::Create).is_ok());
        assert_eq!(
            decode_frame(
                &padded(terminal, MAX_TERMINAL_FRAME + 1),
                "r1",
                Operation::Create
            ),
            Err(ProtocolError::TooLarge)
        );

        let duplicate = br#"{"v":1,"id":"r1","kind":"result","body":{"id":"x","rawId":"x","response":{"a":1,"a":2},"clientExtensionResults":{},"type":"public-key"}}"#;
        assert!(decode_frame(duplicate, "r1", Operation::Create).is_err());
        let deep = format!(
            "{{\"v\":1,\"id\":\"r1\",\"kind\":\"error\",\"body\":{{\"code\":\"invalid_envelope\",\"implemented\":false,\"nested\":{} }}}}",
            format_args!("{}null{}", "[".repeat(34), "]".repeat(34))
        );
        assert!(decode_frame(deep.as_bytes(), "r1", Operation::Create).is_err());

        let options = serde_json::json!({});
        let request = HelperRequest {
            v: 1,
            id: "r1",
            op: Operation::Create,
            origin: "https://example.test",
            options: &options,
            network: NetworkPolicy::default(),
        };
        assert!(encode_request(&request).is_ok());
        let mut too_deep = serde_json::Value::Null;
        for _ in 0..32 {
            too_deep = serde_json::json!([too_deep]);
        }
        let request = HelperRequest {
            options: &too_deep,
            ..request
        };
        assert_eq!(encode_request(&request), Err(ProtocolError::InvalidRequest));
    }
}

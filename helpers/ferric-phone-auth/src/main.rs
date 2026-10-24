use std::io::{self, BufRead, Write};

use serde_json::{json, Map, Value};

const MAX_REQUEST_BYTES: usize = 1024 * 1024;
const MAX_NATIVE_ID_BYTES: usize = 128;

enum InputLine {
    Complete(Vec<u8>),
    TooLarge,
    MissingDelimiter,
    NoInput,
}

fn read_request_line(reader: &mut impl BufRead) -> io::Result<InputLine> {
    let mut line = Vec::with_capacity(256);
    loop {
        let (chunk_len, consume_len, found_newline) = {
            let available = reader.fill_buf()?;
            if available.is_empty() {
                return Ok(if line.is_empty() {
                    InputLine::NoInput
                } else {
                    InputLine::MissingDelimiter
                });
            }

            let newline = available.iter().position(|byte| *byte == b'\n');
            let content_len = newline.unwrap_or(available.len());
            if content_len > MAX_REQUEST_BYTES.saturating_sub(line.len()) {
                return Ok(InputLine::TooLarge);
            }
            line.extend_from_slice(&available[..content_len]);
            let consume_len = content_len + if newline.is_some() { 1 } else { 0 };
            (content_len, consume_len, newline.is_some())
        };
        reader.consume(consume_len);
        if found_newline {
            return Ok(InputLine::Complete(line));
        }
        debug_assert_eq!(chunk_len, consume_len);
    }
}

fn error_response(id: Option<&str>, code: &str) -> Value {
    json!({
        "v": 1,
        "id": id,
        "kind": "error",
        "body": {
            "code": code,
            "implemented": false
        }
    })
}

fn valid_native_id(value: Option<&Value>) -> Option<&str> {
    let id = value?.as_str()?;
    (!id.is_empty() && id.len() <= MAX_NATIVE_ID_BYTES && id.is_ascii()).then_some(id)
}

fn only_envelope_fields(object: &Map<String, Value>) -> bool {
    const ALLOWED: [&str; 6] = ["v", "id", "op", "origin", "options", "network"];
    object.keys().all(|key| ALLOWED.contains(&key.as_str()))
}

fn process_request(line: &[u8]) -> Value {
    let Ok(Value::Object(envelope)) = serde_json::from_slice(line) else {
        return error_response(None, "invalid_envelope");
    };

    let Some(id) = valid_native_id(envelope.get("id")) else {
        return error_response(None, "invalid_id");
    };
    if envelope.get("v").and_then(Value::as_u64) != Some(1) {
        return error_response(Some(id), "unsupported_version");
    }
    match envelope.get("op").and_then(Value::as_str) {
        Some("create" | "get") => {}
        _ => return error_response(Some(id), "unsupported_operation"),
    }
    if !only_envelope_fields(&envelope) {
        return error_response(Some(id), "unknown_field");
    }
    if !envelope.get("origin").is_some_and(Value::is_string)
        || !envelope.get("options").is_some_and(Value::is_object)
        || !envelope.get("network").is_some_and(Value::is_object)
    {
        return error_response(Some(id), "invalid_envelope");
    }

    // P01 intentionally has no ceremony implementation or success response.
    error_response(Some(id), "not_implemented")
}

fn write_response(mut writer: impl Write, response: &Value) -> io::Result<()> {
    serde_json::to_writer(&mut writer, response).map_err(io::Error::other)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

fn run() -> io::Result<()> {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let stdout = io::stdout();
    let mut writer = stdout.lock();

    let (response, malformed_input) = match read_request_line(&mut reader)? {
        InputLine::Complete(line) => (process_request(&line), false),
        InputLine::TooLarge => (error_response(None, "request_too_large"), true),
        InputLine::MissingDelimiter => (error_response(None, "invalid_envelope"), true),
        InputLine::NoInput => return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "no request")),
    };
    write_response(&mut writer, &response)?;
    if malformed_input {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid request framing"));
    }
    Ok(())
}

fn main() {
    if run().is_err() {
        // Deliberately omit diagnostics: request envelopes can contain secrets.
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::{error_response, process_request, read_request_line, InputLine};
    use serde_json::Value;
    use std::io::Cursor;

    fn code(response: &Value) -> &str {
        response["body"]["code"]
            .as_str()
            .expect("error response has a coarse code")
    }

    fn request(op: &str) -> Vec<u8> {
        serde_json::json!({
            "v": 1,
            "id": "native-opaque-1",
            "op": op,
            "origin": "https://example.test",
            "options": {},
            "network": {}
        })
        .to_string()
        .into_bytes()
    }

    #[test]
    fn supported_operation_names_remain_explicitly_unimplemented() {
        for operation in ["create", "get"] {
            let response = process_request(&request(operation));
            assert_eq!(response["kind"], "error");
            assert_eq!(code(&response), "not_implemented");
            assert_eq!(response["body"]["implemented"], false);
        }
    }

    #[test]
    fn unknown_operation_is_rejected_without_success() {
        let response = process_request(&request("delete"));
        assert_eq!(response["kind"], "error");
        assert_eq!(code(&response), "unsupported_operation");
    }

    #[test]
    fn unsupported_version_and_authority_extension_are_rejected() {
        let mut version = serde_json::from_slice::<Value>(&request("get")).expect("test JSON");
        version["v"] = serde_json::json!(2);
        assert_eq!(code(&process_request(version.to_string().as_bytes())), "unsupported_version");

        let mut extended = serde_json::from_slice::<Value>(&request("create")).expect("test JSON");
        extended["executable"] = serde_json::json!("arbitrary");
        assert_eq!(code(&process_request(extended.to_string().as_bytes())), "unknown_field");
    }

    #[test]
    fn malformed_json_and_oversized_lines_fail_closed() {
        assert_eq!(code(&process_request(b"{not json")), "invalid_envelope");
        let oversized = vec![b'x'; super::MAX_REQUEST_BYTES + 1];
        assert!(matches!(
            read_request_line(&mut Cursor::new(oversized)),
            Ok(InputLine::TooLarge)
        ));
        assert_eq!(code(&error_response(None, "request_too_large")), "request_too_large");
    }
}

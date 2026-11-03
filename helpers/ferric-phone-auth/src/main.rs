use std::io::{self, BufRead, Write};

use ferric_phone_auth::process_request_to_pipe;
use serde_json::{Value, json};

const MAX_REQUEST_BYTES: usize = ferric_phone_auth::MAX_REQUEST_BYTES;

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

fn write_response(mut writer: impl Write, response: &Value) -> io::Result<()> {
    let encoded = serde_json::to_vec(response).map_err(io::Error::other)?;
    if encoded.len().saturating_add(1) > 1024 * 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "terminal response too large",
        ));
    }
    writer.write_all(&encoded)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

fn run() -> io::Result<()> {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let stdout = io::stdout();
    let mut writer = stdout.lock();

    match read_request_line(&mut reader)? {
        InputLine::Complete(line) => return process_request_to_pipe(&line, &mut writer),
        InputLine::TooLarge => {
            write_response(&mut writer, &error_response(None, "request_too_large"))?
        }
        InputLine::MissingDelimiter => {
            write_response(&mut writer, &error_response(None, "invalid_envelope"))?
        }
        InputLine::NoInput => {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "no request"));
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid request framing",
    ))
}

fn main() {
    // An upstream panic may contain request or ephemeral pairing data.
    std::panic::set_hook(Box::new(|_| {}));
    if run().is_err() {
        // Deliberately omit diagnostics: request envelopes can contain secrets.
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::{InputLine, error_response, read_request_line};
    use ferric_phone_auth::process_request;
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
    fn supported_operation_names_never_claim_ceremony_success() {
        for operation in ["create", "get"] {
            let response = process_request(&request(operation));
            assert_eq!(response["kind"], "error");
            assert!(matches!(
                code(&response),
                "unavailable" | "invalid_request" | "transport_unavailable"
            ));
            assert_eq!(response["body"]["implemented"], false);
        }
    }

    #[test]
    fn unknown_operation_is_rejected_without_success() {
        let response = process_request(&request("delete"));
        assert_eq!(response["kind"], "error");
        assert_eq!(code(&response), "invalid_envelope");
    }

    #[test]
    fn unsupported_version_and_authority_extension_are_rejected() {
        let mut version = serde_json::from_slice::<Value>(&request("get")).expect("test JSON");
        version["v"] = serde_json::json!(2);
        assert_eq!(
            code(&process_request(version.to_string().as_bytes())),
            "invalid_envelope"
        );

        let mut extended = serde_json::from_slice::<Value>(&request("create")).expect("test JSON");
        extended["executable"] = serde_json::json!("arbitrary");
        assert_eq!(
            code(&process_request(extended.to_string().as_bytes())),
            "invalid_envelope"
        );
    }

    #[test]
    fn malformed_json_and_oversized_lines_fail_closed() {
        assert_eq!(code(&process_request(b"{not json")), "invalid_envelope");
        let oversized = vec![b'x'; super::MAX_REQUEST_BYTES + 1];
        assert!(matches!(
            read_request_line(&mut Cursor::new(oversized)),
            Ok(InputLine::TooLarge)
        ));
        assert_eq!(
            code(&error_response(None, "request_too_large")),
            "request_too_large"
        );
    }
}

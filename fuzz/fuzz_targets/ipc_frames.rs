#![no_main]

use std::io::Cursor;

use browser_ipc::{Response, parse_request, read_frame, serialize_response, write_frame};
use libfuzzer_sys::fuzz_target;
use serde_json::Value;

fuzz_target!(|data: &[u8]| {
    let mut reader = Cursor::new(data);
    if let Ok(Some(payload)) = read_frame(&mut reader) {
        let _ = parse_request(&payload);
    }

    let mut framed = Vec::new();
    let _ = write_frame(&mut framed, data);
    let response = Response::success("fuzz", Value::Null);
    let _ = serialize_response(&response);
});

//! Private request admission for the phone-auth helper. A prepared request is
//! created only by strict parsing, HTTPS origin checks, usable PSL data, and
//! pinned-library validation. It does not start QR, network, or device work.

mod admission;
mod errors;
// P07a stages the production-shared operation core; P08 alone may connect it
// to the production caBLE channel. Keep the compiled core available meanwhile.
#[allow(dead_code)]
mod hybrid;
mod protocol;

use admission::prepare_request;
use protocol::parse_envelope;
use serde_json::{Value, json};

pub const MAX_REQUEST_BYTES: usize = protocol::MAX_REQUEST_BYTES;

/// Process one private-pipe request and return only a coarse helper response.
/// The prepared credential request never leaves this crate's private boundary.
pub fn process_request(line: &[u8]) -> Value {
    let envelope = match parse_envelope(line) {
        Ok(envelope) => envelope,
        Err(error) => return error_response(None, error.code()),
    };
    let id = envelope.id.clone();
    match prepare_request(envelope) {
        Ok(prepared) => error_response(Some(prepared.id()), "transport_unavailable"),
        Err(error) => error_response(Some(&id), error.code()),
    }
}

fn error_response(id: Option<&str>, code: &str) -> Value {
    json!({
        "v": 1,
        "id": id,
        "kind": "error",
        "body": {"code": code, "implemented": false}
    })
}

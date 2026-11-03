//! Private request admission for the phone-auth helper. A prepared request is
//! created only by strict parsing, HTTPS origin checks, usable PSL data, and
//! pinned-library validation. It does not start QR, network, or device work.

mod admission;
mod errors;
mod hybrid;
mod protocol;
mod suffix_list;

use admission::prepare_request;
use protocol::parse_envelope;
use serde_json::{Value, json};
use std::io::{self, Write};

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
        Ok(prepared) => {
            let id = prepared.id().to_owned();
            match hybrid::transient_device(&prepared) {
                Ok(_private_qr_device) => error_response(Some(&id), "transport_unavailable"),
                Err(_) => error_response(Some(&id), "unavailable"),
            }
        }
        Err(error) => error_response(Some(&id), error.code()),
    }
}

/// Processes a framed private-pipe request and writes its bounded terminal
/// envelope through the same serializer used by hybrid progress/results.
pub fn process_request_to_pipe(line: &[u8], writer: &mut impl Write) -> io::Result<()> {
    let envelope = match parse_envelope(line) {
        Ok(envelope) => envelope,
        Err(error) => return write_error(writer, None, error.code()),
    };
    let direct = envelope.network.direct();
    let id = envelope.id.clone();
    let prepared = match prepare_request(envelope) {
        Ok(prepared) => prepared,
        Err(error) => return write_error(writer, Some(&id), error.code()),
    };
    if !direct {
        return write_error(writer, Some(&id), "transport_unavailable");
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(hybrid::execute_phone_to_pipe(prepared, writer))
}

fn write_error(writer: &mut impl Write, id: Option<&str>, code: &str) -> io::Result<()> {
    hybrid::write_terminal(&error_response(id, code), |encoded| {
        writer.write_all(encoded)?;
        writer.flush()
    })
}

fn error_response(id: Option<&str>, code: &str) -> Value {
    json!({
        "v": 1,
        "id": id,
        "kind": "error",
        "body": {"code": code, "implemented": false}
    })
}

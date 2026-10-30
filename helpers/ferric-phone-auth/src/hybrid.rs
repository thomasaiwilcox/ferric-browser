use std::io::{self, Write};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::time::timeout;

use crate::admission::{PreparedCredentialRequest, PreparedRequest};
use libwebauthn::ops::webauthn::WebAuthnIDLResponse;
use libwebauthn::transport::cable::qr_code_device::{
    CableQrCodeDevice, CableTransports, QrCodeOperationHint,
};
use libwebauthn::transport::{Channel, ChannelSettings, Device};
use libwebauthn::webauthn::{WebAuthn, WebAuthnError};
use qrcode::{Color, QrCode};

const MAX_TERMINAL_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_STDOUT_BYTES: usize = 4 * 1024 * 1024;
const MAX_PROGRESS_LINE_BYTES: usize = 32 * 1024;
const MAX_CEREMONY_TIME: Duration = Duration::from_secs(180);
const MIN_PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperationProgress {
    Submitted,
    Completed,
}

#[derive(Debug)]
pub(crate) enum OperationFailure {
    Authenticator,
    CredentialExcluded,
    Serialization,
    ResponseTooLarge,
    Output,
}

#[derive(Debug)]
pub(crate) struct OperationResult {
    pub(crate) response: Value,
}

/// Builds the library's transient caBLE QR device for an admitted operation.
/// Callers must keep its payload private and admit network policy first.
pub(crate) fn transient_device(
    prepared: &PreparedCredentialRequest,
) -> Result<CableQrCodeDevice, OperationFailure> {
    let hint = if prepared.is_create() {
        QrCodeOperationHint::MakeCredential
    } else {
        QrCodeOperationHint::GetAssertionRequest
    };
    CableQrCodeDevice::new_transient(hint, CableTransports::CloudAssistedOnly)
        .map_err(|_| OperationFailure::Authenticator)
}

/// Starts a single transient phone ceremony after explicit direct-policy
/// admission. QR output must reach the parent before BLE/network work starts.
/// Process termination by the parent is the cancellation boundary.
pub(crate) async fn execute_phone_to_pipe(
    prepared: PreparedCredentialRequest,
    writer: &mut impl Write,
) -> io::Result<()> {
    let id = prepared.id().to_owned();
    let mut output = BoundedOutput::new(|encoded: &[u8]| {
        writer.write_all(encoded)?;
        writer.flush()
    });
    let mut device = match transient_device(&prepared) {
        Ok(device) => device,
        Err(error) => {
            return output
                .emit_value(
                    &error_envelope(&id, failure_code(&error)),
                    MAX_TERMINAL_RESPONSE_BYTES,
                )
                .map_err(pipe_error);
        }
    };
    let frame = qr_frame(&id, &device).map_err(pipe_error)?;
    output
        .emit_value(&frame, MAX_PROGRESS_LINE_BYTES)
        .map_err(pipe_error)?;
    let mut channel = match device.channel(ChannelSettings::default()).await {
        Ok(channel) => channel,
        Err(_) => {
            return output
                .emit_value(
                    &error_envelope(&id, "transport_unavailable"),
                    MAX_TERMINAL_RESPONSE_BYTES,
                )
                .map_err(pipe_error);
        }
    };
    let result = execute_to_pipe(prepared, &mut channel, |line| {
        // The outer output budget also includes the QR frame.
        let next = output
            .planned_bytes
            .checked_add(line.len())
            .filter(|total| *total <= MAX_STDOUT_BYTES)
            .ok_or_else(|| io::Error::other("private output limit"))?;
        output.planned_bytes = next;
        (output.sink)(line)
    })
    .await;
    channel.close().await;
    // execute_to_pipe has already emitted one terminal error on operation
    // failure. A failed output is terminal and must never trigger a retry.
    match result {
        Err(OperationFailure::Output) => Err(pipe_error(OperationFailure::Output)),
        _ => Ok(()),
    }
}

fn pipe_error(_: OperationFailure) -> io::Error {
    io::Error::other("private ceremony output unavailable")
}

fn qr_frame(id: &str, device: &CableQrCodeDevice) -> Result<Value, OperationFailure> {
    let qr = QrCode::new(device.qr_code.to_string().as_bytes())
        .map_err(|_| OperationFailure::Serialization)?;
    let width = qr.width();
    let modules: Vec<u8> = qr
        .into_colors()
        .into_iter()
        .map(|color| u8::from(color == Color::Dark))
        .collect();
    let frame = json!({"v": 1, "id": id, "kind": "qr",
        "body": {"width": width, "modules": modules}});
    encode_bounded(&frame, MAX_PROGRESS_LINE_BYTES)?;
    Ok(frame)
}

/// Executes only a library-prepared request over a pinned library Channel.
/// The caller owns transport selection and must pass the same prepared request
/// to the production CableChannel or the test-only virtual HID channel.
pub(crate) async fn execute_prepared<C, F>(
    prepared: PreparedCredentialRequest,
    channel: &mut C,
    mut progress: F,
) -> Result<OperationResult, OperationFailure>
where
    C: WebAuthn,
    F: FnMut(OperationProgress) -> Result<(), OperationFailure>,
{
    let (id, operation) = prepared.into_parts();
    progress(OperationProgress::Submitted)?;
    let body = match operation {
        PreparedRequest::Create(request) => {
            let response = channel
                .webauthn_make_credential(&request)
                .await
                .map_err(|error| match error {
                    WebAuthnError::ExcludedByMakeCredential => OperationFailure::CredentialExcluded,
                    _ => OperationFailure::Authenticator,
                })?;
            response
                .to_json_value(&request)
                .map_err(|_| OperationFailure::Serialization)?
        }
        PreparedRequest::Get(request) => {
            let response = channel
                .webauthn_get_assertion(&request)
                .await
                .map_err(|_| OperationFailure::Authenticator)?;
            let assertions = response
                .assertions
                .iter()
                .map(|assertion| assertion.to_json_value(&request))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| OperationFailure::Serialization)?;
            // Native account-selection metadata is separate from WebAuthn's
            // credential JSON. Never pass these labels to the requesting RP.
            let accounts: Vec<Option<String>> = response
                .assertions
                .iter()
                .map(|assertion| {
                    let user = assertion.user.as_ref()?;
                    let label = user
                        .name
                        .as_deref()
                        .filter(|name| !name.trim().is_empty())
                        .or(user.display_name.as_deref())?;
                    let label: String = label
                        .chars()
                        .filter(|c| {
                            !c.is_control()
                                && !matches!(*c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
                        })
                        .take(128)
                        .collect();
                    let label = label.trim();
                    (!label.is_empty() && label.len() <= 512).then(|| label.to_owned())
                })
                .collect();
            json!({"assertions": assertions, "accounts": accounts})
        }
    };
    let response = json!({
        "v": 1,
        "id": id,
        "kind": "result",
        "body": body
    });
    // Count through a bounded writer so an oversized response never creates
    // an unbounded encoded copy before the protocol limit is applied.
    let _encoded = encode_bounded(&response, MAX_TERMINAL_RESPONSE_BYTES)?;
    progress(OperationProgress::Completed)?;
    Ok(OperationResult { response })
}

/// Executes one already admitted request and writes bounded typed protocol
/// messages. A failed write after submission is terminal: this layer never
/// retries an authenticator operation.
pub(crate) async fn execute_to_pipe<C, F>(
    prepared: PreparedCredentialRequest,
    channel: &mut C,
    sink: F,
) -> Result<(), OperationFailure>
where
    C: WebAuthn,
    F: FnMut(&[u8]) -> io::Result<()>,
{
    let id = prepared.id().to_owned();
    let mut output = BoundedOutput::new(sink);
    let result = {
        let mut progress = ProgressWriter::new(&mut output, &id);
        timeout(
            MAX_CEREMONY_TIME,
            execute_prepared(prepared, channel, |event| progress.emit(event)),
        )
        .await
    };

    match result {
        Ok(Ok(result)) => output.emit_value(&result.response, MAX_TERMINAL_RESPONSE_BYTES),
        Ok(Err(error)) => {
            let response = error_envelope(&id, failure_code(&error));
            let _ = output.emit_value(&response, MAX_TERMINAL_RESPONSE_BYTES);
            Err(error)
        }
        Err(_) => {
            let response = error_envelope(&id, "timeout");
            output.emit_value(&response, MAX_TERMINAL_RESPONSE_BYTES)?;
            Err(OperationFailure::Authenticator)
        }
    }
}

pub(crate) fn write_terminal<F>(response: &Value, sink: F) -> io::Result<()>
where
    F: FnMut(&[u8]) -> io::Result<()>,
{
    let mut output = BoundedOutput::new(sink);
    output
        .emit_value(response, MAX_TERMINAL_RESPONSE_BYTES)
        .map_err(|_| io::Error::other("private pipe response could not be written"))
}

fn failure_code(error: &OperationFailure) -> &'static str {
    match error {
        OperationFailure::Authenticator => "authenticator_failed",
        OperationFailure::CredentialExcluded => "credential_excluded",
        OperationFailure::Serialization => "serialization_failed",
        OperationFailure::ResponseTooLarge => "response_too_large",
        OperationFailure::Output => "output_failed",
    }
}

fn error_envelope(id: &str, code: &str) -> Value {
    json!({"v": 1, "id": id, "kind": "error", "body": {"code": code}})
}

fn encode_bounded(value: &Value, limit: usize) -> Result<Vec<u8>, OperationFailure> {
    let mut encoded = LimitedBuffer::new(limit);
    serde_json::to_writer(&mut encoded, value).map_err(|_| OperationFailure::ResponseTooLarge)?;
    encoded
        .write_all(b"\n")
        .map_err(|_| OperationFailure::ResponseTooLarge)?;
    Ok(encoded.finish())
}

struct BoundedOutput<F> {
    sink: F,
    planned_bytes: usize,
}

impl<F> BoundedOutput<F>
where
    F: FnMut(&[u8]) -> io::Result<()>,
{
    fn new(sink: F) -> Self {
        Self {
            sink,
            planned_bytes: 0,
        }
    }

    fn emit_value(&mut self, value: &Value, line_limit: usize) -> Result<(), OperationFailure> {
        let bytes = encode_bounded(value, line_limit)?;
        let next_total = self
            .planned_bytes
            .checked_add(bytes.len())
            .ok_or(OperationFailure::ResponseTooLarge)?;
        if next_total > MAX_STDOUT_BYTES {
            return Err(OperationFailure::ResponseTooLarge);
        }
        // Count before invoking the sink so a partial failed write cannot be
        // followed by another line that exceeds the stdout budget.
        self.planned_bytes = next_total;
        (self.sink)(&bytes).map_err(|_| OperationFailure::Output)
    }
}

struct LimitedBuffer {
    bytes: Vec<u8>,
    limit: usize,
}

impl LimitedBuffer {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(limit.min(4096)),
            limit,
        }
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

impl io::Write for LimitedBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let next =
            self.bytes.len().checked_add(bytes.len()).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "protocol line too large")
            })?;
        if next > self.limit {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "protocol line too large",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct ProgressWriter<'a, F> {
    output: &'a mut BoundedOutput<F>,
    id: &'a str,
    last_at: Option<Instant>,
    last_event: Option<OperationProgress>,
}

impl<F> ProgressWriter<'_, F>
where
    F: FnMut(&[u8]) -> io::Result<()>,
{
    fn new<'a>(output: &'a mut BoundedOutput<F>, id: &'a str) -> ProgressWriter<'a, F> {
        ProgressWriter {
            output,
            id,
            last_at: None,
            last_event: None,
        }
    }

    fn emit(&mut self, event: OperationProgress) -> Result<(), OperationFailure> {
        // Repeated states coalesce. If distinct updates arrive faster than the
        // wire budget, omit the intermediate update; terminal output remains.
        if self.last_event == Some(event) {
            return Ok(());
        }
        let now = Instant::now();
        if self
            .last_at
            .is_some_and(|last| now.duration_since(last) < MIN_PROGRESS_INTERVAL)
        {
            self.last_event = Some(event);
            return Ok(());
        }
        let status = match event {
            OperationProgress::Submitted => "submitted",
            OperationProgress::Completed => "completed",
        };
        let value = json!({
            "v": 1,
            "id": self.id,
            "kind": "status",
            "body": {"status": status}
        });
        self.output.emit_value(&value, MAX_PROGRESS_LINE_BYTES)?;
        self.last_at = Some(now);
        self.last_event = Some(event);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_PROGRESS_LINE_BYTES, MAX_TERMINAL_RESPONSE_BYTES, OperationFailure, OperationProgress,
        execute_prepared, execute_to_pipe,
    };
    use crate::admission::{PreparedCredentialRequest, prepare_test_request_with_psl};
    use crate::protocol::parse_envelope;
    use libwebauthn::ops::webauthn::DatFilePublicSuffixList;
    use libwebauthn::transport::cable::qr_code_device::{
        CableQrCodeDevice, CableTransports, QrCodeOperationHint,
    };
    use libwebauthn::transport::{Channel, ChannelSettings, Device};
    use libwebauthn_tests::virt::get_virtual_device;
    use std::env;
    use std::fs;
    use std::io;
    use std::path::Path;
    use std::process::{Command, Stdio};

    const FIXTURE_PSL: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/data/public_suffix_list.dat");

    async fn prepare(
        op: &str,
        options: serde_json::Value,
        psl: &DatFilePublicSuffixList,
    ) -> PreparedCredentialRequest {
        let envelope = serde_json::json!({
            "v": 1,
            "id": format!("t067a-{op}"),
            "op": op,
            "origin": "https://example.org",
            "options": options,
            "network": {}
        });
        let envelope = parse_envelope(envelope.to_string().as_bytes()).expect("strict envelope");
        prepare_test_request_with_psl(envelope, psl)
            .await
            .expect("explicit fixture-only admission")
    }

    fn registration_options() -> serde_json::Value {
        serde_json::json!({
            "rp": {"id": "example.org", "name": "Example"},
            "user": {"id": "AQIDBA", "name": "alice", "displayName": "Alice"},
            "challenge": "AQIDBAUGBwgJCgsMDQ4PEA",
            "pubKeyCredParams": [{"type": "public-key", "alg": -7}],
            "authenticatorSelection": {
                "authenticatorAttachment": "cross-platform",
                "residentKey": "discouraged",
                "userVerification": "preferred"
            },
            "attestation": "none"
        })
    }

    fn decoded(lines: &[Vec<u8>]) -> Vec<serde_json::Value> {
        lines
            .iter()
            .map(|line| serde_json::from_slice(line).expect("one typed JSON line"))
            .collect()
    }

    fn scan_files(path: &Path) -> io::Result<Vec<String>> {
        let mut files = Vec::new();
        for entry in fs::read_dir(path)? {
            let path = entry?.path();
            if path.is_dir() {
                files.extend(scan_files(&path)?);
            } else {
                files.push(path.display().to_string());
            }
        }
        Ok(files)
    }

    #[test]
    fn phone_qr_frame_contains_only_bounded_renderable_modules() {
        use super::qr_frame;
        let first = CableQrCodeDevice::new_transient(
            QrCodeOperationHint::GetAssertionRequest,
            CableTransports::CloudAssistedOnly,
        )
        .unwrap();
        let second = CableQrCodeDevice::new_transient(
            QrCodeOperationHint::GetAssertionRequest,
            CableTransports::CloudAssistedOnly,
        )
        .unwrap();
        let a = qr_frame("private-native-request", &first).unwrap();
        let b = qr_frame("private-native-request", &second).unwrap();
        let width = a["body"]["width"].as_u64().unwrap() as usize;
        let modules = a["body"]["modules"].as_array().unwrap();
        assert_eq!(modules.len(), width * width);
        assert!((21..=177).contains(&width));
        assert!(modules.iter().all(|bit| bit == 0 || bit == 1));
        assert_ne!(a["body"]["modules"], b["body"]["modules"]);
        let encoded = serde_json::to_vec(&a).unwrap();
        assert!(encoded.len() < 32 * 1024);
        assert!(!String::from_utf8(encoded).unwrap().contains("FIDO:/"));
        assert_eq!(first.qr_code.known_tunnel_domains_count, 2);
        assert_eq!(first.qr_code.state_assisted, None);
    }

    #[test]
    fn output_limits_reject_before_sink_emission() {
        let emitted = std::cell::Cell::new(0);
        let mut output = super::BoundedOutput::new(|_: &[u8]| {
            emitted.set(emitted.get() + 1);
            Ok(())
        });
        let progress = serde_json::json!({"data": "x".repeat(MAX_PROGRESS_LINE_BYTES)});
        assert!(matches!(
            output.emit_value(&progress, MAX_PROGRESS_LINE_BYTES),
            Err(OperationFailure::ResponseTooLarge)
        ));
        assert_eq!(
            emitted.get(),
            0,
            "oversized progress is never partially emitted"
        );

        let oversized_terminal =
            serde_json::json!({"data": "z".repeat(MAX_TERMINAL_RESPONSE_BYTES)});
        assert!(matches!(
            output.emit_value(&oversized_terminal, MAX_TERMINAL_RESPONSE_BYTES),
            Err(OperationFailure::ResponseTooLarge)
        ));
        assert_eq!(
            emitted.get(),
            0,
            "oversized terminal is rejected before it reaches the sink"
        );

        let terminal = serde_json::json!({"data": "y".repeat(900_000)});
        for _ in 0..4 {
            output
                .emit_value(&terminal, MAX_TERMINAL_RESPONSE_BYTES)
                .expect("each bounded terminal line fits");
        }
        assert!(matches!(
            output.emit_value(&terminal, MAX_TERMINAL_RESPONSE_BYTES),
            Err(OperationFailure::ResponseTooLarge)
        ));
        assert_eq!(
            emitted.get(),
            4,
            "stdout total cap rejects before a fifth line"
        );
    }

    #[test]
    fn t067_create_get_real_library_envelopes() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime");
        runtime.block_on(async {
            let psl = DatFilePublicSuffixList::from_path(FIXTURE_PSL)
                .expect("controlled official PSL fixture");
            // Construct both transient QR hints locally. Their secret payloads
            // are never displayed or passed to Device::channel.
            let create_qr = CableQrCodeDevice::new_transient(
                QrCodeOperationHint::MakeCredential,
                CableTransports::CloudAssistedOnly,
            )
            .expect("transient create QR");
            let get_qr = CableQrCodeDevice::new_transient(
                QrCodeOperationHint::GetAssertionRequest,
                CableTransports::CloudAssistedOnly,
            )
            .expect("transient get QR");
            assert_eq!(create_qr.to_string(), "CableQrCodeDevice");
            assert_eq!(get_qr.to_string(), "CableQrCodeDevice");

            let mut device = get_virtual_device();
            let mut channel = device
                .channel(ChannelSettings::default())
                .await
                .expect("supported in-process virtual HID channel");

            let mut create_lines = Vec::new();
            execute_to_pipe(
                prepare("create", registration_options(), &psl).await,
                &mut channel,
                |line| {
                    create_lines.push(line.to_vec());
                    Ok(())
                },
            )
            .await
            .expect("real library make-credential operation through helper pipe policy");
            let create_messages: Vec<serde_json::Value> = create_lines
                .iter()
                .map(|line| serde_json::from_slice(line).expect("status/result envelope"))
                .collect();
            assert_eq!(create_messages[0]["kind"], "status");
            assert_eq!(create_messages[0]["body"]["status"], "submitted");
            let created = create_messages
                .iter()
                .find(|message| message["kind"] == "result")
                .expect("one terminal create result");
            assert_eq!(created["body"]["type"], "public-key");
            let credential_id = created["body"]["id"]
                .as_str()
                .expect("serialized dynamic credential id");
            assert_eq!(created["id"], "t067a-create");

            let get_options = serde_json::json!({
                "challenge": "EBESExQVFhcYGRobHB0eHw",
                "rpId": "example.org",
                "allowCredentials": [{"type": "public-key", "id": credential_id}],
                "userVerification": "preferred"
            });
            let mut get_lines = Vec::new();
            execute_to_pipe(
                prepare("get", get_options, &psl).await,
                &mut channel,
                |line| {
                    get_lines.push(line.to_vec());
                    Ok(())
                },
            )
            .await
            .expect("real library get-assertion operation through helper pipe policy");
            let get_messages: Vec<serde_json::Value> = get_lines
                .iter()
                .map(|line| serde_json::from_slice(line).expect("status/result envelope"))
                .collect();
            let asserted = get_messages
                .iter()
                .find(|message| message["kind"] == "result")
                .expect("one terminal get result");
            assert_eq!(
                asserted["body"]["assertions"]
                    .as_array()
                    .map(|values| values.len()),
                Some(1)
            );
            assert_eq!(asserted["id"], "t067a-get");
            assert!(
                get_messages
                    .iter()
                    .any(|m| m["body"]["status"] == "submitted")
            );

            let mut duplicate = registration_options();
            duplicate["excludeCredentials"] =
                serde_json::json!([{"type":"public-key","id":credential_id}]);
            let mut exclusion_lines = Vec::new();
            let exclusion = execute_to_pipe(
                prepare("create", duplicate, &psl).await,
                &mut channel,
                |line| {
                    exclusion_lines.push(line.to_vec());
                    Ok(())
                },
            )
            .await;
            assert!(matches!(
                exclusion,
                Err(OperationFailure::CredentialExcluded)
            ));
            let messages: Vec<serde_json::Value> = exclusion_lines
                .iter()
                .map(|line| serde_json::from_slice(line).unwrap())
                .collect();
            assert_eq!(
                messages
                    .iter()
                    .filter(|message| message["kind"] == "error")
                    .count(),
                1
            );
            assert_eq!(
                messages.last().unwrap()["body"]["code"],
                "credential_excluded"
            );
            assert!(messages.iter().all(|message| message["kind"] != "result"));

            channel.close().await;
        });
    }

    #[test]
    fn t067a_supported_virtual_channel_executes_shared_operations() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime");
        runtime.block_on(async {
            let psl = DatFilePublicSuffixList::from_path(FIXTURE_PSL)
                .expect("controlled official PSL fixture");
            let create_qr = CableQrCodeDevice::new_transient(
                QrCodeOperationHint::MakeCredential,
                CableTransports::CloudAssistedOnly,
            )
            .expect("transient create QR");
            let get_qr = CableQrCodeDevice::new_transient(
                QrCodeOperationHint::GetAssertionRequest,
                CableTransports::CloudAssistedOnly,
            )
            .expect("transient get QR");
            assert_eq!(create_qr.to_string(), "CableQrCodeDevice");
            assert_eq!(get_qr.to_string(), "CableQrCodeDevice");

            let mut device = get_virtual_device();
            let mut channel = device
                .channel(ChannelSettings::default())
                .await
                .expect("supported in-process virtual HID channel");
            let mut progress = Vec::new();
            let created = execute_prepared(
                prepare("create", registration_options(), &psl).await,
                &mut channel,
                |event| {
                    progress.push(event);
                    Ok(())
                },
            )
            .await
            .expect("real library make-credential operation");
            assert_eq!(created.response["kind"], "result");
            assert_eq!(created.response["body"]["type"], "public-key");
            let credential_id = created.response["body"]["id"]
                .as_str()
                .expect("serialized dynamic credential id");
            assert_eq!(created.response["id"], "t067a-create");

            let get_options = serde_json::json!({
                "challenge": "EBESExQVFhcYGRobHB0eHw",
                "rpId": "example.org",
                "allowCredentials": [{"type": "public-key", "id": credential_id}],
                "userVerification": "preferred"
            });
            let asserted = execute_prepared(
                prepare("get", get_options, &psl).await,
                &mut channel,
                |event| {
                    progress.push(event);
                    Ok(())
                },
            )
            .await
            .expect("real library get-assertion operation");
            assert_eq!(asserted.response["kind"], "result");
            assert_eq!(
                asserted.response["body"]["assertions"]
                    .as_array()
                    .map(Vec::len),
                Some(1)
            );
            assert_eq!(asserted.response["id"], "t067a-get");
            assert_eq!(
                progress,
                [
                    OperationProgress::Submitted,
                    OperationProgress::Completed,
                    OperationProgress::Submitted,
                    OperationProgress::Completed
                ]
            );
            channel.close().await;
        });
    }

    #[test]
    fn t068_transient_helper_writes_no_pairing_metadata() {
        const CHILD: &str = "FERRIC_T068_DISPOSABLE_CHILD";
        if env::var_os(CHILD).is_some() {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("test runtime");
            runtime.block_on(async {
                let qr = CableQrCodeDevice::new_transient(
                    QrCodeOperationHint::MakeCredential,
                    CableTransports::CloudAssistedOnly,
                )
                .expect("transient nonpersistent QR device");
                assert_eq!(qr.to_string(), "CableQrCodeDevice");

                let psl = DatFilePublicSuffixList::from_path(FIXTURE_PSL)
                    .expect("controlled official PSL fixture");
                let mut device = get_virtual_device();
                let mut channel = device
                    .channel(ChannelSettings::default())
                    .await
                    .expect("test-only virtual HID channel");
                let mut lines = Vec::new();
                execute_to_pipe(
                    prepare("create", registration_options(), &psl).await,
                    &mut channel,
                    |line| {
                        lines.push(line.to_vec());
                        Ok(())
                    },
                )
                .await
                .expect("real transient create flow");
                assert!(decoded(&lines).iter().any(|line| line["kind"] == "result"));
                channel.close().await;
            });
            return;
        }

        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("monotonic fixture time")
            .as_nanos();
        let root = env::temp_dir().join(format!("ferric-p07b-t068-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&root).expect("disposable observer root");
        for name in ["home", "config", "data", "cache", "state", "runtime", "tmp"] {
            fs::create_dir_all(root.join(name)).expect("disposable profile/XDG/output root");
        }
        let status = Command::new(env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "hybrid::tests::t068_transient_helper_writes_no_pairing_metadata",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env("HOME", root.join("home"))
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("XDG_DATA_HOME", root.join("data"))
            .env("XDG_CACHE_HOME", root.join("cache"))
            .env("XDG_STATE_HOME", root.join("state"))
            .env("XDG_RUNTIME_DIR", root.join("runtime"))
            .env("TMPDIR", root.join("tmp"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("isolated transient helper subprocess");
        assert!(status.success(), "child test exited with {status}");
        assert!(
            scan_files(&root)
                .expect("observe disposable roots")
                .is_empty(),
            "transient operation must not persist pairing or credential metadata"
        );
        fs::remove_dir_all(&root).expect("remove only this disposable fixture root");
    }

    #[test]
    fn t069_uncertain_create_is_not_replayed() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime");
        runtime.block_on(async {
            let psl = DatFilePublicSuffixList::from_path(FIXTURE_PSL)
                .expect("controlled official PSL fixture");
            let mut device = get_virtual_device();
            let mut channel = device
                .channel(ChannelSettings::default())
                .await
                .expect("test-only virtual HID channel");
            let mut submissions = 0;
            let mut terminal_attempts = 0;
            let mut lost_reply = None;
            let result = execute_to_pipe(
                prepare("create", registration_options(), &psl).await,
                &mut channel,
                |line| {
                    let message: serde_json::Value =
                        serde_json::from_slice(line).expect("typed private-pipe envelope");
                    if message["kind"] == "status" && message["body"]["status"] == "submitted" {
                        submissions += 1;
                    }
                    if message["kind"] == "result" {
                        terminal_attempts += 1;
                        lost_reply = Some(message);
                        return Err(io::Error::new(
                            io::ErrorKind::BrokenPipe,
                            "injected reply loss",
                        ));
                    }
                    Ok(())
                },
            )
            .await;
            assert!(matches!(result, Err(OperationFailure::Output)));
            assert_eq!(submissions, 1, "there was exactly one device submission");
            assert_eq!(
                terminal_attempts, 1,
                "the terminal response was attempted once"
            );
            let lost = lost_reply.expect("actual create completed before response loss");
            let credential_id = lost["body"]["id"]
                .as_str()
                .expect("dynamic device credential ID")
                .to_owned();

            let options = serde_json::json!({
                "challenge": "EBESExQVFhcYGRobHB0eHw",
                "rpId": "example.org",
                "allowCredentials": [{"type": "public-key", "id": credential_id}],
                "userVerification": "preferred"
            });
            let mut get_lines = Vec::new();
            execute_to_pipe(prepare("get", options, &psl).await, &mut channel, |line| {
                get_lines.push(line.to_vec());
                Ok(())
            })
            .await
            .expect("get confirms the create side effect remained on the authenticator");
            let get_result = decoded(&get_lines)
                .into_iter()
                .find(|message| message["kind"] == "result")
                .expect("get terminal envelope");
            assert_eq!(
                get_result["body"]["assertions"].as_array().map(Vec::len),
                Some(1)
            );
            channel.close().await;
        });
    }
}

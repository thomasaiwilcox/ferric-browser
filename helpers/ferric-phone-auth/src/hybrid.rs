use serde_json::{Value, json};

use crate::admission::{PreparedCredentialRequest, PreparedRequest};
use libwebauthn::ops::webauthn::WebAuthnIDLResponse;
use libwebauthn::webauthn::WebAuthn;

const MAX_TERMINAL_RESPONSE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperationProgress {
    Submitted,
    Completed,
}

#[derive(Debug)]
pub(crate) enum OperationFailure {
    Authenticator,
    Serialization,
    ResponseTooLarge,
}

#[derive(Debug)]
pub(crate) struct OperationResult {
    pub(crate) id: String,
    pub(crate) response: Value,
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
    F: FnMut(OperationProgress),
{
    let (id, operation) = prepared.into_parts();
    progress(OperationProgress::Submitted);
    let body = match operation {
        PreparedRequest::Create(request) => {
            let response = channel
                .webauthn_make_credential(&request)
                .await
                .map_err(|_| OperationFailure::Authenticator)?;
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
            json!({"assertions": assertions})
        }
    };
    let response = json!({
        "v": 1,
        "id": id,
        "kind": "result",
        "body": body
    });
    let encoded = serde_json::to_vec(&response).map_err(|_| OperationFailure::Serialization)?;
    if encoded.len() > MAX_TERMINAL_RESPONSE_BYTES {
        return Err(OperationFailure::ResponseTooLarge);
    }
    progress(OperationProgress::Completed);
    Ok(OperationResult { id, response })
}

#[cfg(test)]
mod tests {
    use super::{OperationProgress, execute_prepared};
    use crate::admission::{PreparedCredentialRequest, prepare_test_request_with_psl};
    use crate::protocol::parse_envelope;
    use libwebauthn::ops::webauthn::DatFilePublicSuffixList;
    use libwebauthn::transport::cable::qr_code_device::{
        CableQrCodeDevice, CableTransports, QrCodeOperationHint,
    };
    use libwebauthn::transport::{Channel, ChannelSettings, Device};
    use libwebauthn_tests::virt::get_virtual_device;

    const FIXTURE_PSL: &str =
        "/home/tom/Projects/ferric-everyday-work/builds/P06-fixtures/public_suffix_list.dat";

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

    #[test]
    fn t067a_supported_virtual_channel_executes_shared_operations() {
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

            let mut progress = Vec::new();
            let created = execute_prepared(
                prepare("create", registration_options(), &psl).await,
                &mut channel,
                |event| progress.push(event),
            )
            .await
            .expect("real library make-credential operation");
            assert_eq!(created.response["kind"], "result");
            assert_eq!(created.response["body"]["type"], "public-key");
            let credential_id = created.response["body"]["id"]
                .as_str()
                .expect("serialized dynamic credential id");
            assert_eq!(created.id, "t067a-create");

            let get_options = serde_json::json!({
                "challenge": "EBESExQVFhcYGRobHB0eHw",
                "rpId": "example.org",
                "allowCredentials": [{"type": "public-key", "id": credential_id}],
                "userVerification": "preferred"
            });
            let asserted = execute_prepared(
                prepare("get", get_options, &psl).await,
                &mut channel,
                |event| progress.push(event),
            )
            .await
            .expect("real library get-assertion operation");
            assert_eq!(asserted.response["kind"], "result");
            assert_eq!(
                asserted.response["body"]["assertions"]
                    .as_array()
                    .map(|values| values.len()),
                Some(1)
            );
            assert_eq!(asserted.id, "t067a-get");
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
}

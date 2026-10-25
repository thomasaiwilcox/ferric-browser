use std::str::FromStr;

use libwebauthn::ops::webauthn::{
    GetAssertionRequest, MakeCredentialRequest, OriginValidation, RelatedOrigins, RequestOrigin,
    RequestSettings, Scheme, SystemPublicSuffixList,
};
use serde_json::{Map, Value};

use crate::errors::AdmissionError;
use crate::protocol::{Envelope, Operation};

#[derive(Debug)]
enum PreparedRequest {
    Create(MakeCredentialRequest),
    Get(GetAssertionRequest),
}

/// Only a fully admitted, library-prepared request can inhabit this type.
/// Fields and constructors remain private to the helper library.
#[derive(Debug)]
pub(crate) struct PreparedCredentialRequest {
    id: String,
    request: PreparedRequest,
}

impl PreparedCredentialRequest {
    pub(crate) fn into_transport_unavailable(self) -> (String, Operation) {
        let operation = match self.request {
            PreparedRequest::Create(request) => {
                drop(request);
                Operation::Create
            }
            PreparedRequest::Get(request) => {
                drop(request);
                Operation::Get
            }
        };
        (self.id, operation)
    }
}

pub(crate) fn prepare_request(
    envelope: Envelope,
) -> Result<PreparedCredentialRequest, AdmissionError> {
    prepare_request_with_psl_loader(envelope, SystemPublicSuffixList::auto)
}

fn prepare_request_with_psl_loader<P, E>(
    envelope: Envelope,
    load_psl: impl FnOnce() -> Result<P, E>,
) -> Result<PreparedCredentialRequest, AdmissionError>
where
    P: libwebauthn::ops::webauthn::PublicSuffixList,
{
    let psl = require_psl(load_psl())?;
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .map_err(|_| AdmissionError::InvalidRequest)?
        .block_on(prepare_with_psl(envelope, &psl))
}

fn require_psl<T, E>(result: Result<T, E>) -> Result<T, AdmissionError> {
    result.map_err(|_| AdmissionError::PublicSuffixListUnavailable)
}

async fn prepare_with_psl(
    envelope: Envelope,
    psl: &dyn libwebauthn::ops::webauthn::PublicSuffixList,
) -> Result<PreparedCredentialRequest, AdmissionError> {
    // No transport can proceed until C07 network enforcement is implemented.
    let _network_policy = envelope.network;
    let request_origin =
        RequestOrigin::from_str(&envelope.origin).map_err(|_| AdmissionError::InvalidOrigin)?;
    // libwebauthn permits http://localhost for development; production helper
    // admission explicitly requires HTTPS.
    if request_origin.origin.scheme != Scheme::Https || request_origin.is_cross_origin() {
        return Err(AdmissionError::InvalidOrigin);
    }
    let host = request_origin.origin.host.as_str();
    let host_without_ipv6_brackets = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);
    if host == "localhost"
        || host.ends_with(".localhost")
        || host_without_ipv6_brackets
            .parse::<std::net::IpAddr>()
            .is_ok()
    {
        return Err(AdmissionError::InvalidOrigin);
    }

    let options = envelope.options.into_value();
    validate_options(envelope.op, &options)?;
    let options_json =
        serde_json::to_string(&options).map_err(|_| AdmissionError::InvalidEnvelope)?;
    let settings = RequestSettings {
        origin: OriginValidation::Validate {
            public_suffix_list: psl,
            related_origins: RelatedOrigins::Disabled,
        },
    };

    let request = match envelope.op {
        Operation::Create => PreparedRequest::Create(
            MakeCredentialRequest::prepare(&request_origin, &options_json, &settings)
                .await
                .map_err(|_| AdmissionError::InvalidRequest)?,
        ),
        Operation::Get => PreparedRequest::Get(
            GetAssertionRequest::prepare(&request_origin, &options_json, &settings)
                .await
                .map_err(|_| AdmissionError::InvalidRequest)?,
        ),
    };

    Ok(PreparedCredentialRequest {
        id: envelope.id,
        request,
    })
}

fn object(value: &Value) -> Result<&Map<String, Value>, AdmissionError> {
    value.as_object().ok_or(AdmissionError::InvalidRequest)
}

fn keys(value: &Value, allowed: &[&str]) -> Result<(), AdmissionError> {
    if object(value)?
        .keys()
        .all(|key| allowed.contains(&key.as_str()))
    {
        Ok(())
    } else {
        Err(AdmissionError::UnsupportedOptions)
    }
}

fn nested_keys(value: &Value, key: &str, allowed: &[&str]) -> Result<(), AdmissionError> {
    if let Some(value) = object(value)?.get(key) {
        keys(value, allowed)?;
    }
    Ok(())
}

fn array_nested_keys(value: &Value, key: &str, allowed: &[&str]) -> Result<(), AdmissionError> {
    if let Some(items) = object(value)?.get(key) {
        let items = items.as_array().ok_or(AdmissionError::InvalidRequest)?;
        for item in items {
            keys(item, allowed)?;
        }
    }
    Ok(())
}

fn validate_options(operation: Operation, options: &Value) -> Result<(), AdmissionError> {
    match operation {
        Operation::Create => validate_create(options),
        Operation::Get => validate_get(options),
    }
}

fn validate_create(options: &Value) -> Result<(), AdmissionError> {
    keys(
        options,
        &[
            "rp",
            "user",
            "challenge",
            "pubKeyCredParams",
            "timeout",
            "excludeCredentials",
            "authenticatorSelection",
            "hints",
            "attestation",
            "attestationFormats",
            "extensions",
        ],
    )?;
    nested_keys(options, "rp", &["id", "name"])?;
    nested_keys(options, "user", &["id", "name", "displayName"])?;
    array_nested_keys(options, "pubKeyCredParams", &["type", "alg"])?;
    array_nested_keys(options, "excludeCredentials", &["type", "id", "transports"])?;
    enum_value(
        options,
        "attestation",
        &["none", "indirect", "direct", "enterprise"],
    )?;
    if let Some(selection) = object(options)?.get("authenticatorSelection") {
        keys(
            selection,
            &[
                "authenticatorAttachment",
                "residentKey",
                "requireResidentKey",
                "userVerification",
            ],
        )?;
        if selection
            .get("authenticatorAttachment")
            .and_then(Value::as_str)
            == Some("platform")
        {
            return Err(AdmissionError::UnsupportedOptions);
        }
        enum_value(selection, "authenticatorAttachment", &["cross-platform"])?;
        enum_value(
            selection,
            "residentKey",
            &["required", "preferred", "discouraged"],
        )?;
        enum_value(
            selection,
            "userVerification",
            &["required", "preferred", "discouraged"],
        )?;
    }
    if let Some(extensions) = object(options)?.get("extensions") {
        keys(
            extensions,
            &[
                "credProps",
                "credProtect",
                "credBlob",
                "largeBlob",
                "minPinLength",
                "hmacCreateSecret",
                "prf",
                "appidExclude",
            ],
        )?;
        nested_keys(extensions, "credProtect", &["policy", "enforcePolicy"])?;
        nested_keys(extensions, "largeBlob", &["support"])?;
        if let Some(large_blob) = object(extensions)?.get("largeBlob") {
            enum_value(large_blob, "support", &["preferred", "required"])?;
        }
        nested_keys(extensions, "prf", &["eval"])?;
        if let Some(prf) = object(extensions)?.get("prf") {
            nested_keys(prf, "eval", &["first", "second"])?;
        }
    }
    if object(options)?
        .get("attestationFormats")
        .is_some_and(|v| v != &Value::Array(vec![]))
    {
        return Err(AdmissionError::UnsupportedOptions);
    }
    Ok(())
}

fn validate_get(options: &Value) -> Result<(), AdmissionError> {
    keys(
        options,
        &[
            "challenge",
            "timeout",
            "rpId",
            "allowCredentials",
            "userVerification",
            "hints",
            "extensions",
        ],
    )?;
    enum_value(
        options,
        "userVerification",
        &["required", "preferred", "discouraged"],
    )?;
    array_nested_keys(options, "allowCredentials", &["type", "id", "transports"])?;
    if let Some(extensions) = object(options)?.get("extensions") {
        // hmacGetSecret is parsed by upstream but absent from its prepared request.
        keys(extensions, &["getCredBlob", "largeBlob", "prf", "appid"])?;
        nested_keys(extensions, "largeBlob", &["support", "read", "write"])?;
        if let Some(large_blob) = object(extensions)?.get("largeBlob") {
            enum_value(large_blob, "support", &["preferred", "required"])?;
        }
        nested_keys(extensions, "prf", &["eval", "evalByCredential"])?;
        if let Some(prf) = object(extensions)?.get("prf") {
            nested_keys(prf, "eval", &["first", "second"])?;
            if let Some(by_credential) = object(prf)?.get("evalByCredential") {
                for input in object(by_credential)?.values() {
                    keys(input, &["first", "second"])?;
                }
            }
        }
    }
    Ok(())
}

fn enum_value(value: &Value, key: &str, accepted: &[&str]) -> Result<(), AdmissionError> {
    if let Some(value) = object(value)?.get(key) {
        if !value
            .as_str()
            .is_some_and(|value| accepted.contains(&value))
        {
            return Err(AdmissionError::UnsupportedOptions);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{PreparedRequest, prepare_request_with_psl_loader, prepare_with_psl};
    use crate::errors::AdmissionError;
    use crate::protocol::parse_envelope;
    use libwebauthn::ops::webauthn::{
        AuthenticatorAttachment, DatFilePublicSuffixList, PublicSuffixList, ResidentKeyRequirement,
        UserVerificationRequirement,
    };

    const FIXTURE_PSL: &str =
        "/home/tom/Projects/ferric-everyday-work/builds/P06-fixtures/public_suffix_list.dat";

    fn envelope(op: &str, origin: &str, options: serde_json::Value) -> crate::protocol::Envelope {
        let value = serde_json::json!({
            "v": 1,
            "id": "opaque-request-1",
            "op": op,
            "origin": origin,
            "options": options,
            "network": {}
        });
        parse_envelope(value.to_string().as_bytes()).expect("strict test envelope")
    }

    fn make_options(rp_id: &str) -> serde_json::Value {
        serde_json::json!({
            "rp": {"id": rp_id, "name": "Example"},
            "user": {"id": "AQIDBA", "name": "alice", "displayName": "Alice"},
            "challenge": "AQIDBAUGBwgJCgsMDQ4PEA",
            "pubKeyCredParams": [{"type": "public-key", "alg": -7}],
            "excludeCredentials": [{"type": "public-key", "id": "AAECAw"}],
            "authenticatorSelection": {
                "authenticatorAttachment": "cross-platform",
                "residentKey": "required",
                "userVerification": "required"
            },
            "attestation": "none"
        })
    }

    fn fixture() -> DatFilePublicSuffixList {
        DatFilePublicSuffixList::from_path(FIXTURE_PSL)
            .expect("controlled external official PSL fixture is required")
    }

    fn run<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime")
            .block_on(future)
    }

    #[test]
    fn pinned_create_preparation_preserves_credential_semantics() {
        let psl = fixture();
        let prepared = run(prepare_with_psl(
            envelope(
                "create",
                "https://login.example.com",
                make_options("example.com"),
            ),
            &psl,
        ))
        .expect("valid RP parent domain");
        let PreparedRequest::Create(request) = prepared.request else {
            panic!("operation preserved");
        };
        assert_eq!(request.relying_party.id, "example.com");
        assert_eq!(
            request.challenge,
            [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
        );
        assert_eq!(request.user.id, [1, 2, 3, 4]);
        assert_eq!(request.exclude.as_ref().expect("exclude list").len(), 1);
        assert_eq!(request.resident_key, Some(ResidentKeyRequirement::Required));
        assert_eq!(
            request.authenticator_attachment,
            Some(AuthenticatorAttachment::CrossPlatform)
        );
        assert_eq!(
            request.user_verification,
            UserVerificationRequirement::Required
        );
        assert_eq!(request.algorithms.len(), 1);
    }

    #[test]
    fn get_parent_domain_and_idn_are_prepared_by_pinned_library() {
        let psl = fixture();
        assert_eq!(
            psl.registrable_domain("login.example.com").as_deref(),
            Some("example.com")
        );
        let options = serde_json::json!({
            "challenge": "AQIDBAUGBwgJCgsMDQ4PEA",
            "rpId": "example.com",
            "allowCredentials": [{"type": "public-key", "id": "AAECAw"}],
            "userVerification": "required"
        });
        let prepared = run(prepare_with_psl(
            envelope("get", "https://login.example.com", options),
            &psl,
        ))
        .expect("valid RP parent domain");
        let PreparedRequest::Get(request) = prepared.request else {
            panic!("operation preserved");
        };
        assert_eq!(request.relying_party_id, "example.com");
        assert_eq!(request.challenge.len(), 16);
        assert_eq!(request.allow.len(), 1);
        assert_eq!(
            request.user_verification,
            UserVerificationRequirement::Required
        );

        let default_port = serde_json::json!({
            "challenge": "AQIDBAUGBwgJCgsMDQ4PEA",
            "rpId": "example.com"
        });
        let normalized = run(prepare_with_psl(
            envelope("get", "https://login.example.com:443", default_port),
            &psl,
        ))
        .expect("default HTTPS port is canonicalized");
        let PreparedRequest::Get(request) = normalized.request else {
            panic!("operation preserved");
        };
        assert_eq!(request.origin, "https://login.example.com");

        let idn_options = serde_json::json!({
            "challenge": "AQIDBAUGBwgJCgsMDQ4PEA",
            "rpId": "例え.com"
        });
        let idn = run(prepare_with_psl(
            envelope("get", "https://login.例え.com", idn_options),
            &psl,
        ))
        .expect("IDN is normalized and checked by pinned URL/RP and PSL logic");
        let PreparedRequest::Get(request) = idn.request else {
            panic!("operation preserved");
        };
        assert_eq!(request.relying_party_id, "xn--r8jz45g.com");
    }

    #[test]
    fn public_suffix_sibling_and_non_https_origins_are_rejected() {
        let psl = fixture();
        let public_suffix = run(prepare_with_psl(
            envelope("create", "https://login.example.com", make_options("com")),
            &psl,
        ));
        assert!(matches!(public_suffix, Err(AdmissionError::InvalidRequest)));

        let sibling = run(prepare_with_psl(
            envelope(
                "create",
                "https://login.example.com",
                make_options("other.com"),
            ),
            &psl,
        ));
        assert!(matches!(sibling, Err(AdmissionError::InvalidRequest)));

        let insecure = run(prepare_with_psl(
            envelope("create", "http://localhost", make_options("localhost")),
            &psl,
        ));
        assert!(matches!(insecure, Err(AdmissionError::InvalidOrigin)));
        let opaque = run(prepare_with_psl(
            envelope(
                "create",
                "data:text/plain,opaque",
                make_options("example.com"),
            ),
            &psl,
        ));
        assert!(matches!(opaque, Err(AdmissionError::InvalidOrigin)));
    }

    #[test]
    fn unsupported_requirements_and_duplicate_keys_fail_closed() {
        let psl = fixture();
        let mut unsupported = make_options("example.com");
        unsupported["extensions"] = serde_json::json!({"unknownRequiredThing": true});
        assert!(matches!(
            run(prepare_with_psl(
                envelope("create", "https://example.com", unsupported),
                &psl,
            )),
            Err(AdmissionError::UnsupportedOptions)
        ));
        assert!(parse_envelope(
            br#"{"v":1,"id":"x","op":"get","origin":"https://example.com","options":{"challenge":"AQIDBAUGBwgJCgsMDQ4PEA","challenge":"other"},"network":{}}"#
        )
        .is_err());
        assert!(parse_envelope(
            br#"{"v":1,"id":"x","op":"get","origin":"https://example.com","options":{"challenge":"AQIDBAUGBwgJCgsMDQ4PEA"},"network":{"direct":true}}"#
        )
        .is_err());
    }

    #[test]
    fn missing_or_unusable_psl_maps_to_unavailable() {
        let valid_request = envelope(
            "get",
            "https://example.com",
            serde_json::json!({"challenge": "AQIDBAUGBwgJCgsMDQ4PEA"}),
        );
        assert!(matches!(
            prepare_request_with_psl_loader(valid_request, || {
                DatFilePublicSuffixList::from_path(
                    "/home/tom/Projects/ferric-everyday-work/builds/P06-fixtures/missing-psl.dat",
                )
            }),
            Err(AdmissionError::PublicSuffixListUnavailable)
        ));
    }

    #[test]
    fn invalid_enums_and_deep_json_do_not_fall_through_pinned_defaults() {
        let psl = fixture();
        let mut invalid_uv = serde_json::json!({"challenge": "AQIDBAUGBwgJCgsMDQ4PEA"});
        invalid_uv["userVerification"] = serde_json::json!("surprise");
        assert!(matches!(
            run(prepare_with_psl(
                envelope("get", "https://example.com", invalid_uv),
                &psl,
            )),
            Err(AdmissionError::UnsupportedOptions)
        ));

        let mut deep = serde_json::Value::Null;
        for _ in 0..34 {
            deep = serde_json::json!([deep]);
        }
        let raw = serde_json::json!({
            "v": 1,
            "id": "opaque-request-1",
            "op": "get",
            "origin": "https://example.com",
            "options": deep,
            "network": {}
        });
        assert!(parse_envelope(raw.to_string().as_bytes()).is_err());
    }
}

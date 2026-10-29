use std::collections::BTreeMap;

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::errors::AdmissionError;

pub(crate) const MAX_REQUEST_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_NATIVE_ID_BYTES: usize = 128;
const MAX_JSON_DEPTH: usize = 32;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Operation {
    Create,
    Get,
}

/// Only the trusted parent may admit direct egress. An empty policy retains
/// the disabled protocol behavior; proxy and unknown policies fail parsing.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NetworkPolicy {
    #[serde(default)]
    mode: Option<NetworkMode>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum NetworkMode {
    Direct,
}

impl NetworkPolicy {
    pub(crate) fn direct(&self) -> bool {
        self.mode == Some(NetworkMode::Direct)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Envelope {
    pub(crate) v: u8,
    pub(crate) id: String,
    pub(crate) op: Operation,
    pub(crate) origin: String,
    pub(crate) options: StrictJson,
    pub(crate) network: NetworkPolicy,
}

/// Recursively rejects duplicate keys before converting to serde_json::Value.
#[derive(Debug)]
pub(crate) enum StrictJson {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<Self>),
    Object(BTreeMap<String, Self>),
}

impl<'de> Deserialize<'de> for StrictJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StrictVisitor;
        impl<'de> Visitor<'de> for StrictVisitor {
            type Value = StrictJson;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a JSON value without duplicate object keys")
            }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(StrictJson::Null)
            }
            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(StrictJson::Null)
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(StrictJson::Bool(v))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(StrictJson::Number(v.into()))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(StrictJson::Number(v.into()))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
                serde_json::Number::from_f64(v)
                    .map(StrictJson::Number)
                    .ok_or_else(|| E::custom("non-finite JSON number"))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(StrictJson::String(v.to_owned()))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
                Ok(StrictJson::String(v))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = seq.next_element()? {
                    values.push(value);
                }
                Ok(StrictJson::Array(values))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut values = BTreeMap::new();
                while let Some((key, value)) = map.next_entry::<String, StrictJson>()? {
                    if values.insert(key, value).is_some() {
                        return Err(de::Error::custom("duplicate object key"));
                    }
                }
                Ok(StrictJson::Object(values))
            }
        }
        deserializer.deserialize_any(StrictVisitor)
    }
}

impl StrictJson {
    fn within_depth(&self, depth: usize) -> bool {
        if depth > MAX_JSON_DEPTH {
            return false;
        }
        match self {
            Self::Array(values) => values.iter().all(|value| value.within_depth(depth + 1)),
            Self::Object(values) => values.values().all(|value| value.within_depth(depth + 1)),
            Self::Null | Self::Bool(_) | Self::Number(_) | Self::String(_) => true,
        }
    }

    pub(crate) fn into_value(self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Bool(value) => Value::Bool(value),
            Self::Number(value) => Value::Number(value),
            Self::String(value) => Value::String(value),
            Self::Array(values) => Value::Array(values.into_iter().map(Self::into_value).collect()),
            Self::Object(values) => Value::Object(
                values
                    .into_iter()
                    .map(|(key, value)| (key, value.into_value()))
                    .collect(),
            ),
        }
    }
}

pub(crate) fn parse_envelope(line: &[u8]) -> Result<Envelope, AdmissionError> {
    if line.len() > MAX_REQUEST_BYTES {
        return Err(AdmissionError::InvalidEnvelope);
    }
    let envelope: Envelope =
        serde_json::from_slice(line).map_err(|_| AdmissionError::InvalidEnvelope)?;
    if envelope.v != 1
        || envelope.id.is_empty()
        || envelope.id.len() > MAX_NATIVE_ID_BYTES
        || !envelope.id.is_ascii()
        || envelope.origin.is_empty()
        || !envelope.options.within_depth(1)
    {
        return Err(AdmissionError::InvalidEnvelope);
    }
    Ok(envelope)
}

#[cfg(test)]
mod network_tests {
    use super::NetworkPolicy;

    #[test]
    fn only_explicit_direct_policy_enables_transport() {
        for disabled in ["{}", r#"{"mode":null}"#] {
            let policy: NetworkPolicy = serde_json::from_str(disabled).unwrap();
            assert!(!policy.direct());
        }
        let policy: NetworkPolicy = serde_json::from_str(r#"{"mode":"direct"}"#).unwrap();
        assert!(policy.direct());
        for invalid in [
            r#"{"mode":"proxy"}"#,
            r#"{"mode":"offline"}"#,
            r#"{"mode":"direct","proxy":"http://localhost"}"#,
            r#"{"mode":"direct","mode":"direct"}"#,
        ] {
            assert!(serde_json::from_str::<NetworkPolicy>(invalid).is_err());
        }
    }

    #[test]
    fn actual_transport_filters_reject_redirect_and_private_routes() {
        use libwebauthn::transport::cable::direct_egress::{
            public_address, relay_domain_allowed, relay_url_allowed,
        };
        for host in ["cable.ua5v.com", "cable.auth.com"] {
            assert!(relay_domain_allowed(host));
            assert!(relay_url_allowed(
                &format!("wss://{host}/cable/connect/opaque/opaque")
                    .parse()
                    .unwrap()
            ));
        }
        for url in [
            "ws://cable.ua5v.com/cable",
            "wss://cable.ua5v.com:444/cable",
            "wss://cable.ua5v.com.evil.example/cable",
            "wss://user@cable.auth.com/cable",
            "wss://cable.auth.com/cable?secret",
            "wss://cable.auth.com/cable#fragment",
            "wss://127.0.0.1/cable",
            "wss://[::1]/cable",
        ] {
            assert!(!relay_url_allowed(&url.parse().unwrap()), "{url}");
        }
        for address in [
            "0.0.0.0",
            "10.1.2.3",
            "100.64.0.1",
            "127.0.0.1",
            "169.254.1.1",
            "172.16.0.1",
            "192.168.1.1",
            "192.0.2.1",
            "198.18.0.1",
            "198.51.100.1",
            "203.0.113.1",
            "224.0.0.1",
            "255.255.255.255",
            "::",
            "::1",
            "fc00::1",
            "fe80::1",
            "ff02::1",
            "2001:db8::1",
            "2002:7f00:1::",
            "3fff::1",
            "::ffff:127.0.0.1",
        ] {
            assert!(!public_address(address.parse().unwrap()), "{address}");
        }
        for address in [
            "8.8.8.8",
            "1.1.1.1",
            "2606:4700:4700::1111",
            "2001:4860:4860::8888",
        ] {
            assert!(public_address(address.parse().unwrap()), "{address}");
        }
    }
}

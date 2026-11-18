//! Borrow untrusted JSON before allocating its bounded, known fields. In
//! particular, escaped strings cannot grow Serde's scratch buffer without a
//! wire-size check, and ignored envelope metadata never becomes a Value tree.
use super::{
    DecodedHintPayload as Payload, HintCandidate, MAX_HINT_CANDIDATES, Value, parse_hint_candidate,
};
use serde::{Deserialize, Deserializer, de};
use serde_json::value::RawValue;
use std::{collections::BTreeSet, fmt};

// Match the former Value decoder's recursion bound. RawValue's skipping
// implementation uses scratch proportional to nesting, so guard it first.
fn check_nesting(raw: &str) -> Result<(), String> {
    let mut depth = 0_u8;
    let mut quoted = false;
    let mut escaped = false;
    for byte in raw.bytes() {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
        } else {
            match byte {
                b'"' => quoted = true,
                b'[' | b'{' => {
                    depth += 1;
                    if depth >= 128 {
                        return Err("hint JSON nesting limit exceeded".into());
                    }
                }
                b']' | b'}' => {
                    depth = depth.saturating_sub(1);
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn string(raw: &RawValue, maximum: usize) -> Result<String, String> {
    // Every decoded UTF-8 byte needs at most six source bytes (\u00XX).
    // Check before unescaping, including before Serde allocates scratch.
    if !raw.get().starts_with('"') || raw.get().len() > maximum * 6 + 2 {
        return Err("hint string exceeds its bounded contract".into());
    }
    let value: String = serde_json::from_str(raw.get()).map_err(|error| error.to_string())?;
    if value.len() > maximum {
        return Err("hint string exceeds its bounded contract".into());
    }
    Ok(value)
}

fn key(raw: &RawValue) -> Result<Option<String>, String> {
    // The longest recognized key is related_element_id. Larger keys cannot
    // be recognized, and need not be unescaped or allocated to reject/ignore.
    if raw.get().len() > "related_element_id".len() * 6 + 2 {
        return Ok(None);
    }
    let value: String = serde_json::from_str(raw.get()).map_err(|error| error.to_string())?;
    Ok((value.len() <= "related_element_id".len()).then_some(value))
}

#[derive(Clone, Copy)]
enum Object {
    Candidate,
    Geometry,
    Presentation,
    Anchor,
}

impl Object {
    fn keys(self) -> &'static [&'static str] {
        match self {
            Self::Candidate => super::CANDIDATE_KEYS,
            Self::Geometry => &["x", "y", "width", "height"],
            Self::Presentation => &["anchor", "fragments"],
            Self::Anchor => &["x", "y"],
        }
    }

    fn field(self, key: &str, raw: &RawValue) -> Result<Value, String> {
        let value = match (self, key) {
            (Self::Candidate, "element_id" | "related_element_id") => {
                if key == "related_element_id" && raw.get() == "null" {
                    Value::Null
                } else {
                    if raw.get().len() > 10 || !raw.get().bytes().all(|byte| byte.is_ascii_digit())
                    {
                        return Err("hint element identity must be a 32-bit integer".into());
                    }
                    Value::from(serde_json::from_str::<u32>(raw.get()).map_err(|e| e.to_string())?)
                }
            }
            (Self::Candidate, "kind") => Value::String(string(raw, "focusable-surface".len())?),
            (Self::Candidate, "frame_path") => Value::String(string(raw, 256)?),
            (Self::Candidate, "frame_owner") => Value::String(string(raw, 42)?),
            (Self::Candidate, "text") => Value::String(string(raw, 2_048)?),
            (Self::Candidate, "href") => {
                if raw.get() == "null" {
                    Value::Null
                } else {
                    Value::String(string(raw, 8_192)?)
                }
            }
            (Self::Candidate, "visible") => Value::Bool(match raw.get() {
                "true" => true,
                "false" => false,
                _ => return Err("hint candidate visible must be a boolean".into()),
            }),
            (Self::Candidate, "geometry") => Self::Geometry.decode(raw)?,
            (Self::Candidate, "presentation") => {
                if raw.get() == "null" {
                    Value::Null
                } else {
                    Self::Presentation.decode(raw)?
                }
            }
            (Self::Geometry, "x" | "y" | "width" | "height") | (Self::Anchor, "x" | "y") => {
                if !matches!(raw.get().as_bytes().first(), Some(b'-' | b'0'..=b'9')) {
                    return Err("hint geometry must contain numbers".into());
                }
                // Keep exactly the existing JSON number rounding. The locked
                // feature set is std + raw_value (no arbitrary_precision or
                // float_roundtrip), whose number path needs no heap scratch.
                // Check the token type first: a string-shaped type error would
                // otherwise unescape attacker-sized text before failing.
                Value::Number(serde_json::from_str(raw.get()).map_err(|e| e.to_string())?)
            }
            (Self::Presentation, "anchor") => Self::Anchor.decode(raw)?,
            (Self::Presentation, "fragments") => {
                if !raw.get().starts_with('[') {
                    return Err("hint fragments must be an array".into());
                }
                let mut decoder = serde_json::Deserializer::from_str(raw.get());
                let fragments = decoder
                    .deserialize_seq(Fragments)
                    .map_err(|e| e.to_string())?;
                decoder.end().map_err(|e| e.to_string())?;
                Value::Array(fragments)
            }
            _ => return Err("hint candidate contains an unsupported field".into()),
        };
        Ok(value)
    }

    fn decode(self, raw: &RawValue) -> Result<Value, String> {
        if !raw.get().starts_with('{') {
            return Err("hint candidate geometry and presentation must be objects".into());
        }
        let mut decoder = serde_json::Deserializer::from_str(raw.get());
        let value = decoder.deserialize_map(self).map_err(|e| e.to_string())?;
        decoder.end().map_err(|e| e.to_string())?;
        Ok(value)
    }
}

impl<'de> de::Visitor<'de> for Object {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded hint object")
    }

    fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut object = serde_json::Map::new();
        while let Some(raw_key) = map.next_key::<&RawValue>()? {
            let name = key(raw_key)
                .map_err(de::Error::custom)?
                .ok_or_else(|| de::Error::custom("unsupported hint field"))?;
            if object.contains_key(&name) {
                return Err(de::Error::custom("duplicate hint field"));
            }
            if !self.keys().contains(&name.as_str()) {
                return Err(de::Error::custom("unsupported hint field"));
            }
            let raw = map.next_value::<&RawValue>()?;
            let value = self.field(&name, raw).map_err(de::Error::custom)?;
            object.insert(name, value);
        }
        Ok(Value::Object(object))
    }
}

struct Fragments;

impl<'de> de::Visitor<'de> for Fragments {
    type Value = Vec<Value>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("at most eight hint fragments")
    }

    fn visit_seq<A: de::SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        let mut rows = Vec::new();
        while rows.len() < 8 {
            let Some(raw) = sequence.next_element::<&RawValue>()? else {
                return Ok(rows);
            };
            rows.push(Object::Geometry.decode(raw).map_err(de::Error::custom)?);
        }
        reject_next(&mut sequence, "hint fragment limit exceeded")?;
        Ok(rows)
    }
}

// Error immediately on the next value, before parsing/skipping an attacker-
// sized rejected row. SeqAccess only invokes this seed when a value exists.
fn reject_next<'de, A: de::SeqAccess<'de>>(
    sequence: &mut A,
    message: &'static str,
) -> Result<(), A::Error> {
    struct Reject(&'static str);
    impl<'de> de::DeserializeSeed<'de> for Reject {
        type Value = ();
        fn deserialize<D: Deserializer<'de>>(self, _: D) -> Result<(), D::Error> {
            Err(de::Error::custom(self.0))
        }
    }
    sequence.next_element_seed(Reject(message))?;
    Ok(())
}

struct Rows;

impl<'de> de::Visitor<'de> for Rows {
    type Value = Vec<HintCandidate>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("at most 5000 hint candidates")
    }

    fn visit_seq<A: de::SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        let mut rows = Vec::new();
        let mut identities = BTreeSet::new();
        while rows.len() < MAX_HINT_CANDIDATES {
            let Some(raw) = sequence.next_element::<&RawValue>()? else {
                return Ok(rows);
            };
            let value = Object::Candidate.decode(raw).map_err(de::Error::custom)?;
            let candidate = parse_hint_candidate(&value).map_err(de::Error::custom)?;
            if !identities.insert((candidate.frame_owner.clone(), candidate.element_id)) {
                return Err(de::Error::custom(
                    "hint candidate element_id values must be unique",
                ));
            }
            rows.push(candidate);
        }
        reject_next(&mut sequence, "hint candidate limit exceeded")?;
        Ok(rows)
    }
}

impl<'de> Deserialize<'de> for Payload {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Envelope;
        impl<'de> de::Visitor<'de> for Envelope {
            type Value = Payload;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a hint candidate array or envelope")
            }
            fn visit_seq<A: de::SeqAccess<'de>>(self, sequence: A) -> Result<Payload, A::Error> {
                Ok(Payload {
                    candidates: de::Visitor::visit_seq(Rows, sequence)?,
                    selector_revision: None,
                    scope_lease_id: None,
                })
            }
            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Payload, A::Error> {
                let mut candidates = None;
                let mut selector_revision = None;
                let mut scope_lease_id = None;
                while let Some(raw_key) = map.next_key::<&RawValue>()? {
                    let name = key(raw_key).map_err(de::Error::custom)?;
                    match name.as_deref() {
                        Some("candidates") => {
                            if candidates.is_some() {
                                return Err(de::Error::custom("duplicate hint candidates"));
                            }
                            // Borrow before asking Serde for an array: a wrong-type
                            // escaped string must not allocate during a type error.
                            let raw: &RawValue = map.next_value()?;
                            if !raw.get().starts_with('[') {
                                return Err(de::Error::custom(
                                    "hint payload candidates must be an array",
                                ));
                            }
                            let mut decoder = serde_json::Deserializer::from_str(raw.get());
                            candidates =
                                Some(decoder.deserialize_seq(Rows).map_err(de::Error::custom)?);
                            decoder.end().map_err(de::Error::custom)?;
                        }
                        Some("selector_revision") => {
                            if selector_revision.is_some() {
                                return Err(de::Error::custom("duplicate selector revision"));
                            }
                            selector_revision =
                                Some(string(map.next_value()?, 16).map_err(de::Error::custom)?);
                        }
                        Some("scope_lease_id") => {
                            if scope_lease_id.is_some() {
                                return Err(de::Error::custom("duplicate scope lease"));
                            }
                            let raw: &RawValue = map.next_value()?;
                            // Preserve null as no lease; renderer strings still need exact
                            // equality with the native-issued current lease at the caller.
                            scope_lease_id = Some(if raw.get() == "null" {
                                None
                            } else {
                                Some(string(raw, 128).map_err(de::Error::custom)?)
                            });
                        }
                        _ => {
                            let _: &RawValue = map.next_value()?;
                        }
                    }
                }
                Ok(Payload {
                    candidates: candidates.ok_or_else(|| {
                        de::Error::custom("hint payload candidates must be an array")
                    })?,
                    selector_revision,
                    scope_lease_id: scope_lease_id.flatten(),
                })
            }
        }
        deserializer.deserialize_any(Envelope)
    }
}

pub(super) fn decode(raw: &str) -> Result<Payload, String> {
    if !matches!(raw.trim_start().as_bytes().first(), Some(b'[' | b'{')) {
        return Err("hint payload must be an array or object".into());
    }
    check_nesting(raw)?;
    serde_json::from_str(raw).map_err(|error| format!("hint candidate JSON is invalid: {error}"))
}

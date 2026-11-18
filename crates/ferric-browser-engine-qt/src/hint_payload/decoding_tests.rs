#![cfg(test)]

use super::*;
use serde_json::json;

fn row(id: usize) -> Value {
    json!({"element_id":id,"kind":"button","frame_path":"0","text":"Target",
        "href":null,"geometry":{"x":1,"y":2,"width":30,"height":20}})
}

#[test]
fn retains_full_ordinary_capacity_without_an_annotation_byte_cap() {
    let rows: Vec<_> = (1..=MAX_HINT_CANDIDATES)
        .map(|id| {
            let mut value = row(id);
            value["href"] = json!("x".repeat(8_192));
            value
        })
        .collect();
    let wire = serde_json::to_string(&rows).unwrap();
    assert!(wire.len() > 2 * 1024 * 1024);
    let parsed = parse_hint_payload(&wire).unwrap();
    assert_eq!(parsed.len(), MAX_HINT_CANDIDATES);
    assert_eq!(parsed.last().unwrap().href.as_ref().unwrap().len(), 8_192);
    assert_eq!(parsed.last().unwrap().element_id, 5_000);
}

#[test]
fn rejects_the_next_candidate_before_decoding_its_value() {
    let mut wire =
        serde_json::to_string(&(1..=MAX_HINT_CANDIDATES).map(row).collect::<Vec<_>>()).unwrap();
    wire.pop();
    wire.push_str(", {\"text\": \"unterminated");
    // An EOF/syntax error here would mean the rejected row was parsed first.
    assert!(
        parse_hint_payload(&wire)
            .unwrap_err()
            .contains("candidate limit exceeded")
    );
}

#[test]
fn preserves_unicode_and_escaped_strings_at_exact_field_limits() {
    for text in ["a".repeat(512), "é".repeat(512), "😀".repeat(512)] {
        let mut value = row(1);
        value["text"] = json!(text);
        let raw = value.to_string();
        let escaped = raw
            .replace('a', "\\u0061")
            .replace('é', "\\u00e9")
            .replace('😀', "\\ud83d\\ude00");
        for wire in [raw, escaped] {
            assert_eq!(
                parse_hint_payload(&format!("[{wire}]")).unwrap()[0].text,
                text
            );
        }
    }
    for text in ["a".repeat(513), "é".repeat(513), "😀".repeat(513)] {
        let mut value = row(1);
        value["text"] = json!(text);
        assert!(parse_hint_payload(&format!("[{value}]")).is_err());
    }
    let mut value = row(1);
    value["href"] = json!("é".repeat(4096));
    let escaped = value.to_string().replace('é', "\\u00e9");
    assert_eq!(
        parse_hint_payload(&format!("[{escaped}]")).unwrap()[0]
            .href
            .as_ref()
            .unwrap()
            .len(),
        8_192
    );
    value["href"] = json!("é".repeat(4097));
    assert!(parse_hint_payload(&format!("[{value}]")).is_err());
}

#[test]
fn rejects_duplicate_recognized_fields_instead_of_collapsing_them() {
    let value = row(1).to_string();
    for wire in [
        format!("[{}]", value.replacen("\"element_id\":1", "\"element_id\":2,\"element_id\":1", 1)),
        format!("[{}]", value.replacen("\"x\":1", "\"x\":2,\"x\":1", 1)),
        format!("{{\"candidates\":[],\"candidates\":[{value}]}}"),
        format!("{{\"candidates\":[],\"candi\\u0064ates\":[{value}]}}"),
        "{\"candidates\":[],\"selector_revision\":\"0000000000000000\",\"selector_revision\":\"0000000000000000\"}".into(),
        "{\"candidates\":[],\"scope_lease_id\":null,\"scope_lease_id\":null}".into(),
    ] {
        assert!(parse_hint_payload(&wire).unwrap_err().contains("duplicate"), "{wire}");
    }
}

#[test]
fn rejects_non_string_href_instead_of_treating_it_as_absent() {
    for invalid in [json!(1), json!(false), json!([]), json!({})] {
        let mut value = row(1);
        value["href"] = invalid;
        assert!(parse_hint_candidate(&value).is_err());
        assert!(parse_hint_payload(&format!("[{value}]")).is_err());
    }
    for remove in [false, true] {
        let mut value = row(1);
        if remove {
            value.as_object_mut().unwrap().remove("href");
        }
        assert!(
            parse_hint_payload(&format!("[{value}]")).unwrap()[0]
                .href
                .is_none()
        );
    }
}

#[test]
fn ignores_unbounded_envelope_metadata_without_retain_or_key_allocation() {
    let ignored = "\\u0061".repeat(100_000);
    let wire = format!(
        "{{\"{ignored}\":\"{ignored}\",\"candi\\u0064ates\":[{}],\"another_unknown_key_longer_than_recognized\":{{\"text\":\"{ignored}\"}}}}",
        row(1)
    );
    let parsed = parse_hint_envelope(&wire).unwrap();
    assert_eq!(parsed.candidates.len(), 1);
    assert!(parsed.selector_revision.is_none());
    assert!(parsed.scope_lease_id.is_none());
    let malformed = format!("{{\"candidates\":[{}],\"ignored\": [1,]}}", row(1));
    assert!(parse_hint_envelope(&malformed).is_err());
    assert!(parse_hint_envelope(&format!("{wire} null")).is_err());
}

#[test]
fn rejects_oversized_escaped_candidate_keys_and_values_with_bounded_errors() {
    let huge = "\\u0061".repeat(100_000);
    for wire in [
        format!("[{{\"{huge}\":null}}]"),
        format!(
            "[{}]",
            row(1)
                .to_string()
                .replace("\"Target\"", &format!("\"{huge}\""))
        ),
        format!(
            "[{}]",
            row(1)
                .to_string()
                .replace("\"href\":null", &format!("\"href\":\"{huge}\""))
        ),
    ] {
        let error = parse_hint_payload(&wire).unwrap_err();
        assert!(error.len() < 512);
        assert!(!error.contains(&huge[..120]));
    }
}

#[test]
fn bounds_envelope_identity_fields_and_preserves_their_exact_values() {
    let wire = json!({"candidates":[row(1)],"selector_revision":"0123456789abcdef","scope_lease_id":"s".repeat(128)}).to_string();
    let parsed = parse_hint_envelope(&wire).unwrap();
    assert_eq!(
        parsed.selector_revision.as_deref(),
        Some("0123456789abcdef")
    );
    assert_eq!(parsed.scope_lease_id.unwrap().len(), 128);
    for (name, value) in [
        ("selector_revision", "r".repeat(17)),
        ("scope_lease_id", "s".repeat(129)),
    ] {
        let mut envelope = json!({"candidates":[]});
        envelope[name] = json!(value);
        assert!(parse_hint_envelope(&envelope.to_string()).is_err());
    }
}

#[test]
fn caps_fragments_before_parsing_the_rejected_fragment() {
    let mut value = row(1);
    let rect = value["geometry"].clone();
    value["presentation"] = json!({"anchor":{"x":1,"y":2},"fragments":vec![rect;8]});
    assert_eq!(
        parse_hint_payload(&format!("[{value}]")).unwrap()[0]
            .presentation
            .as_ref()
            .unwrap()
            .fragments
            .len(),
        8
    );
    value["presentation"]["fragments"]
        .as_array_mut()
        .unwrap()
        .push(json!("invalid ninth geometry"));
    let invalid = format!("[{value}]");
    assert!(
        parse_hint_payload(&invalid)
            .unwrap_err()
            .contains("fragment limit exceeded")
    );
}

#[test]
fn nesting_guard_matches_the_former_value_decoder_boundary() {
    for depth in [126, 127, 128, 129, 10_000] {
        let raw = format!(
            "{{\"candidates\":[],\"ignored\":{}0{}}}",
            "[".repeat(depth - 1),
            "]".repeat(depth - 1)
        );
        assert_eq!(
            parse_hint_payload(&raw).is_ok(),
            serde_json::from_str::<Value>(&raw).is_ok(),
            "depth {depth}"
        );
    }
    let quoted = json!({"candidates":[], "ignored":"[\\\"{".repeat(10_000)}).to_string();
    assert_eq!(parse_hint_payload(&quoted).unwrap().len(), 0);
}

#[test]
fn oversized_wrong_type_strings_do_not_escape_field_or_diagnostic_bounds() {
    let huge = format!("\"{}\"", "\\u0061".repeat(100_000));
    let base = row(1).to_string();
    let mut wires = vec![huge.clone(), format!("{{\"candidates\":{huge}}}")];
    for (name, old) in [
        ("element_id", "1"),
        ("geometry", "{\"height\":20,\"width\":30,\"x\":1,\"y\":2}"),
        ("visible", "true"),
        ("presentation", "null"),
    ] {
        let mut value = row(1);
        if name == "visible" {
            value[name] = json!(true);
        }
        if name == "presentation" {
            value[name] = Value::Null;
        }
        let raw = value.to_string();
        let needle = format!("\"{name}\":{old}");
        assert!(raw.contains(&needle));
        wires.push(format!(
            "[{}]",
            raw.replace(&needle, &format!("\"{name}\":{huge}"))
        ));
    }
    for body in [
        format!("\"anchor\":{huge},\"fragments\":[]"),
        format!("\"anchor\":{{\"x\":1,\"y\":2}},\"fragments\":{huge}"),
    ] {
        wires.push(format!(
            "[{}]",
            base.replacen('}', &format!("}},\"presentation\":{{{body}}}"), 1)
        ));
    }
    for wire in wires {
        let error = parse_hint_payload(&wire).unwrap_err();
        assert!(error.len() < 512);
    }
}

#[test]
fn geometry_numbers_preserve_the_existing_json_decoder_rounding() {
    for numeric in [
        "9007199254740995e-1",
        "0.123456789012345678901234567890",
        "-0.0",
        "1.234567890123456e+150",
        "1e-300",
    ] {
        let raw = row(1)
            .to_string()
            .replace("\"x\":1", &format!("\"x\":{numeric}"));
        let old: Value = serde_json::from_str(&raw).unwrap();
        let expected = parse_hint_candidate(&old).unwrap();
        let actual = parse_hint_payload(&format!("[{raw}]")).unwrap();
        assert_eq!(
            actual[0].geometry.x.to_bits(),
            expected.geometry.x.to_bits(),
            "{numeric}"
        );
    }
    // Finite geometry has no arbitrary numeric-spelling cap.
    let raw = row(1)
        .to_string()
        .replace("\"x\":1", &format!("\"x\":1.{}", "0".repeat(100_000)));
    assert_eq!(
        parse_hint_payload(&format!("[{raw}]")).unwrap()[0]
            .geometry
            .x
            .to_bits(),
        1.0_f64.to_bits()
    );
}

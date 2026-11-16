//! Decoded-value and finite-range boundaries from the Interaction contract.
use ferric_browser_config::{InteractionNumericRange, InteractionPredicates};
use serde_json::{Value, json};

fn validate(value: Value) -> bool {
    serde_json::from_value::<InteractionPredicates>(value)
        .is_ok_and(|predicates| predicates.validate().is_ok())
}

#[test]
fn decoded_multibyte_match_and_token_bounds_are_exact() {
    for bytes in [255, 256, 257] {
        let value = format!("{}{}", "é".repeat(bytes / 2), "x".repeat(bytes % 2));
        assert_eq!(value.len(), bytes);
        for operator in ["equals", "contains"] {
            let mut predicate = json!({"hint_name":{}});
            predicate["hint_name"][operator] = json!(value);
            assert_eq!(validate(predicate), bytes <= 256, "name {operator}/{bytes}");
        }
        for operator in ["equals", "prefix"] {
            let mut predicate = json!({"attributes":[{"name":"data-id"}]});
            predicate["attributes"][0][operator] = json!(value);
            assert_eq!(
                validate(predicate),
                bytes <= 256,
                "attribute {operator}/{bytes}"
            );
        }
        assert_eq!(validate(json!({"role":value})), bytes <= 256);
        assert_eq!(
            validate(json!({"attributes":[{"name":value,"present":true}]})),
            bytes <= 256
        );
        // Bound decoded UTF-8, not JSON escape syntax or source character count.
        let escaped = format!("{}{}", "\\u00e9".repeat(bytes / 2), "x".repeat(bytes % 2));
        let encoded = format!(r#"{{"hint_name":{{"equals":"{escaped}"}}}}"#);
        let decoded: InteractionPredicates = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.validate().is_ok(), bytes <= 256);
        assert_eq!(decoded.hint_name.unwrap().equals.unwrap(), value);
    }
}

#[test]
fn every_range_uses_inclusive_finite_ordered_bounds() {
    for (field, ceiling) in [
        ("width_px", 1_000_000_f64),
        ("height_px", 1_000_000_f64),
        ("viewport_x", 1_f64),
        ("viewport_y", 1_f64),
    ] {
        for (min, max, accepted) in [
            (0., 0., true),
            (-0., ceiling, true),
            (ceiling, ceiling, true),
            (0., ceiling, true),
            (-f64::MIN_POSITIVE, ceiling, false),
            (0., f64::from_bits(ceiling.to_bits() + 1), false),
            (ceiling, 0., false),
            (f64::NAN, ceiling, false),
            (0., f64::NAN, false),
            (f64::NEG_INFINITY, ceiling, false),
            (0., f64::INFINITY, false),
        ] {
            let mut predicates = InteractionPredicates::default();
            let range = Some(InteractionNumericRange { min, max });
            match field {
                "width_px" => predicates.width_px = range,
                "height_px" => predicates.height_px = range,
                "viewport_x" => predicates.viewport_x = range,
                "viewport_y" => predicates.viewport_y = range,
                _ => unreachable!(),
            }
            assert_eq!(
                predicates.validate().is_ok(),
                accepted,
                "{field}/{min}/{max}"
            );
        }
    }
}

#[test]
fn href_is_exact_origin_and_path_only_without_an_invented_match_value_cap() {
    for origin in [
        "https://example.test",
        "http://example.test:8766",
        "https://[::1]",
    ] {
        assert!(validate(json!({"href":{"origin":origin}})));
    }
    for origin in [
        "https://EXAMPLE.test",
        "https://example.test:443",
        "https://example.test/",
        "https://user@example.test",
        "null",
    ] {
        assert!(!validate(json!({"href":{"origin":origin}})), "{origin}");
    }
    for path in ["/", "/watch*", "/literal%2F*", "/literal[brackets]"] {
        assert!(validate(json!({"href":{"path":path}})));
    }
    for path in [
        "",
        "relative",
        "/watch?q=x",
        "/watch#x",
        "/white space",
        "/control\n",
    ] {
        assert!(!validate(json!({"href":{"path":path}})), "{path:?}");
    }
    assert!(validate(
        json!({"href":{"path":format!("/{}", "x".repeat(512))}})
    ));
    assert!(!validate(json!({"href":{}})));
    assert!(!validate(json!({"href":{"path":"/","query":"x"}})));
}

//! Explicit P02 contracts: invocation families are not discovery-selector
//! families, and structural collections have no implicit item/primary choice.
use ferric_browser_config::{
    HintSelectorFamily, InteractionCollection, InteractionCollectionOrder, InteractionFamily,
    InteractionPolicyPatch,
};
use serde_json::{Value, json};

fn collection() -> Value {
    json!({"id":"items", "families":["links"], "codes":["1","2"],
        "container_selector":"#items", "item_selector":".item",
        "primary_selector":"a.primary", "order":"dom"})
}

fn validate_collection(value: &Value) -> Result<(), String> {
    let patch: InteractionPolicyPatch = serde_json::from_value(json!({"collections":[value]}))
        .map_err(|error| error.to_string())?;
    patch.validate_authored().map(|_| ())
}

#[test]
fn invocation_family_schema_is_distinct_and_explicit() {
    use InteractionFamily as F;
    for (name, expected, selector_family) in [
        ("all", F::All, true),
        ("actions", F::Actions, false),
        ("focus", F::Focus, false),
        ("links", F::Links, true),
        ("inputs", F::Inputs, true),
        ("buttons", F::Buttons, true),
        ("images", F::Images, true),
        ("media", F::Media, true),
        ("scrollables", F::Scrollables, true),
        ("regions", F::Regions, false),
        ("surfaces", F::Surfaces, false),
    ] {
        let family: InteractionFamily = serde_json::from_value(json!(name)).unwrap();
        assert_eq!(family, expected);
        assert_eq!(serde_json::to_value(family).unwrap(), json!(name));
        assert_eq!(
            serde_json::from_value::<HintSelectorFamily>(json!(name)).is_ok(),
            selector_family
        );
        let mut value = collection();
        value["families"] = json!([name]);
        validate_collection(&value).unwrap();
    }
    for enabled in [false, true] {
        for families in [json!([]), json!(["links", "links"]), json!(["unknown"])] {
            for kind in ["anchors", "collections"] {
                let entry = if kind == "anchors" {
                    json!({"id":"anchor", "code":"m", "families":families, "target_selector":"a"})
                } else {
                    let mut value = collection();
                    value["families"] = families.clone();
                    value
                };
                let mut value = json!({"enabled":enabled});
                value[kind] = json!([entry]);
                let rejected = serde_json::from_value::<InteractionPolicyPatch>(value)
                    .map_or(true, |patch| patch.validate_authored().is_err());
                assert!(rejected, "{kind}, enabled={enabled}, families={families}");
            }
        }
    }
}

#[test]
fn collection_selection_and_identity_shapes_are_explicit() {
    validate_collection(&collection()).unwrap();
    let mut own = collection();
    own.as_object_mut().unwrap().remove("container_selector");
    own.as_object_mut().unwrap().remove("primary_selector");
    own["container_self"] = json!(true);
    own["primary_self"] = json!(true);
    validate_collection(&own).unwrap();
    for field in [
        "container_selector",
        "item_selector",
        "primary_selector",
        "order",
    ] {
        let mut missing = collection();
        missing.as_object_mut().unwrap().remove(field);
        assert!(validate_collection(&missing).is_err(), "missing {field}");
    }
    for (selector, own) in [
        ("container_selector", "container_self"),
        ("primary_selector", "primary_self"),
    ] {
        let mut both = collection();
        both[own] = json!(true);
        assert!(validate_collection(&both).is_err(), "both {selector}/{own}");
        let mut disabled_self = collection();
        disabled_self.as_object_mut().unwrap().remove(selector);
        disabled_self[own] = json!(false);
        assert!(validate_collection(&disabled_self).is_err(), "false {own}");
    }
    for attributes in [
        json!([]),
        json!(["data-a", "data-b", "data-c"]),
        json!(["data-a", "data-b", "data-c", "data-d"]),
    ] {
        let mut value = collection();
        value["identity_attributes"] = attributes;
        validate_collection(&value).unwrap();
    }
    for attributes in [
        json!(["data-a", "data-a"]),
        json!(["Data-ID"]),
        json!([""]),
        json!(["data-a", "data-b", "data-c", "data-d", "data-e"]),
    ] {
        let mut value = collection();
        value["identity_attributes"] = attributes;
        assert!(validate_collection(&value).is_err());
    }
    let mut unsupported = collection();
    unsupported["whole_card_visible_fraction"] = json!(0.5);
    assert!(validate_collection(&unsupported).is_err());
}

#[test]
fn collection_orders_and_tolerance_bounds_are_exact() {
    use InteractionCollectionOrder as O;
    for (name, order) in [
        ("dom", O::Dom),
        ("top-to-bottom", O::TopToBottom),
        ("row-major-ltr", O::RowMajorLtr),
        ("row-major-rtl", O::RowMajorRtl),
    ] {
        let mut value = collection();
        value["order"] = json!(name);
        let mut entry: InteractionCollection = serde_json::from_value(value).unwrap();
        assert_eq!(entry.order, order);
        for tolerance in [
            None,
            Some(0.),
            Some(8.),
            Some(32.),
            Some(-0.01),
            Some(32.01),
            Some(f64::NAN),
            Some(f64::INFINITY),
            Some(f64::NEG_INFINITY),
        ] {
            entry.row_tolerance_px = tolerance;
            let patch = InteractionPolicyPatch {
                collections: Some(vec![entry.clone()]),
                ..Default::default()
            };
            let expected = tolerance.is_none_or(|value| {
                matches!(order, O::RowMajorLtr | O::RowMajorRtl)
                    && value.is_finite()
                    && (0.0..=32.0).contains(&value)
            });
            assert_eq!(
                patch.validate_authored().is_ok(),
                expected,
                "{name} {tolerance:?}"
            );
        }
    }
}

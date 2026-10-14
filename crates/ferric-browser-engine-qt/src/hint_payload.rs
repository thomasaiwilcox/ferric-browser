//! Validation and presentation serialization for page-script hint results.
//!
//! Page data is untrusted. This module is the sole decoder for the bounded
//! result contract emitted by `BrowserScripts.js`.

use ferric_browser_core::{
    HintCandidate, HintGeometry, HintKind, HintPoint, HintPresentationGeometry, LabeledHint,
    MAX_HINT_CANDIDATES,
};
use serde_json::Value;
use std::collections::BTreeSet;

const MAX_HINT_FRAME_DEPTH: usize = 8;

pub(super) fn parse_hint_candidate(value: &Value) -> Result<HintCandidate, String> {
    const CANDIDATE_KEYS: &[&str] = &[
        "element_id",
        "related_element_id",
        "kind",
        "frame_path",
        "frame_owner",
        "text",
        "href",
        "geometry",
        "visible",
        "presentation",
    ];

    let object = value
        .as_object()
        .ok_or_else(|| "hint candidate must be an object".to_owned())?;
    if object
        .keys()
        .any(|key| !CANDIDATE_KEYS.contains(&key.as_str()))
    {
        return Err("hint candidate contains an unsupported field".into());
    }
    if object
        .get("visible")
        .is_some_and(|visible| !visible.is_boolean())
    {
        return Err("hint candidate visible must be a boolean".into());
    }
    let kind = parse_hint_kind(
        object
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| "hint candidate kind must be a string".to_owned())?,
    )?;
    let element_id = object
        .get("element_id")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| "hint candidate element_id must be a positive 32-bit integer".to_owned())?;
    let related_element_id = match object.get("related_element_id") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .filter(|value| *value > 0)
                .ok_or_else(|| {
                    "hint candidate related_element_id must be a positive 32-bit integer".to_owned()
                })?,
        ),
    };
    let is_pair = matches!(kind, HintKind::EditorFocus | HintKind::LabelControl);
    if is_pair != related_element_id.is_some()
        || related_element_id.is_some_and(|related| related == element_id)
    {
        return Err("hint candidate has an invalid related element identity".into());
    }
    let frame_path = object
        .get("frame_path")
        .and_then(Value::as_str)
        .ok_or_else(|| "hint candidate frame_path must be a string".to_owned())?;
    let frame_owner = match object.get("frame_owner") {
        None => "",
        Some(value) => value.as_str().ok_or("hint frame owner must be a string")?,
    };
    if !frame_owner.is_empty()
        && (frame_owner.len() != 42
            || !frame_owner.starts_with("frame-")
            || !frame_owner[6..]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() || byte == b'-'))
    {
        return Err("invalid hint frame owner".into());
    }
    let text = object
        .get("text")
        .and_then(Value::as_str)
        .ok_or_else(|| "hint candidate text must be a string".to_owned())?;
    if frame_path.is_empty()
        || frame_path.len() > 256
        || !valid_hint_frame_path(frame_path)
        || text.chars().count() > 512
        || text.len() > 2_048
        || frame_path.chars().any(char::is_control)
        || text.chars().any(char::is_control)
    {
        return Err("hint candidate text or frame path is invalid".into());
    }
    let href = object
        .get("href")
        .and_then(|href| if href.is_null() { None } else { href.as_str() })
        .map(str::to_owned);
    if href.as_ref().is_some_and(|href| {
        href.is_empty() || href.len() > 8_192 || href.chars().any(char::is_control)
    }) {
        return Err("hint candidate href is invalid".into());
    }
    let geometry = parse_geometry(object.get("geometry").ok_or("hint geometry is missing")?)?;
    let presentation = match object.get("presentation") {
        None | Some(Value::Null) => None,
        Some(value) => Some(parse_presentation(value, geometry)?),
    };
    Ok(HintCandidate {
        element_id,
        related_element_id,
        kind,
        frame_path: frame_path.to_owned(),
        frame_owner: frame_owner.to_owned(),
        text: text.to_owned(),
        href,
        geometry,
        presentation,
    })
}

fn parse_geometry(value: &Value) -> Result<HintGeometry, String> {
    const GEOMETRY_KEYS: &[&str] = &["x", "y", "width", "height"];
    let geometry = value
        .as_object()
        .ok_or_else(|| "hint candidate geometry must be an object".to_owned())?;
    if geometry.len() != GEOMETRY_KEYS.len()
        || geometry
            .keys()
            .any(|key| !GEOMETRY_KEYS.contains(&key.as_str()))
    {
        return Err("hint candidate geometry contains unsupported fields".into());
    }
    let number = |name: &str| {
        geometry
            .get(name)
            .and_then(Value::as_f64)
            .ok_or_else(|| format!("hint candidate geometry {name} must be a number"))
    };
    let geometry = HintGeometry {
        x: number("x")?,
        y: number("y")?,
        width: number("width")?,
        height: number("height")?,
    };
    if !geometry.x.is_finite()
        || !geometry.y.is_finite()
        || !geometry.width.is_finite()
        || !geometry.height.is_finite()
        || geometry.width <= 0.0
        || geometry.height <= 0.0
    {
        return Err("hint candidate geometry is invalid".into());
    }
    Ok(geometry)
}

fn parse_presentation(
    value: &Value,
    bounds: HintGeometry,
) -> Result<HintPresentationGeometry, String> {
    let object = value
        .as_object()
        .ok_or("hint presentation must be an object")?;
    if object.len() != 2 || !object.contains_key("anchor") || !object.contains_key("fragments") {
        return Err("hint presentation contains unsupported fields".into());
    }
    let point = object["anchor"]
        .as_object()
        .ok_or("hint anchor must be an object")?;
    if point.len() != 2 || !point.contains_key("x") || !point.contains_key("y") {
        return Err("hint anchor contains unsupported fields".into());
    }
    let anchor = HintPoint {
        x: point["x"]
            .as_f64()
            .ok_or("hint anchor x must be a number")?,
        y: point["y"]
            .as_f64()
            .ok_or("hint anchor y must be a number")?,
    };
    let rows = object["fragments"]
        .as_array()
        .ok_or("hint fragments must be an array")?;
    if rows.is_empty() || rows.len() > 8 {
        return Err("hint fragments exceed the bounded contract".into());
    }
    let fragments = rows
        .iter()
        .map(parse_geometry)
        .collect::<Result<Vec<_>, _>>()?;
    let inside = |x: f64, y: f64, rect: HintGeometry| {
        x.is_finite()
            && y.is_finite()
            && x >= rect.x - 1e-6
            && y >= rect.y - 1e-6
            && x <= rect.x + rect.width + 1e-6
            && y <= rect.y + rect.height + 1e-6
    };
    if fragments
        .iter()
        .any(|r| !inside(r.x, r.y, bounds) || !inside(r.x + r.width, r.y + r.height, bounds))
        || !fragments.iter().any(|r| inside(anchor.x, anchor.y, *r))
    {
        return Err("hint presentation lies outside its target geometry".into());
    }
    Ok(HintPresentationGeometry { anchor, fragments })
}

pub(super) fn parse_hint_payload(raw: &str) -> Result<Vec<HintCandidate>, String> {
    let values = serde_json::from_str::<Value>(raw)
        .map_err(|error| format!("hint candidate JSON is invalid: {error}"))?;
    let values = if let Some(values) = values.as_array() {
        values
    } else {
        let object = values
            .as_object()
            .ok_or_else(|| "hint payload must be an array or object".to_owned())?;
        object
            .get("candidates")
            .and_then(Value::as_array)
            .ok_or_else(|| "hint payload candidates must be an array".to_owned())?
    };
    if values.len() > MAX_HINT_CANDIDATES {
        return Err(format!(
            "hint candidate count {} exceeds {MAX_HINT_CANDIDATES}",
            values.len()
        ));
    }
    let candidates = values
        .iter()
        .map(parse_hint_candidate)
        .collect::<Result<Vec<_>, _>>()?;
    let unique_ids = candidates
        .iter()
        .map(|candidate| (&candidate.frame_owner, candidate.element_id))
        .collect::<BTreeSet<_>>();
    if unique_ids.len() != candidates.len() {
        return Err("hint candidate element_id values must be unique".into());
    }
    Ok(candidates)
}

pub(super) fn validate_hint_candidates_for_family(
    family: &str,
    candidates: &[HintCandidate],
) -> Result<(), String> {
    let (maximum, expected_kind): (Option<usize>, fn(HintKind) -> bool) = match family {
        "regions" => (Some(ferric_browser_core::MAX_HINT_REGIONS), |kind| {
            matches!(kind, HintKind::Region | HintKind::ScrollableRegion)
        }),
        "surfaces" => (Some(ferric_browser_core::MAX_HINT_SURFACES), |kind| {
            matches!(
                kind,
                HintKind::Surface | HintKind::FocusableSurface | HintKind::Frame
            )
        }),
        _ => (None, |kind| {
            kind == HintKind::Frame || !ferric_browser_core::hint_kind_is_region_or_surface(kind)
        }),
    };
    if maximum.is_some_and(|maximum| candidates.len() > maximum)
        || candidates.iter().any(|candidate| {
            !expected_kind(candidate.kind)
                || maximum.is_some()
                    && (candidate.href.is_some() || candidate.related_element_id.is_some())
        })
    {
        return Err(format!(
            "{family} candidates exceed their typed family contract"
        ));
    }
    Ok(())
}

pub(super) fn hint_kind_name(kind: HintKind) -> &'static str {
    match kind {
        HintKind::Link => "link",
        HintKind::Button => "button",
        HintKind::Input => "input",
        HintKind::Select => "select",
        HintKind::Textarea => "textarea",
        HintKind::ContentEditable => "contenteditable",
        HintKind::Image => "image",
        HintKind::Media => "media",
        HintKind::Scrollable => "scrollable",
        HintKind::Aria => "aria",
        HintKind::Focusable => "focusable",
        HintKind::Frame => "frame",
        HintKind::EditorFocus => "editor-focus",
        HintKind::LabelControl => "label-control",
        HintKind::Region => "region",
        HintKind::ScrollableRegion => "scrollable-region",
        HintKind::Surface => "surface",
        HintKind::FocusableSurface => "focusable-surface",
    }
}

fn hint_action_label(kind: HintKind, rapid: bool, target: &str) -> &'static str {
    use ferric_browser_core::{HintIntent, hint_intent_for_candidate};
    if target == "pointer" {
        return "Pointer click · Enter confirms";
    }
    if target == "choose" {
        return "Choose action";
    }
    let uses_url = super::hint_policy::hint_uses_url_action(kind, rapid, target, false);
    match hint_intent_for_candidate(kind, uses_url) {
        HintIntent::OpenUrl => match target {
            "current" => "Open link",
            "tab" => "Open in new tab",
            "tab-bg" => "Open in background tab",
            "window" => "Open in new window",
            "yank" => "Copy link",
            "clean-yank" => "Copy clean link",
            "download" => "Download",
            "userscript" => "Run userscript",
            "ephemeral" => "Open private window",
            _ => "Open externally",
        },
        HintIntent::Focus => "Focus",
        HintIntent::SetScrollTarget => "Set scroll target",
        HintIntent::ActionPalette => "Choose action",
        _ => "Activate",
    }
}

pub(super) fn hint_json(hints: &[LabeledHint], rapid: bool, target: &str) -> Value {
    Value::Array(
        hints
            .iter()
            .map(|hint| {
                serde_json::json!({
                    "label": hint.label,
                    "element_id": hint.candidate.element_id,
                    "related_element_id": hint.candidate.related_element_id,
                    "kind": hint_kind_name(hint.candidate.kind),
                    "frame_path": hint.candidate.frame_path,
                    "frame_owner": hint.candidate.frame_owner,
                    "text": hint.candidate.text,
                    "href": hint.candidate.href,
                    "action": hint_action_label(hint.candidate.kind, rapid, target),
                    "presentation": hint.candidate.presentation.as_ref().map(|presentation| serde_json::json!({
                        "anchor":{"x":presentation.anchor.x,"y":presentation.anchor.y},
                        "fragments":presentation.fragments.iter().map(|r|serde_json::json!({"x":r.x,"y":r.y,"width":r.width,"height":r.height})).collect::<Vec<_>>()
                    })),
                    "x": hint.candidate.geometry.x,
                    "y": hint.candidate.geometry.y,
                    "width": hint.candidate.geometry.width,
                    "height": hint.candidate.geometry.height
                })
            })
            .collect(),
    )
}

pub(super) fn valid_hint_frame_path(path: &str) -> bool {
    let mut segments = path.split('.');
    if segments.next() != Some("0") {
        return false;
    }
    let mut depth = 0;
    segments.all(|segment| {
        depth += 1;
        depth <= MAX_HINT_FRAME_DEPTH
            && !segment.is_empty()
            && segment.parse::<usize>().is_ok_and(|index| index <= 5_000)
    })
}

fn parse_hint_kind(value: &str) -> Result<HintKind, String> {
    match value {
        "link" => Ok(HintKind::Link),
        "button" => Ok(HintKind::Button),
        "input" => Ok(HintKind::Input),
        "select" => Ok(HintKind::Select),
        "textarea" => Ok(HintKind::Textarea),
        "contenteditable" => Ok(HintKind::ContentEditable),
        "image" => Ok(HintKind::Image),
        "media" => Ok(HintKind::Media),
        "scrollable" => Ok(HintKind::Scrollable),
        "aria" => Ok(HintKind::Aria),
        "focusable" => Ok(HintKind::Focusable),
        "frame" => Ok(HintKind::Frame),
        "editor-focus" => Ok(HintKind::EditorFocus),
        "label-control" => Ok(HintKind::LabelControl),
        "region" => Ok(HintKind::Region),
        "scrollable-region" => Ok(HintKind::ScrollableRegion),
        "surface" => Ok(HintKind::Surface),
        "focusable-surface" => Ok(HintKind::FocusableSurface),
        _ => Err(format!("unknown hint kind: {value}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_duplicate_or_unsafe_page_script_candidates() {
        let candidate = serde_json::json!({
            "element_id": 1,
            "kind": "link",
            "frame_path": "0.1",
            "text": "Example",
            "href": "https://example.test/",
            "geometry": {"x": 1.0, "y": 2.0, "width": 10.0, "height": 10.0}
        });
        assert!(parse_hint_candidate(&candidate).is_ok());
        assert!(!valid_hint_frame_path("1.0"));
        assert!(!valid_hint_frame_path("0.1.2.3.4.5.6.7.8.9"));
        assert!(
            parse_hint_payload(
                &serde_json::json!({
                    "candidates": [candidate.clone(), candidate]
                })
                .to_string()
            )
            .is_err()
        );
    }

    #[test]
    fn accepts_bounded_unicode_text_by_byte_and_character_count() {
        let candidate = |text: String| {
            serde_json::json!({
                "element_id": 1,
                "kind": "link",
                "frame_path": "0",
                "text": text,
                "href": "https://example.test/",
                "geometry": {"x": 1.0, "y": 2.0, "width": 10.0, "height": 10.0}
            })
        };

        assert!(parse_hint_candidate(&candidate("é".repeat(512))).is_ok());
        assert!(parse_hint_candidate(&candidate("é".repeat(513))).is_err());
        assert!(parse_hint_candidate(&candidate("😀".repeat(512))).is_ok());
        assert!(parse_hint_candidate(&candidate("😀".repeat(513))).is_err());
        assert!(parse_hint_candidate(&candidate("unsafe\u{0085}text".into())).is_err());
    }

    #[test]
    fn frame_owners_namespace_ids_and_reject_invalid_leases() {
        let row = serde_json::json!({"element_id":1,"kind":"button","frame_path":"0","text":"Accept","href":null,"geometry":{"x":0,"y":0,"width":100,"height":30}});
        let mut child = row.clone();
        child["frame_owner"] = "frame-01234567-89ab-cdef-0123-456789abcdef".into();
        assert_eq!(
            parse_hint_payload(&serde_json::json!([row, child.clone()]).to_string())
                .unwrap()
                .len(),
            2
        );
        assert!(
            parse_hint_payload(&serde_json::json!([child.clone(), child.clone()]).to_string())
                .is_err()
        );
        child["frame_owner"] = "https://example.com/frame".into();
        assert!(parse_hint_candidate(&child).is_err());
    }

    #[test]
    fn rejects_extra_authority_fields_and_invalid_geometry() {
        let mut value = serde_json::json!({
            "element_id": 1,
            "kind": "button",
            "frame_path": "0",
            "text": "Safe label",
            "href": null,
            "geometry": {"x": 1.0, "y": 2.0, "width": 10.0, "height": 10.0},
            "visible": true
        });
        assert!(parse_hint_candidate(&value).is_ok());

        value["native_action"] = serde_json::json!("click");
        assert!(parse_hint_candidate(&value).is_err());
        value.as_object_mut().unwrap().remove("native_action");

        value["visible"] = serde_json::json!("yes");
        assert!(parse_hint_candidate(&value).is_err());
        value["visible"] = serde_json::json!(false);

        value["geometry"]["native_action"] = serde_json::json!(true);
        assert!(parse_hint_candidate(&value).is_err());
        value["geometry"]
            .as_object_mut()
            .unwrap()
            .remove("native_action");
        value["geometry"]["width"] = serde_json::json!(0.0);
        assert!(parse_hint_candidate(&value).is_err());
    }

    #[test]
    fn accepts_only_closed_typed_region_and_surface_kinds() {
        let candidate = |kind: &str| {
            serde_json::json!({
                "element_id": 1,
                "kind": kind,
                "frame_path": "0",
                "text": "Main region",
                "href": null,
                "geometry": {"x": 1.0, "y": 2.0, "width": 16.0, "height": 16.0}
            })
        };
        for (name, expected) in [
            ("region", HintKind::Region),
            ("scrollable-region", HintKind::ScrollableRegion),
            ("surface", HintKind::Surface),
            ("focusable-surface", HintKind::FocusableSurface),
        ] {
            let parsed = parse_hint_candidate(&candidate(name)).expect("typed facet candidate");
            assert_eq!(parsed.kind, expected);
            assert_eq!(hint_kind_name(parsed.kind), name);
        }
        let mut candidate = candidate("surface");
        candidate["capability"] = serde_json::json!("focus");
        assert!(parse_hint_candidate(&candidate).is_err());
    }

    #[test]
    fn enforces_family_caps_and_rejects_facet_authority_outside_explicit_families() {
        let candidate = |element_id, kind| HintCandidate {
            element_id,
            related_element_id: None,
            kind,
            frame_path: "0".into(),
            frame_owner: String::new(),
            text: "Target".into(),
            href: None,
            presentation: None,
            geometry: HintGeometry {
                x: 0.0,
                y: 0.0,
                width: 16.0,
                height: 16.0,
            },
        };
        assert!(
            validate_hint_candidates_for_family("regions", &[candidate(1, HintKind::Region)])
                .is_ok()
        );
        assert!(
            validate_hint_candidates_for_family(
                "surfaces",
                &[candidate(1, HintKind::FocusableSurface)]
            )
            .is_ok()
        );
        assert!(
            validate_hint_candidates_for_family("all", &[candidate(1, HintKind::Surface)]).is_err()
        );
        assert!(
            validate_hint_candidates_for_family("regions", &[candidate(1, HintKind::Surface)])
                .is_err()
        );

        let too_many = (1..=257)
            .map(|id| candidate(id, HintKind::Region))
            .collect::<Vec<_>>();
        assert!(validate_hint_candidates_for_family("regions", &too_many).is_err());

        let mut with_href = candidate(1, HintKind::Surface);
        with_href.href = Some("https://untrusted.example/".into());
        assert!(validate_hint_candidates_for_family("surfaces", &[with_href]).is_err());
    }

    #[test]
    fn pair_candidates_require_a_distinct_retained_related_element() {
        let mut value = serde_json::json!({
            "element_id": 4,
            "related_element_id": 9,
            "kind": "editor-focus",
            "frame_path": "0",
            "text": "Editor",
            "href": null,
            "geometry": {"x": 1.0, "y": 2.0, "width": 10.0, "height": 10.0}
        });
        assert_eq!(
            parse_hint_candidate(&value)
                .expect("well-formed exact pair")
                .related_element_id,
            Some(9)
        );
        value["related_element_id"] = serde_json::json!(4);
        assert!(parse_hint_candidate(&value).is_err());
        value["related_element_id"] = serde_json::json!(null);
        assert!(parse_hint_candidate(&value).is_err());
        value["kind"] = serde_json::json!("input");
        value["related_element_id"] = serde_json::json!(9);
        assert!(parse_hint_candidate(&value).is_err());
    }
    #[test]
    fn presentation_is_bounded_and_cannot_supply_an_outside_anchor() {
        let mut value = serde_json::json!({
            "anchor":{"x":2.0,"y":3.0},
            "fragments":[{"x":1.0,"y":2.0,"width":10.0,"height":10.0}]
        });
        let bounds = HintGeometry {
            x: 1.0,
            y: 2.0,
            width: 10.0,
            height: 10.0,
        };
        assert_eq!(
            parse_presentation(&value, bounds).unwrap().fragments.len(),
            1
        );
        value["anchor"]["x"] = serde_json::json!(99.0);
        assert!(parse_presentation(&value, bounds).is_err());
        value["anchor"]["x"] = serde_json::json!(2.0);
        value["fragments"] = serde_json::json!(vec![
            serde_json::json!({"x":1,"y":2,"width":10,"height":10});
            9
        ]);
        assert!(parse_presentation(&value, bounds).is_err());
        value["fragments"] = serde_json::json!([{ "x":0,"y":2,"width":10,"height":10 }]);
        assert!(parse_presentation(&value, bounds).is_err());
    }

    #[test]
    fn preview_action_uses_the_same_url_and_focus_policy_as_selection() {
        assert_eq!(
            hint_action_label(HintKind::Image, false, "current"),
            "Activate"
        );
        assert_eq!(
            hint_action_label(HintKind::Image, false, "yank"),
            "Copy link"
        );
        assert_eq!(
            hint_action_label(HintKind::Focusable, false, "current"),
            "Focus"
        );
        assert_eq!(
            hint_action_label(HintKind::Button, false, "pointer"),
            "Pointer click · Enter confirms"
        );
    }
}

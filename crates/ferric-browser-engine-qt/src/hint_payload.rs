//! Validation and presentation serialization for page-script hint results.
//!
//! Page data is untrusted. This module is the sole decoder for the bounded
//! result contract emitted by `BrowserScripts.js`.

use ferric_browser_core::{
    HintCandidate, HintGeometry, HintKind, LabeledHint, MAX_HINT_CANDIDATES,
};
use serde_json::Value;
use std::collections::BTreeSet;

const MAX_HINT_FRAME_DEPTH: usize = 8;

pub(super) fn parse_hint_candidate(value: &Value) -> Result<HintCandidate, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "hint candidate must be an object".to_owned())?;
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
    let frame_path = object
        .get("frame_path")
        .and_then(Value::as_str)
        .ok_or_else(|| "hint candidate frame_path must be a string".to_owned())?;
    let text = object
        .get("text")
        .and_then(Value::as_str)
        .ok_or_else(|| "hint candidate text must be a string".to_owned())?;
    if frame_path.is_empty()
        || frame_path.len() > 256
        || !valid_hint_frame_path(frame_path)
        || text.chars().count() > 512
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
    let geometry = object
        .get("geometry")
        .and_then(Value::as_object)
        .ok_or_else(|| "hint candidate geometry must be an object".to_owned())?;
    let number = |name: &str| {
        geometry
            .get(name)
            .and_then(Value::as_f64)
            .ok_or_else(|| format!("hint candidate geometry {name} must be a number"))
    };
    Ok(HintCandidate {
        element_id,
        kind,
        frame_path: frame_path.to_owned(),
        text: text.to_owned(),
        href,
        geometry: HintGeometry {
            x: number("x")?,
            y: number("y")?,
            width: number("width")?,
            height: number("height")?,
        },
    })
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
        .map(|candidate| candidate.element_id)
        .collect::<BTreeSet<_>>();
    if unique_ids.len() != candidates.len() {
        return Err("hint candidate element_id values must be unique".into());
    }
    Ok(candidates)
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
    }
}

pub(super) fn hint_json(hints: &[LabeledHint]) -> Value {
    Value::Array(
        hints
            .iter()
            .map(|hint| {
                serde_json::json!({
                    "label": hint.label,
                    "element_id": hint.candidate.element_id,
                    "kind": hint_kind_name(hint.candidate.kind),
                    "frame_path": hint.candidate.frame_path,
                    "text": hint.candidate.text,
                    "href": hint.candidate.href,
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
    fn accepts_bounded_unicode_text_by_character_count() {
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
        assert!(parse_hint_candidate(&candidate("unsafe\u{0085}text".into())).is_err());
    }
}

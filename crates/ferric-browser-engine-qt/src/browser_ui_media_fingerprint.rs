use crate::{BrowserUiRust, MediaFingerprintKeyWorker};
use ferric_browser_storage::MediaFingerprintKey;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use uuid::Uuid;

pub(super) struct NormalizedMediaPageState {
    pub(super) json: String,
    pub(super) source_urls_by_candidate: BTreeMap<u16, String>,
}

impl BrowserUiRust {
    pub(super) fn media_fingerprint_key_status(&mut self) -> &'static str {
        self.poll_media_fingerprint_key();
        let Some(profile_id) = self
            .profile_id
            .filter(|_| self.profile_persistence.is_durable())
        else {
            return "unavailable";
        };
        if self.media_fingerprint_key_for(profile_id).is_some() {
            return "ready";
        }
        if self
            .media_fingerprint_key_request
            .is_some_and(|(requested_profile, _)| requested_profile == profile_id)
            && self
                .media_fingerprint_key_worker
                .as_ref()
                .is_some_and(MediaFingerprintKeyWorker::is_pending)
        {
            return "pending";
        }
        "unavailable"
    }

    pub(super) fn media_fingerprint_key_is_pending(&self) -> bool {
        let Some(profile_id) = self
            .profile_id
            .filter(|_| self.profile_persistence.is_durable())
        else {
            return false;
        };
        self.media_fingerprint_key_request
            .is_some_and(|(requested_profile, _)| requested_profile == profile_id)
            && self
                .media_fingerprint_key_worker
                .as_ref()
                .is_some_and(MediaFingerprintKeyWorker::is_pending)
    }

    pub(super) fn request_media_fingerprint_key(&mut self, create_missing: bool) -> bool {
        self.poll_media_fingerprint_key();
        let Some(profile_id) = self
            .profile_id
            .filter(|_| self.profile_persistence.is_durable())
        else {
            return false;
        };
        if self
            .media_fingerprint_key
            .as_ref()
            .is_some_and(|(loaded_profile, key)| *loaded_profile == profile_id && key.is_current())
        {
            return true;
        }
        if self.media_fingerprint_key_failure == Some(profile_id) {
            if !create_missing {
                return false;
            }
            self.media_fingerprint_key_failure = None;
        }
        if self.media_fingerprint_key_request == Some((profile_id, create_missing)) {
            return false;
        }
        if self
            .media_fingerprint_key_worker
            .as_ref()
            .is_some_and(MediaFingerprintKeyWorker::is_pending)
        {
            return false;
        }
        let Some(roots) = self.storage_roots.clone() else {
            return false;
        };
        if self.media_fingerprint_key_worker.is_none() {
            self.media_fingerprint_key_worker = MediaFingerprintKeyWorker::spawn().ok();
        }
        let Some(worker) = self.media_fingerprint_key_worker.as_mut() else {
            return false;
        };
        if worker.request(roots, profile_id, create_missing).is_err() {
            return false;
        }
        self.media_fingerprint_key_request = Some((profile_id, create_missing));
        false
    }

    pub(super) fn poll_media_fingerprint_key(&mut self) {
        let Some(worker) = self.media_fingerprint_key_worker.as_mut() else {
            return;
        };
        let Some((profile_id, result)) = worker.poll() else {
            return;
        };
        if self
            .media_fingerprint_key_request
            .is_some_and(|(requested_profile, _)| requested_profile == profile_id)
        {
            self.media_fingerprint_key_request = None;
        }
        if self.profile_id == Some(profile_id) && self.profile_persistence.is_durable() {
            match result {
                Ok(key) => {
                    self.media_fingerprint_key = Some((profile_id, key));
                    self.media_fingerprint_key_failure = None;
                }
                Err(_) => {
                    self.media_fingerprint_key = None;
                    self.media_fingerprint_key_failure = Some(profile_id);
                }
            }
        }
    }

    pub(super) fn media_fingerprint_key_for(
        &self,
        profile_id: Uuid,
    ) -> Option<&MediaFingerprintKey> {
        if !self.profile_persistence.is_durable() || self.profile_id != Some(profile_id) {
            return None;
        }
        self.media_fingerprint_key
            .as_ref()
            .filter(|(loaded_profile, key)| *loaded_profile == profile_id && key.is_current())
            .map(|(_, key)| key)
    }

    pub(super) fn normalize_media_page_state(
        &mut self,
        raw_json: &str,
        create_missing_key: bool,
    ) -> Option<NormalizedMediaPageState> {
        if raw_json.is_empty() || raw_json.len() > 256 * 1024 {
            return None;
        }
        self.poll_media_fingerprint_key();
        let profile_id = self.profile_id;
        let key_ready = profile_id
            .and_then(|profile| self.media_fingerprint_key_for(profile))
            .is_some();
        if !key_ready && create_missing_key {
            self.request_media_fingerprint_key(true);
        }
        let key = profile_id.and_then(|profile| self.media_fingerprint_key_for(profile));
        normalize_media_page_state_json(raw_json, |source| {
            key.and_then(|key| key.fingerprint(source))
        })
    }
}

fn normalize_media_page_state_json(
    raw_json: &str,
    mut fingerprint_source: impl FnMut(&str) -> Option<String>,
) -> Option<NormalizedMediaPageState> {
    let mut value: Value = serde_json::from_str(raw_json).ok()?;
    let object = value.as_object_mut()?;
    let candidates = object.get_mut("candidates")?.as_array_mut()?;
    let mut kept_candidates = Vec::with_capacity(candidates.len());
    let mut kept_media_ids = BTreeMap::new();
    let mut dropped_frames = Vec::new();
    let mut source_urls_by_candidate = BTreeMap::new();

    for mut candidate in std::mem::take(candidates) {
        let is_media = candidate.get("kind").and_then(Value::as_str) == Some("media");
        if !is_media {
            kept_candidates.push(candidate);
            continue;
        }
        let Some(source_url) = candidate
            .get("source_fingerprint")
            .and_then(Value::as_str)
            .filter(|source| !source.is_empty() && source.len() <= 2048)
            .map(str::to_owned)
        else {
            if let Some(frame_id) = candidate.get("frame_id").and_then(Value::as_u64) {
                dropped_frames.push(frame_id);
            }
            continue;
        };
        let Some(fingerprint) = fingerprint_source(&source_url) else {
            if let Some(frame_id) = candidate.get("frame_id").and_then(Value::as_u64) {
                dropped_frames.push(frame_id);
            }
            continue;
        };
        let Some(candidate_id) = candidate
            .get("candidate_id")
            .and_then(Value::as_u64)
            .and_then(|candidate_id| u16::try_from(candidate_id).ok())
        else {
            continue;
        };
        let frame_id = candidate.get("frame_id").and_then(Value::as_u64);
        if let Some(candidate_object) = candidate.as_object_mut() {
            candidate_object.insert("source_fingerprint".to_owned(), json!(fingerprint));
        }
        if frame_id == Some(0) {
            source_urls_by_candidate.insert(candidate_id, source_url);
        }
        if let Some(frame_id) = frame_id {
            kept_media_ids.insert((frame_id, candidate_id), ());
        }
        kept_candidates.push(candidate);
    }
    object.insert("candidates".to_owned(), json!(kept_candidates));

    if let Some(positions) = object.get_mut("positions").and_then(Value::as_array_mut) {
        positions.retain(|position| {
            if position.get("kind").and_then(Value::as_str) != Some("media-time") {
                return true;
            }
            let candidate_id = position
                .get("candidate_id")
                .and_then(Value::as_u64)
                .and_then(|value| u16::try_from(value).ok());
            let frame_id = position.get("frame_id").and_then(Value::as_u64);
            frame_id
                .zip(candidate_id)
                .is_some_and(|key| kept_media_ids.contains_key(&key))
        });
    }
    let captured_media = object
        .get("positions")
        .and_then(Value::as_array)
        .map_or(0, |positions| {
            positions
                .iter()
                .filter(|position| {
                    position.get("kind").and_then(Value::as_str) == Some("media-time")
                })
                .count()
        });
    if let Some(coverage) = object.get_mut("coverage").and_then(Value::as_array_mut) {
        for entry in coverage {
            if entry.get("category").and_then(Value::as_str) == Some("media") {
                if let Some(entry) = entry.as_object_mut() {
                    entry.insert("captured".to_owned(), json!(captured_media));
                }
            }
        }
    }
    if !dropped_frames.is_empty() {
        if let Some(exclusions) = object.get_mut("exclusions").and_then(Value::as_array_mut) {
            for frame_id in dropped_frames {
                if exclusions.len() >= 128 {
                    break;
                }
                exclusions.push(json!({"frame_id": frame_id, "reason": "unsupported"}));
            }
        }
    }

    let json = serde_json::to_string(&value).ok()?;
    (json.len() <= 256 * 1024).then_some(NormalizedMediaPageState {
        json,
        source_urls_by_candidate,
    })
}

#[cfg(test)]
mod tests {
    use super::normalize_media_page_state_json;

    const SOURCE: &str = "https://media.example.test/private/lecture.mp4?token=secret";

    fn raw_observation() -> String {
        serde_json::json!({
            "schema_version": 1,
            "helper_version": "ferric-page-state-5",
            "nonce": "11111111111111111111111111111111",
            "capture_fingerprint": "2222222222222222222222222222222222222222222222222222222222222222",
            "requested_categories": ["geometry", "media"],
            "frames": [{
                "id": 0,
                "parent": null,
                "origin": "https://example.test",
                "route_fingerprint": "3333333333333333333333333333333333333333333333333333333333333333"
            }],
            "candidates": [{
                "frame_id": 0,
                "candidate_id": 1,
                "kind": "media",
                "id_fingerprint": "4444444444444444444444444444444444444444444444444444444444444444",
                "name_fingerprint": null,
                "label_fingerprint": null,
                "context_fingerprint": "5555555555555555555555555555555555555555555555555555555555555555",
                "source_fingerprint": SOURCE
            }],
            "positions": [{
                "frame_id": 0,
                "candidate_id": 1,
                "kind": "media-time",
                "value": 10,
                "paused": true
            }],
            "controls": [],
            "coverage": [
                {"category": "geometry", "requested": 1, "captured": 0, "truncated": false},
                {"category": "media", "requested": 1, "captured": 1, "truncated": false}
            ],
            "exclusions": [],
            "sensitive_editable": [],
            "nodes_visited": 1
        })
        .to_string()
    }

    #[test]
    fn native_normalization_persists_only_keyed_fingerprint_and_keeps_url_ephemeral() {
        let normalized = normalize_media_page_state_json(&raw_observation(), |_| {
            Some(format!("hmac-sha256-v1-k1:{}", "a".repeat(64)))
        })
        .expect("bounded observation normalizes");

        assert!(!normalized.json.contains(SOURCE));
        assert!(normalized.json.contains("hmac-sha256-v1-k1:"));
        assert!(normalized.json.contains(&"a".repeat(64)));
        assert_eq!(
            normalized
                .source_urls_by_candidate
                .get(&1)
                .map(String::as_str),
            Some(SOURCE)
        );
        let value: serde_json::Value =
            serde_json::from_str(&normalized.json).expect("normalized JSON");
        assert_eq!(value["positions"].as_array().map(Vec::len), Some(1));
        assert_eq!(value["coverage"][1]["captured"], 1);
    }

    #[test]
    fn unavailable_key_removes_media_and_marks_it_unsupported_without_leaking_url() {
        let normalized = normalize_media_page_state_json(&raw_observation(), |_| None)
            .expect("bounded observation remains usable");

        assert!(!normalized.json.contains(SOURCE));
        assert!(normalized.source_urls_by_candidate.is_empty());
        let value: serde_json::Value =
            serde_json::from_str(&normalized.json).expect("normalized JSON");
        assert_eq!(value["candidates"].as_array().map(Vec::len), Some(0));
        assert_eq!(value["positions"].as_array().map(Vec::len), Some(0));
        assert_eq!(value["coverage"][1]["captured"], 0);
        assert_eq!(value["exclusions"][0]["reason"], "unsupported");
    }
}

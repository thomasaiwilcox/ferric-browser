use serde::{Deserialize, Serialize};

const REGISTRY_SOURCE: &str = include_str!("../../../packaging/compatibility-quirks.toml");
const REGISTRY_SCHEMA: u32 = 1;
const MAX_QUIRKS: usize = 256;

/// The adapter deliberately leaves the user agent under `QtWebEngine`'s
/// control. Keeping this policy explicit makes diagnostics and review able to
/// distinguish the engine's real identity from a site workaround or spoof.
#[must_use]
pub fn user_agent_snapshot() -> serde_json::Value {
    serde_json::json!({
        "status": "configured",
        "mode": "engine-default",
        "override": false,
        "provenance": "qtwebengine-default",
        "reason": "no application-specific user-agent override is configured"
    })
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CompatibilityQuirk {
    pub id: String,
    pub site_pattern: String,
    pub engine_min: String,
    pub engine_max: String,
    pub symptom: String,
    pub supported_change: String,
    pub upstream_issue: String,
    pub regression_fixture: String,
    pub date_added: String,
    pub review_condition: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CompatibilityRegistry {
    pub schema: u32,
    pub registry_version: u32,
    #[serde(default)]
    pub quirks: Vec<CompatibilityQuirk>,
}

pub fn load() -> Result<CompatibilityRegistry, String> {
    let registry = toml::from_str::<CompatibilityRegistry>(REGISTRY_SOURCE)
        .map_err(|error| format!("compatibility registry is invalid: {error}"))?;
    validate(&registry)?;
    Ok(registry)
}

#[must_use]
pub fn active_ids(
    registry: &CompatibilityRegistry,
    host: Option<&str>,
    engine_version: Option<&str>,
) -> Vec<String> {
    let Some(host) = host.and_then(normalize_host) else {
        return Vec::new();
    };
    let Some(engine_version) = engine_version.and_then(parse_version) else {
        return Vec::new();
    };
    registry
        .quirks
        .iter()
        .filter(|quirk| {
            quirk.enabled
                && host_matches(&host, &quirk.site_pattern)
                && parse_version(&quirk.engine_min).is_some_and(|minimum| engine_version >= minimum)
                && parse_version(&quirk.engine_max).is_some_and(|maximum| engine_version <= maximum)
        })
        .map(|quirk| quirk.id.clone())
        .collect()
}

#[must_use]
pub fn diagnostic_snapshot() -> serde_json::Value {
    diagnostic_snapshot_for_host(None)
}

#[must_use]
pub fn diagnostic_snapshot_for_host(host: Option<&str>) -> serde_json::Value {
    diagnostic_snapshot_for_host_and_engine(host, engine_version())
}

#[must_use]
pub fn diagnostic_snapshot_for_host_and_engine(
    host: Option<&str>,
    engine_version: Option<&str>,
) -> serde_json::Value {
    match load() {
        Ok(registry) => serde_json::json!({
            "status": "available",
            "schema": registry.schema,
            "registry_version": registry.registry_version,
            "entry_count": registry.quirks.len(),
            "active_ids": active_ids(&registry, host, engine_version),
            "engine_version": engine_version,
            "provenance": "compiled-reviewed-data",
            "reason": "compatibility workarounds are scoped reviewed data; no unsigned patches are loaded"
        }),
        Err(reason) => serde_json::json!({
            "status": "unavailable",
            "schema": REGISTRY_SCHEMA,
            "entry_count": 0,
            "active_ids": [],
            "provenance": "compiled-reviewed-data",
            "reason": reason
        }),
    }
}

fn validate(registry: &CompatibilityRegistry) -> Result<(), String> {
    if registry.schema != REGISTRY_SCHEMA {
        return Err(format!(
            "unsupported compatibility registry schema {}",
            registry.schema
        ));
    }
    if registry.quirks.len() > MAX_QUIRKS {
        return Err(format!(
            "at most {MAX_QUIRKS} compatibility quirks are supported"
        ));
    }
    let mut ids = std::collections::BTreeSet::new();
    for quirk in &registry.quirks {
        if !valid_slug(&quirk.id) || !ids.insert(&quirk.id) {
            return Err("compatibility quirk IDs must be unique lowercase slugs".into());
        }
        if !valid_host_pattern(&quirk.site_pattern) {
            return Err(format!(
                "compatibility quirk {} has an invalid site pattern",
                quirk.id
            ));
        }
        let minimum = parse_version(&quirk.engine_min).ok_or_else(|| {
            format!(
                "compatibility quirk {} has an invalid engine minimum",
                quirk.id
            )
        })?;
        let maximum = parse_version(&quirk.engine_max).ok_or_else(|| {
            format!(
                "compatibility quirk {} has an invalid engine maximum",
                quirk.id
            )
        })?;
        if minimum > maximum {
            return Err(format!(
                "compatibility quirk {} has a reversed engine range",
                quirk.id
            ));
        }
        for (name, value) in [
            ("symptom", quirk.symptom.as_str()),
            ("supported_change", quirk.supported_change.as_str()),
            ("upstream_issue", quirk.upstream_issue.as_str()),
            ("regression_fixture", quirk.regression_fixture.as_str()),
            ("date_added", quirk.date_added.as_str()),
            ("review_condition", quirk.review_condition.as_str()),
        ] {
            if value.is_empty() || value.len() > 1024 || value.chars().any(char::is_control) {
                return Err(format!(
                    "compatibility quirk {} has an invalid {name}",
                    quirk.id
                ));
            }
        }
        if !valid_date(&quirk.date_added) {
            return Err(format!(
                "compatibility quirk {} has an invalid date",
                quirk.id
            ));
        }
    }
    Ok(())
}

fn engine_version() -> Option<&'static str> {
    option_env!("QTWEBENGINE_VERSION")
}

fn parse_version(value: &str) -> Option<(u32, u32, u32, u32)> {
    let parts = value.split('.').collect::<Vec<_>>();
    if parts.is_empty() || parts.len() > 4 || parts.iter().any(|part| part.is_empty()) {
        return None;
    }
    let mut parsed = [0_u32; 4];
    for (index, part) in parts.into_iter().enumerate() {
        parsed[index] = part.parse().ok()?;
    }
    Some((parsed[0], parsed[1], parsed[2], parsed[3]))
}

fn valid_date(value: &str) -> bool {
    let parts = value.split('-').collect::<Vec<_>>();
    parts.len() == 3
        && parts[0].len() == 4
        && parts[1].len() == 2
        && parts[2].len() == 2
        && parts
            .iter()
            .all(|part| part.bytes().all(|byte| byte.is_ascii_digit()))
}

fn valid_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn valid_host_pattern(value: &str) -> bool {
    if value.is_empty() || value.len() > 253 || value.contains('*') && !value.starts_with("*.") {
        return false;
    }
    let value = value.strip_prefix("*.").unwrap_or(value);
    !value.is_empty()
        && !value.contains("..")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-')
}

fn normalize_host(value: &str) -> Option<String> {
    let host = value.trim().trim_end_matches('.').to_ascii_lowercase();
    (!host.is_empty() && valid_host_pattern(&host)).then_some(host)
}

fn host_matches(host: &str, pattern: &str) -> bool {
    if let Some(suffix) = pattern.strip_prefix("*.") {
        host != suffix && host.ends_with(&format!(".{suffix}"))
    } else {
        host == pattern
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CompatibilityQuirk, CompatibilityRegistry, active_ids,
        diagnostic_snapshot_for_host_and_engine, load, user_agent_snapshot,
    };

    #[test]
    fn user_agent_policy_keeps_the_engine_identity() {
        let snapshot = user_agent_snapshot();
        assert_eq!(snapshot["status"], "configured");
        assert_eq!(snapshot["mode"], "engine-default");
        assert_eq!(snapshot["override"], false);
    }

    #[test]
    fn shipped_registry_is_valid_reviewed_data() {
        let registry = load().expect("shipped compatibility registry validates");
        assert_eq!(registry.schema, 1);
        assert!(registry.quirks.is_empty());
    }

    #[test]
    fn active_matching_requires_site_and_engine_range() {
        let registry = CompatibilityRegistry {
            schema: 1,
            registry_version: 1,
            quirks: vec![CompatibilityQuirk {
                id: "fixture-quirk".into(),
                site_pattern: "*.example.test".into(),
                engine_min: "120.0.0.0".into(),
                engine_max: "122.9.9.9".into(),
                symptom: "fixture symptom".into(),
                supported_change: "fixture change".into(),
                upstream_issue: "https://issues.example.test/1".into(),
                regression_fixture: "tests/compatibility/fixture.html".into(),
                date_added: "2026-09-17".into(),
                review_condition: "remove after upstream fix".into(),
                enabled: true,
            }],
        };
        assert_eq!(
            active_ids(&registry, Some("www.example.test"), Some("121.0.1.0")),
            ["fixture-quirk"]
        );
        assert!(active_ids(&registry, Some("example.test"), Some("121.0.1.0")).is_empty());
        assert!(active_ids(&registry, Some("www.example.test"), Some("123.0.0.0")).is_empty());
        assert!(active_ids(&registry, Some("www.example.test"), None).is_empty());
    }

    #[test]
    fn diagnostic_snapshot_accepts_runtime_engine_version() {
        let snapshot = diagnostic_snapshot_for_host_and_engine(None, Some("6.11.2"));
        assert_eq!(snapshot["engine_version"], "6.11.2");
        assert_eq!(snapshot["provenance"], "compiled-reviewed-data");
    }
}

//! Validation for the Versioned Browsing qualification ledger. This is
//! evidence metadata, not a runtime feature switch.

use serde_json::Value;
use std::{collections::BTreeSet, fs, path::Path};

const NAMES: [&str; 10] = [
    "versioning.structural",
    "versioning.live_retention",
    "versioning.archive_capture",
    "versioning.archive_offline",
    "versioning.semantic_restore",
    "versioning.clean_branch",
    "versioning.cookie_identity",
    "versioning.cookie_snapshot",
    "versioning.cookie_seed_verify",
    "versioning.cookie_merge_apply",
];
const FINGERPRINT_FIELDS: [&str; 5] = [
    "qt",
    "qt_webengine",
    "chromium_base",
    "chromium_security_patch",
    "qt_webengine_package",
];
const ENTRY_FIELDS: [&str; 7] = [
    "primitive_gate",
    "production_enabled",
    "backend",
    "scope",
    "evidence_ids",
    "reason_code",
    "reason",
];

pub(super) fn check_file(path: &Path) -> Result<(), String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    validate(&contents, None)?;
    let record: Value = serde_json::from_str(&contents)
        .map_err(|error| format!("invalid versioning capability JSON: {error}"))?;
    let ledger = fs::read_to_string(
        "CurrentImplementation/ferric-versioned-browsing/IMPLEMENTATION_BACKLOG.json",
    )
    .map_err(|error| format!("could not read versioning task ledger: {error}"))?;
    let tasks: Value = serde_json::from_str(&ledger)
        .map_err(|error| format!("invalid versioning task ledger: {error}"))?;
    for certificate in record["release_qualification"]
        .as_object()
        .ok_or("missing release qualification map")?
        .values()
    {
        let id = certificate["qualification_id"]
            .as_str()
            .ok_or("release certificate lacks task ID")?;
        let qualified = tasks["tasks"].as_array().is_some_and(|tasks| {
            tasks.iter().any(|task| {
                task["id"] == id
                    && task["phase"] == "P8"
                    && task["status"] == "verified"
                    && task["tests_run"]
                        .as_array()
                        .is_some_and(|tests| !tests.is_empty())
                    && task["evidence_path"]
                        .as_str()
                        .is_some_and(|evidence| Path::new(evidence).is_file())
            })
        });
        if !qualified {
            return Err(format!(
                "{id}: release qualification is not verified in the task ledger"
            ));
        }
    }
    Ok(())
}

fn validate(contents: &str, expected_fingerprint: Option<&Value>) -> Result<(), String> {
    let record: Value = serde_json::from_str(contents)
        .map_err(|error| format!("invalid versioning capability JSON: {error}"))?;
    if record.get("schema_version").and_then(Value::as_u64) != Some(1) {
        return Err("unknown versioning capability schema".into());
    }
    require_fields(
        &record,
        &[
            "schema_version",
            "engine_fingerprint",
            "fingerprint_source",
            "release_qualification",
            "capabilities",
        ],
        "qualification record",
    )?;
    let source = record
        .get("fingerprint_source")
        .and_then(Value::as_str)
        .ok_or("missing fingerprint source")?;
    if source.is_empty() || source.len() > 128 || source.contains('@') || source.contains("://") {
        return Err("invalid fingerprint source".into());
    }
    require_fields(
        &record["engine_fingerprint"],
        &FINGERPRINT_FIELDS,
        "fingerprint",
    )?;
    let fingerprint = record
        .get("engine_fingerprint")
        .and_then(Value::as_object)
        .ok_or("missing engine fingerprint")?;
    for field in FINGERPRINT_FIELDS {
        let value = fingerprint
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("missing fingerprint field {field}"))?;
        if value.is_empty()
            || value.len() > 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'))
        {
            return Err(format!("invalid fingerprint field {field}"));
        }
    }
    if expected_fingerprint.is_some_and(|expected| expected != &Value::Object(fingerprint.clone()))
    {
        return Err("versioning engine fingerprint mismatch; requalification required".into());
    }
    let capabilities = record
        .get("capabilities")
        .and_then(Value::as_object)
        .ok_or("missing versioning capabilities")?;
    let certificates = record
        .get("release_qualification")
        .and_then(Value::as_object)
        .ok_or("missing release qualification map")?;
    let expected_names = NAMES.into_iter().collect::<BTreeSet<_>>();
    if capabilities
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        != expected_names
    {
        return Err("versioning capability name set differs from the declared contract".into());
    }
    for (name, entry) in capabilities {
        validate_capability(name, entry, certificates.get(name))?;
    }
    validate_certificates(&record, capabilities, certificates)
}

fn validate_certificates(
    record: &Value,
    capabilities: &serde_json::Map<String, Value>,
    certificates: &serde_json::Map<String, Value>,
) -> Result<(), String> {
    for (name, certificate) in certificates {
        if !capabilities.contains_key(name) {
            return Err(format!("{name}: release certificate has no capability"));
        }
        require_fields(
            certificate,
            &[
                "qualification_id",
                "engine_fingerprint",
                "scope",
                "evidence_ids",
            ],
            "release certificate",
        )?;
        let id = certificate["qualification_id"]
            .as_str()
            .ok_or("release certificate lacks task ID")?;
        if !id.starts_with("VB-") || !safe_identifier(id) {
            return Err(format!("{name}: invalid release qualification ID"));
        }
        if certificate["engine_fingerprint"] != record["engine_fingerprint"]
            || certificate["scope"] != capabilities[name]["scope"]
            || !certificate["evidence_ids"]
                .as_array()
                .is_some_and(|ids| ids.iter().any(|value| value.as_str() == Some(id)))
        {
            return Err(format!(
                "{name}: release certificate does not match scope, engine or evidence"
            ));
        }
    }
    Ok(())
}

fn validate_capability(
    name: &str,
    entry: &Value,
    certificate: Option<&Value>,
) -> Result<(), String> {
    require_fields(entry, &ENTRY_FIELDS, name)?;
    let gate = entry
        .get("primitive_gate")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{name}: missing primitive gate"))?;
    if !matches!(gate, "supported" | "unsupported" | "unqualified") {
        return Err(format!("{name}: invalid primitive gate"));
    }
    let enabled = entry
        .get("production_enabled")
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("{name}: missing production enablement"))?;
    let backend = entry
        .get("backend")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{name}: missing backend"))?;
    if backend.is_empty() || backend.len() > 64 || !safe_identifier(backend) {
        return Err(format!("{name}: invalid backend"));
    }
    let scope = entry
        .get("scope")
        .ok_or_else(|| format!("{name}: missing scope"))?;
    if !scope.is_null()
        && !scope
            .as_str()
            .is_some_and(|value| !value.is_empty() && value.len() <= 128 && safe_identifier(value))
    {
        return Err(format!("{name}: invalid scope"));
    }
    let evidence = entry
        .get("evidence_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{name}: missing evidence IDs"))?;
    if evidence.len() > 32
        || evidence.iter().any(|id| {
            !id.as_str().is_some_and(|value| {
                !value.is_empty() && value.len() <= 64 && safe_identifier(value)
            })
        })
    {
        return Err(format!("{name}: invalid evidence IDs"));
    }
    for field in ["reason_code", "reason"] {
        let value = entry
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{name}: missing {field}"))?;
        if value.is_empty() || value.len() > 256 || value.contains('@') || value.contains("://") {
            return Err(format!("{name}: invalid {field}"));
        }
    }
    if !safe_identifier(entry["reason_code"].as_str().unwrap_or_default()) {
        return Err(format!("{name}: invalid reason code"));
    }
    if gate == "supported" && (scope.is_null() || evidence.is_empty()) {
        return Err(format!(
            "{name}: support needs a bounded scope and evidence"
        ));
    }
    if enabled && gate != "supported" {
        return Err(format!(
            "{name}: production cannot enable an unqualified primitive"
        ));
    }
    if enabled && certificate.is_none() {
        return Err(format!(
            "{name}: production enablement needs a separate release certificate"
        ));
    }
    if !enabled && certificate.is_some() {
        return Err(format!(
            "{name}: release certificate exists for a disabled feature"
        ));
    }
    Ok(())
}

fn safe_identifier(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn require_fields(value: &Value, fields: &[&str], label: &str) -> Result<(), String> {
    let actual = value
        .as_object()
        .ok_or_else(|| format!("{label}: expected object"))?;
    if actual.keys().map(String::as_str).collect::<BTreeSet<_>>()
        != fields.iter().copied().collect::<BTreeSet<_>>()
    {
        return Err(format!("{label}: unexpected or missing fields"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate;
    use serde_json::{Value, json};

    const RECORD: &str = include_str!("../../../docs/testing/versioned-browsing/capabilities.json");

    #[test]
    fn qualification_record_parses_and_rejects_unknown_schema() {
        validate(RECORD, None).expect("checked-in capability record");
        let mut record: Value = serde_json::from_str(RECORD).expect("JSON fixture");
        record["schema_version"] = json!(2);
        assert!(validate(&record.to_string(), None).is_err());
    }

    #[test]
    fn fingerprint_mismatch_requires_requalification() {
        let mut fingerprint: Value =
            serde_json::from_str::<Value>(RECORD).expect("JSON fixture")["engine_fingerprint"]
                .clone();
        fingerprint["qt"] = json!("9.0.0");
        assert!(validate(RECORD, Some(&fingerprint)).is_err());
    }

    #[test]
    fn flag_cannot_enable_an_unqualified_capability() {
        let mut record: Value = serde_json::from_str(RECORD).expect("JSON fixture");
        record["capabilities"]["versioning.archive_capture"]["production_enabled"] = json!(true);
        assert!(validate(&record.to_string(), None).is_err());
    }

    #[test]
    fn primitive_support_alone_cannot_enable_production() {
        let mut record: Value = serde_json::from_str(RECORD).expect("JSON fixture");
        record["capabilities"]["versioning.live_retention"]["production_enabled"] = json!(true);
        assert!(validate(&record.to_string(), None).is_err());
    }

    #[test]
    fn observation_revision_cannot_be_smuggled_in_as_a_transaction_boundary() {
        let mut record: Value = serde_json::from_str(RECORD).expect("JSON fixture");
        record["capabilities"]["versioning.cookie_snapshot"]["mirror_revision"] = json!(42);
        assert!(validate(&record.to_string(), None).is_err());
    }
}

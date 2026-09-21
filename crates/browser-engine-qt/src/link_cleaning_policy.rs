use browser_core::{CleaningRule, CleaningRules, HostPattern, clean_link};
use browser_storage::StorageRoots;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;

const MANIFEST_SOURCE: &str = include_str!("../../../packaging/link-cleaning-rules.toml");
const CHECKSUM_SOURCE: &str = include_str!("../../../packaging/link-cleaning-rules.sha256");
const MAX_CORPUS_CASES: usize = 64;
const MAX_TEXT_BYTES: usize = 8 * 1024;
const MAX_CACHED_BUNDLE_BYTES: usize = 1024 * 1024;
const MAX_VALIDATOR_BYTES: usize = 4096;
const ACCEPTED_BUNDLE_FILE: &str = "accepted.bundle";
const ACCEPTED_METADATA_FILE: &str = "accepted.meta";

#[derive(Debug, Deserialize)]
struct Manifest {
    schema: u32,
    revision: String,
    source: String,
    changelog: String,
    rules: Vec<ManifestRule>,
    corpus: Vec<CorpusCase>,
}

#[derive(Debug, Deserialize)]
struct ManifestRule {
    id: String,
    host: String,
    parameters: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CorpusCase {
    id: String,
    input: String,
    expected: String,
    expected_changed: bool,
    expected_removed: Vec<String>,
}

struct ReviewedBundle {
    manifest: Manifest,
    rules: CleaningRules,
    checksum: String,
}

pub fn reviewed_rules() -> CleaningRules {
    let bundle = reviewed_bundle().unwrap_or_else(|error| {
        panic!("compiled clean-link rule bundle is invalid: {error}");
    });
    verify_corpus(&bundle.rules, &bundle.manifest.corpus).unwrap_or_else(|error| {
        panic!("compiled clean-link rule corpus is invalid: {error}");
    });
    bundle.rules
}

pub fn active_rules(roots: Option<&StorageRoots>) -> CleaningRules {
    roots
        .and_then(|roots| cached_bundle(roots).ok())
        .map(|bundle| bundle.rules)
        .unwrap_or_else(reviewed_rules)
}

#[must_use]
pub fn diagnostic_snapshot(roots: Option<&StorageRoots>) -> Value {
    let compiled = reviewed_bundle().and_then(|bundle| {
        let passed = verify_corpus(&bundle.rules, &bundle.manifest.corpus)?;
        Ok((bundle, passed))
    });
    let cached = roots.and_then(|roots| {
        let path = accepted_bundle_path(roots);
        if path.is_file() {
            Some(cached_bundle(roots).map(|bundle| {
                let passed = verify_corpus(&bundle.rules, &bundle.manifest.corpus);
                (bundle, passed)
            }))
        } else {
            None
        }
    });
    match cached {
        Some(Ok((bundle, Ok(passed)))) => bundle_diagnostic(
            bundle,
            passed,
            json!({
                "status": "accepted",
                "last_accepted_snapshot": "cache",
                "automatic": "configured-normal-profile"
            }),
        ),
        Some(Ok((bundle, Err(error)))) => json!({
            "status": "invalid",
            "reason": format!("cached clean-link corpus audit failed: {error}"),
            "revision": bundle.manifest.revision,
            "update": {
                "status": "rejected-fallback",
                "last_accepted_snapshot": "compiled-reviewed",
            "automatic": "configured-normal-profile"
            }
        }),
        Some(Err(error)) => match compiled {
            Ok((bundle, passed)) => bundle_diagnostic(
                bundle,
                passed,
                json!({
                    "status": "rejected-fallback",
                    "last_accepted_snapshot": "compiled-reviewed",
                    "automatic": "configured-normal-profile",
                    "reason": error
                }),
            ),
            Err(compiled_error) => json!({
                "status": "invalid",
                "reason": format!("compiled audit failed: {compiled_error}; cached candidate rejected"),
                "update": {
                    "status": "rejected-fallback",
                    "automatic": "configured-normal-profile"
                }
            }),
        },
        None => match compiled {
            Ok((bundle, passed)) => bundle_diagnostic(
                bundle,
                passed,
                json!({
                    "status": "not-configured",
                    "last_accepted_snapshot": "compiled-reviewed",
                    "automatic": "configured-normal-profile"
                }),
            ),
            Err(error) => json!({
                "status": "invalid",
                "reason": format!("clean-link rule audit failed: {error}"),
                "update": {
                    "status": "not-configured",
                "automatic": "configured-normal-profile"
                }
            }),
        },
    }
}

fn bundle_diagnostic(bundle: ReviewedBundle, passed: usize, update: Value) -> Value {
    let mut update = update;
    if let Some(object) = update.as_object_mut() {
        object.insert(
            "transport".into(),
            Value::String(
                "explicit bounded HTTPS fetches feed checksum, corpus, and atomic acceptance with conditional validators"
                    .into(),
            ),
        );
    }
    json!({
        "status": "available",
        "revision": bundle.manifest.revision,
        "source": bundle.manifest.source,
        "manifest_sha256": bundle.checksum,
        "rule_count": bundle.rules.rules.len(),
        "corpus_cases": bundle.manifest.corpus.len(),
        "corpus_passed": passed,
        "changelog": bundle.manifest.changelog,
        "update": update,
        "reason": "active rule revision and corpus audit metadata; URLs from corpus cases are excluded"
    })
}

pub fn accept_downloaded_bundle_with_metadata(
    roots: &StorageRoots,
    manifest_bytes: &[u8],
    expected_checksum: &str,
    etag: &str,
    last_modified: &str,
) -> Result<Value, String> {
    if manifest_bytes.is_empty() || manifest_bytes.len() > MAX_CACHED_BUNDLE_BYTES {
        return Err("downloaded clean-link manifest exceeds the size limit".into());
    }
    validate_validator("ETag", etag)?;
    validate_validator("Last-Modified", last_modified)?;
    let manifest = std::str::from_utf8(manifest_bytes)
        .map_err(|_| "downloaded clean-link manifest is not UTF-8".to_owned())?;
    let checksum = super::network_policy::sha256_hex(manifest_bytes);
    if expected_checksum.trim() != checksum {
        return Err("downloaded clean-link manifest checksum mismatch".into());
    }
    let bundle = parse_bundle(manifest, &checksum)?;
    let passed = verify_corpus(&bundle.rules, &bundle.manifest.corpus)?;
    let mut envelope = format!("sha256={checksum}\n---\n").into_bytes();
    envelope.extend_from_slice(manifest_bytes);
    atomic_write_private(&accepted_bundle_path(roots), &envelope)?;
    let metadata_status = if etag.is_empty() && last_modified.is_empty() {
        "not-provided"
    } else {
        let metadata = format!("{etag}\n{last_modified}\n");
        if atomic_write_private(&accepted_metadata_path(roots), metadata.as_bytes()).is_ok() {
            "stored"
        } else {
            "unavailable"
        }
    };
    Ok(json!({
        "status": "accepted",
        "revision": bundle.manifest.revision,
        "manifest_sha256": checksum,
        "corpus_passed": passed,
        "corpus_cases": bundle.manifest.corpus.len(),
        "validator_metadata": metadata_status
    }))
}

fn cached_bundle(roots: &StorageRoots) -> Result<ReviewedBundle, String> {
    let bytes = fs::read(accepted_bundle_path(roots))
        .map_err(|error| format!("cached bundle read failed: {error}"))?;
    if bytes.is_empty() || bytes.len() > MAX_CACHED_BUNDLE_BYTES + 128 {
        return Err("cached clean-link bundle exceeds the size limit".into());
    }
    let separator = b"\n---\n";
    let Some(separator_start) = bytes
        .windows(separator.len())
        .position(|window| window == separator)
    else {
        return Err("cached clean-link bundle envelope is invalid".into());
    };
    let header = std::str::from_utf8(&bytes[..separator_start])
        .map_err(|_| "cached clean-link bundle header is invalid".to_owned())?;
    let expected_checksum = header
        .strip_prefix("sha256=")
        .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| "cached clean-link bundle checksum is invalid".to_owned())?;
    let manifest = &bytes[separator_start + separator.len()..];
    let manifest = std::str::from_utf8(manifest)
        .map_err(|_| "cached clean-link bundle manifest is not UTF-8".to_owned())?;
    let bundle = parse_bundle(manifest, expected_checksum)?;
    verify_corpus(&bundle.rules, &bundle.manifest.corpus)?;
    Ok(bundle)
}

fn accepted_bundle_path(roots: &StorageRoots) -> PathBuf {
    roots.cache.join("link-cleaning").join(ACCEPTED_BUNDLE_FILE)
}

fn accepted_metadata_path(roots: &StorageRoots) -> PathBuf {
    roots
        .cache
        .join("link-cleaning")
        .join(ACCEPTED_METADATA_FILE)
}

fn validate_validator(label: &str, value: &str) -> Result<(), String> {
    if value.len() > MAX_VALIDATOR_BYTES || value.chars().any(char::is_control) {
        return Err(format!(
            "{label} validator is oversized or contains controls"
        ));
    }
    Ok(())
}

fn atomic_write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "candidate path has no parent".to_owned())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temporary = parent.join(format!(".{}.tmp-{}", ACCEPTED_BUNDLE_FILE, Uuid::new_v4()));
    let result = (|| {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|error| error.to_string())?;
        file.write_all(bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        fs::rename(&temporary, path).map_err(|error| error.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn parse_bundle(manifest_source: &str, expected_checksum: &str) -> Result<ReviewedBundle, String> {
    let manifest = toml::from_str::<Manifest>(manifest_source)
        .map_err(|error| format!("manifest parse error: {error}"))?;
    validate_manifest(&manifest)?;
    let actual_checksum = super::network_policy::sha256_hex(manifest_source.as_bytes());
    if actual_checksum != expected_checksum {
        return Err("manifest checksum does not match the accepted bytes".into());
    }
    let rules = manifest
        .rules
        .iter()
        .map(manifest_rule)
        .collect::<Result<Vec<_>, _>>()?;
    let rules = CleaningRules::new(manifest.revision.clone(), manifest.source.clone(), rules)
        .map_err(|error| error.to_string())?;
    validate_corpus_metadata(&manifest.corpus)?;
    Ok(ReviewedBundle {
        manifest,
        rules,
        checksum: actual_checksum,
    })
}

fn validate_manifest(manifest: &Manifest) -> Result<(), String> {
    if manifest.schema != 1 {
        return Err(format!("unsupported manifest schema {}", manifest.schema));
    }
    validate_text("revision", &manifest.revision)?;
    validate_text("source", &manifest.source)?;
    validate_text("changelog", &manifest.changelog)?;
    if manifest.rules.is_empty() || manifest.rules.len() > 512 {
        return Err("rule count is outside the bounded range".into());
    }
    if manifest.corpus.is_empty() || manifest.corpus.len() > MAX_CORPUS_CASES {
        return Err("corpus count is outside the bounded range".into());
    }
    Ok(())
}

fn reviewed_bundle() -> Result<ReviewedBundle, String> {
    let expected_checksum = CHECKSUM_SOURCE.trim();
    if expected_checksum.len() != 64
        || !expected_checksum
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("manifest checksum metadata is invalid".into());
    }
    parse_bundle(MANIFEST_SOURCE, expected_checksum)
}

fn manifest_rule(rule: &ManifestRule) -> Result<CleaningRule, String> {
    validate_text("rule id", &rule.id)?;
    let host = match rule.host.as_str() {
        "*" => HostPattern::Any,
        value if value.strip_prefix("*.").is_some() => {
            let base = value.strip_prefix("*.").unwrap_or_default();
            validate_host(base)?;
            HostPattern::Subdomains(base.to_ascii_lowercase())
        }
        value => {
            validate_host(value)?;
            HostPattern::Exact(value.to_ascii_lowercase())
        }
    };
    let parameters = rule.parameters.iter().cloned().collect::<BTreeSet<_>>();
    if parameters.len() != rule.parameters.len() {
        return Err(format!("rule {} contains duplicate parameters", rule.id));
    }
    Ok(CleaningRule {
        id: rule.id.clone(),
        host,
        remove_parameters: parameters,
    })
}

fn validate_corpus_metadata(corpus: &[CorpusCase]) -> Result<(), String> {
    let mut ids = BTreeSet::new();
    for case in corpus {
        validate_text("corpus id", &case.id)?;
        if !ids.insert(case.id.clone()) {
            return Err(format!("duplicate corpus id {}", case.id));
        }
        validate_text("corpus input", &case.input)?;
        validate_text("corpus expected", &case.expected)?;
        if case.expected_removed.iter().any(String::is_empty) {
            return Err(format!("corpus {} has an empty removed parameter", case.id));
        }
    }
    Ok(())
}

fn verify_corpus(rules: &CleaningRules, corpus: &[CorpusCase]) -> Result<usize, String> {
    let mut passed = 0;
    for case in corpus {
        let result =
            clean_link(&case.input, rules).map_err(|error| format!("{}: {error}", case.id))?;
        if result.cleaned != case.expected
            || result.changed != case.expected_changed
            || result.removed_parameters != case.expected_removed
        {
            return Err(format!(
                "{} expected changed={}, removed={:?}, output {:?}; got changed={}, removed={:?}, output {:?}",
                case.id,
                case.expected_changed,
                case.expected_removed,
                case.expected,
                result.changed,
                result.removed_parameters,
                result.cleaned
            ));
        }
        passed += 1;
    }
    Ok(passed)
}

fn validate_text(label: &str, value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(format!("{label} is empty, oversized, or contains controls"));
    }
    Ok(())
}

fn validate_host(host: &str) -> Result<(), String> {
    if host.is_empty()
        || host.len() > 253
        || !host.is_ascii()
        || host.starts_with('.')
        || host.ends_with('.')
        || host.contains("..")
        || !host.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'.' || byte == b'-'
        })
    {
        return Err(format!("invalid rule host {host:?}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use browser_storage::{RootSpec, StorageRoots};
    use std::fs;

    use super::{
        CHECKSUM_SOURCE, MANIFEST_SOURCE, accept_downloaded_bundle_with_metadata, active_rules,
        diagnostic_snapshot, reviewed_bundle, verify_corpus,
    };

    #[test]
    fn shipped_bundle_checksum_and_corpus_are_valid() {
        let bundle = reviewed_bundle().expect("shipped clean-link bundle");
        let passed = verify_corpus(&bundle.rules, &bundle.manifest.corpus).expect("corpus");
        assert_eq!(passed, bundle.manifest.corpus.len());
        assert_eq!(bundle.checksum, CHECKSUM_SOURCE.trim());
        assert!(!MANIFEST_SOURCE.is_empty());
    }

    #[test]
    fn diagnostics_expose_revision_without_corpus_urls() {
        let value = diagnostic_snapshot(None);
        assert_eq!(value["status"], "available");
        assert_eq!(value["revision"], "builtin-2");
        assert_eq!(value["corpus_passed"], value["corpus_cases"]);
        assert!(!value.to_string().contains("example.test"));
        assert_eq!(value["update"]["automatic"], "configured-normal-profile");
    }

    #[test]
    fn downloaded_candidate_is_atomic_and_invalid_candidate_keeps_last_good() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("temporary roots");
        let candidate =
            MANIFEST_SOURCE.replace("revision = \"builtin-2\"", "revision = \"downloaded-test\"");
        let checksum = super::super::network_policy::sha256_hex(candidate.as_bytes());
        let accepted =
            accept_downloaded_bundle_with_metadata(&roots, candidate.as_bytes(), &checksum, "", "")
                .expect("candidate accepted");
        assert_eq!(accepted["status"], "accepted");
        assert_eq!(active_rules(Some(&roots)).revision, "downloaded-test");

        assert!(
            accept_downloaded_bundle_with_metadata(&roots, b"invalid", &checksum, "", "").is_err()
        );
        assert_eq!(active_rules(Some(&roots)).revision, "downloaded-test");
        assert!(roots.cache.join("link-cleaning/accepted.bundle").is_file());
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn accepted_candidates_store_bounded_conditional_metadata() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("temporary roots");
        let candidate =
            MANIFEST_SOURCE.replace("revision = \"builtin-2\"", "revision = \"metadata-test\"");
        let checksum = super::super::network_policy::sha256_hex(candidate.as_bytes());
        accept_downloaded_bundle_with_metadata(
            &roots,
            candidate.as_bytes(),
            &checksum,
            "\"revision-1\"",
            "Wed, 17 Sep 2026 00:00:00 GMT",
        )
        .expect("candidate accepted");
        assert_eq!(
            fs::read_to_string(roots.cache.join("link-cleaning/accepted.meta"))
                .expect("validator metadata"),
            "\"revision-1\"\nWed, 17 Sep 2026 00:00:00 GMT\n"
        );
        assert!(
            accept_downloaded_bundle_with_metadata(
                &roots,
                candidate.as_bytes(),
                &checksum,
                "bad\nvalidator",
                ""
            )
            .is_err()
        );
        assert_eq!(active_rules(Some(&roots)).revision, "metadata-test");
        roots.cleanup().expect("cleanup");
    }
}

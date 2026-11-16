//! Qualified native delivery facts. These never grant source or receiver authority.

use ferric_browser_config::{
    InteractionPageReceiver, InteractionRemapContext, NativeReceiverScopeRequirement,
};
use ferric_browser_core::LogicalKeyToken;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::LazyLock,
};

const SOURCE: &str = include_str!("../../../packaging/page-key-capabilities.json");
static MANIFEST: LazyLock<Option<Manifest>> = LazyLock::new(|| parse_manifest(SOURCE));

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Build {
    package_version: String,
    os: String,
    arch: String,
    qt_webengine: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Backend {
    qualification: String,
    source_input: String,
    excluded_groups: Vec<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TokenGroup {
    id: String,
    qualification: String,
    tokens: Vec<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: u32,
    build: Build,
    backends: BTreeMap<String, Backend>,
    tested_layouts: Vec<String>,
    contexts: Vec<String>,
    receiver_scopes: Vec<String>,
    token_groups: Vec<TokenGroup>,
    limits: Vec<String>,
    evidence: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", content = "reason", rename_all = "kebab-case")]
pub(super) enum Qualification {
    Qualified,
    Unqualified(&'static str),
}

impl Qualification {
    pub(super) const fn reason(self) -> Option<&'static str> {
        match self {
            Self::Qualified => None,
            Self::Unqualified(reason) => Some(reason),
        }
    }
}

fn parse_manifest(source: &str) -> Option<Manifest> {
    let manifest: Manifest = serde_json::from_str(source).ok()?;
    if manifest.schema != 1 || manifest.token_groups.len() != 9 {
        return None;
    }
    let mut tokens = BTreeSet::new();
    let mut groups = BTreeSet::new();
    for group in &manifest.token_groups {
        if group.tokens.is_empty() || !groups.insert(&group.id) {
            return None;
        }
        for token in &group.tokens {
            if LogicalKeyToken::parse(token).ok()?.as_str() != token || !tokens.insert(token) {
                return None;
            }
        }
    }
    Some(manifest)
}

pub(super) const fn context_name(context: InteractionRemapContext) -> &'static str {
    match context {
        InteractionRemapContext::PassthroughOnce => "passthrough-once",
        InteractionRemapContext::Passthrough => "passthrough",
        InteractionRemapContext::NormalPageKey => "normal-page-key",
        InteractionRemapContext::HintPageKey => "hint-page-key",
    }
}

pub(super) const fn scope_name(scope: NativeReceiverScopeRequirement) -> &'static str {
    match scope {
        NativeReceiverScopeRequirement::TopDocument => "top-document",
        NativeReceiverScopeRequirement::FocusedFrame => "focused-frame",
        NativeReceiverScopeRequirement::TopOrFocusedFrame => "top-or-focused-frame",
    }
}

pub(super) const fn receiver_scope(
    receiver: &InteractionPageReceiver,
) -> NativeReceiverScopeRequirement {
    match receiver {
        InteractionPageReceiver::TopDocument {} => NativeReceiverScopeRequirement::TopDocument,
        InteractionPageReceiver::FocusedFrame { .. } => {
            NativeReceiverScopeRequirement::FocusedFrame
        }
        InteractionPageReceiver::TopOrFocusedFrame { .. } => {
            NativeReceiverScopeRequirement::TopOrFocusedFrame
        }
    }
}

pub(super) fn current_assessment(
    destination: &LogicalKeyToken,
    transformed: bool,
    context: InteractionRemapContext,
    scope: NativeReceiverScopeRequirement,
) -> Qualification {
    assess(
        destination,
        transformed,
        context,
        scope,
        &crate::qt_platform_name(),
        &crate::qt_webengine_version(),
    )
}

pub(super) fn current_snapshot() -> Value {
    snapshot(
        Some(&crate::qt_platform_name()),
        Some(&crate::qt_webengine_version()),
    )
}

fn build_qualification(manifest: &Manifest, backend: &str, version: &str) -> Qualification {
    if manifest.build.package_version != env!("CARGO_PKG_VERSION")
        || manifest.build.os != std::env::consts::OS
        || manifest.build.arch != std::env::consts::ARCH
        || manifest.build.qt_webengine != version
    {
        return Qualification::Unqualified("Native page keys are not qualified for this build");
    }
    if manifest
        .backends
        .get(backend)
        .is_none_or(|entry| entry.qualification != "qualified")
    {
        return Qualification::Unqualified(
            "Native page keys are not qualified for this window backend",
        );
    }
    Qualification::Qualified
}

pub(super) fn assess(
    destination: &LogicalKeyToken,
    transformed: bool,
    context: InteractionRemapContext,
    scope: NativeReceiverScopeRequirement,
    backend: &str,
    version: &str,
) -> Qualification {
    let Some(manifest) = MANIFEST.as_ref() else {
        return Qualification::Unqualified("Native page-key capability manifest is unavailable");
    };
    let build = build_qualification(manifest, backend, version);
    if build != Qualification::Qualified {
        return build;
    }
    if !manifest
        .contexts
        .iter()
        .any(|value| value == context_name(context))
        || !manifest
            .receiver_scopes
            .iter()
            .any(|value| value == scope_name(scope))
    {
        return Qualification::Unqualified(
            "Native page keys are not qualified for this context or receiver scope",
        );
    }
    // Original delivery preserves the native event, including Unicode input.
    // It does not assert that a virtual Unicode destination can be constructed.
    if !transformed {
        return Qualification::Qualified;
    }
    let Some(group) = manifest.token_groups.iter().find(|group| {
        group
            .tokens
            .iter()
            .any(|token| token == destination.as_str())
    }) else {
        return Qualification::Unqualified(
            "Destination syntax is valid but native delivery is not qualified",
        );
    };
    if group.qualification != "qualified"
        || manifest.backends[backend]
            .excluded_groups
            .contains(&group.id)
    {
        return Qualification::Unqualified(
            "Destination metadata is not qualified for this window backend",
        );
    }
    Qualification::Qualified
}

pub(super) fn snapshot(backend: Option<&str>, version: Option<&str>) -> Value {
    let Some(manifest) = MANIFEST.as_ref() else {
        return json!({"schema":1,"runtime":{"qualification":"unavailable","reason":"invalid embedded capability manifest"}});
    };
    let mut value = serde_json::to_value(manifest).unwrap_or(Value::Null);
    value["runtime"] = match (backend, version) {
        (Some(backend), Some(version)) => json!({
            "backend":backend,"qt_webengine":version,"os":std::env::consts::OS,
            "arch":std::env::consts::ARCH,"package_version":env!("CARGO_PKG_VERSION"),
            "qualification":build_qualification(manifest,backend,version)
        }),
        _ => {
            json!({"qualification":{"status":"not-probed"},"reason":"Qt window backend has not been observed"})
        }
    };
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferric_browser_config::InteractionPolicyPatch;

    #[test]
    fn manifest_retains_all_groups_and_canonical_native_tokens() {
        let manifest = parse_manifest(SOURCE).unwrap();
        assert_eq!(
            manifest
                .token_groups
                .iter()
                .map(|group| group.tokens.len())
                .sum::<usize>(),
            130
        );
        for group in &manifest.token_groups {
            for token in &group.tokens {
                let policy: InteractionPolicyPatch = serde_json::from_value(json!({"remaps":[{
                    "id":"qualification","from":"p","to":token,"contexts":["passthrough"]
                }]}))
                .unwrap();
                let compiled = policy.compile_authored_keys().unwrap();
                let requirement = compiled.capability_requirements().next().unwrap();
                assert_eq!(
                    serde_json::to_value(requirement.destination_class()).unwrap(),
                    group.id
                );
            }
        }
        assert_eq!(manifest.contexts.len(), 4);
        assert_eq!(manifest.receiver_scopes.len(), 3);
        assert_eq!(manifest.tested_layouts, ["us", "gb", "de"]);
    }

    #[test]
    fn malformed_or_noncanonical_manifests_do_not_qualify_delivery() {
        for change in ["schema", "duplicate", "alias"] {
            let mut value: Value = serde_json::from_str(SOURCE).unwrap();
            match change {
                "schema" => value["schema"] = json!(2),
                "duplicate" => {
                    value["token_groups"][0]["tokens"][1] =
                        value["token_groups"][0]["tokens"][0].clone()
                }
                _ => value["token_groups"][4]["tokens"][0] = json!("Return"),
            }
            assert!(parse_manifest(&value.to_string()).is_none());
        }
    }

    #[test]
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    fn qualification_is_separate_from_syntax_scope_and_original_delivery() {
        let token = LogicalKeyToken::parse("Ctrl+c").unwrap();
        let scope = NativeReceiverScopeRequirement::TopDocument;
        let context = InteractionRemapContext::HintPageKey;
        assert!(
            assess(&token, true, context, scope, "wayland", "6.11.2")
                .reason()
                .is_some()
        );
        let token = LogicalKeyToken::parse("Ctrl+k").unwrap();
        assert_eq!(
            assess(&token, true, context, scope, "wayland", "6.11.2"),
            Qualification::Qualified
        );
        assert!(
            assess(&token, true, context, scope, "xcb", "6.11.2")
                .reason()
                .is_some()
        );
        assert!(
            assess(&token, true, context, scope, "wayland", "6.12.0")
                .reason()
                .is_some()
        );
        let unicode = LogicalKeyToken::parse("é").unwrap();
        assert_eq!(
            assess(&unicode, false, context, scope, "wayland", "6.11.2"),
            Qualification::Qualified
        );
        assert!(
            assess(&unicode, true, context, scope, "wayland", "6.11.2")
                .reason()
                .is_some()
        );
        let back = LogicalKeyToken::parse("Back").unwrap();
        assert!(
            assess(&back, true, context, scope, "offscreen", "6.11.2")
                .reason()
                .is_some()
        );
    }
}

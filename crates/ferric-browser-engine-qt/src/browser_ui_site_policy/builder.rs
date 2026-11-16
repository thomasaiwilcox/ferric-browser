//! Inert builder diagnostics over the existing URL matcher and logical keys.
use ferric_browser_config::{
    Config, InteractionRemapContext, NativeReceiverScopeRequirement, SiteRule,
};
use ferric_browser_core::{LogicalKeyToken, MAX_URL_BYTES, canonical_origin, canonicalize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn location(input: &str) -> Result<(String, String), String> {
    if input.len() > MAX_URL_BYTES {
        return Err("builder URL exceeds the URL limit".into());
    }
    let canonical = canonicalize(input).map_err(|error| error.to_string())?;
    let origin = canonical_origin(&canonical).ok_or("Choose an HTTP(S) page for the builder")?;
    let (_, authority_and_path) = canonical.split_once("://").ok_or("invalid page URL")?;
    let path = authority_and_path
        .find(['/', '?', '#'])
        .map_or("/", |index| &authority_and_path[index..]);
    let path = if path.starts_with('/') {
        path.split(['?', '#']).next().unwrap_or("/")
    } else {
        "/"
    };
    Ok((origin, path.into()))
}

pub(super) fn scope(url: &str, pattern: Option<&str>, samples: &[String]) -> Result<Value, String> {
    if samples.len() > 8 {
        return Err("Use at most eight explicit URL samples".into());
    }
    let (origin, path) = location(url)?;
    // '*' has wildcard meaning in the existing path grammar. Do not describe
    // such a path as exact or silently broaden its scope.
    let exact = (!path.contains('*')).then(|| format!("{origin}{path}"));
    let mut result = json!({"origin":origin,"exact_pattern":exact,
        "site_pattern":format!("{origin}/*"),"url":format!("{origin}{path}"),
        "note":"Patterns ignore query and fragment. Scope keeps the exact scheme, host and port; subdomains are not added.",
        "activation_permitted":false});
    if let Some(pattern) = pattern {
        let rule = SiteRule {
            id: "builder-sample".into(),
            pattern: pattern.into(),
            priority: 0,
            set: BTreeMap::new(),
            interaction: None,
        };
        ferric_browser_config::validate_site_rule(&rule).map_err(|error| error.to_string())?;
        let config = Config {
            site_rules: vec![rule],
            ..Config::default()
        };
        let matches = samples.iter().map(|sample| {
            let (origin, path) = location(sample)?;
            let normalized = format!("{origin}{path}");
            Ok(json!({"url":normalized,
                "matches":!ferric_browser_config::matching_site_rules(&config, &normalized).is_empty()}))
        }).collect::<Result<Vec<_>, String>>()?;
        result["pattern"] = json!(pattern);
        result["current_page_matches"] = json!(
            !ferric_browser_config::matching_site_rules(&config, &format!("{origin}{path}"))
                .is_empty()
        );
        result["samples"] = json!(matches);
    }
    Ok(result)
}

pub(super) fn key(input: &str) -> Result<Value, String> {
    let token = LogicalKeyToken::parse(input).map_err(|error| error.to_string())?;
    let contexts = [
        InteractionRemapContext::PassthroughOnce,
        InteractionRemapContext::Passthrough,
        InteractionRemapContext::NormalPageKey,
        InteractionRemapContext::HintPageKey,
    ];
    let scopes = [
        NativeReceiverScopeRequirement::TopDocument,
        NativeReceiverScopeRequirement::FocusedFrame,
        NativeReceiverScopeRequirement::TopOrFocusedFrame,
    ];
    let qualifications = contexts.into_iter().flat_map(|context| scopes.into_iter().map(move |scope| (context, scope)))
        .map(|(context, scope)| json!({"context":context,"receiver_scope":scope,
            "qualification":crate::page_key_capabilities::current_assessment(&token, true, context, scope)}))
        .collect::<Vec<_>>();
    Ok(
        json!({"canonical":token.as_str(),"qualifications":qualifications,
        "note":"Syntax and native capability only. A mapping still requires explicit source admission, an eligible receiver and a physical key gesture.",
        "activation_permitted":false}),
    )
}

// Read-only admission. Actual receiver, mode, capability and current mapping
// are checked by the existing native router after the editor relinquishes focus.
pub(super) fn test_page(
    state: &ferric_browser_application::BrowserApplication,
    draft: &str,
    revision: u64,
    rule: &str,
    mapping: &str,
    context: InteractionRemapContext,
) -> Result<Value, String> {
    let view = state.site_policy_draft(draft)?;
    if view.revision != revision || view.dirty {
        return Err("Apply the current reviewed draft before testing on the page".into());
    }
    if view.configuration_revision != state.configuration_inspection_inputs().sources().revision()
        || !view
            .operation
            .as_ref()
            .is_some_and(|operation| operation.active && operation.revision == revision)
    {
        return Err(
            "This draft is not the current active revision; review and apply it first".into(),
        );
    }
    let patch = view
        .rules
        .iter()
        .find(|entry| entry.id == rule)
        .and_then(|entry| entry.proposed.as_ref())
        .and_then(|rule| rule.interaction.as_ref())
        .filter(|patch| patch.enabled != Some(false))
        .ok_or("Choose an enabled Interaction rule in this draft")?;
    let remap = patch
        .remaps
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|remap| remap.id == mapping && remap.contexts.contains(&context))
        .ok_or("Choose a mapping and one of its declared contexts")?;
    let source = LogicalKeyToken::parse(&remap.from).map_err(|error| error.to_string())?;
    let destination = LogicalKeyToken::parse(&remap.to).map_err(|error| error.to_string())?;
    Ok(json!({"source":source.as_str(),"destination":destination.as_str(),"context":context}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_is_exact_and_samples_use_the_shared_matcher_without_credentials() {
        let value = scope(
            "HTTPS://user:secret@EXAMPLE.test.:443/a?q=secret#fragment",
            Some("https://example.test/a"),
            &[
                "https://example.test/a?different=1".into(),
                "https://sub.example.test/a".into(),
                "https://example.test:8443/a".into(),
                "https://example.test/another".into(),
            ],
        )
        .unwrap();
        assert_eq!(value["origin"], "https://example.test");
        assert_eq!(value["exact_pattern"], "https://example.test/a");
        assert_eq!(value["samples"][0]["matches"], true);
        for index in 1..4 {
            assert_eq!(value["samples"][index]["matches"], false);
        }
        assert!(!value.to_string().contains("secret"));
        assert!(scope("file:///tmp/a", None, &[]).is_err());
        assert!(scope("https://example.test/*", None, &[]).unwrap()["exact_pattern"].is_null());
        assert!(scope("https://example.test/", Some("bad pattern"), &[]).is_err());
        assert!(scope("https://example.test/", None, &vec![String::new(); 9]).is_err());
    }
}

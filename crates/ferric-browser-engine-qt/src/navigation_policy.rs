//! Qt-independent navigation facts used to record journeys and resolve input safely.

use ferric_browser_config::Config;
use ferric_browser_core::{JourneyEdgeKind, NavigationContext, resolve_search_query};
use ferric_browser_storage::Quickmark;
use serde_json::Value;

use crate::input_validation::is_bounded_untrusted_text;

#[must_use]
pub(super) fn strip_url_fragment(url: &str) -> &str {
    url.split_once('#')
        .map_or(url, |(without_fragment, _)| without_fragment)
}

#[must_use]
pub(super) fn recordable_same_document_change(
    pending_navigation: bool,
    previous: Option<&str>,
    url: &str,
) -> bool {
    !pending_navigation && previous.is_some_and(|previous| previous != url)
}

#[must_use]
pub(super) fn journey_transition_after_load(
    explicit: Option<(JourneyEdgeKind, String)>,
    was_traversal: bool,
    redirect_hops: Option<u8>,
) -> Option<(JourneyEdgeKind, String)> {
    let redirect_source = redirect_hops.map(|hops| {
        if hops <= 1 {
            "redirect".to_owned()
        } else {
            format!("redirect-chain:{hops}")
        }
    });
    if let Some((transition, source)) = explicit {
        return Some((
            transition,
            redirect_source.map_or(source.clone(), |redirect| format!("{source};{redirect}")),
        ));
    }
    if was_traversal {
        None
    } else {
        redirect_source.map(|source| (JourneyEdgeKind::Redirect, source))
    }
}

pub(super) fn configured_search_url(
    config_value: &Value,
    requested_engine: Option<&str>,
    query: &str,
) -> Result<(String, String), String> {
    let config = serde_json::from_value::<Config>(config_value.clone())
        .map_err(|error| format!("search configuration is invalid: {error}"))?;
    let engine = requested_engine
        .unwrap_or(config.navigation.default_search.as_str())
        .to_owned();
    if !is_bounded_untrusted_text(&engine) {
        return Err("search engine name must be a nonempty bounded value".into());
    }
    let template = config
        .search_engines
        .get(&engine)
        .ok_or_else(|| format!("configured search engine not found: {engine}"))?;
    let url = resolve_search_query(template, query).map_err(|error| error.to_string())?;
    Ok((engine, url.to_string()))
}

pub(super) fn navigation_context(
    config_value: &Value,
    quickmarks: &[Quickmark],
    trusted_local_input: bool,
) -> NavigationContext {
    let config = serde_json::from_value::<Config>(config_value.clone()).unwrap_or_default();
    let search_keywords = config
        .search_engines
        .iter()
        .map(|(name, template)| (name.clone(), template.replace("{query}", "{}")))
        .collect();
    let default_search_template = config
        .search_engines
        .get(&config.navigation.default_search)
        .map(|template| template.replace("{query}", "{}"));
    let quickmarks = quickmarks
        .iter()
        .map(|mark| (mark.name.clone(), mark.url.clone()))
        .collect();
    NavigationContext {
        quickmarks,
        search_keywords,
        default_search_template,
        trusted_local_input,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferric_browser_config::Config;

    #[test]
    fn redirect_chains_are_recorded_unless_navigation_is_a_traversal() {
        assert_eq!(
            journey_transition_after_load(None, false, Some(3)),
            Some((JourneyEdgeKind::Redirect, "redirect-chain:3".into()))
        );
        assert_eq!(journey_transition_after_load(None, true, Some(3)), None);
    }

    #[test]
    fn search_resolution_uses_the_typed_configuration_graph() {
        let config = serde_json::to_value(Config::default()).expect("serialize default config");
        let (engine, url) =
            configured_search_url(&config, None, "ferric browser").expect("resolve default search");
        assert_eq!(engine, "ddg");
        assert!(url.contains("ferric%20browser"));
    }
}

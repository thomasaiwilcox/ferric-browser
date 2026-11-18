//! Read-only catalogue projection over the existing source resolver.
use crate::{CxxQtType, Value, qobject};
use ferric_browser_application::ConfigurationLayers;
use ferric_browser_config::{
    PolicySourceIdentity, SitePolicySourceRevisions, SiteRule, resolve_site_policy_rules,
};
use serde_json::json;

fn source_token(source: &PolicySourceIdentity) -> String {
    match source {
        PolicySourceIdentity::Authored { document_id } => format!("authored:{document_id}"),
        PolicySourceIdentity::Generated => "generated".into(),
        PolicySourceIdentity::Temporary => "temporary".into(),
        _ => "in-memory".into(),
    }
}

pub(super) fn rules(
    layers: &ConfigurationLayers,
    revision: u64,
    url: &str,
    all: bool,
) -> Result<Value, String> {
    let resolved = resolve_site_policy_rules(
        &layers.base,
        &layers.base_authored,
        &layers.runtime,
        &layers.temporary,
        SitePolicySourceRevisions {
            authored: revision,
            generated: revision,
            temporary: revision,
        },
    )
    .map_err(|error| error.to_string())?;
    let (_, trace) = resolved.select_with_trace(url);
    let mut entries = Vec::new();
    let mut append = |source: PolicySourceIdentity, raw: &[SiteRule]| {
        for rule in raw {
            let same = |origin: &ferric_browser_config::PolicyFieldOrigin| {
                origin.source() == &source
                    && origin.rule().is_some_and(|entry| entry.id() == rule.id)
            };
            let matched = trace
                .survivors
                .iter()
                .find(|entry| same(&entry.origin))
                .map(|entry| entry.matched);
            if !all && matched != Some(true) {
                continue;
            }
            let shadowed = trace.shadowed.iter().find(|entry| same(&entry.replaced));
            let retired = trace.replaced_arrays.iter().find(|entry| {
                source
                    == PolicySourceIdentity::Authored {
                        document_id: entry.array.document_id,
                    }
            });
            entries.push(json!({"source":source_token(&source),"id":rule.id,"rule":rule,
                "matched":matched,"read_only":!matches!(source,PolicySourceIdentity::Generated|PolicySourceIdentity::Temporary),
                "shadowed_by":shadowed.map(|entry|json!({"source":source_token(entry.replacement.source()),
                    "id":entry.replacement.rule().map(ferric_browser_config::PolicyRuleLocation::id)})),
                "retired_by":retired.map(|entry|format!("authored:{}",entry.replaced_by.document_id))}));
        }
    };
    if layers.base_authored.documents().is_empty() {
        append(PolicySourceIdentity::InMemory, &layers.base.site_rules);
    } else {
        for (index, document) in layers.base_authored.documents().iter().enumerate() {
            append(
                PolicySourceIdentity::Authored {
                    document_id: u32::try_from(index)
                        .map_err(|_| "authored catalogue is too large")?,
                },
                document.site_rules().unwrap_or_default(),
            );
        }
    }
    append(PolicySourceIdentity::Generated, &layers.runtime.site_rules);
    append(
        PolicySourceIdentity::Temporary,
        &layers.temporary.site_rules,
    );
    Ok(json!(entries))
}

impl qobject::BrowserUi {
    pub(super) fn list_site_policies(&self, url: Option<&str>, all: bool) -> Result<Value, String> {
        let current = self.rust().current_url.to_string();
        let url = url.unwrap_or(&current);
        let valid_url = ferric_browser_application::InteractionInspectionRequest::new(url, false);
        if !all || url != current {
            valid_url.as_ref().map_err(Clone::clone)?;
        }
        let url = valid_url.is_ok().then_some(url);
        let state = self.rust().state.as_ref().ok_or("profile is unavailable")?;
        let inputs = state.configuration_inspection_inputs();
        let sources = inputs.sources();
        let desired = rules(
            sources.desired_layers(),
            sources.revision(),
            url.unwrap_or(""),
            all,
        )?;
        let active = rules(
            sources.reload_layers(),
            sources.reload_revision(),
            url.unwrap_or(""),
            all,
        )?;
        let saved = if state
            .profile_snapshot()
            .is_some_and(|profile| profile.durable)
        {
            Some(
                state
                    .capture_saved_runtime_edits()
                    .map_err(|error| error.to_string())
                    .map(|capture| {
                        json!({
                            "profile_revision":capture.shared_profile_revision(),
                            "configuration_revision":capture.configuration_revision(),
                            "generated":capture.layers().runtime.site_rules
                        })
                    }),
            )
        } else {
            None
        };
        let drafts = state.site_policy_draft_summaries()?;
        let profile_instance = inputs.profile_instance().map(|id| id.to_string());
        if !state.configuration_inspection_is_current(&inputs) {
            return Err("configuration changed during Site Policies inspection".into());
        }
        Ok(
            json!({"catalogue":true,"profile_instance":profile_instance,"drafts":drafts,
            "url":url,"all":all,"configuration_revision":sources.revision(),
            "active_revision":sources.reload_revision(),"desired":desired,"active":active,
            "saved":saved.as_ref().and_then(|value|value.as_ref().ok()),
            "saved_error":saved.as_ref().and_then(|value|value.as_ref().err()),
            "persistence":if saved.is_some(){"profile"}else{"session-only"}}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferric_browser_config::Config;

    #[test]
    fn list_does_not_revive_a_shadowed_rule_when_its_replacement_does_not_match() {
        let mut layers = ConfigurationLayers::new(Config::default());
        let rule = |pattern: &str| SiteRule {
            id: "player".into(),
            pattern: pattern.into(),
            priority: 0,
            set: std::collections::BTreeMap::new(),
            interaction: None,
        };
        layers.base.site_rules.push(rule("https://example.test/*"));
        layers.runtime.site_rules.push(rule("https://other.test/*"));
        assert_eq!(
            rules(&layers, 7, "https://example.test/", false).unwrap(),
            json!([])
        );
        let all = rules(&layers, 7, "https://example.test/", true).unwrap();
        assert_eq!(all[0]["matched"], Value::Null);
        assert_eq!(all[0]["shadowed_by"]["source"], "generated");
        assert_eq!(all[1]["matched"], false);
    }
}

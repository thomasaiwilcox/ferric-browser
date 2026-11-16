//! Explicit read-only protocol projection. Profile storage roots and source
//! filesystem paths never enter the default or detailed query response.
use ferric_browser_application::{
    ConfigurationInspectionCandidateStage, InspectedSettingValue, InspectionBoundary,
    InspectionValidationStage, SettingActivationBoundary, SettingExplanation, SettingInspection,
    SettingInspectionPending,
};
use ferric_browser_config::{
    HintSelectorFieldOrigins, HintSelectorInputField, HintSelectorValueOrigin, PolicyFieldOrigin,
    PolicySourceIdentity, SitePolicySelectionTrace,
};
use serde_json::{Value, json};

pub(super) fn project(
    report: &SettingInspection,
    sequence: u64,
    explain: bool,
) -> Result<Value, String> {
    let active = report.active.as_ref().map_err(Clone::clone)?;
    let (source, scope) = summary(active);
    let pending = matches!(report.pending, SettingInspectionPending::Pending { .. });
    let desired = report.desired.as_ref().ok();
    let mut result = json!({
        "sequence":sequence, "key":report.key, "value":active.value,
        "source":source, "scope":scope, "apply_time":report.metadata.apply_time,
        "supported_scopes":report.metadata.supported_scopes,
        "site_scope":if report.metadata.supported_scopes.contains(&"site") {
            json!({"supported":true,"mechanism":"validated-site-rule"})
        } else { json!({"supported":false,"mechanism":"profile-or-global-value",
            "reason":"changing an engine-wide value on tab focus would leak across tabs"}) },
        "metadata":{"type":report.metadata.value_type,"default":report.metadata.default_value,
            "supported_scopes":report.metadata.supported_scopes,"apply_time":report.metadata.apply_time,
            "prerequisite":report.metadata.prerequisite,"sensitivity":report.metadata.sensitivity},
        "restart_required":report.metadata.apply_time=="restart" || matches!(report.pending,SettingInspectionPending::Pending{boundary:SettingActivationBoundary::Restart,..}),
        "reload_required":matches!(report.metadata.apply_time,"reload"|"startup") || matches!(report.pending,SettingInspectionPending::Pending{boundary:SettingActivationBoundary::ConfigurationReload|SettingActivationBoundary::Startup,..}),
        "pending":pending, "pending_state":match report.pending {
            SettingInspectionPending::Unchanged=>"unchanged", SettingInspectionPending::Pending{..}=>"pending",
            SettingInspectionPending::Unknown=>"unknown" },
        "pending_boundary":match report.pending { SettingInspectionPending::Pending{boundary,..}=>Some(activation(boundary)), _=>None },
        "pending_value":if pending { desired.and_then(|desired| desired.value.as_ref()) } else { None },
        "desired_value":desired.and_then(|desired| desired.value.as_ref()),
        "desired_error":report.desired.as_ref().err(), "provenance_diagnostic":active.provenance_diagnostic,
        "configuration_revision":report.configuration_revision,"reload_revision":report.reload_revision,
        "startup_revision":report.startup_revision,"candidate_revision":report.candidate_revision,
        "candidate_stage":report.candidate_stage.map(|stage| match stage {
            ConfigurationInspectionCandidateStage::Unprepared=>"unprepared",
            ConfigurationInspectionCandidateStage::Prepared=>"prepared",
            ConfigurationInspectionCandidateStage::WritePending=>"write-pending" }),
        "profile":{"name":report.profile.name,"generation":report.profile.generation,"instance":report.profile_instance.to_string()},
        "validation":validation(active), "desired_validation":desired.map(validation),
    });
    if explain {
        result["explanation"] = active.explanation.as_ref().map_or(Value::Null, |value| {
            explanation(value, InspectionBoundary::Reload)
        });
        if let Some(detail) = result["explanation"].as_object_mut() {
            detail.insert("winner".into(), json!(source));
            detail.insert("scope".into(), json!(scope));
        }
        result["desired_explanation"] = desired
            .and_then(|value| value.explanation.as_ref())
            .map_or(Value::Null, |value| {
                explanation(
                    value,
                    if report.candidate_stage.is_some() {
                        InspectionBoundary::Candidate
                    } else {
                        InspectionBoundary::Desired
                    },
                )
            });
    }
    Ok(result)
}

fn activation(boundary: SettingActivationBoundary) -> &'static str {
    match boundary {
        SettingActivationBoundary::Publication => "publication",
        SettingActivationBoundary::ConfigurationReload => "reload",
        SettingActivationBoundary::Startup => "startup",
        SettingActivationBoundary::Restart => "restart",
    }
}

fn validation(value: &InspectedSettingValue) -> Value {
    let stage = |stage| match stage {
        InspectionValidationStage::Passed => "passed",
        InspectionValidationStage::Unknown => "unknown",
    };
    json!({"raw_static":stage(value.validation.raw_static),"effective_combination":stage(value.validation.effective_combination),
        "engine_css":stage(value.validation.engine_css),"live_document":stage(value.validation.live_document)})
}

fn summary(value: &InspectedSettingValue) -> (String, &'static str) {
    let Some(detail) = &value.explanation else {
        return ("unknown".into(), "unknown");
    };
    let origins: Vec<&PolicyFieldOrigin> = match detail {
        SettingExplanation::Selectors { trace, .. } => trace
            .fields
            .mode
            .iter()
            .chain(&trace.fields.selectors)
            .chain(&trace.fields.empty_list)
            .map(|value| value.source.as_ref())
            .collect(),
        SettingExplanation::Proxy { trace, .. } => trace.winning_origin().into_iter().collect(),
        SettingExplanation::Ordinary { fields, .. } => fields
            .values()
            .filter_map(|origin| origin.as_ref().map(|origin| origin.origin.as_ref()))
            .collect(),
    };
    let scope = if origins.iter().any(|origin| origin.rule().is_some()) {
        "site"
    } else if !origins.is_empty()
        && origins
            .iter()
            .all(|origin| matches!(origin.source(), PolicySourceIdentity::Profile { .. }))
    {
        "profile"
    } else {
        "global"
    };
    let source = if let Some(first) = origins.first() {
        if origins.iter().all(|origin| origin == first)
            && !matches!(detail,SettingExplanation::Ordinary{fields,..} if fields.values().any(Option::is_none))
        {
            source_label(first)
        } else {
            "composed".into()
        }
    } else {
        "default".into()
    };
    (source, scope)
}

fn source_label(origin: &PolicyFieldOrigin) -> String {
    if let Some(rule) = origin.rule() {
        return format!("site-rule:{}", rule.id());
    }
    match origin.source() {
        PolicySourceIdentity::InMemory => "in-memory".into(),
        PolicySourceIdentity::Authored { document_id } => format!("authored:{document_id}"),
        PolicySourceIdentity::Profile { .. } => "profile".into(),
        PolicySourceIdentity::Generated => "runtime".into(),
        PolicySourceIdentity::CommandLine => "cli".into(),
        PolicySourceIdentity::Temporary => "temporary".into(),
    }
}

pub(super) fn boundary(value: InspectionBoundary) -> &'static str {
    match value {
        InspectionBoundary::Desired => "desired",
        InspectionBoundary::Reload => "reload",
        InspectionBoundary::Startup => "startup",
        InspectionBoundary::Candidate => "candidate",
    }
}

pub(super) fn origin(value: &PolicyFieldOrigin, at: InspectionBoundary) -> Value {
    let source = match value.source() {
        PolicySourceIdentity::InMemory => json!({"kind":"in-memory"}),
        PolicySourceIdentity::Authored { document_id } => {
            json!({"kind":"authored","document_id":document_id})
        }
        PolicySourceIdentity::Profile { name } => json!({"kind":"profile","name":name}),
        PolicySourceIdentity::Generated => json!({"kind":"generated"}),
        PolicySourceIdentity::CommandLine => json!({"kind":"command-line"}),
        PolicySourceIdentity::Temporary => json!({"kind":"temporary"}),
    };
    json!({"source":source,"source_revision":value.source_revision(),"boundary":boundary(at),
        "rule":value.rule().map(|rule| json!({"id":rule.id(),"authored_index":rule.authored_index(),"priority":rule.priority()}))})
}

fn selector_origin(value: &HintSelectorValueOrigin, at: InspectionBoundary) -> Value {
    let input = match value.input {
        HintSelectorInputField::Mode => json!({"field":"mode"}),
        HintSelectorInputField::Selectors => json!({"field":"selectors"}),
        HintSelectorInputField::Selector(index) => json!({"field":"selectors","index":index}),
        HintSelectorInputField::DefaultSelectors => json!({"field":"selectors","default":true}),
    };
    json!({"origin":origin(&value.source,at),"input":input})
}

fn selector_fields(fields: &HintSelectorFieldOrigins, at: InspectionBoundary) -> Value {
    json!({"mode":fields.mode.as_ref().map(|value| selector_origin(value,at)),
        "selectors":fields.selectors.iter().map(|value| selector_origin(value,at)).collect::<Vec<_>>(),
        "empty_list":fields.empty_list.as_ref().map(|value| selector_origin(value,at))})
}

pub(super) fn selection(value: &SitePolicySelectionTrace, at: InspectionBoundary) -> Value {
    let array = |array: ferric_browser_config::AuthoredSitePolicyArray| {
        json!({
            "document_id":array.document_id,"source_revision":array.source_revision,"boundary":boundary(at)
        })
    };
    json!({"survivors":value.survivors.iter().map(|entry|json!({"origin":origin(&entry.origin,at),"matched":entry.matched})).collect::<Vec<_>>(),
        "shadowed":value.shadowed.iter().map(|entry|json!({"replaced":origin(&entry.replaced,at),"replacement":origin(&entry.replacement,at)})).collect::<Vec<_>>(),
        "authored_array":value.authored_array.map(array),
        "replaced_arrays":value.replaced_arrays.iter().map(|entry|json!({
            "reason":"array-replaced","array":array(entry.array),"replaced_by":array(entry.replaced_by),
            "rules":entry.rules.iter().map(|rule|json!({"origin":origin(&rule.origin,at),"raw":rule.raw})).collect::<Vec<_>>()
        })).collect::<Vec<_>>()})
}

fn explanation(value: &SettingExplanation, ordinary_rules: InspectionBoundary) -> Value {
    match value {
        SettingExplanation::Selectors {
            boundary: at,
            trace,
            selection: selected,
        } => {
            let contributors = trace.contributions.iter().map(|entry| {
                use ferric_browser_config::HintSelectorContributionOutcome as Outcome;
                let (status, diagnostic) = match &entry.outcome { Outcome::Applied=>("applied",None), Outcome::EffectiveOverflow(error)=>("effective-overflow",Some(error)), Outcome::BlockedByEarlierOverflow=>("blocked-by-earlier-overflow",None) };
                json!({"origin":origin(&entry.source,*at),"value":entry.policy,"selectors_explicit":entry.selectors_explicit,"status":status,"diagnostic":diagnostic,
                    "duplicates":entry.duplicates.iter().map(|duplicate|json!({"input_index":duplicate.input_index,"retained_index":duplicate.retained_index,
                        "retained_origin":selector_origin(&duplicate.retained_origin,*at)})).collect::<Vec<_>>()})
            }).collect::<Vec<_>>();
            json!({"kind":"selectors","seed":selector_fields(&trace.seed,*at),"fields":selector_fields(&trace.fields,*at),"contributors":contributors,"selection":selection(selected,*at)})
        }
        SettingExplanation::Proxy {
            globals,
            rules,
            trace,
            selection: selected,
        } => {
            let entries = trace
                .seed
                .iter()
                .chain(&trace.contributions)
                .collect::<Vec<_>>();
            json!({"kind":"proxy","fields":{"value":trace.winning_origin().map(|value|origin(value,if value.rule().is_some(){*rules}else{*globals}))},
                "contributors":entries.iter().enumerate().map(|(index,entry)|json!({"origin":origin(&entry.origin,if entry.origin.rule().is_some(){*rules}else{*globals}),
                    "value":entry.value,"status":if index+1==entries.len(){"winner"}else{"replaced"}})).collect::<Vec<_>>(),"selection":selection(selected,*rules)})
        }
        SettingExplanation::Ordinary {
            fields,
            contributions,
            selection: selected,
        } => json!({"kind":"ordinary",
            "fields":fields.iter().map(|(path,value)|json!({"path":path,"origin":value.as_ref().map(|value|origin(&value.origin,value.boundary))})).collect::<Vec<_>>(),
            "contributors":contributions.iter().map(|entry|json!({"path":entry.path,"origin":origin(&entry.origin.origin,entry.origin.boundary),"value":entry.input,"status":"observed"})).collect::<Vec<_>>(),
            "selection":selected.as_ref().map(|selected| selection(selected,ordinary_rules))}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferric_browser_application::{BrowserApplication, ProfileOpenRequest, ProfileStorage};
    use ferric_browser_config::{Config, ContextsConfig, RuntimeOverrides};
    use std::{
        fs,
        time::{Duration, Instant},
    };

    fn application(raw: &str) -> BrowserApplication {
        let dir =
            std::env::temp_dir().join(format!("ferric-query-projection-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("config.toml"), format!("schema_version=3\n{raw}")).unwrap();
        let loaded = ferric_browser_config::load(dir.join("config.toml")).unwrap();
        let mut app = BrowserApplication::new(Config::default());
        app.request_profile_open(ProfileOpenRequest {
            name: "work".into(),
            label: "Work".into(),
            privacy: crate::PrivacyKind::Private,
            storage: ProfileStorage::Transient,
            base_configuration: loaded.config,
            base_authored: loaded.authored,
            initial_profile_overrides: RuntimeOverrides::default(),
            command_line_overrides: RuntimeOverrides::default(),
            config_path: None,
            script_catalog_root: None,
            contexts: ContextsConfig::default(),
            context_configuration_error: None,
        })
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(result) = app.poll_profile_open() {
                assert!(result.unwrap().configuration_error.is_none());
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        fs::remove_dir_all(dir).unwrap();
        app
    }

    fn report(raw: &str, key: &str) -> SettingInspection {
        application(raw)
            .configuration_inspection_inputs()
            .inspect_setting(
                &ferric_browser_application::SettingInspectionRequest::new(key, None, true)
                    .unwrap(),
            )
            .unwrap()
    }

    #[test]
    fn mixed_explicit_and_typed_default_fields_are_composed() {
        let report = report(
            "[action_targets.viewer]\nsubject_types=['url']\nexecutable='mpv'\n",
            "action_targets.viewer",
        );
        let value = project(&report, 7, true).unwrap();
        assert_eq!(value["source"], "composed");
        assert!(
            value["explanation"]["fields"]
                .as_array()
                .unwrap()
                .iter()
                .any(|field| field["origin"].is_null())
        );
        let text = serde_json::to_string(&value).unwrap();
        assert!(!text.contains("ferric-query-projection-"));
        assert!(value["profile"].get("roots").is_none());
    }

    #[test]
    fn valid_large_setting_get_delivers_a_bounded_rpc_failure_without_truncation() {
        use ferric_browser_ipc::{PendingRequest, Request, Response, read_frame};
        use std::os::unix::net::UnixStream;

        let mut argv = vec!["editor".to_owned()];
        argv.extend((0..254).map(|_| "x".repeat(4096)));
        argv.push("{file}".into());
        let raw = format!(
            "[tools]\neditor={}\n",
            serde_json::to_string(&argv).unwrap()
        );
        let report = report(&raw, "tools.editor");
        assert_eq!(report.pending, SettingInspectionPending::Unchanged);
        let value = project(&report, 10, false).unwrap();
        assert_eq!(value["value"], json!(argv));
        assert_eq!(value["desired_value"], value["value"]);
        let response = Response::success("large-setting", value);
        assert!(matches!(
            ferric_browser_ipc::serialize_response(&response),
            Err(ferric_browser_ipc::FrameError::TooLarge(_))
        ));
        let (server, mut client) = UnixStream::pair().unwrap();
        client
            .set_read_timeout(Some(std::time::Duration::from_secs(1)))
            .unwrap();
        PendingRequest::new(
            Request {
                id: "large-setting".into(),
                method: "config.get".into(),
                params: json!({"key":"tools.editor"}),
            },
            server,
        )
        .respond(&response)
        .unwrap();
        let received: Response =
            serde_json::from_slice(&read_frame(&mut client).unwrap().unwrap()).unwrap();
        assert_eq!(received.id, "large-setting");
        assert!(received.result.is_none());
        assert_eq!(
            received.error.unwrap().details.unwrap()["reason"],
            "response_too_large"
        );
    }

    #[test]
    fn default_selector_is_null_and_explanations_are_opt_in() {
        let report = report("", "hints.selectors.buttons");
        let value = project(&report, 8, false).unwrap();
        assert!(value["value"].is_null());
        assert_eq!(value["source"], "default");
        assert_eq!(value["pending"], false);
        assert!(value.get("explanation").is_none());
        assert!(value.get("desired_explanation").is_none());
        assert_eq!(value["validation"]["engine_css"], "unknown");
        assert_eq!(value["validation"]["live_document"], "unknown");
    }

    #[test]
    fn include_array_retirement_is_explained_without_reviving_rules_or_paths() {
        let child = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../ferric-browser-config/tests/fixtures/site-policy/includes/included.toml"
        );
        let raw = format!(
            "include=[{}]\nsite_rules=[]\n[hints.selectors.links]\nmode='replace'\nselectors=[]\n",
            toml::Value::String(child.into())
        );
        let report = report(&raw, "hints.selectors.links");
        let simple = project(&report, 11, false).unwrap();
        assert!(simple.get("explanation").is_none());
        let detailed = project(&report, 11, true).unwrap();
        assert_eq!(detailed["value"]["selectors"], json!([]));
        let selected = &detailed["explanation"]["selection"];
        assert_eq!(selected["survivors"], json!([]));
        assert_eq!(selected["shadowed"], json!([]));
        assert_eq!(selected["authored_array"]["document_id"], 1);
        assert_eq!(selected["authored_array"]["boundary"], "reload");
        let retired = &selected["replaced_arrays"][0];
        assert_eq!(retired["reason"], "array-replaced");
        assert_eq!(retired["array"]["document_id"], 0);
        assert_eq!(retired["replaced_by"]["document_id"], 1);
        assert!(retired["replaced_by"].get("rule").is_none());
        assert_eq!(retired["rules"][0]["raw"]["id"], "hidden");
        assert_eq!(
            retired["rules"][0]["raw"]["interaction"]["anchors"][0]["id"],
            "hidden-control"
        );
        assert!(!serde_json::to_string(&detailed).unwrap().contains(child));
        assert_eq!(detailed["validation"]["engine_css"], "unknown");
    }

    #[test]
    fn desired_failure_does_not_replace_known_active_or_claim_no_pending_state() {
        let mut report = report("", "content.images");
        report.desired = Err("prospective combination conflict".into());
        report.pending = SettingInspectionPending::Unknown;
        let value = project(&report, 9, false).unwrap();
        assert_eq!(value["value"], true);
        assert_eq!(value["pending_state"], "unknown");
        assert_eq!(value["desired_error"], "prospective combination conflict");
        report.active = Err("active combination unavailable".into());
        assert_eq!(
            project(&report, 10, false).unwrap_err(),
            "active combination unavailable"
        );
    }

    #[test]
    fn live_setting_with_a_pending_site_rule_reports_actual_reload_requirement() {
        let mut app = application("");
        let mut layers = app.configuration_layers().clone();
        layers
            .runtime
            .site_rules
            .push(ferric_browser_config::SiteRule {
                id: "pending".into(),
                pattern: "https://example.test/*".into(),
                priority: 0,
                interaction: None,
                set: std::collections::BTreeMap::from([(
                    "content.images".into(),
                    toml::Value::Boolean(false),
                )]),
            });
        app.update_configuration(layers).unwrap();
        let report = app
            .configuration_inspection_inputs()
            .inspect_setting(
                &ferric_browser_application::SettingInspectionRequest::new(
                    "content.images",
                    Some("https://example.test/page"),
                    true,
                )
                .unwrap(),
            )
            .unwrap();
        let value = project(&report, 11, true).unwrap();
        assert_eq!(value["value"], true);
        assert_eq!(value["pending_value"], false);
        assert_eq!(value["pending_boundary"], "reload");
        assert_eq!(value["reload_required"], true);
        assert_ne!(value["metadata"]["apply_time"], "reload");
        assert_eq!(value["explanation"]["winner"], value["source"]);
        assert_eq!(value["explanation"]["scope"], value["scope"]);
    }
}

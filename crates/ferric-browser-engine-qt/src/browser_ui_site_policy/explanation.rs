//! Explicit diagnostic projection. No page text, discovery, input submission or
//! configuration publication occurs here.
use crate::configuration_inspection_projection::{boundary, origin, selection};
use crate::{CxxQtType, QString, Value, qobject};
use ferric_browser_application::{
    InspectedInteractionValue, InspectionOrigin, InspectionValidationStage,
    InteractionCompilationState, InteractionInspectionExplanation, InteractionInspectionRequest,
    SettingInspectionPending,
};
use ferric_browser_config::{
    EffectiveInteractionLabels, InteractionContributionPatch, InteractionField,
    InteractionPolicyPatch, InteractionRemapContext,
};
use ferric_browser_core::{
    HintCodeDeclaration, HintReservationCategory, HintReservationReason, HintReservationState,
    LogicalKeyToken, SitePolicyExplanation, SitePolicyRuleRef,
};
use serde_json::json;

fn inspected_origin(value: &InspectionOrigin) -> Value {
    origin(&value.origin, value.boundary)
}

fn labels(value: &EffectiveInteractionLabels) -> Value {
    json!({"alphabet":value.alphabet(),"min_chars":value.min_chars(),
        "fallback_prefix":value.fallback_prefix(),"excluded_chars":value.excluded_chars(),
        "auto_follow":value.auto_follow()})
}

fn declaration(value: &HintCodeDeclaration) -> Value {
    json!({"kind":match value.category {
        HintReservationCategory::Anchor=>"anchor",HintReservationCategory::Collection=>"collection"},
        "id":value.rule_id,"slot":value.slot,"rank":value.slot+1,"code":value.code})
}

fn codebook(
    book: &ferric_browser_core::CheckedHintCodebook,
    code: Option<&str>,
    labels: &EffectiveInteractionLabels,
) -> Value {
    json!({"configured_alphabet":book.configured_alphabet(),"effective_alphabet":book.effective_alphabet(),
        "capacity_by_suffix_width":(1..=8).map(|width|{
            let total=labels.fallback_prefix().len()+usize::from(width);
            json!({"suffix_width":width,"capacity":book.capacity(width),
                "eligible_for_initial_allocation":total>=usize::from(labels.min_chars()) && total<=8})
        }).collect::<Vec<_>>(),
        "reservations":book.declarations().iter().filter(|entry|code.is_none_or(|code|entry.code==code))
            .map(declaration).collect::<Vec<_>>(),
        "requested_code":code.map(|code|json!({"code":code,
            "reserved":book.declarations().iter().any(|entry|entry.code==code),
            "reservation_prefix_overlap":book.declarations().iter().any(|entry|
                entry.code.starts_with(code)||code.starts_with(&entry.code)),
            "excluded_characters":code.chars().filter_map(|character|book.exclusion(character).map(|reason|
                json!({"character":character.to_string(),"reason":match reason {
                    ferric_browser_core::HintCodeExclusion::Control=>"control",
                    ferric_browser_core::HintCodeExclusion::Configured=>"configured",
                    ferric_browser_core::HintCodeExclusion::PageSource=>"page-source"}
                }))).collect::<Vec<_>>() })),"live_document":"not-checked"})
}

fn explanation(value: &InteractionInspectionExplanation, subject: &SitePolicyExplanation) -> Value {
    let values = &value.values;
    let code = match subject {
        SitePolicyExplanation::Code(code) => Some(code.as_str()),
        _ => None,
    };
    json!({
        "values":{"enabled":values.enabled,"labels":labels(&values.labels),
            "ordinary_labels":labels(&values.ordinary_labels),"anchors":values.anchors,
            "collections":values.collections,"hint_page_keys":values.hint_page_keys,
            "page_receiver":values.page_receiver,"remaps":values.remaps},
        "fields":value.fields.iter().map(|(field,source)|json!({"path":field.path(),
            "origin":source.as_ref().map(inspected_origin),"typed_default":source.is_none()})).collect::<Vec<_>>(),
        "ordinary_labels":value.ordinary_labels.iter().map(|(field,source)|json!({
            "path":InteractionField::Labels(*field).path(),"origin":source.as_ref().map(inspected_origin),
            "typed_default":source.is_none()})).collect::<Vec<_>>(),
        "contributions":value.contributions.iter().map(|entry|json!({"origin":inspected_origin(&entry.origin),
            "patch":match &entry.patch {
                InteractionContributionPatch::SharedLabels(patch)=>json!({"kind":"shared-labels","value":patch}),
                InteractionContributionPatch::SiteSection(patch)=>json!({"kind":"site-section","value":patch})}
        })).collect::<Vec<_>>(),
        "selection":selection(&value.selection,value.rules_boundary),
        "rules_boundary":boundary(value.rules_boundary),"globals_boundary":boundary(value.globals_boundary),
        "codebooks":value.codebooks.iter().map(|(family,book)|{
            let mut value=codebook(book,code,&values.labels);
            value["family"]=json!(family);
            value
        }).collect::<Vec<_>>()
    })
}

fn key_contexts(
    value: &InteractionInspectionExplanation,
    key: &LogicalKeyToken,
) -> Result<Value, String> {
    let values = &value.values;
    let keys = InteractionPolicyPatch {
        hint_page_keys: Some(values.hint_page_keys.clone()),
        remaps: Some(values.remaps.clone()),
        ..InteractionPolicyPatch::default()
    }
    .compile_authored_keys()?;
    let contexts = [
        InteractionRemapContext::NormalPageKey,
        InteractionRemapContext::HintPageKey,
        InteractionRemapContext::PassthroughOnce,
        InteractionRemapContext::Passthrough,
    ];
    Ok(
        json!({"source":key.as_str(),"protected_recovery":key.is_protected_rescue(),
        "enabled":values.enabled,"receiver":values.page_receiver,
        "contexts":contexts.into_iter().map(|context|{
            let remap = keys.remap(context,key);
            let destination = remap.map_or(key,ferric_browser_config::CompiledInteractionRemap::destination);
            json!({"context":context,"remap_id":remap.map(ferric_browser_config::CompiledInteractionRemap::id),
                "source":key.as_str(),"destination":destination.as_str(),"transformed":destination!=key,
                "source_admission":match context {
                    InteractionRemapContext::NormalPageKey=>"requires-normal-mixed-page-key",
                    InteractionRemapContext::HintPageKey=>if keys.hint_page_keys().contains(key){"configured"}else{"not-configured"},
                    _=>"requires-explicit-passthrough-mode"},
                "qualification":crate::page_key_capabilities::current_assessment(destination,destination!=key,
                    context,crate::page_key_capabilities::receiver_scope(&values.page_receiver))})
        }).collect::<Vec<_>>() }),
    )
}

fn inspected(
    value: &Result<InspectedInteractionValue, String>,
    subject: &SitePolicyExplanation,
) -> Value {
    let value = match value {
        Ok(value) => value,
        Err(error) => return json!({"error":error}),
    };
    let validation = |stage| match stage {
        InspectionValidationStage::Passed => "passed",
        InspectionValidationStage::Unknown => "unknown",
    };
    json!({"state":match value.state { InteractionCompilationState::Disabled=>"disabled",
        InteractionCompilationState::NoOp=>"no-op",InteractionCompilationState::Conflicted=>"conflicted",InteractionCompilationState::Ready=>"ready"},
        "enabled":value.enabled,"anchor_count":value.anchor_count,"collection_count":value.collection_count,
        "hint_page_key_count":value.hint_page_key_count,"remap_count":value.remap_count,
        "validation":{"raw_static":validation(value.validation.raw_static),
            "effective_combination":validation(value.validation.effective_combination),
            "engine_css":validation(value.validation.engine_css),"live_document":validation(value.validation.live_document)},
        "explanation":value.explanation.as_ref().map(|value|explanation(value,subject)),
        "key":match subject { SitePolicyExplanation::Key(key)=>value.explanation.as_ref().map(|value|
            match key_contexts(value,key){Ok(value)=>value,Err(error)=>json!({"error":error})}), _=>None }})
}

fn reservation_state(state: HintReservationState) -> Value {
    let reason = |reason| match reason {
        HintReservationReason::Missing => "missing",
        HintReservationReason::Ambiguous => "ambiguous",
        HintReservationReason::Incomplete => "incomplete",
        HintReservationReason::AnchorShadowed => "anchor-shadowed",
        HintReservationReason::Stale => "stale",
        HintReservationReason::MeaningChanged => "meaning-changed",
    };
    match state {
        HintReservationState::PendingInitial => json!({"status":"pending"}),
        HintReservationState::Live => json!({"status":"observed-live"}),
        HintReservationState::Unavailable(why) => {
            json!({"status":"unavailable","reason":reason(why)})
        }
        HintReservationState::Retired(why) => json!({"status":"retired","reason":reason(why)}),
        HintReservationState::Consumed => json!({"status":"consumed"}),
    }
}

impl qobject::BrowserUi {
    fn observed_hint_codes(&self, code: Option<&str>, same_url: bool) -> Value {
        let rust = self.rust();
        if !same_url || !crate::hint_request_capture::retained_request_is_current(rust) {
            return json!({"status":"unavailable","reason":"no current Hint session for this page"});
        }
        let Some(input) = &rust.interaction.hint_interaction else {
            return json!({"status":"unavailable","reason":"Hint discovery is not complete"});
        };
        json!({"status":"observed-session","requires_freshness_check":true,
            "family":rust.interaction.hint_policy_proof.as_ref().map(|proof|proof.family()),
            "ordinary_codes_remaining":input.ordinary_codes_remaining(),
            "ordinary_suffix_width":input.ordinary_suffix_width(),
            "refresh_required":input.snapshot().refresh_required,
            "reservations":input.reservations().iter().filter(|entry|
                code.is_none_or(|code|entry.declaration().code==code)).map(|entry|
                json!({"declaration":declaration(entry.declaration()),"observation":reservation_state(entry.state())})).collect::<Vec<_>>()})
    }

    fn exact_rule_explanation(
        &self,
        target: &SitePolicyRuleRef,
        url: &str,
    ) -> Result<Value, crate::PublicError> {
        let state = self.rust().state.as_ref().ok_or("profile is unavailable")?;
        let sources = state.configuration_policy_sources();
        let select = |layers: &ferric_browser_application::ConfigurationLayers,
                      revision|
         -> Result<Option<Value>, String> {
            let rows = super::catalogue::rules(layers, revision, url, true)?;
            Ok(rows
                .as_array()
                .and_then(|rows| {
                    rows.iter().find(|row| {
                        row["source"] == target.source.token() && row["id"] == target.id
                    })
                })
                .cloned())
        };
        let desired = select(sources.desired_layers(), sources.revision())?;
        let active = select(sources.reload_layers(), sources.reload_revision())?;
        let saved = if state
            .profile_snapshot()
            .is_some_and(|profile| profile.durable)
        {
            Some(
                state
                    .capture_saved_runtime_edits()
                    .map_err(|error| error.to_string())
                    .and_then(|capture| select(capture.layers(), capture.configuration_revision())),
            )
        } else {
            None
        };
        if desired.is_none()
            && active.is_none()
            && saved
                .as_ref()
                .and_then(|saved| saved.as_ref().ok())
                .is_none_or(Option::is_none)
        {
            if let Some(Err(error)) = &saved {
                return Err(crate::PublicError::new(
                    ferric_browser_ipc::ErrorCode::Busy,
                    "The saved policy source is unavailable. Try again.",
                    error.clone(),
                ));
            }
            return Err(crate::PublicError::new(
                ferric_browser_ipc::ErrorCode::NotFound,
                "The rule does not exist in the selected source.",
                "exact source/ID is absent from active, desired and saved catalogues",
            ));
        }
        Ok(
            json!({"source":target.source.token(),"id":target.id,"desired":desired,"active":active,
            "saved":saved.as_ref().and_then(|saved|saved.as_ref().ok()),
            "saved_error":saved.as_ref().and_then(|saved|saved.as_ref().err())}),
        )
    }

    pub(super) fn explain_site_policy(
        &self,
        url: Option<&str>,
        rule: Option<&SitePolicyRuleRef>,
        subject: &SitePolicyExplanation,
    ) -> Result<Value, crate::PublicError> {
        let current = self.rust().current_url.to_string();
        let url = url.unwrap_or(&current);
        let request = InteractionInspectionRequest::new(url, true);
        let raw = rule
            .map(|rule| self.exact_rule_explanation(rule, if request.is_ok() { url } else { "" }))
            .transpose()?;
        let request = match request {
            Ok(request) => request,
            Err(error) if raw.is_some() && *subject == SitePolicyExplanation::Effective => {
                return Ok(json!({"rule":raw,"context_error":error,"activation_permitted":false}));
            }
            Err(error) => return Err(super::invalid_request(error)),
        };
        let report = self.inspect_configuration_interaction(&request)?;
        let live_key = match subject {
            SitePolicyExplanation::Key(key) if url == current => json!({
                "binding":self.ipc_bindings_explain(&json!({"keychain":key.as_str()}))?,
                "page_route":self.current_page_key_route(&QString::from(key.as_str())).map(|mut route|{
                    if let Some(route)=route.as_object_mut(){ route.remove("receiver");route.remove("hint_context");route.remove("destination_key"); }
                    route
                }),"raw_passthrough":self.rust().raw_passthrough}),
            SitePolicyExplanation::Key(_) => {
                json!({"status":"unavailable","reason":"requested URL is not the selected page"})
            }
            _ => Value::Null,
        };
        Ok(
            json!({"url":url,"rule":raw,"configuration_revision":report.configuration_revision,
            "active_revision":report.reload_revision,"candidate_revision":report.candidate_revision,
            "pending_state":match report.pending {SettingInspectionPending::Unchanged=>"unchanged",
                SettingInspectionPending::Pending{..}=>"pending",SettingInspectionPending::Unknown=>"unknown"},
            "active":inspected(&report.active,subject),"desired":inspected(&report.desired,subject),
            "live_key":live_key,"live_codes":self.observed_hint_codes(match subject {
                SitePolicyExplanation::Code(code)=>Some(code),_=>None},url==current),
            "activation_permitted":false}),
        )
    }
}

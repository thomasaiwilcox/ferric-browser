//! The existing native collector with a nonactivating management intent.
use super::{
    Arc, BrowserUiRust, CxxQtType, HintRequestCapture, Pin, QString, RetainedHintRequest, qobject,
};
use ferric_browser_config::InteractionFamily;
use ferric_browser_core::{
    HintInteraction, HintRetentionBudget, HintTarget, ShutdownState, TabId, ValidatedUrl,
};

pub(super) fn is_current(request: &RetainedHintRequest, rust: &BrowserUiRust) -> bool {
    let (Some(preview), Some(pending), Some(state), Some(window)) = (
        &request.preview,
        &rust.configuration_io.site_policy_preview,
        &rust.state,
        rust.window,
    ) else {
        return false;
    };
    rust.view_alive
        && !rust.profile_bootstrap_pending
        && !rust.ipc_shutdown_gate
        && matches!(
            rust.core_mode,
            ferric_browser_core::Mode::Normal | ferric_browser_core::Mode::Command
        )
        && state.shutdown() == ShutdownState::Running
        && preview.is_current(state)
        && Arc::ptr_eq(preview, &pending.prepared)
        && pending.last_request == request.generations.request_id
        && pending.accepts(request.family)
        && pending.target.tab == request.tab
        && (!request.capture_only || rust.active_tab_index == request.index)
        && state.capture_target(request.tab) == Some(pending.target)
        && usize::try_from(request.index)
            .ok()
            .and_then(|index| rust.tab_ids.get(index))
            == Some(&request.tab)
        && rust
            .interaction
            .prompt_view_identities
            .get(&request.tab)
            .is_some_and(|known| *known == request.view.to_string())
        && state.tabs().get(&request.tab).is_some_and(|tab| {
            tab.url.as_deref() == preview.receipt().url.as_deref()
                && tab.window == window
                && state
                    .windows()
                    .get(&window)
                    .is_some_and(|owner| owner.profile == tab.profile)
        })
}

pub(crate) fn begin_hint_preview_request(
    mut ui: Pin<&mut qobject::BrowserUi>,
    tab: &QString,
    index: i32,
    view: &QString,
    view_generation: u64,
    preview_id: &QString,
    family: &QString,
) -> Box<HintRequestCapture> {
    capture(
        ui.as_mut().rust_mut().get_mut(),
        tab,
        index,
        view,
        view_generation,
        preview_id,
        family,
    )
    .unwrap_or_else(|| Box::new(HintRequestCapture::default()))
}

fn capture(
    rust: &mut BrowserUiRust,
    tab: &QString,
    index: i32,
    view: &QString,
    view_generation: u64,
    preview_id: &QString,
    family: &QString,
) -> Option<Box<HintRequestCapture>> {
    if view_generation == 0 || view.is_empty() || view.len() > 128 {
        return None;
    }
    let tab = TabId::from_display(&tab.to_string())?;
    let family: InteractionFamily =
        serde_json::from_value(serde_json::json!(family.to_string())).ok()?;
    let pending = rust.configuration_io.site_policy_preview.as_ref()?;
    let capture_only = pending.capture_family.is_some();
    if pending.prepared.receipt().id != preview_id.to_string()
        || pending.target.tab != tab
        || !pending.accepts(family)
    {
        return None;
    }
    let prepared = Arc::clone(&pending.prepared);
    let snapshot = Arc::clone(prepared.snapshot()?);
    let url = ValidatedUrl::parse(prepared.receipt().url.as_deref()?).ok()?;
    let policies = crate::hint_policy_cache::CachedHintPolicies::prepare(
        Arc::clone(&snapshot),
        ferric_browser_application::SitePolicyMatchContext::for_url(&url),
        QString::from(url.as_str()),
        snapshot.sources().revision(),
    )
    .ok()?;
    let family_name = serde_json::to_value(family).ok()?.as_str()?.to_owned();
    let selectors = policies.preview_family(&family_name)?;
    let policy = snapshot.interaction()?.policy()?.clone();
    let proxy = matches!(
        snapshot.discovery().proxy_targets().ok()?,
        ferric_browser_config::HintProxyTargets::Validated
    );
    let target = HintTarget {
        tab,
        generation: pending.target.generation,
        document: pending.target.document,
    };
    let request_id = rust.interaction.next_hint_request_id.checked_add(1)?;
    rust.interaction.next_hint_request_id = request_id;
    rust.configuration_io
        .site_policy_preview
        .as_mut()?
        .last_request = request_id;
    let request = Arc::new(RetainedHintRequest {
        owner: None,
        preview: Some(prepared),
        capture_only,
        snapshot,
        policy: Some(policy),
        selectors: selectors.clone(),
        invocation: super::Invocation {
            options: crate::hint_policy::HintOptions {
                family: family_name,
                target: "open".into(),
                rapid: false,
                script: None,
                first: false,
                index: 0,
            },
            pointer: false,
            proxy,
            search: false,
            search_scope: QString::default(),
        },
        family,
        scope: None,
        target,
        tab,
        index,
        view: view.clone(),
        generations: crate::interaction_proof::AnnotationGenerations {
            selector_revision: selectors.revision,
            view_generation,
            request_id,
            ..Default::default()
        },
    });
    if !is_current(&request, rust) {
        return None;
    }
    let ledger = crate::hint_collection_work::CollectionLedger::new(
        request_id,
        crate::hint_collection_work::VerificationSchedule::graph_documents(),
    )?;
    Some(Box::new(HintRequestCapture {
        request: Some(request),
        ledger: Some(ledger),
        ..Default::default()
    }))
}

fn report(
    ui: &qobject::BrowserUi,
    captured: &mut HintRequestCapture,
) -> Result<(Arc<RetainedHintRequest>, serde_json::Value), String> {
    let request = captured
        .request
        .as_ref()
        .filter(|request| request.preview.is_some() && request.is_current(ui.rust()))
        .cloned()
        .ok_or("preview capture is stale")?;
    if super::classification(ui.rust(), captured) == "ordinary" {
        return Ok((
            request,
            serde_json::json!({"state":"not-requested","reason":"effective Interaction is disabled or has no enhancement for this invocation","live_matched":false}),
        ));
    }
    let (candidates, checked) = captured
        .collection
        .take()
        .ok_or("preview collection is incomplete or already consumed")?;
    crate::hint_payload::validate_hint_candidates_for_family(
        &request.invocation.options.family,
        &candidates,
    )?;
    let compiled = checked
        .policy_snapshot()
        .compile_hint_codebook(request.family)
        .map_err(|error| error.to_string())?;
    let Some(compiled) = compiled else {
        return Ok((
            request,
            serde_json::json!({"state":"collected","candidate_count":candidates.len(),"reservations":[],"enhanced":false,
            "documents":checked.document_count(),"roots":checked.root_count(),"diagnostics":checked.preview_diagnostics()}),
        ));
    };
    let codebook = compiled.into_parts().2;
    let assignments = checked.special_assignments(&candidates, codebook.declarations(), None);
    let model = HintInteraction::new_enhanced(
        candidates,
        codebook,
        assignments,
        HintRetentionBudget::Bytes(crate::interaction_proof::MAX_INTERACTION_ANNOTATION_BYTES),
    )
    .map_err(|error| error.to_string())?;
    let reservations = model.reservations().iter().map(|slot|serde_json::json!({"rule":slot.declaration().rule_id,
        "category":format!("{:?}",slot.declaration().category),"rank":slot.declaration().slot,"code":slot.declaration().code,
        "state":format!("{:?}",slot.state())})).collect::<Vec<_>>();
    Ok((
        Arc::clone(&request),
        serde_json::json!({"state":"collected","reservations":reservations,
        "candidate_count":model.hints().len(),"ordinary_codes_remaining":model.ordinary_codes_remaining(),
        "ordinary_suffix_width":model.ordinary_suffix_width(),"documents":checked.document_count(),"roots":checked.root_count(),
        "diagnostics":checked.preview_diagnostics(),
        "selector_revision":request.generations.selector_revision,"request_id":request.generations.request_id,
        "view_generation":request.generations.view_generation,"activation_permitted":false}),
    ))
}

pub(super) fn publish(
    mut ui: Pin<&mut qobject::BrowserUi>,
    captured: &mut HintRequestCapture,
) -> QString {
    if captured
        .request
        .as_ref()
        .is_some_and(|request| request.capture_only)
    {
        return builder_inventory(ui.as_ref().get_ref(), captured, None);
    }
    let result = report(ui.as_ref().get_ref(), captured).and_then(|(request, result)| {
        let preview = request.preview.as_ref().ok_or("not a preview")?;
        ui.as_mut()
            .record_site_policy_preview(&preview.receipt().id, request.family, result)
    });
    QString::from(
        match result {
            Ok(value) => value,
            Err(error) => serde_json::json!({"error":error}),
        }
        .to_string(),
    )
}

fn builder_inventory(
    ui: &qobject::BrowserUi,
    captured: &HintRequestCapture,
    label: Option<&str>,
) -> QString {
    let result = (|| {
        let request = captured
            .request
            .as_ref()
            .filter(|request| request.capture_only && request.is_current(ui.rust()))
            .ok_or("builder capture is stale")?;
        let (candidates, _) = captured
            .collection
            .as_ref()
            .ok_or("builder collection is incomplete")?;
        let hints = ferric_browser_core::assign_labels(candidates.clone())
            .map_err(|error| error.to_string())?;
        if let Some(label) = label {
            let hint = hints
                .iter()
                .find(|hint| hint.label == label)
                .ok_or("unknown capture label")?;
            return Ok(crate::hint_payload::candidate_json(&hint.candidate));
        }
        let rows = hints
            .iter()
            .map(|hint| {
                let mut candidate = crate::hint_payload::candidate_json(&hint.candidate);
                candidate["text"] = serde_json::json!(if matches!(
                    hint.candidate.kind,
                    ferric_browser_core::HintKind::Input
                        | ferric_browser_core::HintKind::Select
                        | ferric_browser_core::HintKind::Textarea
                        | ferric_browser_core::HintKind::ContentEditable
                        | ferric_browser_core::HintKind::EditorFocus
                        | ferric_browser_core::HintKind::LabelControl
                ) {
                    "Editable control".into()
                } else {
                    hint.candidate.text.chars().take(32).collect::<String>()
                });
                candidate["href"] = serde_json::Value::Null;
                serde_json::json!({"label":hint.label,"candidate":candidate})
            })
            .collect::<Vec<_>>();
        let value = serde_json::json!({"capture":true,"rows":rows,"activation_permitted":false,
            "selector_revision":request.generations.selector_revision.to_string()});
        if value.to_string().len() > crate::interaction_proof::MAX_INTERACTION_ANNOTATION_BYTES {
            return Err(
                "Capture summaries exceed the bound; choose a narrower invocation family".into(),
            );
        }
        Ok::<_, String>(value)
    })();
    QString::from(
        match result {
            Ok(value) => value,
            Err(error) => serde_json::json!({"error":error}),
        }
        .to_string(),
    )
}

pub(crate) fn hint_builder_candidate(
    ui: &qobject::BrowserUi,
    captured: &HintRequestCapture,
    label: &QString,
) -> QString {
    if label.len() > 8 {
        return QString::from(r#"{"error":"invalid capture label"}"#);
    }
    builder_inventory(ui, captured, Some(&label.to_string()))
}

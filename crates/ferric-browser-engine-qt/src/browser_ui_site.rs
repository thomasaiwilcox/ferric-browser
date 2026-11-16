use super::{
    CxxQtType, FocusObservation, Instant, MAX_DIAGNOSTICS_EXPORT_BYTES, Mode, ParsedCommand, Path,
    Pin, PrivacyKind, QString, QStringList, SITE_DOCTOR_EXPERIMENT_TIMEOUT, SiteExperiment, Uuid,
    Value, atomic_write_private, diagnostics, display_url, focus_mode_transition,
    matching_site_rules, qobject, safe_ipc_url, safe_site_host, safe_site_origin,
    setting_supports_site_scope, site_doctor_remaining_seconds, toml_string_array_literal,
};

impl qobject::BrowserUi {
    pub(super) fn page_key_focus_observed_for(
        mut self: Pin<&mut Self>,
        index: i32,
        url: &QString,
        frame_path: &QString,
        epoch: i32,
        receipt: &QString,
    ) {
        if receipt.len() > 4096 {
            return;
        }
        let Ok(receipt) =
            serde_json::from_str::<super::focus_policy::FocusProbeReceipt>(&receipt.to_string())
        else {
            return;
        };
        let Some(target) = self.as_ref().target_for_index(index) else {
            return;
        };
        if receipt.target.tab_id != target.tab.to_string()
            || receipt.target.generation != target.generation.to_string()
            || receipt.target.document_id != target.document.to_string()
        {
            return;
        }
        let Some(state) = receipt.state else {
            if self.as_ref().rust().input_epoch == epoch
                && self.as_ref().rust().active_tab_index == index
                && self.as_ref().rust().current_url.to_string() == url.to_string()
            {
                let mut rust = self.as_mut().rust_mut();
                let interaction = &mut rust.as_mut().get_mut().interaction;
                interaction.page_key_focus_key = None;
                for ((candidate, _), observation) in &mut interaction.focus_observations {
                    if *candidate == index {
                        observation.page_key = None;
                    }
                }
            }
            return;
        };
        if state.kind.len() > 32 || !(1..=1_000_000).contains(&state.sequence) {
            return;
        }
        if receipt.receiver.is_none()
            && self
                .as_mut()
                .retain_native_focus_generation(index, url, frame_path, epoch, &state)
        {
            return;
        }
        if let Some(receiver) = receipt.receiver.as_ref()
            && !self.as_mut().accept_page_receiver_generation(
                index,
                url,
                frame_path,
                epoch,
                state.sequence,
                receiver,
            )
        {
            return;
        }
        self.as_mut().page_focus_observed_for(
            index,
            url,
            frame_path,
            state.sequence,
            epoch,
            state.context,
            state.editable,
            state.user_activated,
        );
        // Mode projection can synchronously change epoch/owner. Such a result
        // may update legacy focus behavior but cannot authorize a new route.
        if self.as_ref().target_for_index(index) != Some(target)
            || self.as_ref().rust().input_epoch != epoch
        {
            return;
        }
        let Some(receiver) = receipt.receiver else {
            return;
        };
        if receiver.key.is_empty() || receiver.key.len() > 128 || receiver.origin.len() > 2048 {
            return;
        }
        let mut rust = self.as_mut().rust_mut();
        let interaction = &mut rust.as_mut().get_mut().interaction;
        let key = (index, frame_path.to_string());
        for ((candidate, path), observation) in &mut interaction.focus_observations {
            if *candidate == index && path != &key.1 {
                observation.page_key = None;
            }
        }
        // Escape suppresses replay of legacy facts for automatic Insert entry.
        // A freshly validated native noneditable receiver can still restore
        // this epoch's routing facts without replaying that mode transition.
        if state.focused
            && !state.editable
            && state.kind == "noneditable"
            && interaction
                .focus_observations
                .get(&key)
                .is_none_or(|old| old.epoch != epoch || old.url != url.to_string())
        {
            interaction.focus_observations.insert(
                key.clone(),
                FocusObservation {
                    url: url.to_string(),
                    sequence: state.sequence,
                    epoch,
                    context: state.context,
                    editable: false,
                    user_activated: state.user_activated,
                    page_key: None,
                    frame_document: Some(receiver.key.clone()),
                },
            );
        }
        if let Some(observation) =
            interaction
                .focus_observations
                .get_mut(&key)
                .filter(|observation| {
                    observation.sequence == state.sequence
                        && observation.epoch == epoch
                        && observation.url == url.to_string()
                })
        {
            observation.frame_document = Some(receiver.key.clone());
            observation.page_key = Some(super::focus_policy::PageKeyFocus {
                target,
                focused: state.focused,
                kind: state.kind,
                receiver,
            });
        }
        // Native publication above revokes every other receiver for this tab.
        // Legacy observations remain intact; a stale key never grants a route.
        interaction.page_key_focus_key = interaction
            .focus_observations
            .get(&key)
            .and_then(|observation| observation.page_key.as_ref())
            .map(|_| key);
    }

    fn retain_native_focus_generation(
        mut self: Pin<&mut Self>,
        index: i32,
        url: &QString,
        path: &QString,
        epoch: i32,
        state: &super::focus_policy::FocusProbeState,
    ) -> bool {
        let key = (index, path.to_string());
        if !self.as_ref().page_key_routing_active()
            || !self
                .as_ref()
                .rust()
                .interaction
                .focus_observations
                .get(&key)
                .is_some_and(|observation| {
                    observation.epoch == epoch
                        && observation.url == url.to_string()
                        && observation.frame_document.is_some()
                })
        {
            return false;
        }
        // Unassociated counters cannot be compared across documents. Unsafe
        // legacy facts may revoke routing, but cannot overwrite the native
        // generation/sequence or grant a receiver. A native probe restores it.
        if (state.editable || !state.focused || state.kind != "noneditable")
            && let Some(observation) = self
                .as_mut()
                .rust_mut()
                .get_mut()
                .interaction
                .focus_observations
                .get_mut(&key)
        {
            observation.page_key = None;
        }
        true
    }

    fn accept_page_receiver_generation(
        mut self: Pin<&mut Self>,
        index: i32,
        url: &QString,
        path: &QString,
        epoch: i32,
        sequence: i32,
        receiver: &super::focus_policy::PageKeyReceiver,
    ) -> bool {
        let frame_path = path.to_string();
        let current = self.as_ref();
        let rust = current.rust();
        if index != rust.active_tab_index
            || epoch != rust.input_epoch
            || rust.current_url.to_string() != url.to_string()
            || !(1..=1_000_000).contains(&sequence)
            || frame_path.is_empty()
            || frame_path.len() > 128
            || !frame_path
                .chars()
                .all(|character| character.is_ascii_digit() || character == '.')
            || receiver.key.is_empty()
            || receiver.key.len() > 128
            || receiver.origin.len() > 2048
        {
            return false;
        }
        let key = (index, frame_path);
        let previous = rust.interaction.focus_observations.get(&key);
        let replaced = previous
            .and_then(|observation| observation.frame_document.as_ref())
            .is_some_and(|document| document != &receiver.key);
        let ended_editing = replaced
            && rust.core_mode == Mode::Insert
            && previous.is_some_and(|observation| observation.editable);
        if replaced {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .interaction
                .focus_observations
                .remove(&key);
            rust.as_mut()
                .get_mut()
                .interaction
                .focus_suppressions
                .remove(&key);
        }
        if ended_editing {
            self.as_mut().escape();
            return false;
        }
        true
    }

    // Qt invokables need a QObject receiver, including pure presentation helpers.
    #[allow(clippy::unused_self)]
    pub(super) fn prompt_origin(&self, url: &QString) -> QString {
        QString::from(safe_site_origin(&url.to_string()).unwrap_or_default())
    }

    // Qt invokables require a QObject receiver for pure presentation helpers.
    #[allow(clippy::unused_self)]
    pub(super) fn prompt_host(&self, url: &QString) -> QString {
        QString::from(
            safe_site_origin(&url.to_string())
                .and_then(|origin| safe_site_host(&origin))
                .unwrap_or_default(),
        )
    }

    #[allow(clippy::unused_self)]
    pub(super) fn client_certificate_origin(&self, authority: &QString) -> QString {
        QString::from(
            super::url_safety::safe_client_certificate_origin(&authority.to_string())
                .unwrap_or_default(),
        )
    }

    // Scalar arguments preserve the existing QML bridge contract.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn page_focus_observed_for(
        mut self: Pin<&mut Self>,
        index: i32,
        url: &QString,
        frame_path: &QString,
        sequence: i32,
        epoch: i32,
        context: i32,
        editable: bool,
        user_activated: bool,
    ) {
        if epoch != self.as_ref().rust().input_epoch
            || index != self.as_ref().rust().active_tab_index
            || !(1..=1_000_000).contains(&sequence)
        {
            return;
        }
        let frame_path = frame_path.to_string();
        if frame_path.is_empty()
            || frame_path.len() > 128
            || frame_path.chars().any(char::is_control)
            || !frame_path
                .chars()
                .all(|character| character.is_ascii_digit() || character == '.')
        {
            return;
        }
        let url = url.to_string();
        let Some(tab) = self.as_ref().get_ref().tab_for_index(index) else {
            return;
        };
        let native_prompt_revalidation = self
            .as_ref()
            .rust()
            .interaction
            .native_prompt_leases
            .top()
            .is_some_and(|lease| {
                lease.completion_pending
                    && lease.expected_input_epoch == epoch
                    && lease.owner.tab == tab.to_string()
                    && lease
                        .focus_identity
                        .as_ref()
                        .is_some_and(|focus| focus.frame_path == frame_path)
            });
        let known_url = self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .and_then(|state| state.tabs().get(&tab))
            .and_then(|tab| tab.url.as_deref())
            .unwrap_or("about:blank")
            .to_owned();
        if known_url != url {
            return;
        }
        let key = (index, frame_path);
        if self
            .as_ref()
            .rust()
            .interaction
            .focus_suppressions
            .get(&key)
            .is_some_and(|suppressed| sequence <= *suppressed)
            && !native_prompt_revalidation
        {
            return;
        }
        if self
            .as_ref()
            .rust()
            .interaction
            .focus_observations
            .get(&key)
            .is_some_and(|observation| {
                observation.epoch == epoch
                    && observation.url == url
                    && sequence <= observation.sequence
                    && !native_prompt_revalidation
            })
        {
            return;
        }
        let frame_document = self
            .as_ref()
            .rust()
            .interaction
            .focus_observations
            .get(&key)
            .and_then(|observation| observation.frame_document.clone());
        let context_ended = self
            .as_ref()
            .rust()
            .interaction
            .focus_observations
            .get(&key)
            .is_some_and(|old| {
                old.ends_transient_context(self.as_ref().rust().core_mode, epoch, context, editable)
            });
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.interaction.focus_suppressions.remove(&key);
            this.interaction.focus_observations.insert(
                key.clone(),
                FocusObservation {
                    url: url.clone(),
                    sequence,
                    epoch,
                    context,
                    editable,
                    user_activated,
                    page_key: None,
                    frame_document,
                },
            );
        }
        if context_ended {
            self.as_mut().escape();
            return;
        }
        if native_prompt_revalidation {
            return;
        }
        let (focused_editable, user_activated) = self
            .as_ref()
            .rust()
            .interaction
            .focus_observations
            .iter()
            .filter(|((candidate, _), observation)| {
                *candidate == index && observation.epoch == epoch && observation.url == url
            })
            .fold((false, false), |(focused, activated), (_, observation)| {
                (
                    focused || observation.editable,
                    activated || (observation.editable && observation.user_activated),
                )
            });
        let site_entry_mode = self.as_ref().get_ref().configured_site_entry_mode(&url);
        match focus_mode_transition(
            self.as_ref().rust().core_mode,
            focused_editable,
            user_activated,
            site_entry_mode.as_deref(),
        ) {
            Some(Mode::Insert) => self.as_mut().enter_insert(),
            Some(Mode::Normal) => self.as_mut().escape(),
            _ => {}
        }
    }

    pub(super) fn update_current_url(mut self: Pin<&mut Self>, url: QString) {
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .selected_context_projection_depth += 1;
        self.as_mut().project_current_url(url);
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .selected_context_projection_depth -= 1;
    }

    fn project_current_url(mut self: Pin<&mut Self>, url: QString) {
        let Some(input) = super::policy_refresh_capture::UrlProjectionInput::capture(
            self.as_ref().get_ref(),
            url,
        ) else {
            return;
        };
        let input = std::sync::Arc::new(input);
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .url_projection_input = Some(std::sync::Arc::clone(&input));
        let current = |ui: &Self| {
            input.is_current(ui)
                && ui
                    .rust()
                    .url_projection_input
                    .as_ref()
                    .is_some_and(|latest| std::sync::Arc::ptr_eq(latest, &input))
        };
        macro_rules! notify {
            ($effect:expr) => {{
                if !current(self.as_ref().get_ref()) {
                    return;
                }
                $effect;
                if !current(self.as_ref().get_ref()) {
                    return;
                }
            }};
        }
        let display = display_url(&input.url.to_string());
        notify!(self.as_mut().request_interaction_policy_refresh());
        notify!(self.as_mut().set_current_url(input.url.clone()));
        notify!(self.as_mut().set_display_url(QString::from(display)));
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .completed_url_projection = Some(input);
    }

    /// Invalidation is synchronous; compilation belongs to the existing
    /// runtime-work timer. Tab/settings bindings can reach here from key routing.
    pub(super) fn request_interaction_policy_refresh(mut self: Pin<&mut Self>) {
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.interaction.interaction_policy_refresh_pending = true;
            this.interaction.cached_hint_policies = None;
            if let Some(state) = this.state.as_mut() {
                state.invalidate_interaction_snapshot();
            }
        }
        self.runtime_work_available();
    }

    pub(super) fn poll_interaction_policy_refresh(mut self: Pin<&mut Self>) {
        // A synchronous Qt observer can poll while selected-tab properties
        // are still being published. Leave that work pending until the outer
        // projection finishes; this hook remains the sole cold preparation.
        if self.as_ref().rust().selected_context_projection_depth != 0 {
            return;
        }
        if self
            .as_ref()
            .rust()
            .interaction
            .interaction_policy_refresh_pending
            && !self.as_ref().active_tab_projection_is_current()
        {
            self.as_mut().sync_active_tab_properties();
            if !self.as_ref().active_tab_projection_is_current() {
                return;
            }
        }
        let pending = {
            let mut rust = self.as_mut().rust_mut();
            std::mem::take(
                &mut rust
                    .as_mut()
                    .get_mut()
                    .interaction
                    .interaction_policy_refresh_pending,
            )
        };
        if pending {
            let url = self.as_ref().rust().current_url.to_string();
            self.as_mut().refresh_current_interaction_policy(&url);
        }
    }

    /// Consume only the current owner/window/tab/URL; no queued old capture can
    /// republish a retired owner. Background settings queries never refresh it.
    fn refresh_current_interaction_policy(mut self: Pin<&mut Self>, url: &str) {
        let Some(capture) =
            super::policy_refresh_capture::PolicyRefreshCapture::capture(self.as_ref().get_ref())
        else {
            self.as_mut().install_mixed_policy(
                super::mixed_policy_runtime::MixedPolicyState::Unavailable(
                    "the selected policy owner is unavailable".into(),
                ),
            );
            return;
        };
        let snapshot = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            match (this.state.as_mut(), this.window) {
                (Some(state), Some(window)) => state.refresh_site_policy_snapshot(window, url),
                _ => None,
            }
        };
        // Only the selected App/Core scope can install window-wide Mixed
        // routing. Per-view ordinary site-setting reads never publish it.
        let mixed = snapshot
            .as_ref()
            .ok_or_else(|| "the selected page policy context is unavailable".to_owned())
            .and_then(|_| {
                let binding = self.as_ref();
                let rust = binding.rust();
                let state = rust
                    .state
                    .as_ref()
                    .ok_or("application state is unavailable")?;
                let tab = rust
                    .window
                    .and_then(|window| state.windows().get(&window))
                    .and_then(|window| window.active_tab)
                    .ok_or("the selected tab is unavailable")?;
                if rust.tab != Some(tab)
                    || usize::try_from(rust.active_tab_index)
                        .ok()
                        .and_then(|index| rust.tab_ids.get(index))
                        != Some(&tab)
                {
                    return Err("the selected tab policy owner changed".into());
                }
                let layers = state.configuration_layers();
                super::mixed_policy_runtime::resolve_for_layers(
                    &layers.base,
                    &layers.profile,
                    &layers.runtime,
                    &layers.command_line,
                    &layers.temporary,
                    state.configuration(),
                    url,
                )
            });
        self.as_mut().install_mixed_policy(
            super::mixed_policy_runtime::MixedPolicyState::from_result(mixed),
        );
        // Qt notifications above can reenter tab/profile navigation. Its new
        // pending work owns preparation; this snapshot cannot republish hints.
        if !capture.is_current(self.as_ref().get_ref()) {
            return;
        }
        let current = snapshot.as_ref().map(|snapshot| snapshot.revision());
        let previous = self
            .as_ref()
            .rust()
            .interaction
            .compiled_interaction_revision;
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .interaction
            .compiled_interaction_revision = current;
        let meaning = |revision: ferric_browser_config::InteractionPolicyRevision| {
            (
                revision.profile,
                revision.profile_revision,
                revision.policy_revision,
            )
        };
        if previous.map(meaning) != current.map(meaning) {
            self.as_mut().note_hint_configuration_change();
            if !capture.is_current(self.as_ref().get_ref()) {
                return;
            }
        }
        let Some(snapshot) = snapshot else {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .interaction
                .cached_hint_policies = None;
            self.apply_hint_proxy_targets(ferric_browser_config::HintProxyTargets::Off);
            return;
        };
        // The ordinary consumers remain independent of Interaction conflicts.
        // Proxy failure closes proxies without replacing valid selectors.
        match snapshot.discovery().proxy_targets() {
            Ok(policy) => {
                self.as_mut().apply_hint_proxy_targets(policy.clone());
            }
            Err(error) => {
                self.as_mut()
                    .apply_hint_proxy_targets(ferric_browser_config::HintProxyTargets::Off);
                if !capture.is_current(self.as_ref().get_ref()) {
                    return;
                }
                self.as_mut().set_status_text(QString::from(format!(
                    "Hint proxy discovery unavailable: {error}"
                )));
            }
        }
        if !capture.is_current(self.as_ref().get_ref()) {
            return;
        }
        let owner_revision = self.as_ref().rust().interaction.hint_configuration_revision;
        let current_url = self.as_ref().rust().current_url.clone();
        if self
            .as_ref()
            .rust()
            .interaction
            .cached_hint_policies
            .as_ref()
            .is_some_and(|cache| cache.matches(&snapshot, &current_url, owner_revision))
        {
            return;
        }
        let Ok(validated) = ferric_browser_core::ValidatedUrl::parse(url) else {
            return;
        };
        let context = ferric_browser_application::SitePolicyMatchContext::for_url(&validated);
        match super::hint_policy_cache::CachedHintPolicies::prepare(
            snapshot,
            context,
            current_url,
            owner_revision,
        ) {
            Ok(cache) => {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .interaction
                    .cached_hint_policies = Some(cache);
            }
            Err(error) => {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .interaction
                    .cached_hint_policies = None;
                self.set_status_text(QString::from(format!(
                    "Hint selectors unavailable: {error}"
                )));
            }
        }
    }

    pub(super) fn interaction_policy_snapshot(&self) -> QString {
        use ferric_browser_application::InteractionCompilationState;
        let rust = self.rust();
        let snapshot = rust.state.as_ref().and_then(|state| {
            let url =
                ferric_browser_core::ValidatedUrl::parse(rust.current_url.to_string()).ok()?;
            let context = ferric_browser_config::SiteMatchContext::for_url(&url)?;
            state.cached_interaction_snapshot(rust.window?, &context)
        });
        let mut value = if let Some(snapshot) = snapshot {
            let revision = snapshot.revision();
            serde_json::json!({
                "state": match snapshot.state() {
                    InteractionCompilationState::Disabled => "disabled",
                    InteractionCompilationState::NoOp => "no-op",
                    InteractionCompilationState::Conflicted => "conflicted",
                    InteractionCompilationState::Ready => "ready",
                },
                "validation_stage": "source-compilation",
                "configuration_revision": revision.configuration_revision.to_string(),
                "profile_revision": revision.profile_revision.to_string(),
                "policy_revision": revision.policy_revision.to_string(),
                "reload_revision": snapshot.sources().reload_revision().to_string(),
                "enabled": snapshot.policy().map(ferric_browser_config::EffectiveInteractionPolicy::enabled),
                "anchors": snapshot.policy().map(|policy| policy.anchors().len()),
                "anchor_source_revision": snapshot.policy()
                    .and_then(|policy| policy.origin(ferric_browser_config::InteractionField::Anchors))
                    .map(|origin| origin.source_revision().to_string()),
                "diagnostic": snapshot.diagnostic(),
                "native_page_keys": snapshot.policy().map(|policy| {
                    policy.keys().capability_requirements().map(|requirement| {
                        serde_json::json!({
                            "syntax":"valid", "destination":requirement.destination().as_str(),
                            "token_group":requirement.destination_class(), "context":requirement.context(),
                            "receiver_scope":requirement.receiver_scope(),
                            "qualification":crate::page_key_capabilities::current_assessment(
                                requirement.destination(),requirement.source()!=requirement.destination(),
                                requirement.context(),requirement.receiver_scope()),
                            "authorization":"checked-on-physical-press"
                        })
                    }).collect::<Vec<_>>()
                }),
            })
        } else {
            serde_json::json!({
                "state": "unavailable",
                "validation_stage": "source-compilation",
                "reason": rust.state.as_ref().and_then(ferric_browser_application::BrowserApplication::interaction_cache_unavailable)
                    .map_or("context-not-current", |reason| match reason {
                        ferric_browser_application::InteractionCacheUnavailable::OwnerNotReady => "owner-not-ready",
                        ferric_browser_application::InteractionCacheUnavailable::UnsupportedContext => "unsupported-context",
                        ferric_browser_application::InteractionCacheUnavailable::RevisionExhausted => "revision-exhausted",
                        ferric_browser_application::InteractionCacheUnavailable::PendingRefresh => "pending-refresh",
                        ferric_browser_application::InteractionCacheUnavailable::RejectedEngineContext => "rejected-engine-context",
                    }),
            })
        };
        value["compilations"] = serde_json::json!(
            rust.state
                .as_ref()
                .map_or(
                    0,
                    ferric_browser_application::BrowserApplication::interaction_compilation_count
                )
                .to_string()
        );
        QString::from(value.to_string())
    }

    pub(super) fn refresh_site_status(mut self: Pin<&mut Self>) -> QString {
        if self.as_ref().rust().profile_persistence.is_durable()
            && (self.as_ref().rust().library.storage_library.is_none()
                || self.as_ref().rust().library.storage_library_dirty.get())
        {
            self.as_mut().request_storage_library();
        }
        let permission_origin = self.as_ref().rust().state.as_ref().and_then(|state| {
            let origin = state
                .active_tab()
                .and_then(|tab| tab.url.as_deref())
                .and_then(safe_site_origin)?;
            (!state.saved_permission_preferences(&origin).complete).then_some(origin)
        });
        if let Some(origin) = permission_origin {
            let _ = self.as_mut().request_permission_preferences(&origin);
        }
        let result = self.as_ref().ipc_site_status(&Value::Null);
        match result {
            Ok(value) => {
                let serialized = value.to_string();
                self.as_mut().set_site_status(QString::from(&serialized));
                QString::from(serialized)
            }
            Err(error) => {
                self.as_mut().set_status_text(QString::from(error));
                self.set_site_status(QString::from("{}"));
                QString::default()
            }
        }
    }

    pub(super) fn site_report(mut self: Pin<&mut Self>, include_host: bool) -> QString {
        if self.as_ref().rust().profile_persistence.is_durable()
            && (self.as_ref().rust().library.storage_library.is_none()
                || self.as_ref().rust().library.storage_library_dirty.get())
        {
            self.as_mut().request_storage_library();
        }
        match self.as_ref().ipc_site_report(include_host) {
            Ok(report) => QString::from(
                serde_json::to_string_pretty(&report)
                    .unwrap_or_else(|_| "{\"error\":\"site report serialization failed\"}".into()),
            ),
            Err(error) => {
                self.set_status_text(QString::from(format!("Site report unavailable: {error}")));
                QString::default()
            }
        }
    }

    pub(super) fn site_doctor_proposal(self: Pin<&mut Self>) -> QString {
        let proposal = super::site_doctor_completion::proposal_projection(self.as_ref().rust())
            .unwrap_or_else(|| serde_json::json!({"state": "none"}));
        QString::from(
            serde_json::to_string_pretty(&proposal)
                .unwrap_or_else(|_| "{\"state\":\"unavailable\"}".into()),
        )
    }

    pub(super) fn apply_site_doctor_proposal(
        mut self: Pin<&mut Self>,
        proposal_id: &QString,
        confirmed: bool,
    ) -> bool {
        if !confirmed {
            self.set_status_text(QString::from(
                "Applying a Site Doctor fix requires explicit confirmation",
            ));
            return false;
        }
        match self.as_mut().queue_configuration_preparation(
            super::browser_ui_configuration_preparation::DeferredConfigurationAction::SiteDoctor(
                proposal_id.clone(),
            ),
            None,
            &mut None,
        ) {
            Ok(_) => true,
            Err(error) => {
                self.set_status_text(QString::from(error));
                false
            }
        }
    }

    pub(super) fn prepare_site_doctor_proposal(
        mut self: Pin<&mut Self>,
        proposal_id: &QString,
        confirmed: bool,
        preparation: Option<
            &super::browser_ui_configuration_preparation::ConfigurationPreparationTurn,
        >,
    ) -> bool {
        if !confirmed {
            self.set_status_text(QString::from(
                "Applying a Site Doctor fix requires explicit confirmation",
            ));
            return false;
        }
        let Some(proposal) = self.as_ref().rust().last_site_doctor_result.clone() else {
            self.set_status_text(QString::from(
                "No Site Doctor durable-fix proposal is pending",
            ));
            return false;
        };
        let requested_id = proposal_id.to_string();
        if proposal.get("id").and_then(Value::as_str) != Some(requested_id.as_str()) {
            self.set_status_text(QString::from(
                "This Site Doctor suggestion has changed. Refresh Site Doctor and review it again.",
            ));
            return false;
        }
        if proposal.get("kind").and_then(Value::as_str) != Some("blocker-exception") {
            self.set_status_text(QString::from(
                "This Site Doctor result has no supported durable fix",
            ));
            return false;
        }
        let current_origin = self.as_ref().rust().state.as_ref().and_then(|state| {
            let tab = state.active_tab()?;
            let profile = state.profiles().get(&tab.profile)?;
            if profile.privacy.is_transient() {
                return None;
            }
            tab.url.as_deref().and_then(safe_site_origin)
        });
        if current_origin.as_deref().is_none()
            || proposal.get("origin").and_then(Value::as_str) != current_origin.as_deref()
        {
            self.set_status_text(QString::from(
                "This page changed. Run a new site test before applying a fix.",
            ));
            return false;
        }
        if self
            .as_ref()
            .rust()
            .profile_persistence
            .lacks_durable_storage()
            || self.as_ref().rust().storage_roots.is_none()
        {
            self.set_status_text(QString::from(
                "Durable Site Doctor fixes require a normal profile with storage",
            ));
            return false;
        }
        if proposal.get("state").and_then(Value::as_str) != Some("pending-confirmation") {
            self.set_status_text(QString::from(
                "Run and review a new Site Doctor test before applying another fix.",
            ));
            return false;
        }
        let mutation = |key: &str| -> Option<Vec<String>> {
            serde_json::from_value(proposal.get("mutation")?.get(key)?.clone()).ok()
        };
        let (Some(before), Some(after), Some(experiment)) = (
            mutation("before"),
            mutation("after"),
            proposal.get("experiment_id").and_then(Value::as_str),
        ) else {
            self.set_status_text(QString::from(
                "This Site Doctor proposal has an invalid mutation",
            ));
            return false;
        };
        let Some(config) = self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .map(|state| state.configuration().clone())
        else {
            self.set_status_text(QString::from("Site Doctor configuration is unavailable"));
            return false;
        };
        if config.blocking.bypass_sites != before {
            self.set_status_text(QString::from(
                "Blocking settings changed during the experiment. Run it again before applying this suggestion.",
            ));
            return false;
        }
        let command = ParsedCommand {
            name: "set".into(),
            arguments: vec![format!(
                "blocking.bypass_sites={}",
                toml_string_array_literal(&after)
            )],
        };
        let Some(owner) = self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .and_then(super::BrowserApplication::capture_configuration_request)
        else {
            return false;
        };
        let subject = super::site_doctor_completion::SiteDoctorSubject {
            id: requested_id,
            experiment: experiment.to_owned(),
            origin: current_origin.unwrap_or_default(),
            before,
            after,
            owner,
        };
        if !subject.matches(Some(&proposal)) {
            self.set_status_text(QString::from(
                "This Site Doctor proposal is no longer current",
            ));
            return false;
        }
        let (fix, completion) = super::site_doctor_completion::SiteDoctorFix::new(subject);
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.site_doctor_fix = Some(fix);
            if let Some(object) = this
                .last_site_doctor_result
                .as_mut()
                .and_then(Value::as_object_mut)
            {
                object.insert("state".into(), Value::String("applying".into()));
            }
        }
        let result = self
            .as_mut()
            .prepare_runtime_config_command_with_post_action(
                &command,
                preparation,
                Some(completion),
            );
        self.as_mut().poll_site_doctor_fix();
        match result {
            Ok(_) => true,
            Err(error) => {
                self.set_status_text(QString::from(format!(
                    "Site Doctor durable fix was not applied: {error}"
                )));
                false
            }
        }
    }

    pub(super) fn site_data_clear_plan(self: Pin<&mut Self>, origin: &QString) -> QString {
        let requested = origin.to_string();
        let plan: Result<Value, String> = (|| {
            let canonical_requested = safe_site_origin(&requested)
                .ok_or_else(|| "site-data-clear requires one exact HTTP(S) origin".to_owned())?;
            let ledger = self.as_ref().ipc_site_status(&Value::Null)?;
            if ledger.get("private").and_then(Value::as_bool) == Some(true) {
                return Err("site-data-clear is unavailable for private sessions".into());
            }
            let current = ledger
                .get("origin")
                .and_then(Value::as_str)
                .ok_or_else(|| "the active document has no eligible HTTP(S) origin".to_owned())?;
            if current != canonical_requested {
                return Err("site-data-clear may target only the active exact origin".into());
            }
            Ok(serde_json::json!({
                "schema": 1,
                "origin": canonical_requested,
                "scope": "active-exact-origin",
                "categories": {
                    "local_storage": {"status": "available", "mechanism": "page-origin-api"},
                    "cache_storage": {"status": "available", "mechanism": "page-origin-api"},
                    "service_workers": {"status": "available", "mechanism": "page-origin-api"},
                    "cookies": {"status": "page-visible-only", "mechanism": "document.cookie", "note": "HttpOnly and other profile cookie-store entries are not cleared"},
                    "http_cache": {"status": "profile-wide-only", "mechanism": "WebEngineProfile.clearHttpCache", "selected": false}
                },
                "profile_reset": {"status": "not-offered", "reason": "a requested origin never escalates to whole-profile deletion"},
                "confirmation": "required",
                "excluded": ["other_origins", "profile_files", "permissions", "history", "downloads"]
            }))
        })();
        QString::from(match plan {
            Ok(value) => value.to_string(),
            Err(error) => serde_json::json!({"error": error}).to_string(),
        })
    }

    pub(super) fn site_data_clear(
        mut self: Pin<&mut Self>,
        origin: &QString,
        confirmed: bool,
    ) -> bool {
        let plan = self.as_mut().site_data_clear_plan(origin).to_string();
        let Ok(plan_value) = serde_json::from_str::<Value>(&plan) else {
            self.set_status_text(QString::from("Site-data scope could not be determined"));
            return false;
        };
        if let Some(error) = plan_value.get("error").and_then(Value::as_str) {
            self.set_status_text(QString::from(error));
            return false;
        }
        if !confirmed {
            self.set_status_text(QString::from(
                "Site-data clearing is a destructive action; review scope and confirm",
            ));
            return false;
        }
        if self.as_ref().rust().pending_engine_action.is_some() {
            self.set_status_text(QString::from(
                "Another browser operation is already pending",
            ));
            return false;
        }
        let canonical = plan_value
            .get("origin")
            .and_then(Value::as_str)
            .unwrap_or_default();
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(format!("site-data-clear\t{canonical}"));
        self.set_status_text(QString::from(
            "Site-data clear requested; only supported active-origin categories will be cleared",
        ));
        true
    }

    pub(super) fn site_data_clear_finished(
        mut self: Pin<&mut Self>,
        origin: &QString,
        result: &QString,
    ) -> bool {
        let expected = format!("site-data-clear\t{origin}");
        let pending = self.as_ref().rust().pending_engine_action.clone();
        if pending.as_deref() != Some(expected.as_str()) {
            return false;
        }
        let Ok(value) = serde_json::from_str::<Value>(&result.to_string()) else {
            self.set_status_text(QString::from(
                "Site-data clear returned invalid category results",
            ));
            return false;
        };
        if let Some(error) = value.get("error").and_then(Value::as_str) {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_engine_action = None;
            self.set_status_text(QString::from(format!("Site-data clear failed: {error}")));
            return false;
        }
        if !value.is_object() || value.get("pending").and_then(Value::as_bool) == Some(true) {
            self.set_status_text(QString::from("Site-data clear has not completed"));
            return false;
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = None;
        self.set_status_text(QString::from(
            "Site-data clear completed for supported active-origin categories; cookies and HTTP cache were not fully cleared",
        ));
        true
    }

    pub(super) fn execute_site_data_clear_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        let (origin, confirmed) = match command.arguments.as_slice() {
            [origin] => (origin.as_str(), false),
            [origin, flag] if flag == "--confirm" => (origin.as_str(), true),
            _ => return Err("site-data-clear requires ORIGIN and optional --confirm".into()),
        };
        let plan = self
            .as_mut()
            .site_data_clear_plan(&QString::from(origin))
            .to_string();
        let value = serde_json::from_str::<Value>(&plan)
            .map_err(|error| format!("site-data scope response was invalid: {error}"))?;
        if let Some(error) = value.get("error").and_then(Value::as_str) {
            return Err(error.into());
        }
        if !confirmed {
            self.set_status_text(QString::from(
                "Site-data scope preview ready; repeat with --confirm to clear supported categories",
            ));
            return Ok(value);
        }
        if !self.as_mut().site_data_clear(&QString::from(origin), true) {
            return Err(self.as_ref().rust().status_text.to_string());
        }
        Ok(serde_json::json!({
            "status": "accepted",
            "pending": true,
            "plan": value
        }))
    }

    pub(super) fn spellcheck_dictionaries(self: Pin<&mut Self>) -> QStringList {
        let _ = self;
        diagnostics::installed_dictionary_names()
            .into_iter()
            .map(QString::from)
            .collect()
    }

    pub(super) fn export_diagnostics(mut self: Pin<&mut Self>, path: &QString) -> bool {
        if !self
            .as_ref()
            .rust()
            .diagnostic_work
            .diagnostics_preview_ready
        {
            self.set_status_text(QString::from(
                "Diagnostics export requires reviewing the visible preview first",
            ));
            return false;
        }
        let path = path.to_string();
        let path_ref = Path::new(&path);
        if path.is_empty()
            || path.len() > 4096
            || path.chars().any(char::is_control)
            || !path_ref.is_absolute()
        {
            self.set_status_text(QString::from(
                "Diagnostics export requires an absolute local file path",
            ));
            return false;
        }
        if path_ref.exists() {
            self.set_status_text(QString::from(
                "Diagnostics export refuses to overwrite an existing file",
            ));
            return false;
        }
        let Some(payload) = self
            .as_ref()
            .rust()
            .diagnostic_work
            .diagnostics_preview_payload
            .clone()
        else {
            self.set_status_text(QString::from("Diagnostics export is unavailable"));
            return false;
        };
        if payload.len() > MAX_DIAGNOSTICS_EXPORT_BYTES {
            self.set_status_text(QString::from(
                "Diagnostics export is too large; reduce retained in-memory diagnostics first",
            ));
            return false;
        }
        match atomic_write_private(path_ref, payload.as_bytes()) {
            Ok(()) => {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .diagnostic_work
                    .diagnostics_preview_ready = false;
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .diagnostic_work
                    .diagnostics_preview_payload = None;
                self.as_mut().set_diagnostics_available(false);
                self.set_status_text(QString::from(
                    "Report saved. Refresh Diagnostics to prepare another report.",
                ));
                true
            }
            Err(error) => {
                self.set_status_text(QString::from(format!("Diagnostics export failed: {error}")));
                false
            }
        }
    }

    pub(super) fn record_request_resolution(
        mut self: Pin<&mut Self>,
        request_kind: &QString,
        outcome: &QString,
    ) {
        let kind = request_kind.to_string();
        let outcome = outcome.to_string();
        if !matches!(
            kind.as_str(),
            "permission"
                | "file-dialog"
                | "page-dialog"
                | "authentication"
                | "desktop-media"
                | "client-certificate"
                | "webauth"
                | "download"
        ) || !matches!(
            outcome.as_str(),
            "resolved" | "stale" | "duplicate" | "error"
        ) {
            return;
        }
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        let entry = this
            .request_resolution_counts
            .entry((kind, outcome))
            .or_default();
        *entry = entry.saturating_add(1);
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn begin_site_doctor_experiment(
        mut self: Pin<&mut Self>,
        kind: &QString,
    ) -> QString {
        let kind = kind.to_string();
        let blocking_experiment = kind == "blocking-bypass";
        let fresh_view_experiment = kind == "fresh-view";
        let compiled_defaults_experiment = kind == "compiled-defaults";
        if !blocking_experiment
            && kind != "userscripts-off"
            && !fresh_view_experiment
            && !compiled_defaults_experiment
        {
            self.set_status_text(QString::from("Unsupported Site Doctor experiment"));
            return QString::default();
        }
        if self.as_ref().rust().active_site_experiment.is_some() {
            self.set_status_text(QString::from("A Site Doctor experiment is already active"));
            return QString::default();
        }
        if kind == "blocking-bypass" && !self.as_ref().rust().blocking_enabled {
            self.set_status_text(QString::from(
                "Blocking bypass experiment has no effect while blocking is disabled",
            ));
            return QString::default();
        }
        let private = self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .and_then(|state| {
                let tab = state.active_tab()?;
                state
                    .profiles()
                    .get(&tab.profile)
                    .map(|profile| profile.privacy)
            })
            .is_some_and(PrivacyKind::is_transient);
        if private {
            self.set_status_text(QString::from(
                "Site Doctor experiments are disabled in private sessions",
            ));
            return QString::default();
        }
        let details = {
            let binding = self.as_ref();
            let rust = binding.rust();
            rust.tab.and_then(|tab_id| {
                let state = rust.state.as_ref()?;
                let tab = state.tabs().get(&tab_id)?;
                let url = tab
                    .url
                    .as_deref()
                    .map_or_else(|| rust.current_url.to_string(), ToOwned::to_owned);
                let origin = safe_site_origin(&url)?;
                Some((tab_id, safe_ipc_url(&url), origin))
            })
        };
        let Some((tab, url, origin)) = details else {
            self.set_status_text(QString::from(
                "Site Doctor requires an active HTTP(S) document",
            ));
            return QString::default();
        };
        if kind == "userscripts-off" {
            let current_url = self.as_ref().rust().current_url.to_string();
            let has_matching_scripts = self.as_ref().rust().state.as_ref().is_some_and(|state| {
                state
                    .matching_page_scripts(&current_url, false)
                    .is_ok_and(|scripts| !scripts.is_empty())
            });
            if !has_matching_scripts {
                self.set_status_text(QString::from(
                    "No matching page userscripts are available for this site",
                ));
                return QString::default();
            }
        }
        if compiled_defaults_experiment {
            let has_site_settings = self.as_ref().rust().state.as_ref().is_some_and(|state| {
                let config = state.configuration();
                matching_site_rules(config, &url)
                    .iter()
                    .any(|rule| rule.set.keys().any(|key| setting_supports_site_scope(key)))
            });
            if !has_site_settings {
                self.set_status_text(QString::from(
                    "No supported site settings are configured for this site",
                ));
                return QString::default();
            }
        }
        let temporary_tab = if fresh_view_experiment {
            let index = self.as_mut().new_tab();
            if index < 0 {
                self.set_status_text(QString::from(
                    "Site Doctor could not create a fresh same-profile view",
                ));
                return QString::default();
            }
            let Some(temporary_tab) = self.as_ref().rust().tab else {
                self.set_status_text(QString::from(
                    "Site Doctor fresh view has no live tab identity",
                ));
                return QString::default();
            };
            Some(temporary_tab)
        } else {
            None
        };
        let mut sites = self
            .as_ref()
            .rust()
            .blocking_bypass_sites
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let prior_bypass_sites = sites.clone();
        let bypass_host = safe_site_host(&origin).unwrap_or_default();
        if kind == "blocking-bypass" {
            if sites.iter().any(|site| site == &bypass_host) {
                self.set_status_text(QString::from(
                    "The current site is already using a blocker bypass",
                ));
                return QString::default();
            }
            if sites.len() >= 64 {
                self.set_status_text(QString::from(
                    "Site Doctor cannot add a bypass; site limit reached",
                ));
                return QString::default();
            }
        }
        if kind == "blocking-bypass" {
            sites.push(bypass_host);
        }
        let id = format!("site-experiment-{}", Uuid::new_v4().simple());
        let created_at = Instant::now();
        let experiment = SiteExperiment {
            id: id.clone(),
            kind,
            tab,
            temporary_tab,
            origin,
            url: url.clone(),
            prior_bypass_sites,
            created_at,
        };
        let payload = serde_json::json!({
            "id": experiment.id,
            "kind": experiment.kind,
            "tab": experiment.tab.to_string(),
            "temporary_tab": experiment.temporary_tab.map(|tab| tab.to_string()),
            "origin": experiment.origin,
            "url": experiment.url,
            "state": "pending",
            "timeout_seconds": SITE_DOCTOR_EXPERIMENT_TIMEOUT.as_secs()
        })
        .to_string();
        self.as_mut().publish_site_experiment_presentation(
            &experiment,
            SITE_DOCTOR_EXPERIMENT_TIMEOUT.as_secs(),
        );
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.active_site_experiment = Some(experiment);
        }
        let command = if fresh_view_experiment {
            ferric_browser_application::ViewCommand::SiteDoctorFreshView {
                experiment_id: id.clone(),
                url: ferric_browser_core::ValidatedUrl::parse(url)
                    .expect("validated HTTP(S) experiment URL"),
            }
        } else {
            ferric_browser_application::ViewCommand::SiteDoctorReload {
                experiment_id: id.clone(),
            }
        };
        if self
            .as_mut()
            .queue_view_command_for(command, temporary_tab.unwrap_or(tab))
            .is_err()
        {
            self.as_mut()
                .finish_site_doctor_experiment(&QString::from(id), false);
            return QString::default();
        }
        if blocking_experiment {
            self.as_mut()
                .set_blocking_bypass_sites(sites.iter().map(QString::from).collect());
        }
        self.as_mut()
            .set_status_text(QString::from(if blocking_experiment {
                "Site Doctor: reload with blocker bypass"
            } else if fresh_view_experiment {
                "Site Doctor: open a fresh same-profile view"
            } else if compiled_defaults_experiment {
                "Site Doctor: reload with compiled-default site settings"
            } else {
                "Site Doctor: reload without matching userscripts"
            }));
        QString::from(payload)
    }

    pub(super) fn finish_site_doctor_experiment(
        mut self: Pin<&mut Self>,
        experiment_id: &QString,
        succeeded: bool,
    ) -> bool {
        let id = experiment_id.to_string();
        let Some(experiment) = self.as_ref().rust().active_site_experiment.clone() else {
            return false;
        };
        if experiment.id != id {
            return false;
        }
        let expected_tab = experiment.temporary_tab.unwrap_or(experiment.tab);
        let same_tab = self.as_ref().rust().tab == Some(expected_tab);
        let same_url =
            safe_ipc_url(&self.as_ref().rust().current_url.to_string()) == experiment.url;
        let sites = self
            .as_ref()
            .rust()
            .blocking_bypass_sites
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        if experiment.kind == "blocking-bypass" {
            let temporary_state = experiment
                .prior_bypass_sites
                .iter()
                .cloned()
                .chain(safe_site_host(&experiment.origin))
                .collect::<Vec<_>>();
            if sites == temporary_state {
                self.as_mut().set_blocking_bypass_sites(
                    experiment
                        .prior_bypass_sites
                        .iter()
                        .map(QString::from)
                        .collect(),
                );
            }
        }
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.active_site_experiment = None;
            if let Some(tab) = experiment.temporary_tab {
                this.pending_engine_action = Some(format!("site-doctor-close\t{tab}"));
            }
        }
        self.as_mut().clear_site_experiment_presentation();
        if succeeded && same_tab && same_url {
            let proposal = if experiment.kind == "blocking-bypass" {
                let mut durable_sites = experiment.prior_bypass_sites.clone();
                let host = safe_site_host(&experiment.origin).unwrap_or_default();
                if !durable_sites.contains(&host) {
                    durable_sites.push(host);
                }
                Some(serde_json::json!({
                    "id": format!("site-fix-{}", experiment.id),
                    "kind": "blocker-exception",
                    "experiment_id": experiment.id,
                    "origin": experiment.origin,
                    "likely_cause": "network blocking",
                    "command": ["set", format!("blocking.bypass_sites={}", toml_string_array_literal(&durable_sites))],
                    "mutation": {
                        "layer": "runtime-overrides",
                        "key": "blocking.bypass_sites",
                        "before": experiment.prior_bypass_sites,
                        "after": durable_sites
                    },
                    "provenance": "site-doctor-experiment",
                    "security_effect": "host-scoped network blocking exception; permissions and TLS unchanged",
                    "test_behavior": "save the generated override, reload the blocker policy, and re-test this host",
                    "state": "pending-confirmation"
                }))
            } else {
                None
            };
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .last_site_doctor_result = proposal;
        }
        if succeeded && same_tab && same_url {
            self.set_status_text(QString::from(match experiment.kind.as_str() {
                "blocking-bypass" => "Site Doctor experiment completed; blocker bypass restored",
                "fresh-view" => "Site Doctor experiment completed; fresh view closing",
                "compiled-defaults" => {
                    "Site Doctor experiment completed; compiled defaults restored"
                }
                _ => "Site Doctor experiment completed; userscripts restored",
            }));
            true
        } else {
            self.set_status_text(QString::from(match experiment.kind.as_str() {
                "blocking-bypass" => "Site Doctor experiment expired; blocker bypass restored",
                "fresh-view" => "Site Doctor experiment expired; fresh view closing",
                "compiled-defaults" => "Site Doctor experiment expired; compiled defaults restored",
                _ => "Site Doctor experiment expired; userscripts restored",
            }));
            false
        }
    }

    pub(super) fn tick_site_doctor_experiment(mut self: Pin<&mut Self>) {
        let Some((id, created_at)) = self
            .as_ref()
            .rust()
            .active_site_experiment
            .as_ref()
            .map(|experiment| (experiment.id.clone(), experiment.created_at))
        else {
            return;
        };
        let Some(remaining_seconds) = site_doctor_remaining_seconds(created_at, Instant::now())
        else {
            let _ = self.finish_site_doctor_experiment(&QString::from(id), false);
            return;
        };
        if self.as_ref().rust().site_experiment_remaining_seconds
            == i64::try_from(remaining_seconds).unwrap_or(i64::MAX)
        {
            return;
        }
        self.as_mut()
            .set_site_experiment_remaining_seconds(i64::try_from(remaining_seconds).unwrap_or(0));
    }
}

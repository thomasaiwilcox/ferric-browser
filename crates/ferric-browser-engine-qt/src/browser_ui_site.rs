use super::{
    Config, CxxQtType, FocusObservation, Instant, MAX_DIAGNOSTICS_EXPORT_BYTES, Mode,
    ParsedCommand, Path, Pin, PrivacyKind, QString, QStringList, SITE_DOCTOR_EXPERIMENT_TIMEOUT,
    SiteExperiment, Uuid, Value, atomic_write_private, diagnostics, display_url,
    focus_mode_transition, matching_site_rules, qobject, safe_ipc_url, safe_site_origin,
    setting_supports_site_scope, site_doctor_remaining_seconds, toml_string_array_literal,
    userscript,
};

impl qobject::BrowserUi {
    pub(super) fn page_focus_observed_for(
        mut self: Pin<&mut Self>,
        index: i32,
        url: &QString,
        frame_path: &QString,
        sequence: i32,
        editable: bool,
        user_activated: bool,
    ) {
        if index != self.as_ref().rust().active_tab_index || !(1..=1_000_000).contains(&sequence) {
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
            .focus_suppressions
            .get(&key)
            .is_some_and(|suppressed| sequence <= *suppressed)
        {
            return;
        }
        if self
            .as_ref()
            .rust()
            .focus_observations
            .get(&key)
            .is_some_and(|observation| observation.url == url && sequence <= observation.sequence)
        {
            return;
        }
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.focus_suppressions.remove(&key);
            this.focus_observations.insert(
                key.clone(),
                FocusObservation {
                    url: url.clone(),
                    sequence,
                    editable,
                    user_activated,
                },
            );
        }
        let (focused_editable, user_activated) = self
            .as_ref()
            .rust()
            .focus_observations
            .iter()
            .filter(|((candidate, _), observation)| *candidate == index && observation.url == url)
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
        let display = display_url(&url.to_string());
        self.as_mut().set_current_url(url);
        self.set_display_url(QString::from(display));
    }

    pub(super) fn refresh_site_status(mut self: Pin<&mut Self>) -> QString {
        if self.as_ref().rust().profile_persistence.is_durable()
            && (self.as_ref().rust().storage_library.is_none()
                || self.as_ref().rust().storage_library_dirty.get())
        {
            self.as_mut().request_storage_library();
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
            && (self.as_ref().rust().storage_library.is_none()
                || self.as_ref().rust().storage_library_dirty.get())
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
        let proposal = self
            .as_ref()
            .rust()
            .last_site_doctor_result
            .clone()
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
        let Some(proposal) = self.as_ref().rust().last_site_doctor_result.clone() else {
            self.set_status_text(QString::from(
                "No Site Doctor durable-fix proposal is pending",
            ));
            return false;
        };
        let requested_id = proposal_id.to_string();
        if proposal.get("id").and_then(Value::as_str) != Some(requested_id.as_str()) {
            self.set_status_text(QString::from("Site Doctor proposal ID is stale"));
            return false;
        }
        if proposal.get("kind").and_then(Value::as_str) != Some("blocker-exception") {
            self.set_status_text(QString::from(
                "This Site Doctor result has no supported durable fix",
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
        let before = proposal
            .get("mutation")
            .and_then(Value::as_object)
            .and_then(|mutation| mutation.get("before"))
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let after = proposal
            .get("mutation")
            .and_then(Value::as_object)
            .and_then(|mutation| mutation.get("after"))
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let config = match serde_json::from_value::<Config>(self.as_ref().rust().config.clone()) {
            Ok(config) => config,
            Err(error) => {
                self.set_status_text(QString::from(format!(
                    "Site Doctor proposal could not read effective config: {error}"
                )));
                return false;
            }
        };
        if config.blocking.bypass_sites != before {
            self.set_status_text(QString::from(
                "Site Doctor proposal is stale; blocker settings changed since the experiment",
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
        if let Err(error) = self.as_mut().execute_runtime_config_command(&command) {
            self.set_status_text(QString::from(format!(
                "Site Doctor durable fix was not applied: {error}"
            )));
            return false;
        }
        self.as_mut().reload_blocking_policy();
        if let Some(result) = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .last_site_doctor_result
            .as_mut()
        {
            if let Some(object) = result.as_object_mut() {
                object.insert("state".into(), Value::String("applied".into()));
            }
        }
        self.set_status_text(QString::from(
            "Site Doctor blocker exception saved; policy reloaded",
        ));
        true
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
            self.set_status_text(QString::from("Site-data clear completion is stale"));
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

    pub(super) fn diagnostics_json(mut self: Pin<&mut Self>) -> QString {
        const MAX_DIAGNOSTICS_TEXT: usize = 48 * 1024;

        let result = self.ipc_diagnostics(&Value::Null);
        let mut serialized = match result {
            Ok(value) => serde_json::to_string_pretty(&value)
                .unwrap_or_else(|_| "{\"error\":\"diagnostics serialization failed\"}".into()),
            Err(error) => serde_json::json!({"error": error}).to_string(),
        };
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .diagnostics_preview_payload = Some(serialized.clone());
        if serialized.len() > MAX_DIAGNOSTICS_TEXT {
            serialized.truncate(MAX_DIAGNOSTICS_TEXT);
            serialized.push_str("\n… output truncated by the browser UI");
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .diagnostics_preview_ready = true;
        QString::from(serialized)
    }

    pub(super) fn spellcheck_dictionaries(self: Pin<&mut Self>) -> QStringList {
        let _ = self;
        diagnostics::installed_dictionary_names()
            .into_iter()
            .map(QString::from)
            .collect()
    }

    pub(super) fn export_diagnostics(mut self: Pin<&mut Self>, path: &QString) -> bool {
        if !self.as_ref().rust().diagnostics_preview_ready {
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
        let Some(payload) = self.as_ref().rust().diagnostics_preview_payload.clone() else {
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
                    .diagnostics_preview_ready = false;
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .diagnostics_preview_payload = None;
                self.set_status_text(QString::from("Diagnostics export written"));
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
                Some((tab_id, safe_ipc_url(&url), origin, rust.active_tab_index))
            })
        };
        let Some((tab, url, origin, tab_index)) = details else {
            self.set_status_text(QString::from(
                "Site Doctor requires an active HTTP(S) document",
            ));
            return QString::default();
        };
        if kind == "userscripts-off" {
            let current_url = self.as_ref().rust().current_url.to_string();
            let has_matching_scripts =
                self.as_ref()
                    .rust()
                    .userscript_roots
                    .as_ref()
                    .is_some_and(|roots| {
                        userscript::matching_page_scripts(&roots.config, &current_url, false)
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
            let has_site_settings = serde_json::from_value::<Config>(
                self.as_ref().rust().config.clone(),
            )
            .is_ok_and(|config| {
                matching_site_rules(&config, &url)
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
        let (temporary_tab, temporary_index) = if fresh_view_experiment {
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
            (Some(temporary_tab), Some(index))
        } else {
            (None, None)
        };
        let mut sites = self
            .as_ref()
            .rust()
            .blocking_bypass_sites
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let prior_bypass_sites = sites.clone();
        if kind == "blocking-bypass" {
            if sites.iter().any(|site| site == &origin) {
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
            sites.push(origin.clone());
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
            this.pending_engine_action = Some(if fresh_view_experiment {
                format!(
                    "site-doctor-fresh-view\t{}\t{}\t{}",
                    temporary_index.unwrap_or(-1),
                    id,
                    url
                )
            } else {
                format!("site-doctor-reload\t{tab_index}\t{id}")
            });
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
            self.set_status_text(QString::from("Site Doctor experiment ID is stale"));
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
                .chain(std::iter::once(experiment.origin.clone()))
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
        let cleanup_index = experiment.temporary_tab.and_then(|tab| {
            self.as_ref()
                .rust()
                .tab_ids
                .iter()
                .position(|candidate| *candidate == tab)
        });
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.active_site_experiment = None;
            if let Some(index) = cleanup_index {
                this.pending_engine_action = Some(format!("site-doctor-close\t{index}"));
            }
        }
        self.as_mut().clear_site_experiment_presentation();
        if succeeded && same_tab && same_url {
            let proposal = if experiment.kind == "blocking-bypass" {
                let mut durable_sites = experiment.prior_bypass_sites.clone();
                if !durable_sites.contains(&experiment.origin) {
                    durable_sites.push(experiment.origin.clone());
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

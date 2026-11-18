//! Native editor requests change management state only. Apply/export continue
//! through the shared command/coordinator paths and their existing admission.
use crate::{CxxQtType, Pin, QString, Value, qobject};
use cxx_qt::Threading;
use ferric_browser_core::{
    SitePolicyEdit, SitePolicyExportSubject, SitePolicyRuleRef, SitePolicySource,
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Proposal {
    pattern: String,
    priority: i32,
    interaction: Option<ferric_browser_config::InteractionPolicyPatch>,
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case", deny_unknown_fields)]
enum Request {
    Capture {
        draft: String,
        revision: u64,
        tab: String,
        family: ferric_browser_config::InteractionFamily,
    },
    Scope {
        url: String,
        pattern: Option<String>,
        #[serde(default)]
        samples: Vec<String>,
    },
    Key {
        token: String,
    },
    TestPage {
        draft: String,
        revision: u64,
        rule: String,
        mapping: String,
        context: ferric_browser_config::InteractionRemapContext,
        #[serde(default)]
        prepare_receiver: bool,
        tab: Option<String>,
    },
    Open {
        source: String,
        id: String,
        #[serde(default)]
        new: bool,
        pattern: Option<String>,
    },
    Patch {
        draft: String,
        revision: u64,
        proposals: Vec<Proposal>,
    },
    Copy {
        draft: String,
        revision: u64,
        destination: String,
        new_id: Option<String>,
    },
    Validate {
        draft: String,
        revision: u64,
    },
    Rebase {
        draft: String,
        revision: u64,
    },
    Discard {
        draft: String,
    },
}

fn execute(
    state: &ferric_browser_application::BrowserApplication,
    request: Request,
) -> Result<Value, String> {
    let view = match request {
        Request::Capture { .. } => return Err("capture requires its owning native window".into()),
        Request::Scope {
            url,
            pattern,
            samples,
        } => {
            return super::builder::scope(&url, pattern.as_deref(), &samples);
        }
        Request::Key { token } => return super::builder::key(&token),
        Request::TestPage {
            draft,
            revision,
            rule,
            mapping,
            context,
            ..
        } => {
            return super::builder::test_page(state, &draft, revision, &rule, &mapping, context);
        }
        Request::Open {
            source,
            id,
            new,
            pattern,
        } => state.open_site_policy_draft(&SitePolicyEdit::Rule {
            target: SitePolicyRuleRef {
                source: SitePolicySource::parse(&source)?,
                id,
            },
            new,
            pattern,
            priority: None,
            remove_interaction: false,
            enabled: None,
        })?,
        Request::Patch {
            draft,
            revision,
            proposals,
        } => {
            let current = state.site_policy_draft(&draft)?;
            if current.revision != revision {
                return Err("draft revision changed; review and rebase explicitly".into());
            }
            if proposals.len() != current.rules.len() {
                return Err("draft proposal count changed".into());
            }
            let rules = current
                .rules
                .iter()
                .zip(proposals)
                .map(|(original, proposal)| {
                    let mut rule = original
                        .proposed
                        .as_ref()
                        .or(original.original.as_ref())
                        .or(original.copied_from.as_ref())
                        .cloned()
                        .ok_or("draft rule has no local basis")?;
                    rule.pattern = proposal.pattern;
                    rule.priority = proposal.priority;
                    rule.interaction = proposal.interaction;
                    Ok((rule.interaction.is_some() || !rule.set.is_empty()).then_some(rule))
                })
                .collect::<Result<Vec<_>, String>>()?;
            state.update_site_policy_draft(&draft, revision, rules)?
        }
        Request::Copy {
            draft,
            revision,
            destination,
            new_id,
        } => {
            let view = state.site_policy_draft(&draft)?;
            if view.revision != revision {
                return Err("draft revision changed; review before copying".into());
            }
            let temporary = match SitePolicySource::parse(&destination)? {
                SitePolicySource::Generated => false,
                SitePolicySource::Temporary => true,
                SitePolicySource::Authored(_) => {
                    return Err("authored destinations are read-only".into());
                }
            };
            let document = state.export_site_policy(&SitePolicyExportSubject::Draft(draft))?;
            let document = if let Some(id) = new_id {
                if document.rules().len() != 1 {
                    return Err("choose a new ID only for a single-rule copy".into());
                }
                let mut rules = document.rules().to_vec();
                match &mut rules[0] {
                    ferric_browser_config::SitePolicyExchangeOperation::UpsertInteraction {
                        id: rule_id,
                        ..
                    } => *rule_id = id,
                    ferric_browser_config::SitePolicyExchangeOperation::UnsetInteraction {
                        ..
                    } => return Err("a removal cannot be cloned into a new ID".into()),
                }
                ferric_browser_config::SitePolicyExchange::with_requirements(
                    rules,
                    document.required_capabilities(),
                )?
            } else {
                document
            };
            state.import_site_policy_draft(&document, temporary, true)?
        }
        Request::Validate { draft, revision } => {
            state.validate_site_policy_draft(&draft, revision)?
        }
        Request::Rebase { draft, revision } => state.rebase_site_policy_draft(&draft, revision)?,
        Request::Discard { draft } => {
            state.discard_site_policy_draft(&draft)?;
            return Ok(serde_json::json!({"draft":draft,"discarded":true}));
        }
    };
    serde_json::to_value(view).map_err(|error| error.to_string())
}

impl qobject::BrowserUi {
    pub(crate) fn site_policy_editor_request(self: Pin<&mut Self>, payload: &QString) -> QString {
        let Some(owner) = self.as_ref().rust().state.as_ref().and_then(
            ferric_browser_application::BrowserApplication::capture_configuration_request,
        ) else {
            return QString::default();
        };
        if payload.len() > 2 * 1024 * 1024 || !self.as_ref().rust().view_alive {
            return QString::default();
        }
        let id = uuid::Uuid::new_v4().to_string();
        let request_id = id.clone();
        let payload = payload.clone();
        let queued =
            self.as_ref().qt_thread().queue(move |mut ui| {
                let current =
                    ui.as_ref().rust().view_alive
                        && !ui.as_ref().rust().profile_bootstrap_pending
                        && ui.as_ref().rust().state.as_ref().is_some_and(|state| {
                            state.configuration_request_owner_is_current(&owner)
                        });
                if !current {
                    return;
                }
                let result = (|| {
                    let request: Request = serde_json::from_str(&payload.to_string())
                        .map_err(|error| error.to_string())?;
                    if let Request::Discard { draft } = &request {
                        ui.as_mut().cancel_preview_for_draft(draft);
                    }
                    if let Request::Capture {
                        draft,
                        revision,
                        tab,
                        family,
                    } = request
                    {
                        return ui
                            .as_mut()
                            .start_site_policy_capture(&draft, revision, &tab, family);
                    }
                    let receiver_context = match &request {
                        Request::TestPage {
                            prepare_receiver: true,
                            context,
                            tab,
                            ..
                        } => Some((*context, tab.clone())),
                        _ => None,
                    };
                    let result = execute(
                        ui.as_ref()
                            .rust()
                            .state
                            .as_ref()
                            .ok_or("profile is unavailable")?,
                        request,
                    )?;
                    if let Some((context, tab)) = receiver_context {
                        use ferric_browser_config::InteractionRemapContext as Context;
                        let expected = match context {
                            Context::PassthroughOnce => crate::Mode::PassThroughOnce,
                            Context::Passthrough => crate::Mode::PassThrough,
                            Context::NormalPageKey => crate::Mode::Normal,
                            Context::HintPageKey => crate::Mode::Hint,
                        };
                        let mut rust = ui.as_mut().rust_mut();
                        let rust = rust.as_mut().get_mut();
                        if rust.core_mode != expected
                            || rust.raw_passthrough
                            || rust.interaction.native_prompt_leases.top().is_some()
                            || usize::try_from(rust.active_tab_index)
                                .ok()
                                .and_then(|index| rust.tab_ids.get(index))
                                .map(ToString::to_string)
                                != tab
                        {
                            return Err("Page test mode or native input owner changed".into());
                        }
                        let index = rust.active_tab_index;
                        // Like explicit passthrough entry, allow a fresh native
                        // receipt after Escape suppressed automatic reentry.
                        rust.interaction
                            .focus_suppressions
                            .retain(|(tab, _), _| *tab != index);
                    }
                    Ok(result)
                })();
                let value = match result {
                    Ok(value) => serde_json::json!({"request":request_id,"result":value}),
                    Err(error) => serde_json::json!({"request":request_id,"error":error}),
                };
                ui.as_mut()
                    .set_site_policy_result(QString::from(value.to_string()));
            });
        if queued.is_ok() {
            QString::from(id)
        } else {
            QString::default()
        }
    }

    pub(super) fn open_site_policy_workspace(
        mut self: Pin<&mut Self>,
        url: Option<&str>,
    ) -> Result<Value, String> {
        let mut value = self.as_ref().list_site_policies(url, true)?;
        value["workspace"] = serde_json::json!(true);
        let context = value["url"].as_str().unwrap_or("");
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(format!("show-site-policies\t{context}"));
        Ok(value)
    }
}

//! A single-use observer of the existing writer, with no write/publication authority.
use serde::Serialize;
use std::sync::Weak;
use uuid::Uuid;

use super::{BrowserApplication, SitePolicyDraft, dirty_fields, parse_id};
use crate::{ConfigurationRequestCapture, PreparedConfigurationUpdate, PreparedInteractionEdits};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ApplyState {
    Preparing,
    Writing,
    ActivationPending,
    Applied,
    ReloadRequired,
    Failed,
    CompletionUnknown,
}

#[derive(Clone, Serialize)]
pub struct SitePolicyDraftOperation {
    pub revision: u64,
    state: ApplyState,
    pub saved: Option<bool>,
    pub active: bool,
    pub durability_uncertain: bool,
}

pub(super) struct PendingApply {
    token: Uuid,
    revision: u64,
    activate: bool,
    persist: bool,
    pub(super) admitted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SitePolicyApplyOutcome {
    Published { durability_uncertain: bool },
    SavedInactive { durability_uncertain: bool },
    Failed,
}

/// Not Clone: exactly one validation, persistence or activation phase owns it.
/// Dropping it reports failure/uncertainty, never cancellation of a writer.
pub struct SitePolicyDraftApply {
    session: Weak<crate::profile_session::Session>,
    owner: ConfigurationRequestCapture,
    id: Uuid,
    token: Uuid,
    revision: u64,
    admitted: bool,
    activation: bool,
    unresolved: bool,
}

impl BrowserApplication {
    /// Captures one immutable reviewed version and reserves its apply slot.
    /// CSS validation and write admission remain with the existing coordinator.
    ///
    /// # Errors
    /// Rejects invalid/stale drafts and duplicate pending requests.
    pub fn begin_site_policy_draft_apply(
        &self,
        id: &str,
        revision: u64,
        activate: bool,
    ) -> Result<(PreparedInteractionEdits, SitePolicyDraftApply), String> {
        let prepared = self.prepare_site_policy_draft(id, revision)?;
        let id = parse_id(id)?;
        let profile = self
            .active_profile
            .as_ref()
            .ok_or("an open profile is required")?;
        let owner = self
            .capture_configuration_request()
            .ok_or("configuration owner is unavailable")?;
        let mut catalog = profile
            .session
            .0
            .site_policy_drafts
            .lock()
            .map_err(|_| "draft catalogue is unavailable")?;
        let draft = catalog.drafts.get_mut(&id).ok_or("unknown draft")?;
        draft.check(self, revision, true)?;
        if draft.pending.is_some() {
            return Err("draft apply is already pending".into());
        }
        let token = Uuid::new_v4();
        let persist = matches!(
            draft.source,
            ferric_browser_config::PolicySourceIdentity::Generated
        ) && self
            .profile_snapshot()
            .is_some_and(|profile| profile.durable);
        draft.pending = Some(PendingApply {
            token,
            revision,
            activate,
            persist,
            admitted: false,
        });
        draft.view.operation = Some(SitePolicyDraftOperation {
            revision,
            state: ApplyState::Preparing,
            saved: None,
            active: false,
            durability_uncertain: false,
        });
        Ok((
            prepared,
            SitePolicyDraftApply {
                session: std::sync::Arc::downgrade(&profile.session.0),
                owner,
                id,
                token,
                revision,
                admitted: false,
                activation: false,
                unresolved: true,
            },
        ))
    }
}

impl SitePolicyDraftApply {
    #[must_use]
    pub fn owner_is_current(&self, app: &BrowserApplication) -> bool {
        app.configuration_request_owner_is_current(&self.owner)
    }
    #[must_use]
    pub fn is_current(&self, app: &BrowserApplication) -> bool {
        if !app.configuration_request_is_current(&self.owner) {
            return false;
        }
        self.with_draft(|draft| self.activation || draft.view.revision == self.revision)
            .unwrap_or(false)
    }

    #[must_use]
    pub const fn is_admitted(&self) -> bool {
        self.admitted
    }

    #[must_use]
    pub fn persist(&self) -> bool {
        self.with_draft(|draft| {
            draft
                .pending
                .as_ref()
                .is_some_and(|pending| pending.persist)
        })
        .unwrap_or(false)
    }

    pub fn admitted(&mut self) {
        self.admitted = true;
        self.with_draft(|draft| {
            if let Some(pending) = &mut draft.pending {
                pending.admitted = true;
            }
            if let Some(operation) = &mut draft.view.operation {
                operation.state = ApplyState::Writing;
            }
        });
    }

    /// Reports the save/publication receipt. A returned observer requests the
    /// explicitly selected configuration activation through the same coordinator.
    #[must_use]
    pub fn finish(
        mut self,
        app: Option<&BrowserApplication>,
        outcome: SitePolicyApplyOutcome,
    ) -> Option<Self> {
        let owner_current =
            app.is_some_and(|app| app.configuration_request_owner_is_current(&self.owner));
        let published =
            matches!(outcome, SitePolicyApplyOutcome::Published { .. }) && owner_current;
        let activate = self
            .with_draft(|draft| {
                draft
                    .pending
                    .as_ref()
                    .is_some_and(|pending| pending.activate)
            })
            .unwrap_or(false);
        self.with_draft(|draft| {
            if let Some(operation) = &mut draft.view.operation {
                match outcome {
                    SitePolicyApplyOutcome::Published {
                        durability_uncertain,
                    } => {
                        operation.saved = Some(
                            draft
                                .pending
                                .as_ref()
                                .is_some_and(|pending| pending.persist),
                        );
                        operation.durability_uncertain = durability_uncertain;
                        operation.state = if published && activate {
                            ApplyState::ActivationPending
                        } else {
                            ApplyState::ReloadRequired
                        };
                    }
                    SitePolicyApplyOutcome::SavedInactive {
                        durability_uncertain,
                    } => {
                        operation.saved = Some(true);
                        operation.state = ApplyState::ReloadRequired;
                        operation.durability_uncertain = durability_uncertain;
                    }
                    SitePolicyApplyOutcome::Failed => {
                        operation.saved = Some(false);
                        operation.state = ApplyState::Failed;
                    }
                }
            }
        });
        if published && let Some(app) = app {
            self.refresh_saved_base(app);
        }
        self.unresolved = false;
        if published && activate {
            let Some(owner) = app.and_then(BrowserApplication::capture_configuration_request)
            else {
                self.with_draft(|draft| {
                    if let Some(operation) = &mut draft.view.operation {
                        operation.state = ApplyState::ReloadRequired;
                    }
                    draft.pending = None;
                });
                return None;
            };
            self.owner = owner;
            self.activation = true;
            self.unresolved = true;
            Some(self)
        } else {
            self.with_draft(|draft| draft.pending = None);
            None
        }
    }

    /// Prepares only the exact saved configuration generation for activation.
    ///
    /// # Errors
    /// Rejects intervening configuration/profile changes; saved data is retained.
    pub fn prepare_activation(
        &self,
        app: &BrowserApplication,
    ) -> Result<PreparedConfigurationUpdate, String> {
        if !self.activation || !self.is_current(app) {
            return Err(
                "saved configuration changed before activation; explicitly review and reload it"
                    .into(),
            );
        }
        app.prepare_configuration_reload(app.configuration_layers().clone())
            .map_err(|error| error.to_string())
    }

    pub fn finish_activation(mut self, app: Option<&BrowserApplication>, active: bool) {
        let active = active
            && app.is_some_and(|app| app.configuration_request_owner_is_current(&self.owner));
        self.with_draft(|draft| {
            if let Some(operation) = &mut draft.view.operation {
                operation.active = active;
                operation.state = if active {
                    ApplyState::Applied
                } else {
                    ApplyState::ReloadRequired
                };
            }
        });
        if active && let Some(app) = app {
            self.refresh_saved_base(app);
        }
        self.with_draft(|draft| draft.pending = None);
        self.unresolved = false;
    }

    fn refresh_saved_base(&self, app: &BrowserApplication) {
        let source = self.with_draft(|draft| draft.source.clone());
        let Some(capture) = source
            .as_ref()
            .and_then(|source| app.draft_capture(source).ok())
        else {
            return;
        };
        self.with_draft(|draft| {
            if draft.view.revision != self.revision {
                return;
            }
            for rule in &mut draft.view.rules {
                if let Ok((original, _)) = capture.rule_snapshots(&draft.source, &rule.id) {
                    rule.original.clone_from(&original);
                    rule.proposed = original;
                    rule.copied_from = None;
                }
            }
            draft.view.configuration_revision = capture.configuration_revision();
            draft.view.profile_revision = capture.shared_profile_revision();
            draft.capture = capture;
            draft.view.dirty_fields = dirty_fields(&draft.view.rules);
            draft.view.dirty = !draft.view.dirty_fields.is_empty();
            draft.view.validation = None;
        });
    }

    fn with_draft<T>(&self, apply: impl FnOnce(&mut SitePolicyDraft) -> T) -> Option<T> {
        let session = self.session.upgrade()?;
        let mut catalog = session.site_policy_drafts.lock().ok()?;
        let draft = catalog.drafts.get_mut(&self.id)?;
        let pending = draft.pending.as_ref()?;
        if pending.token != self.token || pending.revision != self.revision {
            return None;
        }
        Some(apply(draft))
    }
}

impl Drop for SitePolicyDraftApply {
    fn drop(&mut self) {
        if !self.unresolved {
            return;
        }
        self.with_draft(|draft| {
            if let Some(operation) = &mut draft.view.operation {
                operation.state = if self.activation {
                    ApplyState::ReloadRequired
                } else if self.admitted {
                    ApplyState::CompletionUnknown
                } else {
                    ApplyState::Failed
                };
                if !self.admitted {
                    operation.saved = Some(false);
                }
            }
            draft.pending = None;
        });
    }
}

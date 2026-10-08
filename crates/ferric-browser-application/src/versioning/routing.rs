//! Application-owned admission for typed versioning requests. Structural
//! commits are available for durable normal profiles; sensitive capabilities
//! remain gated by their separate qualification contracts.

use ferric_browser_core::{
    CapabilityKey, CommandSource, CommitId, EngineEffect, RestoreInto, RestoreMode, ScopeSelector,
    VersioningCommand, VersioningEngineEffect, VersioningFailure, VersioningReason, VersioningRef,
    VersioningRequest, VersioningScope, versioning::SensitiveCategory,
};
use ferric_browser_runtime::{RuntimeEffect, RuntimeError};

use super::CleanBranchSwitchTarget;
use crate::BrowserApplication;

impl BrowserApplication {
    /// Replaces reducer-emitted requests with application decisions and
    /// forwards any reducer effects produced during structural admission.
    pub(crate) fn route_versioning_effects(
        &mut self,
        effects: Vec<RuntimeEffect>,
    ) -> Result<Vec<RuntimeEffect>, RuntimeError> {
        let mut routed = Vec::with_capacity(effects.len());
        for effect in effects {
            match effect {
                RuntimeEffect::Engine(EngineEffect::Versioning(
                    VersioningEngineEffect::Request(request),
                )) => {
                    let (response, mut admission_effects) = self
                        .route_versioning_request(&request)
                        .map_err(RuntimeError::from)?;
                    routed.append(&mut admission_effects);
                    routed.push(RuntimeEffect::Engine(EngineEffect::Versioning(response)));
                }
                other => routed.push(other),
            }
        }
        Ok(routed)
    }

    fn route_versioning_request(
        &mut self,
        request: &VersioningRequest,
    ) -> Result<(VersioningEngineEffect, Vec<RuntimeEffect>), VersioningFailure> {
        if !self.windows().contains_key(&request.window()) {
            return Err(VersioningFailure::new(
                VersioningReason::StaleOwner,
                SensitiveCategory::None,
            ));
        }
        match request.command() {
            VersioningCommand::Status => Ok((
                VersioningEngineEffect::Status {
                    production_enabled: self.structural_commit_owner(request.window()).is_ok(),
                    active_operations: self
                        .versioning
                        .active_count()
                        .saturating_add(u16::from(self.pending_commit.is_some())),
                },
                Vec::new(),
            )),
            VersioningCommand::Commit {
                message,
                scope,
                pages: false,
                editable: false,
            } => self.route_structural_commit(request, message, *scope),
            VersioningCommand::Commit {
                pages: true,
                editable,
                ..
            } => self.reject_sensitive_commit(request, *editable),
            VersioningCommand::CommitList { .. }
            | VersioningCommand::CommitInfo { .. }
            | VersioningCommand::CommitDelete { .. }
            | VersioningCommand::Diff { .. } => self.route_commit_read_command(request),
            VersioningCommand::Restore {
                reference,
                mode: RestoreMode::Live,
                into: RestoreInto::NewWindow,
                policies,
            } => self.route_new_window_restore(request, reference, *policies),
            VersioningCommand::Branch {
                name,
                scope: VersioningScope::Tab,
                clean: true,
            } => {
                if !cfg!(feature = "unqualified-clean-branch-test") {
                    return Err(unavailable_failure(request.command()));
                }
                if !matches!(
                    request.source(),
                    CommandSource::Keyboard | CommandSource::Ui | CommandSource::Cli
                ) {
                    return Err(VersioningFailure::new(
                        VersioningReason::ConfirmationRequired,
                        SensitiveCategory::Profile,
                    ));
                }
                let target = request.target().ok_or_else(stale_owner_failure)?;
                let (branch, operation, commit, effects) =
                    self.begin_clean_branch_fork(request.window(), target, name.as_str())?;
                Ok((
                    VersioningEngineEffect::BranchQueued {
                        branch,
                        operation,
                        commit,
                    },
                    effects,
                ))
            }
            VersioningCommand::Branch {
                name,
                scope: VersioningScope::Window,
                clean: true,
            } => {
                if !cfg!(feature = "unqualified-clean-branch-test") {
                    return Err(unavailable_failure(request.command()));
                }
                if !matches!(
                    request.source(),
                    CommandSource::Keyboard | CommandSource::Ui | CommandSource::Cli
                ) {
                    return Err(VersioningFailure::new(
                        VersioningReason::ConfirmationRequired,
                        SensitiveCategory::Profile,
                    ));
                }
                let (branch, operation, commit, effects) =
                    self.begin_clean_branch_window_fork(request.window(), name.as_str())?;
                Ok((
                    VersioningEngineEffect::BranchQueued {
                        branch,
                        operation,
                        commit,
                    },
                    effects,
                ))
            }
            VersioningCommand::Switch { reference } => {
                if !cfg!(feature = "unqualified-clean-branch-test") {
                    return Err(unavailable_failure(request.command()));
                }
                let target =
                    self.resolve_clean_branch_switch(request.window(), reference.as_str())?;
                let (branch, window, child) = match target {
                    CleanBranchSwitchTarget::Parent { branch, window } => (branch, window, false),
                    CleanBranchSwitchTarget::Child { branch, .. } => {
                        (branch, request.window(), true)
                    }
                };
                Ok((
                    VersioningEngineEffect::SwitchResolved {
                        branch,
                        window,
                        child,
                    },
                    Vec::new(),
                ))
            }
            command => Err(VersioningFailure::new(
                unavailable_reason(command),
                category(command),
            )),
        }
    }

    fn route_structural_commit(
        &mut self,
        request: &VersioningRequest,
        message: &str,
        scope: VersioningScope,
    ) -> Result<(VersioningEngineEffect, Vec<RuntimeEffect>), VersioningFailure> {
        let selector = match scope {
            VersioningScope::Tab => ScopeSelector::Tab {
                window: request.window(),
                target: request.target().ok_or_else(stale_owner_failure)?,
            },
            VersioningScope::Window => ScopeSelector::Window {
                window: request.window(),
            },
            VersioningScope::Context => ScopeSelector::Context {
                window: request.window(),
            },
        };
        let (operation, commit, effects) =
            self.begin_structural_commit(selector, message.to_owned())?;
        Ok((
            VersioningEngineEffect::CommitQueued { operation, commit },
            effects,
        ))
    }

    fn reject_sensitive_commit(
        &self,
        request: &VersioningRequest,
        editable: bool,
    ) -> Result<(VersioningEngineEffect, Vec<RuntimeEffect>), VersioningFailure> {
        self.structural_commit_owner(request.window())?;
        if !matches!(
            request.source(),
            CommandSource::Keyboard | CommandSource::Ui
        ) {
            return Err(VersioningFailure::new(
                VersioningReason::ConfirmationRequired,
                SensitiveCategory::Page,
            ));
        }
        if self.configuration().versioning.page_capture
            == ferric_browser_config::SensitiveCapturePolicy::Deny
        {
            return Err(VersioningFailure::new(
                VersioningReason::SensitiveCaptureDenied,
                SensitiveCategory::Page,
            ));
        }
        if editable
            && self.configuration().versioning.editable_capture
                == ferric_browser_config::SensitiveCapturePolicy::Deny
        {
            return Err(VersioningFailure::new(
                VersioningReason::SensitiveCaptureDenied,
                SensitiveCategory::Editable,
            ));
        }
        Err(unavailable_failure(request.command()))
    }

    fn route_commit_read_command(
        &mut self,
        request: &VersioningRequest,
    ) -> Result<(VersioningEngineEffect, Vec<RuntimeEffect>), VersioningFailure> {
        let response = match request.command() {
            VersioningCommand::CommitList { limit, offset } => {
                let ticket = self.request_commit_list(request.window(), *limit, *offset)?;
                VersioningEngineEffect::CommitListQueued {
                    request_id: ticket.value(),
                }
            }
            VersioningCommand::CommitInfo { reference } => {
                let ticket =
                    self.request_commit_info(request.window(), parse_commit_reference(reference)?)?;
                VersioningEngineEffect::CommitInfoQueued {
                    request_id: ticket.value(),
                }
            }
            VersioningCommand::CommitDelete { reference } => {
                let ticket = self.request_commit_delete_preview(
                    request.window(),
                    parse_commit_reference(reference)?,
                )?;
                VersioningEngineEffect::CommitDeletePreviewQueued {
                    request_id: ticket.value(),
                }
            }
            VersioningCommand::Diff {
                left,
                right,
                kind,
                offset,
            } => {
                let ticket = self.request_commit_diff(
                    request.window(),
                    parse_commit_reference(left)?,
                    parse_commit_reference(right)?,
                    *kind,
                    *offset,
                )?;
                VersioningEngineEffect::DiffQueued {
                    request_id: ticket.value(),
                }
            }
            _ => return Err(unavailable_failure(request.command())),
        };
        Ok((response, Vec::new()))
    }

    fn route_new_window_restore(
        &mut self,
        request: &VersioningRequest,
        reference: &VersioningRef,
        policies: bool,
    ) -> Result<(VersioningEngineEffect, Vec<RuntimeEffect>), VersioningFailure> {
        let commit = parse_commit_reference(reference)?;
        let ticket = if policies {
            self.request_policy_restore_preview(request.window(), commit)?
        } else {
            self.request_new_window_restore_plan(request.window(), commit)?
        };
        let response = if policies {
            VersioningEngineEffect::PolicyRestorePreviewQueued {
                request_id: ticket.value(),
            }
        } else {
            VersioningEngineEffect::RestoreQueued {
                request_id: ticket.value(),
            }
        };
        Ok((response, Vec::new()))
    }
}

fn parse_commit_reference(reference: &VersioningRef) -> Result<CommitId, VersioningFailure> {
    reference.as_str().parse::<CommitId>().map_err(|_| {
        VersioningFailure::new(
            VersioningReason::AmbiguousReference,
            SensitiveCategory::None,
        )
    })
}

fn stale_owner_failure() -> VersioningFailure {
    VersioningFailure::new(VersioningReason::StaleOwner, SensitiveCategory::None)
}

fn unavailable_reason(command: &VersioningCommand) -> VersioningReason {
    use CapabilityKey::{ArchiveOffline, CookieIdentity, LiveRetention};
    if matches!(command, VersioningCommand::MergeApply { .. }) {
        VersioningReason::MergeApplyUnavailable
    } else if matches!(
        command,
        VersioningCommand::Restore {
            into: RestoreInto::Current,
            ..
        }
    ) {
        // INVARIANT: a structural checkpoint cannot preserve unknown live
        // page state. Until a qualified live-stash path is available, current
        // window replacement must fail closed instead of looking like a
        // generic restore-policy/runtime limitation.
        VersioningReason::RetentionUnavailable
    } else if command.required_capabilities().contains(&CookieIdentity) {
        VersioningReason::CookieIdentityLossy
    } else if command.required_capabilities().contains(&ArchiveOffline) {
        VersioningReason::ArchiveIsolationUnavailable
    } else if command.required_capabilities().contains(&LiveRetention) {
        VersioningReason::RetentionUnavailable
    } else if command.required_capabilities().is_empty() {
        VersioningReason::VersioningDisabled
    } else {
        VersioningReason::UnqualifiedRuntime
    }
}

fn unavailable_failure(command: &VersioningCommand) -> VersioningFailure {
    VersioningFailure::new(unavailable_reason(command), category(command))
}

fn category(command: &VersioningCommand) -> SensitiveCategory {
    use CapabilityKey::CookieIdentity;
    if command.required_capabilities().contains(&CookieIdentity) {
        SensitiveCategory::Cookie
    } else if let VersioningCommand::Commit {
        pages: true,
        editable,
        ..
    } = command
    {
        if *editable {
            SensitiveCategory::Editable
        } else {
            SensitiveCategory::Page
        }
    } else if matches!(command, VersioningCommand::Restore { .. }) {
        SensitiveCategory::Page
    } else if matches!(command, VersioningCommand::Branch { .. }) {
        SensitiveCategory::Profile
    } else {
        SensitiveCategory::None
    }
}

#[cfg(test)]
mod tests {
    use ferric_browser_core::{VersioningCommand, VersioningRef};

    use super::{
        RestoreInto, RestoreMode, VersioningReason, category, parse_commit_reference,
        unavailable_reason,
    };
    use ferric_browser_core::versioning::SensitiveCategory;

    #[test]
    fn current_window_restore_refuses_without_qualified_live_retention() {
        let command = VersioningCommand::Restore {
            reference: VersioningRef::new("commit-id").expect("bounded reference"),
            mode: RestoreMode::Live,
            into: RestoreInto::Current,
            policies: false,
        };

        assert_eq!(
            unavailable_reason(&command),
            VersioningReason::RetentionUnavailable
        );
        assert_eq!(category(&command), SensitiveCategory::Page);
    }

    #[test]
    fn ambiguous_commit_prefix_is_not_a_restore_target() {
        let prefix = VersioningRef::new("commit:12345678").expect("bounded prefix");
        let failure = parse_commit_reference(&prefix).expect_err("exact ID required");
        assert_eq!(failure.reason(), VersioningReason::AmbiguousReference);
    }

    #[test]
    fn unsupported_sensitive_capture_stays_classified_as_page_or_editable_data() {
        let archive = VersioningCommand::Commit {
            message: "archive".into(),
            scope: ferric_browser_core::VersioningScope::Window,
            pages: true,
            editable: false,
        };
        let editable = VersioningCommand::Commit {
            message: "archive".into(),
            scope: ferric_browser_core::VersioningScope::Window,
            pages: true,
            editable: true,
        };
        assert_eq!(
            unavailable_reason(&archive),
            VersioningReason::UnqualifiedRuntime
        );
        assert_eq!(category(&archive), SensitiveCategory::Page);
        assert_eq!(category(&editable), SensitiveCategory::Editable);
    }
}

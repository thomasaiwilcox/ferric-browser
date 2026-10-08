//! Same-thread handoff between the exact parent and a prepared ephemeral
//! child BrowserUi. QML supplies object identity, not a native profile claim.

use super::{CxxQtType, Pin, QString, qobject};
use crate::ValidatedUrl;
use ferric_browser_application::versioning::CleanBranchSwitchTarget;
use ferric_browser_core::{
    BranchId, NativeProfileOwner, OperationId, ParseInput, PrivacyKind, SafePageUrl,
    VersioningCommand, VersioningScope, parse_chain,
};

fn canonical_branch(value: &QString) -> Option<BranchId> {
    let raw = value.to_string();
    let id = raw.parse::<BranchId>().ok()?;
    (id.to_string() == raw).then_some(id)
}

fn canonical_operation(value: &QString) -> Option<OperationId> {
    let raw = value.to_string();
    let id = raw.parse::<OperationId>().ok()?;
    (id.to_string() == raw).then_some(id)
}

fn resolve_clean_branch_switch(
    parent: &qobject::BrowserUi,
    child: Option<&qobject::BrowserUi>,
    command: &QString,
) -> QString {
    let input = command.to_string();
    let Ok(parsed) = parse_chain(&input, ParseInput::Interactive) else {
        return QString::from("");
    };
    let [parsed] = parsed.as_slice() else {
        return QString::from("");
    };
    let Ok(VersioningCommand::Switch { reference }) = VersioningCommand::from_parsed(parsed, 1)
    else {
        return QString::from("");
    };
    let parent = parent.rust();
    let Some(application) = parent.state.as_ref() else {
        return QString::from("");
    };
    let source_window = if let Some(child) = child {
        let child = child.rust();
        let (Some(child_application), Some(child_tab), Some((profile, instance))) = (
            child.state.as_ref(),
            child.tab,
            child.native_versioning_profile,
        ) else {
            return QString::from("");
        };
        if child_application.versioning().owner(profile)
            != Some(NativeProfileOwner::new(
                child_application.application_owner(),
                profile,
                instance,
            ))
        {
            return QString::from("");
        }
        let Some(window) = application.clean_branch_source_for_child(child_application, child_tab)
        else {
            return QString::from("");
        };
        window
    } else {
        let (Some(window), Some((profile, instance))) =
            (parent.window, parent.native_versioning_profile)
        else {
            return QString::from("");
        };
        if application
            .windows()
            .get(&window)
            .is_none_or(|state| state.profile != profile)
            || application.versioning().owner(profile)
                != Some(NativeProfileOwner::new(
                    application.application_owner(),
                    profile,
                    instance,
                ))
        {
            return QString::from("");
        }
        window
    };
    match application.resolve_clean_branch_switch(source_window, reference.as_str()) {
        Ok(CleanBranchSwitchTarget::Parent { window, .. }) => {
            QString::from(format!("parent\t{window}"))
        }
        Ok(CleanBranchSwitchTarget::Child { branch, .. }) => {
            QString::from(format!("child\t{branch}"))
        }
        Err(_) => QString::from(""),
    }
}

impl qobject::BrowserUi {
    /// # Safety
    /// QML supplies a live, distinct child QObject for this synchronous
    /// preview. No pointer or manifest escapes into the QML surface.
    pub(crate) unsafe fn preview_versioned_clean_branch_export(
        mut self: Pin<&mut Self>,
        child: *mut Self,
        branch_id: &QString,
        command: &QString,
    ) -> QString {
        if !cfg!(feature = "unqualified-clean-branch-test")
            || child.is_null()
            || std::ptr::eq(self.as_ref().get_ref(), child.cast_const())
        {
            return QString::from("");
        }
        let Some(branch) = canonical_branch(branch_id) else {
            return QString::from("");
        };
        let Ok(parsed) = parse_chain(&command.to_string(), ParseInput::Interactive) else {
            return QString::from("");
        };
        let [parsed] = parsed.as_slice() else {
            return QString::from("");
        };
        let Ok(VersioningCommand::Commit {
            message,
            scope: VersioningScope::Window,
            pages: false,
            editable: false,
        }) = VersioningCommand::from_parsed(parsed, 1)
        else {
            return QString::from("");
        };
        // SAFETY: QML owns this generated QObject until the synchronous call
        // returns; the null/self cases were refused and no reference escapes.
        let Some(child_object) = (unsafe { child.as_ref() }) else {
            return QString::from("");
        };
        let (
            Some(destination_window),
            Some(child_window),
            Some((parent_profile, parent_instance)),
            Some((child_profile, child_instance)),
        ) = (
            self.as_ref().rust().window,
            child_object.rust().window,
            self.as_ref().rust().native_versioning_profile,
            child_object.rust().native_versioning_profile,
        )
        else {
            return QString::from("");
        };
        let Some(child_application) = child_object.rust().state.as_ref() else {
            return QString::from("");
        };
        if child_application.versioning().owner(child_profile)
            != Some(NativeProfileOwner::new(
                child_application.application_owner(),
                child_profile,
                child_instance,
            ))
            || self
                .as_ref()
                .rust()
                .state
                .as_ref()
                .is_none_or(|application| {
                    application.versioning().owner(parent_profile)
                        != Some(NativeProfileOwner::new(
                            application.application_owner(),
                            parent_profile,
                            parent_instance,
                        ))
                })
        {
            return QString::from("");
        }
        let mut parent_rust = self.as_mut().rust_mut();
        let Some(parent_application) = parent_rust.as_mut().get_mut().state.as_mut() else {
            return QString::from("");
        };
        let Ok(preview) = parent_application.preview_clean_branch_structural_export(
            branch,
            child_application,
            destination_window,
            child_window,
            message,
        ) else {
            return QString::from("");
        };
        let destination = parent_application
            .profile_snapshot()
            .map_or_else(|| "Normal profile".to_owned(), |profile| profile.name);
        QString::from(
            serde_json::json!({
                "operation": preview.operation.to_string(),
                "destination": destination,
                "pages": preview.page_count,
            })
            .to_string(),
        )
    }

    /// # Safety
    /// The exact child QObject stays live through this synchronous owner
    /// recheck; the one-use preview is consumed before storage submission.
    pub(crate) unsafe fn confirm_versioned_clean_branch_export(
        mut self: Pin<&mut Self>,
        child: *mut Self,
        operation_id: &QString,
    ) -> bool {
        if !cfg!(feature = "unqualified-clean-branch-test")
            || child.is_null()
            || std::ptr::eq(self.as_ref().get_ref(), child.cast_const())
        {
            return false;
        }
        let Some(operation) = canonical_operation(operation_id) else {
            return false;
        };
        // SAFETY: QML retains the child QObject until this call returns.
        let Some(child_object) = (unsafe { child.as_ref() }) else {
            return false;
        };
        let (
            Some(destination_window),
            Some(child_window),
            Some((parent_profile, parent_instance)),
            Some((child_profile, child_instance)),
            Some(child_application),
        ) = (
            self.as_ref().rust().window,
            child_object.rust().window,
            self.as_ref().rust().native_versioning_profile,
            child_object.rust().native_versioning_profile,
            child_object.rust().state.as_ref(),
        )
        else {
            return false;
        };
        if child_application.versioning().owner(child_profile)
            != Some(NativeProfileOwner::new(
                child_application.application_owner(),
                child_profile,
                child_instance,
            ))
            || self
                .as_ref()
                .rust()
                .state
                .as_ref()
                .is_none_or(|application| {
                    application.versioning().owner(parent_profile)
                        != Some(NativeProfileOwner::new(
                            application.application_owner(),
                            parent_profile,
                            parent_instance,
                        ))
                })
        {
            return false;
        }
        let previous_revision = self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .map_or(0, ferric_browser_application::BrowserApplication::revision);
        let mut parent_rust = self.as_mut().rust_mut();
        let Some(parent_application) = parent_rust.as_mut().get_mut().state.as_mut() else {
            return false;
        };
        let Ok((_, _, effects)) = parent_application.confirm_clean_branch_structural_export(
            operation,
            child_application,
            destination_window,
            child_window,
        ) else {
            return false;
        };
        drop(parent_rust);
        let qt_effects = self
            .as_mut()
            .accept_runtime_effects(previous_revision, effects);
        self.as_mut().set_pending_engine_action(&qt_effects);
        self.as_mut().set_status_text(QString::from(
            "Branch structural export queued for durable publication",
        ));
        true
    }

    pub(crate) fn cancel_versioned_clean_branch_export(
        mut self: Pin<&mut Self>,
        operation_id: &QString,
    ) -> bool {
        let Some(operation) = canonical_operation(operation_id) else {
            return false;
        };
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .state
            .as_mut()
            .is_some_and(|application| application.cancel_clean_branch_structural_export(operation))
    }

    pub(crate) fn switch_versioned_clean_branch_parent(
        self: Pin<&mut Self>,
        command: &QString,
    ) -> QString {
        resolve_clean_branch_switch(self.as_ref().get_ref(), None, command)
    }

    /// # Safety
    /// The supplied QObject must be alive for the synchronous call. It is
    /// never stored or dereferenced after returning.
    pub(crate) unsafe fn switch_versioned_clean_branch_child(
        self: Pin<&mut Self>,
        child: *mut Self,
        command: &QString,
    ) -> QString {
        if child.is_null() || std::ptr::eq(self.as_ref().get_ref(), child.cast_const()) {
            return QString::from("");
        }
        // SAFETY: QML provides a live generated BrowserUi QObject for this
        // synchronous call; the pointer is checked above and never retained.
        let Some(child) = (unsafe { child.as_ref() }) else {
            return QString::from("");
        };
        resolve_clean_branch_switch(self.as_ref().get_ref(), Some(child), command)
    }

    /// # Safety
    /// The supplied child QObject stays alive throughout this exact owner
    /// and initial-view check. No pointer escapes the call.
    pub(crate) unsafe fn validate_versioned_clean_branch_target(
        self: Pin<&mut Self>,
        child: *mut Self,
        branch_id: &QString,
    ) -> bool {
        if child.is_null() || std::ptr::eq(self.as_ref().get_ref(), child.cast_const()) {
            return false;
        }
        let raw = branch_id.to_string();
        let Ok(branch) = raw.parse::<BranchId>() else {
            return false;
        };
        if branch.to_string() != raw {
            return false;
        }
        // SAFETY: the QML call retains this generated QObject until return.
        let Some(child) = (unsafe { child.as_ref() }) else {
            return false;
        };
        let parent = self.as_ref();
        let parent = parent.rust();
        let child = child.rust();
        let (
            Some(application),
            Some(child_application),
            Some(child_tab),
            Some((profile, instance)),
        ) = (
            parent.state.as_ref(),
            child.state.as_ref(),
            child.tab,
            child.native_versioning_profile,
        )
        else {
            return false;
        };
        application.validate_clean_branch_switch_child(branch, child_application, child_tab)
            && child_application.versioning().owner(profile)
                == Some(NativeProfileOwner::new(
                    child_application.application_owner(),
                    profile,
                    instance,
                ))
    }

    /// Only a published transient child with bound native ownership may
    /// replay the structural fork's safe-GET URL. Return the actual
    /// navigation enqueue result so QML can tear down a failed child.
    pub(crate) fn navigate_clean_branch_safe_get(mut self: Pin<&mut Self>, url: &QString) -> bool {
        let raw = url.to_string();
        if raw.len() > 8192
            || !(raw.starts_with("http://") || raw.starts_with("https://"))
            || SafePageUrl::from_observed(Some(&raw), true).replay_url() != Some(raw.as_str())
        {
            return false;
        }
        let Ok(url) = ValidatedUrl::parse(raw) else {
            return false;
        };
        let child_ref = self.as_ref();
        let child = child_ref.rust();
        let (Some(application), Some(window), Some(tab), Some((profile, instance))) = (
            child.state.as_ref(),
            child.window,
            child.tab,
            child.native_versioning_profile,
        ) else {
            return false;
        };
        if application
            .windows()
            .get(&window)
            .is_none_or(|state| state.profile != profile || state.active_tab != Some(tab))
            || application
                .profiles()
                .get(&profile)
                .is_none_or(|state| state.privacy != PrivacyKind::Ephemeral)
            || application.versioning().owner(profile)
                != Some(NativeProfileOwner::new(
                    application.application_owner(),
                    profile,
                    instance,
                ))
            || application.runtime_view().lineage_parent(tab).is_none()
        {
            return false;
        }
        self.as_mut().navigate_url(&url).is_ok()
    }

    pub(crate) fn abort_versioned_clean_branch(
        mut self: Pin<&mut Self>,
        branch_id: &QString,
    ) -> bool {
        let raw = branch_id.to_string();
        let Ok(branch) = raw.parse::<BranchId>() else {
            return false;
        };
        if branch.to_string() != raw {
            return false;
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .state
            .as_mut()
            .is_some_and(|application| application.abort_ready_clean_branch_fork(branch))
    }

    /// # Safety
    /// QML must pass a live, distinct BrowserUi QObject and keep it alive for
    /// the duration of this synchronous call. CXX-Qt performs the QObject
    /// type conversion; the pointer is never stored beyond this call.
    pub(crate) unsafe fn publish_versioned_clean_branch(
        mut self: Pin<&mut Self>,
        child: *mut Self,
        branch_id: &QString,
    ) -> bool {
        // SAFETY: forwarded unchanged to the common synchronous QObject
        // handoff, which checks null and self-aliasing before dereferencing.
        unsafe {
            self.as_mut()
                .publish_clean_branch_child(child, branch_id, false)
        }
    }

    /// # Safety
    /// QML must supply a live, distinct tab-capable child with an exact
    /// off-record view for each ordered tab before invoking this handoff.
    pub(crate) unsafe fn publish_versioned_clean_branch_window(
        mut self: Pin<&mut Self>,
        child: *mut Self,
        branch_id: &QString,
    ) -> bool {
        // SAFETY: forwarded unchanged to the common synchronous QObject
        // handoff, which checks null and self-aliasing before dereferencing.
        unsafe {
            self.as_mut()
                .publish_clean_branch_child(child, branch_id, true)
        }
    }

    /// # Safety
    /// `child` must be a live CXX-Qt BrowserUi QObject for this call only.
    unsafe fn publish_clean_branch_child(
        mut self: Pin<&mut Self>,
        child: *mut Self,
        branch_id: &QString,
        window_scope: bool,
    ) -> bool {
        let parent_pointer = self.as_ref().get_ref() as *const Self;
        if child.is_null() || std::ptr::eq(parent_pointer, child.cast_const()) {
            return false;
        }
        let raw = branch_id.to_string();
        let Ok(branch) = raw.parse::<BranchId>() else {
            return false;
        };
        if branch.to_string() != raw {
            return false;
        }
        // SAFETY: the QML call supplies a live generated BrowserUi QObject;
        // null and self-aliasing were refused above. No reference escapes.
        let Some(child_object) = (unsafe { child.as_mut() }) else {
            return false;
        };
        // SAFETY: CXX-Qt owns the stable QObject allocation for this call.
        let mut child_ui = unsafe { Pin::new_unchecked(child_object) };
        let result = {
            let mut parent_rust = self.as_mut().rust_mut();
            let parent = parent_rust.as_mut().get_mut();
            let mut child_rust = child_ui.as_mut().rust_mut();
            let child = child_rust.as_mut().get_mut();
            let (Some(parent_application), Some(child_application)) =
                (parent.state.as_mut(), child.state.as_mut())
            else {
                return false;
            };
            let Some(child_window) = child.window else {
                return false;
            };
            let Some(window_state) = child_application.windows().get(&child_window) else {
                return false;
            };
            let child_profile = window_state.profile;
            let Some(child_tab) = window_state.active_tab else {
                return false;
            };
            let child_tabs = window_state.tabs.clone();
            let Some((bound_profile, instance)) = child.native_versioning_profile else {
                return false;
            };
            if bound_profile != child_profile
                || child_application.versioning().owner(child_profile)
                    != Some(NativeProfileOwner::new(
                        child_application.application_owner(),
                        child_profile,
                        instance,
                    ))
            {
                return false;
            }
            let parent_revision = parent_application.revision();
            let child_revision = child_application.revision();
            let publication = if window_scope {
                parent_application.publish_clean_branch_window_lineage(
                    branch,
                    child_application,
                    child_profile,
                    &child_tabs,
                )
            } else {
                parent_application.publish_clean_branch_lineage(
                    branch,
                    child_application,
                    child_profile,
                    child_tab,
                )
            };
            publication.map(|(parent_effects, child_effects)| {
                (
                    parent_revision,
                    child_revision,
                    parent_effects,
                    child_effects,
                )
            })
        };
        let Ok((parent_revision, child_revision, parent_effects, child_effects)) = result else {
            return false;
        };
        let parent_qt_effects = self
            .as_mut()
            .accept_runtime_effects(parent_revision, parent_effects);
        let child_qt_effects = child_ui
            .as_mut()
            .accept_runtime_effects(child_revision, child_effects);
        self.as_mut().sync_tab_order_from_core();
        child_ui.as_mut().sync_tab_order_from_core();
        self.as_mut().set_pending_engine_action(&parent_qt_effects);
        child_ui
            .as_mut()
            .set_pending_engine_action(&child_qt_effects);
        true
    }

    /// # Safety
    /// QML must call this while the exact child BrowserUi is still alive,
    /// after its native view-close callback and before profile destruction.
    pub(crate) unsafe fn close_versioned_clean_branch(
        mut self: Pin<&mut Self>,
        child: *mut Self,
        branch_id: &QString,
    ) -> bool {
        let parent_pointer = self.as_ref().get_ref() as *const Self;
        if child.is_null() || std::ptr::eq(parent_pointer, child.cast_const()) {
            return false;
        }
        let raw = branch_id.to_string();
        let Ok(branch) = raw.parse::<BranchId>() else {
            return false;
        };
        if branch.to_string() != raw {
            return false;
        }
        // SAFETY: QML's synchronous destruction callback retains the typed
        // BrowserUi QObject for this call. The reference does not escape.
        let Some(child_object) = (unsafe { child.as_mut() }) else {
            return false;
        };
        // SAFETY: the generated QObject allocation remains stable until the
        // callback returns.
        let child_ui = unsafe { Pin::new_unchecked(child_object) };
        let child_owner = {
            let child_ref = child_ui.as_ref();
            let child = child_ref.rust();
            let Some((profile, instance)) = child.native_versioning_profile else {
                return false;
            };
            let Some(application) = child.state.as_ref() else {
                return false;
            };
            NativeProfileOwner::new(application.application_owner(), profile, instance)
        };
        let (previous_revision, effects) = {
            let mut parent_rust = self.as_mut().rust_mut();
            let parent = parent_rust.as_mut().get_mut();
            let Some(application) = parent.state.as_mut() else {
                return false;
            };
            let previous_revision = application.revision();
            let Ok(effects) = application.close_published_clean_branch(branch, child_owner) else {
                return false;
            };
            (previous_revision, effects)
        };
        let qt_effects = self
            .as_mut()
            .accept_runtime_effects(previous_revision, effects);
        self.as_mut().sync_tab_order_from_core();
        self.as_mut().set_pending_engine_action(&qt_effects);
        true
    }
}

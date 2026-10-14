use super::native_prompt_lease::{
    Completion as PromptCompletion, NativePromptLease, PromptFocusIdentity, PromptOwner,
    focus_matches, owner_matches, prompt_category, restorable_mode, suspension_matches,
};
use super::{CxxQtType, Mode, Pin, QString, TabId, qobject};

impl qobject::BrowserUi {
    // QML's Qt object has no createUuid() API. Generate a stable identity once
    // per view using the application's native UUID implementation.
    #[allow(clippy::unused_self)] // Qt invokables require a QObject receiver.
    pub(super) fn new_prompt_view_identity(self: Pin<&mut Self>) -> QString {
        QString::from(super::Uuid::new_v4().to_string())
    }

    pub(super) fn note_native_prompt_configuration_change(
        mut self: Pin<&mut Self>,
        policy_changed: bool,
    ) {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        this.config_revision = this.config_revision.wrapping_add(1);
        if policy_changed {
            this.policy_revision = this.policy_revision.wrapping_add(1);
        }
    }

    fn prompt_owner(&self, index: i32) -> Option<(TabId, PromptOwner)> {
        let rust = self.rust();
        let tab = self.tab_for_index(index)?;
        Some((
            tab,
            PromptOwner {
                window: rust.window.map(|window| window.to_string()),
                tab: tab.to_string(),
                view: rust.prompt_view_identities.get(&tab).cloned(),
                document: rust
                    .page_document_identities
                    .get(tab)
                    .map(ToOwned::to_owned),
            },
        ))
    }

    fn lease_owner_matches(&self, lease: &NativePromptLease) -> bool {
        let rust = self.rust();
        let Some(tab) = rust
            .tab_ids
            .iter()
            .copied()
            .find(|tab| tab.to_string() == lease.owner.tab)
        else {
            return false;
        };
        let active_tab = self
            .tab_for_index(rust.active_tab_index)
            .map(|tab| tab.to_string());
        let current_owner = PromptOwner {
            window: rust.window.map(|window| window.to_string()),
            tab: tab.to_string(),
            view: rust.prompt_view_identities.get(&tab).cloned(),
            document: rust
                .page_document_identities
                .get(tab)
                .map(ToOwned::to_owned),
        };
        owner_matches(&lease.owner, &current_owner)
            && rust.tab == Some(tab)
            && active_tab.as_deref() == Some(lease.owner.tab.as_str())
    }

    fn lease_epoch_and_config_match(&self, lease: &NativePromptLease) -> bool {
        let rust = self.rust();
        suspension_matches(
            lease,
            rust.input_epoch,
            rust.config_revision,
            rust.policy_revision,
            rust.core_mode,
        )
    }

    fn prompt_focus_identity(&self, index: i32) -> Option<PromptFocusIdentity> {
        let rust = self.rust();
        let tab = self.tab_for_index(index)?;
        let url = rust
            .state
            .as_ref()?
            .tabs()
            .get(&tab)?
            .url
            .as_deref()
            .unwrap_or("about:blank");
        let mut candidates =
            rust.focus_observations
                .iter()
                .filter(|((candidate, _), observation)| {
                    *candidate == index
                        && observation.epoch == rust.input_epoch
                        && observation.url == url
                        && observation.editable
                });
        let ((_, frame_path), observation) = candidates.next()?;
        if candidates.next().is_some() {
            return None;
        }
        Some(PromptFocusIdentity {
            frame_path: frame_path.clone(),
            context: observation.context,
        })
    }

    fn force_prompt_normal(mut self: Pin<&mut Self>) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut().get_mut().native_prompt_transition = true;
        }
        if self.as_ref().rust().core_mode == Mode::Normal {
            self.as_mut().set_core_mode(Mode::Normal);
        } else {
            self.as_mut().escape();
        }
        {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut().get_mut().native_prompt_transition = false;
        }
        self.as_mut().set_mode(QString::from("normal"));
    }

    fn restore_prompt_mode(mut self: Pin<&mut Self>, lease: &NativePromptLease) -> bool {
        let Some(mode) = lease.saved_mode else {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .native_prompt_leases
                .set_last_result("normal-mode-retained");
            return false;
        };
        let owner_matches = self.as_ref().lease_owner_matches(lease);
        let epoch_matches = self.as_ref().lease_epoch_and_config_match(lease);
        if !owner_matches || !epoch_matches {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .native_prompt_leases
                .set_last_result(if owner_matches {
                    "epoch-or-config-changed"
                } else {
                    "owner-changed"
                });
            return false;
        }
        if matches!(mode, Mode::Insert | Mode::InsertLocked) {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .native_prompt_leases
                .set_last_result("focus-rejected");
            return false;
        }
        self.as_mut()
            .set_mode(QString::from(super::ipc_mode_name(mode)));
        self.as_mut().set_core_mode(mode);
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .native_prompt_leases
            .set_last_result("pass-through-restored");
        self.set_status_text(QString::from("Pass Through restored after native prompt"));
        true
    }

    pub(super) fn register_prompt_view_identity(
        mut self: Pin<&mut Self>,
        index: i32,
        view_identity: &QString,
    ) {
        let identity = view_identity.to_string();
        if identity.is_empty() || identity.len() > 128 || identity.chars().any(char::is_control) {
            return;
        }
        if let Some(tab) = self.as_ref().get_ref().tab_for_index(index) {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .prompt_view_identities
                .insert(tab, identity);
        }
    }

    pub(super) fn unregister_prompt_view_identity(
        mut self: Pin<&mut Self>,
        index: i32,
        view_identity: &QString,
    ) {
        let Some(tab) = self.as_ref().get_ref().tab_for_index(index) else {
            return;
        };
        let identity = view_identity.to_string();
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        if this.prompt_view_identities.get(&tab) == Some(&identity) {
            this.prompt_view_identities.remove(&tab);
        }
    }

    pub(super) fn begin_native_prompt_lease(
        mut self: Pin<&mut Self>,
        index: i32,
        category: &QString,
    ) -> QString {
        let Some(category) = prompt_category(&category.to_string()) else {
            return QString::default();
        };
        let Some((_, owner)) = self.as_ref().prompt_owner(index) else {
            return QString::default();
        };
        if self.as_ref().rust().native_prompt_leases.is_active()
            && self
                .as_ref()
                .rust()
                .native_prompt_leases
                .top()
                .is_some_and(|current| current.owner != owner)
        {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .native_prompt_leases
                .clear("nested-owner-changed");
            self.as_mut().force_prompt_normal();
        }
        if self.as_ref().rust().native_prompt_leases.depth()
            >= super::native_prompt_lease::MAX_NATIVE_PROMPT_DEPTH
        {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .native_prompt_leases
                .clear("depth-limit");
            self.as_mut().force_prompt_normal();
            return QString::default();
        }

        let nested = self.as_ref().rust().native_prompt_leases.is_active();
        let current_mode = self.as_ref().rust().core_mode;
        let focus_identity = self.as_ref().prompt_focus_identity(index);
        let saved_mode = if nested {
            None
        } else {
            restorable_mode(current_mode, &owner).filter(|mode| {
                !matches!(mode, Mode::Insert | Mode::InsertLocked) || focus_identity.is_some()
            })
        };
        if !nested {
            {
                let mut rust = self.as_mut().rust_mut();
                rust.as_mut().get_mut().native_prompt_transition = true;
            }
            self.as_mut().escape();
            {
                let mut rust = self.as_mut().rust_mut();
                rust.as_mut().get_mut().native_prompt_transition = false;
            }
        }
        let expected_input_epoch = self.as_ref().rust().input_epoch;
        let config_revision = self.as_ref().rust().config_revision;
        let policy_revision = self.as_ref().rust().policy_revision;
        let focus_identity = if matches!(saved_mode, Some(Mode::Insert | Mode::InsertLocked)) {
            focus_identity
        } else {
            None
        };
        let id = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .native_prompt_leases
            .push(
                category,
                owner,
                saved_mode,
                expected_input_epoch,
                config_revision,
                policy_revision,
                focus_identity,
            );
        id.map_or_else(QString::default, |id| QString::from(id.to_string()))
    }

    pub(super) fn complete_native_prompt_lease(
        mut self: Pin<&mut Self>,
        lease_id: &QString,
    ) -> bool {
        let Ok(id) = lease_id.to_string().parse::<u64>() else {
            return false;
        };
        let Some(lease) = self.as_ref().rust().native_prompt_leases.entry(id).cloned() else {
            return false;
        };
        if !self.as_ref().lease_owner_matches(&lease)
            || !self.as_ref().lease_epoch_and_config_match(&lease)
        {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .native_prompt_leases
                .clear("stale-owner-epoch-or-config");
            return false;
        }
        match self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .native_prompt_leases
            .begin_completion(id)
        {
            PromptCompletion::Stale => false,
            PromptCompletion::OutOfOrder => {
                self.as_mut().force_prompt_normal();
                self.set_status_text(QString::from(
                    "Native prompt completed out of order; mode reset to Normal",
                ));
                false
            }
            PromptCompletion::Nested => false,
            PromptCompletion::AwaitFocus => true,
            PromptCompletion::Final => {
                self.as_mut().restore_prompt_mode(&lease);
                false
            }
        }
    }

    pub(super) fn finish_native_prompt_focus_probe(
        mut self: Pin<&mut Self>,
        lease_id: &QString,
    ) -> bool {
        let Ok(id) = lease_id.to_string().parse::<u64>() else {
            return false;
        };
        let Some(lease) = self.as_ref().rust().native_prompt_leases.entry(id).cloned() else {
            return false;
        };
        if !lease.completion_pending
            || !self.as_ref().lease_owner_matches(&lease)
            || !self.as_ref().lease_epoch_and_config_match(&lease)
        {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .native_prompt_leases
                .clear("stale-focus-probe");
            return false;
        }
        let index = self.as_ref().rust().active_tab_index;
        let observed = self.as_ref().prompt_focus_identity(index);
        let editable = observed.as_ref().is_some_and(|focus| {
            self.as_ref()
                .rust()
                .focus_observations
                .get(&(index, focus.frame_path.clone()))
                .is_some_and(|observation| observation.editable)
        });
        let focus_matches =
            focus_matches(lease.focus_identity.as_ref(), observed.as_ref(), editable);
        let Some(lease) = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .native_prompt_leases
            .finish_pending(id)
        else {
            return false;
        };
        if !focus_matches {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .native_prompt_leases
                .set_last_result("focus-changed");
            return false;
        }
        let Some(mode) = lease.saved_mode else {
            return false;
        };
        self.as_mut()
            .set_mode(QString::from(super::ipc_mode_name(mode)));
        self.as_mut().set_core_mode(mode);
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .native_prompt_leases
            .set_last_result("editing-mode-restored");
        self.set_status_text(QString::from(if mode == Mode::InsertLocked {
            "Insert locked mode restored after native prompt"
        } else {
            "Insert mode restored after native prompt"
        }));
        true
    }

    pub(super) fn has_native_prompt_lease(self: Pin<&mut Self>) -> bool {
        self.as_ref().rust().native_prompt_leases.is_active()
    }

    pub(super) fn invalidate_native_prompt_leases(mut self: Pin<&mut Self>) {
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .native_prompt_leases
            .clear("owner-lifecycle-changed");
    }
}

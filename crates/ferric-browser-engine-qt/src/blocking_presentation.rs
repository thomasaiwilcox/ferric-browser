//! Network-blocking worker projection and QML-facing control surface.
//!
//! Compilation remains off the UI thread in `network_policy`; this adapter
//! applies a completed snapshot to typed presentation properties and exposes
//! the two user intents that can request or toggle a policy change.

use super::{
    CxxQtType, NetworkPolicyWorker, Pin, QString, QStringList, blocking_site_host, network_policy,
    qobject,
};

#[derive(Clone)]
pub(super) struct NetworkPolicyRequestOwner {
    pub(super) generation: u64,
    pub(super) profile_request: std::sync::Arc<()>,
    pub(super) owner: ferric_browser_application::ConfigurationRequestCapture,
    pub(super) reload: bool,
}

impl NetworkPolicyRequestOwner {
    pub(super) fn profile_is_current(&self, rust: &crate::BrowserUiRust) -> bool {
        std::sync::Arc::ptr_eq(
            &self.profile_request,
            &rust.configuration_io.profile_request,
        )
    }

    pub(super) fn stale_bootstrap(&self, rust: &crate::BrowserUiRust) -> bool {
        !self.reload
            && self.profile_is_current(rust)
            && rust.view_alive
            && !rust.ipc_shutdown_gate
            && rust.profile_bootstrap_pending
            && rust.configuration_io.network_policy_request.is_none()
            && rust.state.as_ref().is_some_and(|state| {
                state.configuration_request_owner_is_current(&self.owner)
                    && !state.configuration_request_is_current(&self.owner)
            })
    }
}

impl qobject::BrowserUi {
    pub(super) fn apply_network_policy_snapshot(
        mut self: Pin<&mut Self>,
        policy: network_policy::PolicySnapshot,
        request: &NetworkPolicyRequestOwner,
    ) -> bool {
        let current = |rust: &crate::BrowserUiRust| {
            request.profile_is_current(rust)
                && rust
                    .state
                    .as_ref()
                    .is_some_and(|state| state.configuration_request_is_current(&request.owner))
                && rust
                    .configuration_io
                    .network_policy_request
                    .as_ref()
                    .is_some_and(|pending| pending.generation == request.generation)
        };
        macro_rules! publish {
            ($expression:expr) => {{
                if !current(self.as_ref().rust()) {
                    return false;
                }
                $expression;
                if !current(self.as_ref().rust()) {
                    return false;
                }
            }};
        }
        {
            let mut rust = self.as_mut().rust_mut();
            if !current(&rust) {
                return false;
            }
            let this = rust.as_mut().get_mut();
            if this.credential_policy_epoch != 0 {
                this.credential_policy_epoch =
                    this.credential_policy_epoch.checked_add(1).unwrap_or(0);
            }
        }
        let valid = policy.compile_failures.is_empty();
        if valid {
            publish!(self.as_mut().set_blocking_policy_ready(false));
            publish!(
                self.as_mut()
                    .set_blocking_hosts(policy.blocked_hosts.iter().map(QString::from).collect())
            );
            publish!(self.as_mut().set_blocking_exceptions(
                policy.exception_hosts.iter().map(QString::from).collect(),
            ));
            let blocked_rule_hosts = policy
                .blocked_rule_lists
                .keys()
                .take(1024)
                .map(QString::from)
                .collect::<QStringList>();
            let blocked_rule_list_ids = policy
                .blocked_rule_lists
                .values()
                .take(1024)
                .map(QString::from)
                .collect::<QStringList>();
            publish!(self.as_mut().set_blocking_rule_hosts(blocked_rule_hosts));
            publish!(
                self.as_mut()
                    .set_blocking_rule_list_ids(blocked_rule_list_ids)
            );
            let exception_rule_hosts = policy
                .exception_rule_lists
                .keys()
                .take(1024)
                .map(QString::from)
                .collect::<QStringList>();
            let exception_rule_list_ids = policy
                .exception_rule_lists
                .values()
                .take(1024)
                .map(QString::from)
                .collect::<QStringList>();
            publish!(
                self.as_mut()
                    .set_blocking_exception_rule_hosts(exception_rule_hosts)
            );
            publish!(
                self.as_mut()
                    .set_blocking_exception_rule_list_ids(exception_rule_list_ids)
            );
            publish!(self.as_mut().set_blocking_loaded_lists(QString::from(
                serde_json::to_string(&policy.loaded_lists).unwrap_or_else(|_| "[]".into()),
            )));
            publish!(self.as_mut().set_blocking_list_metadata(QString::from(
                serde_json::to_string(&policy.list_metadata).unwrap_or_else(|_| "[]".into()),
            )));
            publish!(
                self.as_mut().set_blocking_cosmetic_rule_hosts(
                    policy
                        .cosmetic_rules
                        .iter()
                        .map(|rule| QString::from(&rule.host_pattern))
                        .collect(),
                )
            );
            publish!(
                self.as_mut().set_blocking_cosmetic_rule_selectors(
                    policy
                        .cosmetic_rules
                        .iter()
                        .map(|rule| QString::from(&rule.selector))
                        .collect(),
                )
            );
            publish!(
                self.as_mut().set_blocking_cosmetic_exception_hosts(
                    policy
                        .cosmetic_exceptions
                        .iter()
                        .map(|rule| QString::from(&rule.host_pattern))
                        .collect(),
                )
            );
            publish!(
                self.as_mut().set_blocking_cosmetic_exception_selectors(
                    policy
                        .cosmetic_exceptions
                        .iter()
                        .map(|rule| QString::from(&rule.selector))
                        .collect(),
                )
            );
            publish!(self.as_mut().set_blocking_adblock_source_ids(QString::from(
                serde_json::to_string(&policy.adblock_source_ids).unwrap_or_else(|_| "[]".into()),
            )));
            let handle = policy
                .adblock_engine
                .map_or(0, |engine| engine.into_raw() as u64);
            publish!(self.as_mut().set_blocking_adblock_handle(handle));
            publish!(
                self.as_mut().set_blocking_security_deny_hosts(
                    policy
                        .security_deny_hosts
                        .iter()
                        .map(QString::from)
                        .collect(),
                )
            );
        } else {
            publish!(self.as_mut().set_status_text(QString::from(format!(
                "Blocklist compilation failed; current rules retained: {}",
                policy.compile_failures.join(", ")
            ))));
        }
        publish!(self.as_mut().set_blocking_skipped_lists(QString::from(
            serde_json::to_string(&policy.skipped_lists).unwrap_or_else(|_| "[]".into()),
        )));
        if valid {
            publish!(self.as_mut().set_blocking_bypass_sites(
                policy.bypass_sites.iter().map(QString::from).collect()
            ));
            publish!(self.as_mut().set_blocking_policy_ready(true));
        }
        valid
    }

    pub(super) fn request_network_policy(
        mut self: Pin<&mut Self>,
        reload: bool,
    ) -> Result<u64, String> {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        if this
            .configuration_io
            .published_profile_request
            .as_ref()
            .is_none_or(|published| {
                !std::sync::Arc::ptr_eq(published, &this.configuration_io.profile_request)
            })
        {
            return Err("The current profile request has not been installed yet".into());
        }
        let state = this
            .state
            .as_ref()
            .ok_or("application state is unavailable")?;
        let owner = state
            .capture_configuration_request()
            .ok_or("configuration owner is unavailable")?;
        let io = &mut this.configuration_io;
        let generation = io
            .network_policy_generation
            .checked_add(1)
            .ok_or("network policy generation exhausted")?;
        // A replaced profile/configuration cannot install its old result. Retire
        // its worker without waiting for filesystem I/O on the UI thread.
        if io.network_policy_request.as_ref().is_some_and(|request| {
            !std::sync::Arc::ptr_eq(&request.profile_request, &io.profile_request)
                || !state.configuration_request_is_current(&request.owner)
        }) {
            io.network_policy_worker = None;
        }
        if io.network_policy_worker.is_none() {
            io.network_policy_worker = Some(NetworkPolicyWorker::spawn()?);
        }
        io.network_policy_worker
            .as_mut()
            .ok_or("Network policy service is unavailable")?
            .request(generation, this.policy_roots.clone(), this.config.clone())?;
        io.network_policy_generation = generation;
        io.network_policy_request = Some(NetworkPolicyRequestOwner {
            profile_request: std::sync::Arc::clone(&io.profile_request),
            generation,
            owner,
            reload,
        });
        Ok(generation)
    }

    pub(super) fn reload_blocking_policy(mut self: Pin<&mut Self>) -> bool {
        if self.as_mut().request_network_policy(true).is_err() {
            self.as_mut().set_status_text(QString::from(
                "Couldn't reload blocking rules. Existing rules were kept; try again.",
            ));
            return false;
        }
        self.as_mut().runtime_work_available();
        self.as_mut()
            .set_status_text(QString::from("Blocklist policy reload requested"));
        true
    }

    pub(super) fn toggle_blocking_site(mut self: Pin<&mut Self>) -> bool {
        let Some(host) = blocking_site_host(&self.as_ref().rust().current_url.to_string()) else {
            self.set_status_text(QString::from(
                "Site blocker bypass requires an HTTP(S) document",
            ));
            return false;
        };
        let mut sites = self
            .as_ref()
            .rust()
            .blocking_bypass_sites
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        if let Some(index) = sites.iter().position(|site| site == &host) {
            sites.remove(index);
            self.as_mut()
                .set_blocking_bypass_sites(sites.iter().map(QString::from).collect());
            self.set_status_text(QString::from(format!(
                "Site blocker bypass disabled for {host}"
            )));
        } else {
            if sites.len() >= 64 {
                self.set_status_text(QString::from(
                    "Site blocker bypass limit reached (64 sites)",
                ));
                return false;
            }
            sites.push(host.clone());
            self.as_mut()
                .set_blocking_bypass_sites(sites.iter().map(QString::from).collect());
            self.set_status_text(QString::from(format!(
                "Site blocker bypass enabled for {host}"
            )));
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_same_owner_stale_bootstrap_can_request_refresh() {
        let mut rust = crate::BrowserUiRust::default();
        rust.profile_bootstrap_pending = true;
        let state = rust.state.as_mut().unwrap();
        let owner = state.capture_configuration_request().unwrap();
        let mut request = NetworkPolicyRequestOwner {
            profile_request: std::sync::Arc::clone(&rust.configuration_io.profile_request),
            generation: 1,
            owner,
            reload: false,
        };
        assert!(!request.stale_bootstrap(&rust));
        let state = rust.state.as_mut().unwrap();
        let mut layers = state.configuration_layers().clone();
        layers
            .temporary
            .set_literal("content.zoom", "1.25")
            .unwrap();
        state.update_configuration(layers).unwrap();
        assert!(request.stale_bootstrap(&rust));
        request.reload = true;
        assert!(!request.stale_bootstrap(&rust));
        request.reload = false;
        rust.configuration_io.network_policy_request = Some(NetworkPolicyRequestOwner {
            profile_request: std::sync::Arc::clone(&rust.configuration_io.profile_request),
            generation: 2,
            owner: rust
                .state
                .as_ref()
                .unwrap()
                .capture_configuration_request()
                .unwrap(),
            reload: false,
        });
        assert!(!request.stale_bootstrap(&rust));
        rust.configuration_io.network_policy_request = None;
        let original = std::sync::Arc::clone(&rust.configuration_io.profile_request);
        rust.configuration_io.profile_request = std::sync::Arc::new(());
        assert!(!request.profile_is_current(&rust));
        assert!(!request.stale_bootstrap(&rust));
        rust.configuration_io.profile_request = original;
        assert!(request.profile_is_current(&rust));
        assert!(request.stale_bootstrap(&rust));
        rust.state = Some(crate::BrowserApplication::new(crate::Config::default()));
        assert!(!request.stale_bootstrap(&rust));
    }
}

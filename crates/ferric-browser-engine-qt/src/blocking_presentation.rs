//! Network-blocking worker projection and QML-facing control surface.
//!
//! Compilation remains off the UI thread in `network_policy`; this adapter
//! applies a completed snapshot to typed presentation properties and exposes
//! the two user intents that can request or toggle a policy change.

use super::{
    CxxQtType, NetworkPolicyWorker, Pin, QString, QStringList, blocking_site_host, network_policy,
    qobject,
};

impl qobject::BrowserUi {
    pub(super) fn apply_network_policy_snapshot(
        mut self: Pin<&mut Self>,
        policy: network_policy::PolicySnapshot,
    ) {
        if policy.compile_failures.is_empty() {
            self.as_mut()
                .set_blocking_hosts(policy.blocked_hosts.iter().map(QString::from).collect());
            self.as_mut().set_blocking_exceptions(
                policy.exception_hosts.iter().map(QString::from).collect(),
            );
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
            self.as_mut().set_blocking_rule_hosts(blocked_rule_hosts);
            self.as_mut()
                .set_blocking_rule_list_ids(blocked_rule_list_ids);
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
            self.as_mut()
                .set_blocking_exception_rule_hosts(exception_rule_hosts);
            self.as_mut()
                .set_blocking_exception_rule_list_ids(exception_rule_list_ids);
            self.as_mut().set_blocking_loaded_lists(QString::from(
                serde_json::to_string(&policy.loaded_lists).unwrap_or_else(|_| "[]".into()),
            ));
            self.as_mut().set_blocking_list_metadata(QString::from(
                serde_json::to_string(&policy.list_metadata).unwrap_or_else(|_| "[]".into()),
            ));
            self.as_mut().set_blocking_cosmetic_rule_hosts(
                policy
                    .cosmetic_rules
                    .iter()
                    .map(|rule| QString::from(&rule.host_pattern))
                    .collect(),
            );
            self.as_mut().set_blocking_cosmetic_rule_selectors(
                policy
                    .cosmetic_rules
                    .iter()
                    .map(|rule| QString::from(&rule.selector))
                    .collect(),
            );
            self.as_mut().set_blocking_cosmetic_exception_hosts(
                policy
                    .cosmetic_exceptions
                    .iter()
                    .map(|rule| QString::from(&rule.host_pattern))
                    .collect(),
            );
            self.as_mut().set_blocking_cosmetic_exception_selectors(
                policy
                    .cosmetic_exceptions
                    .iter()
                    .map(|rule| QString::from(&rule.selector))
                    .collect(),
            );
            self.as_mut().set_blocking_adblock_source_ids(QString::from(
                serde_json::to_string(&policy.adblock_source_ids).unwrap_or_else(|_| "[]".into()),
            ));
            let handle = policy
                .adblock_engine
                .map_or(0, |engine| engine.into_raw() as u64);
            self.as_mut().set_blocking_adblock_handle(handle);
            self.as_mut().set_blocking_security_deny_hosts(
                policy
                    .security_deny_hosts
                    .iter()
                    .map(QString::from)
                    .collect(),
            );
        } else {
            self.as_mut().set_status_text(QString::from(format!(
                "Blocklist compilation failed; current rules retained: {}",
                policy.compile_failures.join(", ")
            )));
        }
        self.as_mut().set_blocking_skipped_lists(QString::from(
            serde_json::to_string(&policy.skipped_lists).unwrap_or_else(|_| "[]".into()),
        ));
        self.as_mut()
            .set_blocking_bypass_sites(policy.bypass_sites.iter().map(QString::from).collect());
    }

    pub(super) fn reload_blocking_policy(mut self: Pin<&mut Self>) -> bool {
        let roots = self.as_ref().rust().userscript_roots.clone();
        let config = self.as_ref().rust().config.clone();
        let request = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if this.network_policy_worker.is_none() {
                this.network_policy_worker = NetworkPolicyWorker::spawn().ok();
            }
            this.network_policy_worker
                .as_mut()
                .map(|worker| worker.request(roots, config))
        };
        if !matches!(request, Some(Ok(()))) {
            self.as_mut()
                .set_status_text(QString::from("Blocklist policy worker unavailable"));
            return false;
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .network_policy_reload_pending = true;
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

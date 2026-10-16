//! Startup presentation and bounded local causes. Export only typed categories;
//! error strings can contain configuration paths or other private values.
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug)]
pub(super) enum FailureStage {
    Runtime,
    ProfileOpen,
}

impl FailureStage {
    const fn category(self) -> &'static str {
        match self {
            Self::Runtime => "runtime-start",
            Self::ProfileOpen => "profile-open",
        }
    }
}

#[derive(Default)]
pub(super) struct ProfileStartup {
    active: bool,
    failure: Option<(FailureStage, String)>,
    // Fixed slots prevent an unbounded error history. A new successful profile
    // replaces its predecessor's issues; a failed replacement preserves them.
    issues: [Option<String>; 3],
    network_error: Option<String>,
}

const CATEGORIES: [&str; 3] = ["profile-storage", "configuration", "contexts"];
const LABELS: [&str; 3] = ["Profile storage", "Settings", "Browsing contexts"];

fn local_detail(value: &str) -> String {
    let mut characters = value.chars();
    let limited = characters.by_ref().take(2048).collect::<String>();
    let shortened = characters.next().is_some();
    // Preserve parser line numbers and caret alignment in the deliberate
    // details view. Other controls and directional overrides remain inert.
    let mut detail = limited
        .split('\n')
        .map(|line| {
            crate::presentation_text::sanitize_display_text(
                &line.trim_end_matches('\r').replace('\t', "    "),
                false,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    if shortened {
        detail.push_str("\n[Details shortened]");
    }
    detail
}

impl ProfileStartup {
    pub fn begin(&mut self) {
        self.failure = None;
    }

    pub fn fail(&mut self, stage: FailureStage, error: &str) {
        self.failure = Some((stage, local_detail(error)));
    }

    pub fn activate(
        &mut self,
        storage: Option<&str>,
        settings: Option<&str>,
        contexts: Option<&str>,
    ) {
        self.active = true;
        self.failure = None;
        self.issues = [storage, settings, contexts].map(|error| error.map(local_detail));
        self.network_error = None;
    }

    pub fn network_result(&mut self, error: Option<&str>) {
        self.network_error = error.map(local_detail);
    }

    pub fn settings_recovered(&mut self) {
        self.issues[1] = None;
    }

    pub fn settings_failed(&mut self, error: &str) {
        self.issues[1] = Some(local_detail(error));
    }

    pub fn ready_message(&self, private: bool) -> String {
        let ready = if private {
            "Private profile ready"
        } else {
            "Profile ready"
        };
        if self.issues[0].is_some() {
            format!(
                "{ready} — history and saved data are unavailable. Open :diagnostics for details."
            )
        } else if self.issues[1..].iter().any(Option::is_some) {
            format!("{ready} — some settings couldn't be loaded. Open :diagnostics for details.")
        } else {
            ready.into()
        }
    }

    pub fn notice(&self) -> &'static str {
        if self.failure.is_some() {
            if self.active {
                "The requested profile couldn't be opened. Your current profile is still active."
            } else {
                "The profile couldn't be opened."
            }
        } else if self.issues[0].is_some() {
            "Profile storage is unavailable. History and changes to bookmarks or sessions won't be saved."
        } else if self.issues[1].is_some() {
            "Some settings couldn't be loaded."
        } else if self.issues[2].is_some() {
            "Some browsing contexts couldn't be loaded."
        } else if self.network_error.is_some() {
            "Network filtering couldn't be prepared."
        } else {
            ""
        }
    }

    pub fn local_details(&self) -> String {
        let mut rows = Vec::new();
        if let Some((stage, detail)) = &self.failure {
            rows.push(format!(
                "Latest profile attempt ({})\n{detail}",
                stage.category()
            ));
        }
        for (label, error) in LABELS.iter().zip(&self.issues) {
            if let Some(detail) = error {
                rows.push(format!("Active profile — {label}\n{detail}"));
            }
        }
        if let Some(detail) = &self.network_error {
            rows.push(format!("Active profile — Network filtering\n{detail}"));
        }
        rows.join("\n\n")
    }

    pub fn snapshot(&self, pending: bool) -> Value {
        let mut issues = CATEGORIES
            .iter()
            .zip(&self.issues)
            .filter_map(|(category, error)| error.as_ref().map(|_| *category))
            .collect::<Vec<_>>();
        if self.network_error.is_some() {
            issues.push("network-policy");
        }
        json!({
            "latest_attempt": {
                "status": if pending { "pending" } else if self.failure.is_some() { "failed" }
                    else if self.active { "ready" } else { "not-started" },
                "failure_category": self.failure.as_ref().map(|(stage, _)| stage.category())
            },
            "active_profile": {
                "status": if !self.active { "unavailable" } else if issues.is_empty() { "ready" } else { "degraded" },
                "issues": issues
            },
            "provenance": "runtime-memory",
            "privacy": "typed categories only; local error details are excluded"
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_replacement_keeps_active_profile_problems_and_retry_clears_only_attempt() {
        let mut state = ProfileStartup::default();
        state.activate(Some("/private/profile"), Some("secret setting"), None);
        state.fail(FailureStage::ProfileOpen, "replacement token=secret");
        assert_eq!(state.snapshot(false)["latest_attempt"]["status"], "failed");
        assert_eq!(
            state.snapshot(false)["active_profile"]["status"],
            "degraded"
        );
        state.begin();
        assert_eq!(state.snapshot(true)["latest_attempt"]["status"], "pending");
        assert!(state.local_details().contains("/private/profile"));
        assert!(!state.local_details().contains("replacement"));
        state.activate(None, None, None);
        assert!(state.local_details().is_empty());
        assert_eq!(state.snapshot(false)["active_profile"]["status"], "ready");
    }

    #[test]
    fn export_categories_come_from_typed_sources_without_error_prose() {
        let mut state = ProfileStartup::default();
        state.activate(
            Some("runtime-start: /secret/path"),
            Some("token=secret"),
            Some("https://secret.test"),
        );
        state.network_result(Some("profile-open"));
        state.fail(FailureStage::Runtime, "password=secret");
        let snapshot = state.snapshot(false);
        assert_eq!(
            snapshot["active_profile"]["issues"],
            json!([
                "profile-storage",
                "configuration",
                "contexts",
                "network-policy"
            ])
        );
        assert_eq!(
            snapshot["latest_attempt"]["failure_category"],
            "runtime-start"
        );
        assert!(!snapshot.to_string().contains("secret"));
        let status = state.ready_message(false);
        assert!(!status.contains("secret"));
        assert!(status.contains("history and saved data"));
        assert!(state.local_details().contains("https://secret.test"));
    }

    #[test]
    fn local_causes_are_bounded_and_controls_are_neutralized() {
        let mut state = ProfileStartup::default();
        state.fail(
            FailureStage::ProfileOpen,
            &("\u{202e}\0".to_owned() + &"界".repeat(100_000)),
        );
        let details = state.local_details();
        assert!(details.len() < 8192);
        assert!(!details.contains('\0') && !details.contains('\u{202e}'));
        assert!(details.contains("[bidi]"));
        assert!(details.contains("[Details shortened]"));
        assert!(!local_detail(&"a".repeat(2048)).contains("[Details shortened]"));
    }

    #[test]
    fn persistent_notice_tracks_current_issues_and_never_exposes_causes() {
        let mut state = ProfileStartup::default();
        assert!(state.notice().is_empty());
        state.fail(FailureStage::Runtime, "/secret/runtime");
        assert_eq!(state.notice(), "The profile couldn't be opened.");
        state.activate(None, Some("/secret/config"), None);
        assert_eq!(state.notice(), "Some settings couldn't be loaded.");
        state.fail(FailureStage::ProfileOpen, "token=private");
        assert!(state.notice().contains("current profile is still active"));
        assert!(!state.notice().contains("private"));
        state.begin();
        assert_eq!(state.notice(), "Some settings couldn't be loaded.");
        state.settings_recovered();
        assert!(state.notice().is_empty());
        state.network_result(Some("worker stopped"));
        assert_eq!(state.notice(), "Network filtering couldn't be prepared.");
        state.network_result(None);
        assert!(state.notice().is_empty());
    }

    #[test]
    fn parser_lines_and_indentation_remain_readable_without_terminal_controls() {
        let cause = "config.toml: invalid setting\r\n1 | ui = [\n\t|      ^\n\u{1b}[31m\u{202e}";
        let detail = local_detail(cause);
        assert!(detail.starts_with("config.toml: invalid setting\n1 | ui = [\n    |      ^\n"));
        assert!(
            !detail.contains('\r') && !detail.contains('\u{1b}') && !detail.contains('\u{202e}')
        );
        assert!(detail.contains("[bidi]"));
    }

    #[test]
    fn failed_reload_retains_working_profile_and_keeps_private_cause_local_until_recovery() {
        let mut state = ProfileStartup::default();
        state.activate(None, None, None);
        state.settings_failed("/private/config.toml\nsecret setting");
        assert_eq!(state.snapshot(false)["latest_attempt"]["status"], "ready");
        assert_eq!(
            state.snapshot(false)["active_profile"]["status"],
            "degraded"
        );
        assert!(
            state
                .local_details()
                .contains("/private/config.toml\nsecret setting")
        );
        assert!(!state.snapshot(false).to_string().contains("private"));
        assert!(!state.notice().contains("private"));
        state.settings_recovered();
        assert!(state.local_details().is_empty());
        assert!(state.notice().is_empty());
        assert_eq!(state.snapshot(false)["active_profile"]["status"], "ready");
    }

    #[test]
    fn private_settings_failures_are_visible_and_network_recovery_drops_old_cause() {
        let mut state = ProfileStartup::default();
        assert_eq!(
            state.snapshot(false)["latest_attempt"]["status"],
            "not-started"
        );
        state.activate(None, Some("invalid settings"), None);
        assert!(
            state
                .ready_message(true)
                .contains("some settings couldn't be loaded")
        );
        state.network_result(Some("worker stopped"));
        state.network_result(None);
        assert!(!state.local_details().contains("worker stopped"));
        assert_eq!(
            state.snapshot(false)["active_profile"]["issues"],
            json!(["configuration"])
        );
    }
}

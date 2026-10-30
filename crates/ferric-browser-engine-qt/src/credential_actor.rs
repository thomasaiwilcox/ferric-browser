//! One private native actor. C++ owns exactly one instance on the GUI thread.
//! Neither its methods nor its lease have a QML/WebChannel representation.

use ferric_browser_application::{
    CeremonyToken, CredentialCeremonyState, CredentialOperation, CredentialRequestOptions,
    HelperCompletion, TransitionOutcome,
};
use serde_json::Value;

use crate::bridge::qobject::NativeCredentialUpdate;
use crate::browser_ui_credential_binding::CredentialLeaseCapture;
use crate::credential_protocol::{self as protocol, HelperFrame, Json, Operation};

pub(crate) struct CredentialActor {
    state: CredentialCeremonyState,
    active: Option<Active>,
}

struct Active {
    token: CeremonyToken,
    operation: Operation,
    options: Value,
    credentials: Vec<Value>,
    accounts: Vec<String>,
    error: Option<&'static str>,
    qr_seen: bool,
    submitted_seen: bool,
    completed_seen: bool,
}

pub(crate) fn new_credential_actor() -> Box<CredentialActor> {
    Box::new(CredentialActor {
        state: CredentialCeremonyState::for_native_integration(),
        active: None,
    })
}

impl CredentialActor {
    pub(crate) fn admit(
        &mut self,
        lease: &CredentialLeaseCapture,
        is_create: bool,
        options: &[u8],
        now: u64,
    ) -> u64 {
        if self.active.is_some() || options.len() > protocol::MAX_TERMINAL_FRAME {
            return 0;
        }
        let Some(lease) = lease.0.as_ref() else {
            return 0;
        };
        // The strict value rejects duplicate members; serde's depth bound is
        // supplemented by the encoder's smaller explicit protocol depth bound.
        let Ok(Json::Object(fields)) = serde_json::from_slice::<Json>(options) else {
            return 0;
        };
        let Ok(options) = serde_json::to_value(Json::Object(fields)) else {
            return 0;
        };
        let operation = if is_create {
            Operation::Create
        } else {
            Operation::Get
        };
        let rp = if is_create {
            options.pointer("/rp/id")
        } else {
            options.get("rpId")
        };
        if rp.is_some_and(|v| !v.is_string()) {
            return 0;
        }
        let Ok(request) = CredentialRequestOptions::new(
            if is_create {
                CredentialOperation::Create
            } else {
                CredentialOperation::Get
            },
            rp.and_then(Value::as_str),
        ) else {
            return 0;
        };
        // Exercise the bounded encoder before reserving a ceremony. This
        // envelope is never submitted; direct permission is checked later.
        if protocol::encode_request(&protocol::HelperRequest {
            v: 1,
            id: "pending",
            op: operation,
            origin: lease.origin().origin(),
            options: &options,
            network: protocol::NetworkPolicy::default(),
        })
        .is_err()
        {
            return 0;
        }
        let timeout = options
            .get("timeout")
            .and_then(Value::as_f64)
            .filter(|n| n.is_finite() && *n >= 0.0)
            .map_or(180_000, |n| n.min(180_000.0) as u64)
            .max(1);
        let Ok(token) = self
            .state
            .admit_document(lease.clone(), request, now, timeout)
        else {
            return 0;
        };
        let owner = self.state.active_owner().expect("admitted owner").clone();
        self.state.begin_validation(token, &owner, now);
        self.state.await_provider(token, &owner, now);
        self.active = Some(Active {
            token,
            operation,
            options,
            credentials: Vec::new(),
            accounts: Vec::new(),
            error: None,
            qr_seen: false,
            submitted_seen: false,
            completed_seen: false,
        });
        token.sequence()
    }

    pub(crate) fn helper_request(&mut self, token: u64, direct: bool, now: u64) -> Vec<u8> {
        let Some(active) = self.active.as_ref().filter(|a| a.token.sequence() == token) else {
            return Vec::new();
        };
        let Some(owner) = self.state.active_owner().cloned() else {
            return Vec::new();
        };
        if !direct
            || self.state.phone_selected(active.token, &owner, now)
                != TransitionOutcome::PhaseAdvanced
        {
            return Vec::new();
        }
        protocol::encode_request(&protocol::HelperRequest {
            v: 1,
            id: &token.to_string(),
            op: active.operation,
            origin: owner.lease().origin().origin(),
            options: &active.options,
            network: protocol::NetworkPolicy::direct(),
        })
        .unwrap_or_default()
    }

    pub(crate) fn helper_started(&mut self, token: u64, now: u64) -> bool {
        let Some(active) = self.active.as_ref().filter(|a| a.token.sequence() == token) else {
            return false;
        };
        let Some(owner) = self.state.active_owner().cloned() else {
            return false;
        };
        self.state.helper_started(active.token, &owner, now) == TransitionOutcome::PhaseAdvanced
    }

    pub(crate) fn frame(&mut self, token: u64, bytes: &[u8], now: u64) -> NativeCredentialUpdate {
        let invalid = || NativeCredentialUpdate {
            kind: 0,
            width: 0,
            modules: Vec::new(),
            payload: String::new(),
        };
        let Some(active) = self.active.as_mut().filter(|a| a.token.sequence() == token) else {
            return invalid();
        };
        let Some(owner) = self.state.active_owner().cloned() else {
            return invalid();
        };
        if self.state.expire(active.token, &owner, now) == TransitionOutcome::TerminalEmitted {
            return invalid();
        }
        if self.state.phase()
            != Some(ferric_browser_application::CredentialCeremonyPhase::AwaitingPhone)
        {
            return invalid();
        }
        let Ok(frame) = protocol::decode_frame(bytes, &token.to_string(), active.operation) else {
            return invalid();
        };
        let mut update = invalid();
        match frame {
            HelperFrame::Qr { width, modules, .. } => {
                if active.qr_seen {
                    return invalid();
                }
                active.qr_seen = true;
                update.kind = 1;
                update.width = width;
                update.modules = modules;
            }
            HelperFrame::Status { status, .. } => {
                // The one-shot helper emits each progress edge once. Reject
                // repetition before a progress flood can monopolize native UI.
                let seen = if status == "submitted" {
                    &mut active.submitted_seen
                } else {
                    &mut active.completed_seen
                };
                if *seen {
                    return invalid();
                }
                *seen = true;
                update.kind = 2;
                update.payload = status;
            }
            HelperFrame::Result { body, .. } => {
                let Ok(body) = serde_json::to_value(body) else {
                    return invalid();
                };
                let credentials = if active.operation == Operation::Get {
                    let Some(assertions) = body.get("assertions").and_then(Value::as_array) else {
                        return invalid();
                    };
                    if assertions.is_empty() || assertions.len() > 16 {
                        return invalid();
                    }
                    assertions.clone()
                } else {
                    vec![body.clone()]
                };
                let mut labels = Vec::new();
                if credentials.len() > 1 {
                    // Get results preserve native-only account metadata beside
                    // assertions. Creation results never have account choices.
                    let Some(accounts) = body.get("accounts").and_then(Value::as_array) else {
                        return invalid();
                    };
                    let Some(account_labels) = accounts
                        .iter()
                        .map(|label| label.as_str().map(str::to_owned))
                        .collect::<Option<Vec<_>>>()
                    else {
                        return invalid();
                    };
                    let distinct: std::collections::HashSet<_> = account_labels
                        .iter()
                        .map(|label| label.to_lowercase())
                        .collect();
                    if account_labels.len() != credentials.len()
                        || distinct.len() != account_labels.len()
                    {
                        return invalid();
                    }
                    labels = account_labels;
                }
                active.credentials = credentials;
                active.accounts = labels;
                if self
                    .state
                    .helper_result(active.token, &owner, HelperCompletion::Success, now)
                    != TransitionOutcome::PhaseAdvanced
                {
                    return invalid();
                }
                update.kind = 3;
                // Native account choice gets a bounded count, never page
                // markup or arbitrary authenticator labels to render as HTML.
                update.width = active.credentials.len() as u16;
            }
            HelperFrame::Error { code, .. } => {
                if self
                    .state
                    .helper_result(active.token, &owner, HelperCompletion::Failure, now)
                    != TransitionOutcome::PhaseAdvanced
                {
                    return invalid();
                }
                update.kind = 4;
                active.error = Some(match code.as_str() {
                    "security_error" | "invalid_origin" => "SecurityError",
                    "invalid_envelope" | "invalid_request" | "request_too_large" => "TypeError",
                    "unsupported_options" => "NotSupportedError",
                    "credential_excluded" if active.operation == Operation::Create => {
                        "InvalidStateError"
                    }
                    _ => "NotAllowedError",
                });
            }
        }
        update
    }

    pub(crate) fn current(&mut self, token: u64, now: u64) -> bool {
        let Some(active) = self.active.as_ref().filter(|a| a.token.sequence() == token) else {
            return false;
        };
        let Some(owner) = self.state.active_owner().cloned() else {
            return false;
        };
        self.state.expire(active.token, &owner, now) != TransitionOutcome::TerminalEmitted
            && self.state.has_active_ceremony()
    }

    pub(crate) fn response(&mut self, token: u64, index: u16, now: u64) -> String {
        if !self.current(token, now)
            || self.state.phase()
                != Some(ferric_browser_application::CredentialCeremonyPhase::Completing)
        {
            return String::new();
        }
        if let Some(error) = self.active.as_ref().and_then(|a| a.error) {
            return serde_json::json!({"error": error}).to_string();
        }
        self.active
            .as_ref()
            .filter(|a| a.token.sequence() == token)
            .and_then(|a| a.credentials.get(usize::from(index)))
            .and_then(|credential| {
                serde_json::to_string(&serde_json::json!({"credential": credential})).ok()
            })
            .unwrap_or_default()
    }

    pub(crate) fn account_label(&self, token: u64, index: u16) -> String {
        self.active
            .as_ref()
            .filter(|a| a.token.sequence() == token)
            .and_then(|a| a.accounts.get(usize::from(index)))
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn finish(&mut self, token: u64, now: u64, delivered: bool) {
        let Some(active) = self.active.as_ref().filter(|a| a.token.sequence() == token) else {
            return;
        };
        if let Some(owner) = self.state.active_owner().cloned() {
            if !delivered
                || self.state.finish_completion(active.token, &owner, now)
                    != TransitionOutcome::TerminalEmitted
            {
                self.state.abort(active.token, &owner, now);
            }
        }
        self.active = None;
        while self.state.drain_one().is_some() {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferric_browser_application::{
        BrowserApplication, EngineCredentialFacts, ProfileOpenRequest, ProfileStorage,
    };
    use ferric_browser_config::{Config, ContextsConfig, RuntimeOverrides};
    use ferric_browser_core::PrivacyKind;

    fn lease() -> CredentialLeaseCapture {
        let config = Config::default();
        let (mut app, _, _) =
            BrowserApplication::bootstrap(config.clone(), PrivacyKind::Ephemeral, "actor-fixture")
                .unwrap();
        app.request_profile_open(ProfileOpenRequest {
            name: "actor-fixture".into(),
            label: "Actor fixture".into(),
            privacy: PrivacyKind::Ephemeral,
            storage: ProfileStorage::Transient,
            base_configuration: config,
            initial_profile_overrides: RuntimeOverrides::default(),
            command_line_overrides: RuntimeOverrides::default(),
            config_path: None,
            script_catalog_root: None,
            contexts: ContextsConfig::default(),
            context_configuration_error: None,
        })
        .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if let Some(result) = app.poll_profile_open() {
                result.unwrap();
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let tab = app
            .windows()
            .values()
            .find_map(|window| window.active_tab)
            .unwrap();
        CredentialLeaseCapture(Some(
            app.capture_credential_document(
                tab,
                &EngineCredentialFacts {
                    security_origin: "https://login.example.com".into(),
                    profile_generation: 1,
                    page_generation: 2,
                    document_generation: 3,
                    host_generation: 4,
                    presentation_generation: 5,
                    main_frame: true,
                    secure_context: true,
                    permissions_policy_allowed: true,
                    foreground: true,
                },
            )
            .unwrap(),
        ))
    }

    fn credential(id: &str) -> Value {
        serde_json::json!({"id":id,"rawId":id,"type":"public-key","response":{},"clientExtensionResults":{}})
    }

    fn result(token: u64, credentials: Vec<Value>, accounts: Value) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"v":1,"id":token.to_string(),"kind":"result", "body":{"assertions":credentials,"accounts":accounts}})).unwrap()
    }

    #[test]
    fn native_actor_preserves_lease_origin_and_requires_provider_start_and_exact_token() {
        let mut actor = new_credential_actor();
        assert_eq!(
            actor.admit(&CredentialLeaseCapture(None), false, br"{}", 0),
            0
        );
        let lease = lease();
        assert_eq!(
            actor.admit(&lease, false, br#"{"rpId":"a","rpId":"b"}"#, 0),
            0
        );
        let token = actor.admit(
            &lease,
            false,
            br#"{"challenge":"AQ","origin":"https://evil.test"}"#,
            0,
        );
        assert_ne!(token, 0);
        assert_eq!(actor.admit(&lease, true, br"{}", 1), 0);
        assert!(actor.helper_request(token + 1, true, 1).is_empty());
        assert!(actor.helper_request(token, false, 1).is_empty());
        assert!(!actor.helper_started(token, 1));
        let request: Value = serde_json::from_slice(&actor.helper_request(token, true, 1)).unwrap();
        assert_eq!(request["origin"], "https://login.example.com");
        assert_eq!(request["network"]["mode"], "direct");
        assert!(actor.helper_started(token, 2));
        assert_eq!(
            actor
                .frame(
                    token + 1,
                    &result(token, vec![credential("AQ")], serde_json::json!([null])),
                    3
                )
                .kind,
            0
        );
        assert!(actor.current(token, 3));
        assert_eq!(
            actor
                .frame(
                    token,
                    &result(token, vec![credential("AQ")], serde_json::json!([null])),
                    3
                )
                .kind,
            3
        );
        let response: Value = serde_json::from_str(&actor.response(token, 0, 4)).unwrap();
        assert_eq!(response["credential"]["id"], "AQ");
        assert!(response["credential"].get("accounts").is_none());
        assert!(actor.response(token, 1, 4).is_empty());
        actor.finish(token + 1, 4, false);
        assert!(actor.current(token, 4));
        actor.finish(token, 4, true);
        assert!(!actor.current(token, 4));
        assert_ne!(actor.admit(&lease, false, br"{}", 5), token);
    }

    #[test]
    fn native_actor_rejects_late_helper_exit_or_account_choice() {
        let mut actor = new_credential_actor();
        let token = actor.admit(&lease(), false, br#"{"timeout":10}"#, 1);
        assert!(!actor.helper_request(token, true, 2).is_empty());
        assert!(actor.helper_started(token, 3));
        let response = result(
            token,
            vec![credential("AQ"), credential("Ag")],
            serde_json::json!(["Alice", "Bob"]),
        );
        assert_eq!(actor.frame(token, &response, 4).kind, 3);
        assert_eq!(actor.account_label(token, 1), "Bob");
        assert!(actor.response(token, 1, 11).is_empty());
        assert!(!actor.current(token, 11));
        actor.finish(token, 12, false);
        assert_eq!(actor.state.queued_terminal_count(), 0);
    }

    #[test]
    fn native_actor_declines_ambiguous_or_unbounded_account_metadata() {
        for labels in [
            serde_json::json!([null, null]),
            serde_json::json!(["Alice", "alice"]),
            serde_json::json!(["Alice", "a\u{202e}b"]),
            serde_json::json!(["Alice"]),
        ] {
            let mut actor = new_credential_actor();
            let token = actor.admit(&lease(), false, br"{}", 0);
            actor.helper_request(token, true, 1);
            assert!(actor.helper_started(token, 2));
            assert_eq!(
                actor
                    .frame(
                        token,
                        &result(token, vec![credential("AQ"), credential("Ag")], labels),
                        3
                    )
                    .kind,
                0
            );
            assert!(actor.response(token, 0, 3).is_empty());
        }
    }

    #[test]
    fn native_actor_delivers_only_allowlisted_helper_errors() {
        for (code, implemented, expected) in [
            ("security_error", false, "SecurityError"),
            ("invalid_request", false, "TypeError"),
            ("unsupported_options", false, "NotSupportedError"),
            ("authenticator_failed", true, "NotAllowedError"),
        ] {
            let mut actor = new_credential_actor();
            let token = actor.admit(&lease(), false, br"{}", 0);
            actor.helper_request(token, true, 1);
            assert!(actor.helper_started(token, 2));
            let mut body = serde_json::json!({"code":code});
            if !implemented {
                body["implemented"] = Value::Bool(false);
            }
            let frame = serde_json::to_vec(
                &serde_json::json!({"v":1,"id":token.to_string(),"kind":"error","body":body}),
            )
            .unwrap();
            assert_eq!(actor.frame(token, &frame, 3).kind, 4);
            let response: Value = serde_json::from_str(&actor.response(token, 0, 4)).unwrap();
            assert_eq!(response, serde_json::json!({"error":expected}));
            actor.finish(token, 4, true);
            assert!(!actor.current(token, 4));
        }
    }

    #[test]
    fn native_actor_accepts_final_exclusion_only_for_creation() {
        for is_create in [false, true] {
            let mut actor = new_credential_actor();
            let token = actor.admit(&lease(), is_create, br"{}", 0);
            actor.helper_request(token, true, 1);
            assert!(actor.helper_started(token, 2));
            let frame = serde_json::to_vec(&serde_json::json!({"v":1,"id":token.to_string(),"kind":"error","body":{"code":"credential_excluded"}})).unwrap();
            assert_eq!(
                actor.frame(token, &frame, 3).kind,
                if is_create { 4 } else { 0 }
            );
            let response = actor.response(token, 0, 4);
            if is_create {
                assert_eq!(
                    serde_json::from_str::<Value>(&response).unwrap(),
                    serde_json::json!({"error":"InvalidStateError"})
                );
            } else {
                assert!(response.is_empty());
            }
        }
    }
}

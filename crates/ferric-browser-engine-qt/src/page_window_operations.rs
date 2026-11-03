//! Process-lived, scope-preserving receipts for native page handoffs. No `QObject`
//! or profile resource is retained here; native profile drain starts a short
//! grace for minimal transient receipts, then forgets them.
use crate::{BrowserUiRust, QString, WindowId};
use ferric_browser_application::{
    NativeOperationOutcome, NativeOperationRegistry, NativeOperationReporter, NativeOperationScope,
    ProfileInstanceId,
};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

struct Metadata {
    scope: NativeOperationScope,
    private_allowed: bool,
}

#[derive(Default)]
struct Store {
    registry: NativeOperationRegistry,
    metadata: HashMap<String, Metadata>,
    expiry: HashMap<ProfileInstanceId, Instant>,
}

fn store() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(Default::default)
}

type RuntimeWake = Arc<dyn Fn() + Send + Sync>;

fn waker() -> &'static Mutex<Option<RuntimeWake>> {
    static WAKE: OnceLock<Mutex<Option<RuntimeWake>>> = OnceLock::new();
    WAKE.get_or_init(Default::default)
}

pub(super) fn install_waker(wake: RuntimeWake) {
    if let Ok(mut store) = store().lock() {
        store.registry.install_waker(Arc::clone(&wake));
    }
    if let Ok(mut installed) = waker().lock() {
        *installed = Some(Arc::clone(&wake));
    }
    wake();
}

fn wake_controller() {
    let wake = waker().lock().ok().and_then(|installed| installed.clone());
    if let Some(wake) = wake {
        wake();
    }
}

pub(super) fn completions_pending() -> bool {
    store()
        .lock()
        .is_ok_and(|store| store.registry.completions_pending())
}

pub(super) fn maintenance_delay_ms() -> Option<i32> {
    let store = store().lock().ok()?;
    store.delay(Instant::now())
}

impl Store {
    fn delay(&self, now: Instant) -> Option<i32> {
        if self.registry.completions_pending() {
            return Some(1);
        }
        self.expiry.values().min().map(|deadline| {
            i32::try_from(
                deadline
                    .saturating_duration_since(now)
                    .as_millis()
                    .saturating_add(1),
            )
            .unwrap_or(i32::MAX)
        })
    }

    fn prune(&mut self, now: Instant) {
        for (profile, _) in self.expiry.iter().filter(|(_, expiry)| now >= **expiry) {
            self.registry.forget_transient_profile(*profile);
        }
        let receipts = self.registry.records().collect::<Vec<_>>();
        let ids = receipts
            .iter()
            .map(|receipt| receipt.record.id.as_str())
            .collect::<HashSet<_>>();
        let profiles = receipts
            .iter()
            .map(|receipt| receipt.scope.profile())
            .collect::<HashSet<_>>();
        self.metadata.retain(|id, _| ids.contains(id.as_str()));
        self.expiry.retain(|profile, _| profiles.contains(profile));
    }
}

pub(super) fn begin(rust: &mut BrowserUiRust, id: &str) -> Result<NativeOperationReporter, String> {
    store()
        .lock()
        .map_err(|_| "The operation registry is unavailable")?
        .begin(rust, id)
}

pub(super) fn update_policy(rust: &BrowserUiRust) {
    if let Ok(mut store) = store().lock() {
        store.update_policy(rust);
    }
}

pub(super) fn contains(id: &str) -> bool {
    store().lock().is_ok_and(|mut store| {
        store.prune(Instant::now());
        store.registry.get(id).is_some()
    })
}

pub(super) fn available(window: Option<WindowId>) -> bool {
    store().lock().is_ok_and(|mut store| {
        store.prune(Instant::now());
        store
            .registry
            .records()
            .any(|receipt| window.is_none_or(|window| receipt.scope.window() == window))
    })
}

/// `window` is resolved by the actual owner router, never rebound to a retired
/// operation's observing Ui. Aggregate callers exclude owners already queried.
pub(super) fn rows(
    params: &Value,
    window: Option<WindowId>,
    exclude: &[WindowId],
) -> Result<Vec<Value>, String> {
    store()
        .lock()
        .map_err(|_| "The operation registry is unavailable")?
        .rows(params, window, exclude)
}

pub(super) fn cancel(id: &str) -> Result<(), crate::PublicError> {
    store()
        .lock()
        .map_err(|_| crate::PublicError::engine("Operation registry unavailable"))?
        .cancel(id)
}

pub(super) fn publish(rust: &mut BrowserUiRust) {
    update_policy(rust);
    crate::page_window_focus::expire_reporter(rust);
    let completions = if let Ok(mut store) = store().lock() {
        store.prune(Instant::now());
        store.registry.drain_completions()
    } else {
        Vec::new()
    };
    for scoped in completions {
        // The shared event protocol has no per-subscriber private opt-in.
        // Transient receipts are available only through explicit scoped queries.
        if scoped.scope.privacy().is_transient() {
            continue;
        }
        let completion = scoped.completion;
        crate::publish_ipc_event(
            &mut rust.ipc_sequence,
            "operation.completed",
            json!({
                "window_id": scoped.scope.window().to_string(), "operation_id": completion.id,
                "status": completion.state.as_str(), "operation_sequence": completion.sequence,
                "error_code": completion.error.map(|kind| ferric_browser_ipc::ErrorCode::from(kind).as_str()),
            }),
        );
    }
}

impl crate::qobject::BrowserUi {
    /// Only the application-owned actual profile lifetime calls this after its
    /// native drained signal. It removes metadata, never grants a native effect.
    #[allow(
        clippy::unused_self,
        reason = "QML signal sink requires a QObject receiver; only metadata is retired"
    )]
    pub(super) fn native_operation_profile_drained(
        self: std::pin::Pin<&mut Self>,
        profile: &QString,
    ) {
        let profile = profile.to_string();
        if let Ok(mut store) = store().lock() {
            store.retire_profile(&profile, Instant::now());
        }
        wake_controller();
    }
}

pub(super) fn focus_outcome(value: &str) -> Option<NativeOperationOutcome> {
    Some(match value {
        "activated" => NativeOperationOutcome::Activated,
        "unknown" => NativeOperationOutcome::ActivationUnknown,
        "stale" => NativeOperationOutcome::Stale,
        "failed" => NativeOperationOutcome::Failed,
        _ => return None,
    })
}

impl Store {
    fn begin(
        &mut self,
        rust: &mut BrowserUiRust,
        id: &str,
    ) -> Result<NativeOperationReporter, String> {
        let private_allowed = crate::browser_ui_query_permissions::private_queries_enabled(rust);
        if rust.operation(id).is_none() {
            rust.admit_operation(id.to_owned())
                .map_err(|error| error.to_string())?;
        }
        let window = rust.window.ok_or("The operation window is unavailable")?;
        let application = rust
            .state
            .as_mut()
            .ok_or("The operation owner is unavailable")?;
        let scope = NativeOperationScope::capture(application, window)
            .ok_or("The operation profile is unavailable")?;
        self.prune(Instant::now());
        let reporter = self
            .registry
            .begin_handoff(application.operations_mut(), scope, id)
            .map_err(|kind| format!("native operation admission rejected: {kind:?}"))?;
        self.metadata.insert(
            id.to_owned(),
            Metadata {
                scope,
                private_allowed,
            },
        );
        Ok(reporter)
    }
}

impl Store {
    fn rows(
        &mut self,
        params: &Value,
        window: Option<WindowId>,
        exclude: &[WindowId],
    ) -> Result<Vec<Value>, String> {
        let query = crate::operation_projection::operation_query(params)?;
        self.prune(Instant::now());
        let receipts = self.registry.records().collect::<Vec<_>>();
        let mut rows = Vec::new();
        for receipt in receipts {
            if window.is_some_and(|window| receipt.scope.window() != window)
                || exclude.contains(&receipt.scope.window())
                || query.requested.is_some_and(|id| id != receipt.record.id)
            {
                continue;
            }
            let Some(metadata) = self.metadata.get(&receipt.record.id) else {
                continue;
            };
            if receipt.scope.privacy().is_transient()
                && query.include_private
                && !metadata.private_allowed
            {
                return Err(
                    "Private operation queries are disabled by the original owner's configuration."
                        .into(),
                );
            }
            if receipt.scope.privacy().is_transient() && !query.include_private {
                continue;
            }
            let mut value = crate::browser_ui_operations::operation_value(
                &receipt.record,
                query.include_stderr,
            );
            value["window_id"] = json!(receipt.scope.window().to_string());
            rows.push(value);
        }
        Ok(rows)
    }
}

impl Store {
    fn retire_profile(&mut self, profile: &str, now: Instant) {
        let profiles = self
            .metadata
            .values()
            .filter(|metadata| {
                metadata.scope.privacy().is_transient()
                    && metadata.scope.profile().to_string() == profile
            })
            .map(|metadata| metadata.scope.profile())
            .collect::<HashSet<_>>();
        for profile in profiles {
            self.registry.retire_transient_profile(profile);
            self.expiry
                .entry(profile)
                .or_insert_with(|| now + Duration::from_secs(30));
        }
    }
}

impl Store {
    fn update_policy(&mut self, rust: &BrowserUiRust) {
        let Some(profile) = rust
            .state
            .as_ref()
            .and_then(crate::BrowserApplication::profile_instance_id)
        else {
            return;
        };
        let Some(window) = rust.window else { return };
        let allowed = crate::browser_ui_query_permissions::private_queries_enabled(rust);
        for metadata in self.metadata.values_mut().filter(|metadata| {
            metadata.scope.profile() == profile && metadata.scope.window() == window
        }) {
            metadata.private_allowed = allowed;
        }
    }
}

impl Store {
    fn cancel(&mut self, id: &str) -> Result<(), crate::PublicError> {
        self.prune(Instant::now());
        let receipt = self.registry.get(id).ok_or_else(|| {
            crate::PublicError::new(
                ferric_browser_ipc::ErrorCode::NotFound,
                "The operation was not found.",
                "native operation expired",
            )
        })?;
        if receipt.scope.privacy().is_transient()
            && self
                .metadata
                .get(id)
                .is_none_or(|metadata| !metadata.private_allowed)
        {
            return Err(crate::PublicError::new(
                ferric_browser_ipc::ErrorCode::Denied,
                "Private operation controls are disabled by the original owner's configuration.",
                "native operation privacy",
            ));
        }
        Err(crate::PublicError::new(
            if receipt.record.state.is_terminal() {
                ferric_browser_ipc::ErrorCode::InvalidArgument
            } else {
                ferric_browser_ipc::ErrorCode::Unsupported
            },
            "The native operation can no longer be cancelled.",
            "native publication boundary",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BrowserApplication, PrivacyKind};
    use ferric_browser_application::{ProfileOpenRequest, ProfileStorage};

    struct Source {
        rust: BrowserUiRust,
        root: Option<std::path::PathBuf>,
    }
    impl std::ops::Deref for Source {
        type Target = BrowserUiRust;
        fn deref(&self) -> &Self::Target {
            &self.rust
        }
    }
    impl std::ops::DerefMut for Source {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.rust
        }
    }
    impl Drop for Source {
        fn drop(&mut self) {
            self.rust.state.take();
            if let Some(root) = &self.root {
                std::fs::remove_dir_all(root).unwrap();
            }
        }
    }
    fn source(privacy: PrivacyKind, private_queries: bool) -> Source {
        let configuration = ferric_browser_config::Config::default();
        let (mut application, window, _) =
            BrowserApplication::bootstrap(configuration.clone(), privacy, "receipt source")
                .unwrap();
        let root = (privacy == PrivacyKind::Normal).then(|| {
            std::env::temp_dir().join(format!("ferric-native-receipt-{}", crate::Uuid::new_v4()))
        });
        application
            .request_profile_open(ProfileOpenRequest {
                name: "receipt-source".into(),
                label: "Receipt source".into(),
                privacy,
                storage: root.clone().map_or(ProfileStorage::Transient, |root| {
                    ProfileStorage::Durable(ferric_browser_storage::RootSpec::Base(root))
                }),
                base_configuration: configuration,
                initial_profile_overrides: ferric_browser_config::RuntimeOverrides::default(),
                command_line_overrides: ferric_browser_config::RuntimeOverrides::default(),
                config_path: None,
                script_catalog_root: None,
                contexts: ferric_browser_config::ContextsConfig::default(),
                context_configuration_error: None,
            })
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(result) = application.poll_profile_open() {
                result.unwrap();
                break;
            }
            assert!(Instant::now() < deadline, "profile open timeout");
            std::thread::yield_now();
        }
        let mut rust = BrowserUiRust::default();
        rust.state = Some(application);
        rust.window = Some(window);
        rust.config = json!({"ipc":{"private_queries":private_queries}});
        Source { rust, root }
    }

    #[test]
    fn retired_owner_query_keeps_original_identity_and_aggregate_membership() {
        let mut source = source(PrivacyKind::Normal, false);
        let window = source.window.unwrap();
        let mut store = Store::default();
        let reporter = store.begin(&mut source, "native-original").unwrap();
        let rows = store.rows(&json!({}), Some(window), &[]).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["window_id"], window.to_string());
        assert_eq!(
            store.rows(&json!({}), None, &[window]).unwrap(),
            Vec::<serde_json::Value>::new()
        );
        drop(source);
        assert!(reporter.finish(NativeOperationOutcome::Applied));
        let rows = store
            .rows(&json!({"operation_id":"native-original"}), None, &[])
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["window_id"], window.to_string());
        assert_eq!(rows[0]["status"], "completed");
    }

    #[test]
    fn private_policy_belongs_to_original_owner_and_cannot_be_inferred_from_observer() {
        let mut source = source(PrivacyKind::Private, false);
        let mut store = Store::default();
        let reporter = store.begin(&mut source, "native-private").unwrap();
        assert_eq!(
            store.rows(&json!({}), None, &[]).unwrap(),
            Vec::<serde_json::Value>::new()
        );
        assert!(
            store
                .rows(&json!({"include_private":true}), None, &[])
                .is_err()
        );
        drop(source);
        assert!(reporter.finish(NativeOperationOutcome::Applied));
        assert_eq!(
            store.rows(&json!({}), None, &[]).unwrap(),
            Vec::<serde_json::Value>::new()
        );
        assert!(
            store
                .rows(&json!({"include_private":true}), None, &[])
                .is_err()
        );
    }

    #[test]
    fn actual_transient_drain_has_a_fixed_nonextendable_delivery_grace() {
        let mut source = source(PrivacyKind::Private, true);
        let profile = source
            .state
            .as_ref()
            .unwrap()
            .profile_instance_id()
            .unwrap();
        let mut store = Store::default();
        let reporter = store.begin(&mut source, "native-private").unwrap();
        let start = Instant::now();
        store.retire_profile(&profile.to_string(), start);
        assert!(!reporter.finish(NativeOperationOutcome::Applied));
        store.prune(start + Duration::from_secs(29));
        let rows = store
            .rows(&json!({"include_private":true}), None, &[])
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["status"], "failed");
        store.retire_profile(&profile.to_string(), start + Duration::from_secs(20));
        store.prune(start + Duration::from_secs(30));
        assert!(store.registry.get("native-private").is_none());
        assert!(store.metadata.is_empty() && store.expiry.is_empty());
    }

    #[test]
    fn transient_drain_cannot_expire_normal_or_unrelated_profiles() {
        let mut normal = source(PrivacyKind::Normal, false);
        let mut private = source(PrivacyKind::Private, true);
        let mut other = source(PrivacyKind::Private, true);
        let profile = private
            .state
            .as_ref()
            .unwrap()
            .profile_instance_id()
            .unwrap();
        let mut store = Store::default();
        let normal_reporter = store.begin(&mut normal, "normal").unwrap();
        let _private_reporter = store.begin(&mut private, "private").unwrap();
        let other_reporter = store.begin(&mut other, "other").unwrap();
        let start = Instant::now();
        store.retire_profile(&profile.to_string(), start);
        store.prune(start + Duration::from_secs(30));
        assert_eq!(store.registry.records().count(), 2);
        assert!(normal_reporter.finish(NativeOperationOutcome::Applied));
        assert!(other_reporter.finish(NativeOperationOutcome::Applied));
    }

    #[test]
    fn immediate_configuration_revocation_fences_direct_id_query_and_cancel() {
        let mut source = source(PrivacyKind::Private, true);
        let mut store = Store::default();
        let reporter = store.begin(&mut source, "private-revoke").unwrap();
        let params = json!({"operation_id":"private-revoke","include_private":true});
        assert_eq!(store.rows(&params, None, &[]).unwrap().len(), 1);
        // Exactly the configuration commit fence; no publish/maintenance tick.
        source.config = json!({"ipc":{"private_queries":false}});
        store.update_policy(&source);
        assert!(store.rows(&params, None, &[]).is_err());
        assert!(
            store
                .cancel("private-revoke")
                .unwrap_err()
                .to_string()
                .contains("disabled")
        );
        assert!(reporter.finish(NativeOperationOutcome::Applied));
        for source in [
            include_str!("browser_ui_configuration.rs"),
            include_str!("browser_ui_profile_setup.rs"),
        ] {
            let commit = source.find("this.config = ").unwrap();
            assert!(source[commit..].starts_with("this.config = active_config.clone();\n            crate::page_window_operations::update_policy(this);")
                || source[commit..].starts_with("this.config = config;\n            crate::page_window_operations::update_policy(this);"));
        }
    }

    #[test]
    fn process_deadline_remains_scheduled_after_source_retirement_without_queries() {
        let mut source = source(PrivacyKind::Private, true);
        let profile = source
            .state
            .as_ref()
            .unwrap()
            .profile_instance_id()
            .unwrap();
        let mut store = Store::default();
        let _reporter = store.begin(&mut source, "private-expire").unwrap();
        let start = Instant::now();
        store.retire_profile(&profile.to_string(), start);
        drop(source);
        assert_eq!(store.delay(start), Some(1));
        store.registry.drain_completions();
        assert_eq!(store.delay(start), Some(30_001));
        store.prune(start + Duration::from_secs(30));
        assert!(store.delay(start + Duration::from_secs(30)).is_none());
        assert!(store.metadata.is_empty());
    }
}

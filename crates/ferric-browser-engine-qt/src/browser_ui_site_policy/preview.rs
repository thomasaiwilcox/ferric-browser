//! One observer in the existing configuration I/O owner, not a Hint session.
use std::{collections::BTreeMap, sync::Arc};

use ferric_browser_application::PreparedSitePolicyPreview;
use ferric_browser_config::InteractionFamily;
use ferric_browser_core::{TabId, Target};

use crate::{CxxQtType, OperationUpdate, Pin, QString, Value, qobject};

pub(crate) struct PendingSitePolicyPreview {
    pub(crate) prepared: Arc<PreparedSitePolicyPreview>,
    pub(crate) target: Target,
    pub(crate) last_request: u64,
    pub(crate) results: BTreeMap<InteractionFamily, Value>,
    pub(crate) capture_family: Option<InteractionFamily>,
}

impl PendingSitePolicyPreview {
    pub(crate) fn accepts(&self, family: InteractionFamily) -> bool {
        self.capture_family.map_or_else(
            || InteractionFamily::ALL.get(self.results.len()) == Some(&family),
            |expected| expected == family,
        )
    }
}

impl qobject::BrowserUi {
    pub(super) fn start_site_policy_preview(
        self: Pin<&mut Self>,
        draft: &str,
        tab: Option<&str>,
    ) -> Result<Value, String> {
        self.start_site_policy_inspection(draft, tab, None)
    }

    pub(super) fn start_site_policy_capture(
        self: Pin<&mut Self>,
        draft: &str,
        revision: u64,
        tab: &str,
        family: InteractionFamily,
    ) -> Result<Value, String> {
        let state = self.rust().state.as_ref().ok_or("profile is unavailable")?;
        if state.site_policy_draft(draft)?.revision != revision {
            return Err("draft revision changed before capture".into());
        }
        self.start_site_policy_inspection(draft, Some(tab), Some(family))
    }

    fn start_site_policy_inspection(
        mut self: Pin<&mut Self>,
        draft: &str,
        tab: Option<&str>,
        capture_family: Option<InteractionFamily>,
    ) -> Result<Value, String> {
        let this = self.as_ref();
        let rust = this.rust();
        let state = rust.state.as_ref().ok_or("profile is unavailable")?;
        let view = state.site_policy_draft(draft)?;
        let target = tab
            .map(|tab| {
                let tab = TabId::from_display(tab).ok_or("invalid preview tab")?;
                let page = state.tabs().get(&tab).ok_or("preview tab is unavailable")?;
                if Some(page.window) != rust.window {
                    return Err("preview tab belongs to another window".to_owned());
                }
                state
                    .capture_target(tab)
                    .ok_or_else(|| "preview tab has no document".into())
            })
            .transpose()?;
        let url = target
            .and_then(|target| state.tabs().get(&target.tab))
            .and_then(|tab| tab.url.clone());
        if target.is_some() && url.is_none() {
            return Err("preview tab has no URL".into());
        }
        if let Some(url) = &url {
            ferric_browser_core::canonical_origin(url).ok_or("preview requires an HTTP(S) tab")?;
        }
        if capture_family.is_some()
            && target.is_none_or(|target| {
                usize::try_from(rust.active_tab_index)
                    .ok()
                    .and_then(|index| rust.tab_ids.get(index))
                    != Some(&target.tab)
            })
        {
            return Err("Capture requires the selected tab in this window".into());
        }
        self.as_mut().cancel_site_policy_preview("");
        let this = self.as_ref();
        let state = this.rust().state.as_ref().ok_or("profile is unavailable")?;
        let prepared = Arc::new(if capture_family.is_some() {
            state.prepare_site_policy_capture(
                draft,
                view.revision,
                url.as_deref().ok_or("capture requires a page URL")?,
            )?
        } else {
            state.prepare_site_policy_preview(draft, view.revision, url.as_deref())?
        });
        if capture_family.is_some() && !prepared.receipt().valid {
            return Err(prepared.receipt().message.clone());
        }
        let proxy = prepared
            .snapshot()
            .and_then(|snapshot| snapshot.discovery().proxy_targets().ok())
            .is_some_and(|value| *value == ferric_browser_config::HintProxyTargets::Validated);
        let mut receipt =
            serde_json::to_value(prepared.receipt()).map_err(|error| error.to_string())?;
        if let Some(target) =
            target.filter(|_| prepared.receipt().valid && prepared.snapshot().is_some())
        {
            let id = prepared.receipt().id.clone();
            let operation = format!("op-{id}");
            receipt["status"] = serde_json::json!("pending-live");
            if capture_family.is_none() {
                receipt["operation_id"] = serde_json::json!(operation);
            }
            receipt["tab"] = serde_json::json!(target.tab.to_string());
            let mut rust = self.as_mut().rust_mut();
            let rust = rust.as_mut().get_mut();
            if capture_family.is_none() {
                rust.start_operation(
                    operation,
                    OperationUpdate::running("Site policy preview collecting"),
                )?;
            }
            rust.configuration_io.site_policy_preview = Some(PendingSitePolicyPreview {
                prepared,
                target,
                last_request: 0,
                results: BTreeMap::new(),
                capture_family,
            });
            rust.pending_engine_action = Some(if let Some(family) = capture_family {
                let family = serde_json::to_value(family).map_err(|error| error.to_string())?;
                format!(
                    "site-policy-capture\t{id}\t{}\t{proxy}\t{}",
                    target.tab,
                    family.as_str().ok_or("invalid capture family")?
                )
            } else {
                format!("site-policy-preview\t{id}\t{}\t{proxy}", target.tab)
            });
        }
        Ok(receipt)
    }

    pub(crate) fn record_site_policy_preview(
        mut self: Pin<&mut Self>,
        id: &str,
        family: InteractionFamily,
        result: Value,
    ) -> Result<Value, String> {
        let progress =
            serde_json::json!({"id":id,"family":family,"result":result,"status":"collecting"});
        let (prepared, report, complete, collected) = {
            let mut rust = self.as_mut().rust_mut();
            let rust = rust.as_mut().get_mut();
            let pending = rust
                .configuration_io
                .site_policy_preview
                .as_mut()
                .ok_or("preview is closed")?;
            if pending.capture_family.is_some()
                || pending.prepared.receipt().id != id
                || !pending.accepts(family)
                || !rust
                    .state
                    .as_ref()
                    .is_some_and(|state| pending.prepared.is_current(state))
            {
                return Err("preview owner, revision or invocation changed".into());
            }
            pending.last_request = 0;
            pending.results.insert(family, result);
            let complete = pending.results.len() == InteractionFamily::ALL.len();
            let collected = pending
                .results
                .values()
                .any(|result| result["state"] == "collected")
                && pending.results.values().all(|result| {
                    result["state"] == "collected" || result["state"] == "not-requested"
                });
            let invocations = pending
                .results
                .iter()
                .map(|(family, value)| serde_json::json!({"family":family,"result":value}))
                .collect::<Vec<_>>();
            (
                Arc::clone(&pending.prepared),
                serde_json::json!({"tab":pending.target.tab.to_string(),
                "generation":format!("{:?}",pending.target.generation),"document":format!("{:?}",pending.target.document),
                "invocations":invocations,"complete":complete}),
                complete,
                collected,
            )
        };
        if complete {
            let receipt = self
                .as_ref()
                .rust()
                .state
                .as_ref()
                .ok_or("profile is unavailable")?
                .complete_site_policy_preview(&prepared, report, collected)?;
            let value = serde_json::to_value(receipt).map_err(|error| error.to_string())?;
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .configuration_io
                .site_policy_preview = None;
            self.as_mut().rust_mut().as_mut().get_mut().set_operation(
                format!("op-{id}"),
                OperationUpdate::completed("Site policy preview result ready"),
            );
            if !self
                .as_ref()
                .rust()
                .state
                .as_ref()
                .is_some_and(|state| prepared.is_current(state))
            {
                return Ok(value);
            }
            self.as_mut()
                .set_site_policy_result(QString::from(value.to_string()));
            Ok(value)
        } else {
            Ok(progress)
        }
    }

    pub(crate) fn site_policy_preview_failed(
        mut self: Pin<&mut Self>,
        id: &QString,
        family: &QString,
        reason: &QString,
    ) -> QString {
        let result = (|| {
            let family =
                serde_json::from_value::<InteractionFamily>(serde_json::json!(family.to_string()))
                    .map_err(|error| error.to_string())?;
            let reason = reason.to_string().chars().take(512).collect::<String>();
            self.as_mut().record_site_policy_preview(
                &id.to_string(),
                family,
                serde_json::json!({"state":"unavailable","reason":reason}),
            )
        })();
        QString::from(
            match result {
                Ok(value) => value,
                Err(error) => serde_json::json!({"error":error}),
            }
            .to_string(),
        )
    }

    pub(crate) fn site_policy_preview_cancel(mut self: Pin<&mut Self>, id: &QString) {
        self.as_mut().cancel_site_policy_preview(&id.to_string());
    }

    pub(crate) fn site_policy_preview_pending(&self, id: &QString) -> bool {
        self.rust().view_alive
            && !self.rust().profile_bootstrap_pending
            && self
                .rust()
                .configuration_io
                .site_policy_preview
                .as_ref()
                .is_some_and(|pending| pending.prepared.receipt().id == id.to_string())
    }

    pub(crate) fn cancel_preview_for_draft(mut self: Pin<&mut Self>, draft: &str) {
        let matches = self
            .as_ref()
            .rust()
            .configuration_io
            .site_policy_preview
            .as_ref()
            .is_some_and(|pending| pending.prepared.receipt().draft == draft);
        if matches {
            self.as_mut().cancel_site_policy_preview("");
        }
    }

    fn cancel_site_policy_preview(mut self: Pin<&mut Self>, id: &str) {
        let this = self.as_ref();
        let Some(pending) = this.rust().configuration_io.site_policy_preview.as_ref() else {
            return;
        };
        if !id.is_empty() && pending.prepared.receipt().id != id {
            return;
        }
        let pending = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .configuration_io
            .site_policy_preview
            .take();
        if let (Some(pending), Some(state)) = (pending, self.as_ref().rust().state.as_ref()) {
            if pending.capture_family.is_none() {
                let _ = state.complete_site_policy_preview(
                    &pending.prepared,
                    serde_json::json!({"complete":false,"reason":"preview closed or superseded"}),
                    false,
                );
            }
            let id = pending.prepared.receipt().id.clone();
            let mut rust = self.as_mut().rust_mut();
            let rust = rust.as_mut().get_mut();
            if pending.capture_family.is_none() {
                rust.set_operation(format!("op-{id}"), OperationUpdate::cancelled());
            }
            rust.pending_engine_action = Some(format!("site-policy-preview-close\t{id}"));
        }
    }
}

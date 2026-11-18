//! Immutable, nonpublishing preview inputs. The slot bounds in-flight work;
//! retaining a historical receipt retains neither a slot nor an action permit.
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use ferric_browser_config::InteractionPolicyRevision;
use ferric_browser_core::ValidatedUrl;
use serde::Serialize;
use uuid::Uuid;

use super::{BrowserApplication, parse_id};
use crate::{
    CompiledSitePolicySnapshot, ConfigurationRequestCapture, MAX_SITE_POLICY_PREVIEWS,
    SitePolicyMatchContext,
};

static ACTIVE_PREVIEWS: AtomicUsize = AtomicUsize::new(0);

struct PreviewSlot;
impl PreviewSlot {
    fn acquire() -> Result<Self, String> {
        #[allow(
            deprecated,
            reason = "Atomic::try_update is unavailable on MSRV Rust 1.88"
        )]
        let acquired = ACTIVE_PREVIEWS.fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
            (count < MAX_SITE_POLICY_PREVIEWS).then_some(count + 1)
        });
        acquired.map(|_| Self).map_err(|_| {
            "eight policy previews are already in flight; close a preview first".into()
        })
    }
}
impl Drop for PreviewSlot {
    fn drop(&mut self) {
        ACTIVE_PREVIEWS.fetch_sub(1, Ordering::AcqRel);
    }
}

#[derive(Clone, Serialize)]
pub struct SitePolicyPreviewReceipt {
    pub id: String,
    pub draft: String,
    pub revision: u64,
    pub configuration_revision: u64,
    pub candidate_revision: Option<u64>,
    pub profile_revision: u64,
    pub profile_instance: Option<String>,
    pub window: Option<String>,
    pub url: Option<String>,
    pub kind: &'static str,
    pub valid: bool,
    pub message: String,
    pub activation_permitted: bool,
    pub observations: Option<serde_json::Value>,
}

/// A bounded compiler snapshot, with no writer, publisher or Hint-session API.
/// The native collector must independently bind its view/document identities.
pub struct PreparedSitePolicyPreview {
    receipt: SitePolicyPreviewReceipt,
    snapshot: Option<Arc<CompiledSitePolicySnapshot>>,
    owner: ConfigurationRequestCapture,
    _slot: PreviewSlot,
}
impl PreparedSitePolicyPreview {
    #[must_use]
    pub const fn receipt(&self) -> &SitePolicyPreviewReceipt {
        &self.receipt
    }
    #[must_use]
    pub const fn snapshot(&self) -> Option<&Arc<CompiledSitePolicySnapshot>> {
        self.snapshot.as_ref()
    }
    /// Observation freshness only. It does not establish live DOM freshness.
    #[must_use]
    pub fn is_current(&self, app: &BrowserApplication) -> bool {
        if !app.configuration_request_is_current(&self.owner) {
            return false;
        }
        let Some(profile) = app.active_profile.as_ref() else {
            return false;
        };
        let Ok(id) = parse_id(&self.receipt.draft) else {
            return false;
        };
        profile
            .session
            .0
            .site_policy_drafts
            .lock()
            .is_ok_and(|catalog| {
                catalog
                    .drafts
                    .get(&id)
                    .is_some_and(|draft| draft.check(app, self.receipt.revision, false).is_ok())
            })
    }
}

impl BrowserApplication {
    /// Retrieves the last retained receipt by identity for an authorized owner.
    /// # Errors
    /// Rejects unavailable profile stores. Superseded/discarded receipts are absent.
    pub fn site_policy_preview_receipt(
        &self,
        id: &str,
    ) -> Result<Option<SitePolicyPreviewReceipt>, String> {
        let profile = self
            .active_profile
            .as_ref()
            .ok_or("profile is unavailable")?;
        let catalog = profile
            .session
            .0
            .site_policy_drafts
            .lock()
            .map_err(|_| "draft catalogue is unavailable")?;
        Ok(catalog
            .drafts
            .values()
            .filter(|draft| draft.capture.belongs_to(self))
            .find_map(|draft| {
                draft
                    .view
                    .preview
                    .as_ref()
                    .filter(|receipt| receipt.id == id)
            })
            .cloned())
    }

    /// Validates one immutable draft and optionally compiles its prospective
    /// HTTP(S) policy using the same reload projection and consumer compiler.
    /// Static success establishes no selector-engine or live-target receipt.
    /// # Errors
    /// Rejects unavailable/stale owners, revisions, contexts and preview capacity.
    /// Invalid draft content returns a failed receipt while preserving edits.
    pub fn prepare_site_policy_preview(
        &self,
        id: &str,
        revision: u64,
        url: Option<&str>,
    ) -> Result<PreparedSitePolicyPreview, String> {
        self.prepare_site_policy_inspection(id, revision, url, true)
    }

    /// Captures the same immutable prospective policy for an explicit builder
    /// observation, without replacing the draft's validation/preview receipt.
    /// # Errors
    /// Uses the same owner, revision, compilation and capacity checks as preview.
    pub fn prepare_site_policy_capture(
        &self,
        id: &str,
        revision: u64,
        url: &str,
    ) -> Result<PreparedSitePolicyPreview, String> {
        self.prepare_site_policy_inspection(id, revision, Some(url), false)
    }

    fn prepare_site_policy_inspection(
        &self,
        id: &str,
        revision: u64,
        url: Option<&str>,
        retain_receipt: bool,
    ) -> Result<PreparedSitePolicyPreview, String> {
        let slot = PreviewSlot::acquire()?;
        let draft = self.site_policy_draft(id)?;
        if draft.revision != revision {
            return Err("draft revision changed; review before previewing".into());
        }
        let url: Option<ValidatedUrl> = url
            .map(|url| {
                let url = ValidatedUrl::parse(url).map_err(|error| error.to_string())?;
                ferric_browser_config::SiteMatchContext::for_url(&url)
                    .ok_or_else(|| "preview requires an HTTP(S) URL".to_owned())?;
                Ok::<_, String>(url)
            })
            .transpose()?;
        let owner = self
            .capture_configuration_request()
            .ok_or("profile is unavailable")?;
        let sources = if draft.read_only {
            let current = self
                .capture_interaction_edits()
                .map_err(|error| error.to_string())?;
            if current.configuration_revision() != draft.configuration_revision
                || current.shared_profile_revision() != draft.profile_revision
            {
                return Err("authored draft configuration changed; reopen its exact source before previewing".into());
            }
            let sources = self
                .configuration_inspection_inputs()
                .sources()
                .clone()
                .prospective_reload();
            sources
                .validate_raw_sources()
                .map(|()| sources)
                .map_err(|error| error.to_string())
        } else {
            self.prepare_site_policy_draft(id, revision)
                .map(|prepared| prepared.prepared().policy_sources().prospective_reload())
        };
        let mut receipt = SitePolicyPreviewReceipt {
            id: Uuid::new_v4().to_string(), draft:id.into(), revision,
            configuration_revision:draft.configuration_revision, profile_revision:draft.profile_revision,
            candidate_revision:sources.as_ref().ok().map(crate::ConfigurationPolicySources::revision),
            profile_instance:self.configuration_inspection_inputs().profile_instance().map(|instance|instance.to_string()),
            window:self.active_window().map(|window|window.to_string()),
            url:url.as_ref().map(|url| url.as_str().into()), kind:"static-only", valid:sources.is_ok(),
            message:sources.as_ref().err().cloned().unwrap_or_else(|| "Static validation passed; live roots, CSS engine matching and native key delivery were not tested.".into()),
            activation_permitted:false,
            observations:None,
        };
        let snapshot = match (sources, url) {
            (Ok(sources), Some(url)) => {
                match self.compile_preview_snapshot(sources, &url, revision) {
                    Ok(snapshot) => Some(snapshot),
                    Err(message) => {
                        receipt.valid = false;
                        receipt.message = message;
                        None
                    }
                }
            }
            _ => None,
        };
        let profile = self
            .active_profile
            .as_ref()
            .ok_or("profile is unavailable")?;
        let mut catalog = profile
            .session
            .0
            .site_policy_drafts
            .lock()
            .map_err(|_| "draft catalogue is unavailable")?;
        let entry = catalog
            .drafts
            .get_mut(&parse_id(id)?)
            .ok_or("unknown draft")?;
        entry.check(self, revision, false)?;
        if !self.configuration_request_is_current(&owner) {
            return Err("configuration changed during preview preparation".into());
        }
        if retain_receipt {
            entry.view.preview = Some(receipt.clone());
        }
        Ok(PreparedSitePolicyPreview {
            receipt,
            snapshot,
            owner,
            _slot: slot,
        })
    }
    fn compile_preview_snapshot(
        &self,
        sources: crate::ConfigurationPolicySources,
        url: &ValidatedUrl,
        revision: u64,
    ) -> Result<Arc<CompiledSitePolicySnapshot>, String> {
        let profile = self.profile_snapshot().ok_or("profile is unavailable")?;
        let policy_revision = InteractionPolicyRevision {
            profile: self
                .interaction_core_profile()
                .ok_or("profile identity is unavailable")?,
            profile_revision: profile.generation,
            configuration_revision: sources.revision(),
            policy_revision: revision,
        };
        let snapshot = CompiledSitePolicySnapshot::compile(
            sources,
            &profile.name,
            policy_revision,
            url,
            &SitePolicyMatchContext::for_url(url),
        );
        let diagnostic = snapshot
            .interaction()
            .and_then(|interaction| interaction.diagnostic())
            .map(str::to_owned)
            .or_else(|| snapshot.discovery().selectors().err().map(str::to_owned))
            .or_else(|| snapshot.discovery().proxy_policy().err().map(str::to_owned))
            .or_else(|| {
                snapshot
                    .interaction()
                    .and_then(|interaction| interaction.policy())
                    .and_then(|policy| {
                        ferric_browser_config::InteractionFamily::ALL
                            .into_iter()
                            .find_map(|family| {
                                policy
                                    .compile_hint_codebook(family)
                                    .err()
                                    .map(|error| error.to_string())
                            })
                    })
            });
        if let Some(message) = diagnostic {
            return Err(message);
        }
        Ok(Arc::new(snapshot))
    }

    /// Retains an inert native inspection report for this exact preview owner.
    /// Reports have no activation or configuration publication authority.
    /// # Errors
    /// Rejects stale owners/revisions and reports over the existing 2 MiB envelope.
    pub fn complete_site_policy_preview(
        &self,
        preview: &PreparedSitePolicyPreview,
        observations: serde_json::Value,
        collected: bool,
    ) -> Result<SitePolicyPreviewReceipt, String> {
        if (collected && !preview.is_current(self))
            || !self.configuration_request_owner_is_current(&preview.owner)
        {
            return Err("preview draft or configuration owner changed".into());
        }
        if u64::try_from(
            serde_json::to_vec(&observations)
                .map_err(|error| error.to_string())?
                .len(),
        )
        .unwrap_or(u64::MAX)
            > ferric_browser_config::MAX_CONFIGURATION_BYTES
        {
            return Err("preview report exceeds the 2 MiB envelope".into());
        }
        let mut receipt = preview.receipt.clone();
        receipt.kind = if collected {
            "live"
        } else {
            "live-unavailable"
        };
        receipt.valid &= collected;
        receipt.message = if collected { "Native snapshot collected. No target was activated, focused, or sent a key." }
            else { "Live matching is incomplete or not requested by the effective policy. Inspect the per-invocation diagnostics." }.into();
        receipt.observations = Some(observations);
        let profile = self
            .active_profile
            .as_ref()
            .ok_or("profile is unavailable")?;
        let mut catalog = profile
            .session
            .0
            .site_policy_drafts
            .lock()
            .map_err(|_| "draft catalogue is unavailable")?;
        let draft = catalog
            .drafts
            .get_mut(&parse_id(&receipt.draft)?)
            .ok_or("unknown draft")?;
        draft.check(self, draft.view.revision, false)?;
        if draft
            .view
            .preview
            .as_ref()
            .is_none_or(|current| current.id != receipt.id)
        {
            return Err("preview was superseded".into());
        }
        draft.view.preview = Some(receipt.clone());
        Ok(receipt)
    }
}

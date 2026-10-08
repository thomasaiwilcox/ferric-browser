//! Read-only structural restore planning. No stored event, request body,
//! history stack or policy is replayed at this boundary.

use ferric_browser_core::{
    CommitId, Event, LoadingState, LogicalPageId, LogicalWindowId, NativeProfileOwner, ProfileId,
    SafePageUrl, TabId, Target, ValidatedUrl, VersioningEvent, VersioningFailure, VersioningReason,
    WindowId, canonical_origin,
    versioning::{
        PageStateSidecar, SensitiveCategory, page_route_for_url, route_fingerprint_for_url,
    },
};
use ferric_browser_runtime::{RuntimeEffect, RuntimeInput};
use ferric_browser_storage::versioning::{
    ManifestSafeUrl, ManifestScope, ManifestUnavailable, VersionedCaptureManifest,
};
use ferric_browser_storage::{StorageCommand, StorageCompletion};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{BrowserApplication, StorageEffect, StorageTicket};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RestoreSidecarOptions {
    allow_media_restore: bool,
    allow_editable_restore: bool,
}

#[derive(Clone, Eq, PartialEq)]
pub enum RestoreNavigation {
    SafeGet(String),
    Placeholder(ManifestUnavailable),
}

impl std::fmt::Debug for RestoreNavigation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SafeGet(_) => formatter.write_str("SafeGet([redacted])"),
            Self::Placeholder(reason) => {
                formatter.debug_tuple("Placeholder").field(reason).finish()
            }
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct PlannedRestorePage {
    pub source_logical_page: LogicalPageId,
    pub logical_page: LogicalPageId,
    pub runtime_tab: TabId,
    pub navigation: RestoreNavigation,
    pub pinned: bool,
    pub muted: bool,
    pub zoom_hundredths: u16,
}

impl std::fmt::Debug for PlannedRestorePage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PlannedRestorePage")
            .field("source_logical_page", &"[redacted]")
            .field("logical_page", &"[redacted]")
            .field("runtime_tab", &"[redacted]")
            .field("navigation", &self.navigation)
            .field("pinned", &self.pinned)
            .field("muted", &self.muted)
            .field("zoom_hundredths", &self.zoom_hundredths)
            .finish()
    }
}

/// One inactive, replayable page in a restored window. The URL is held only
/// by the live application owner, never by the reducer's durable tab state.
#[derive(Clone)]
pub(crate) struct PendingLazyRestorePage {
    window: WindowId,
    destination_tab: TabId,
    destination_target: Target,
    logical_page: LogicalPageId,
    source_page: LogicalPageId,
    profile_uuid: Uuid,
    native_owner: NativeProfileOwner,
    url: String,
}

#[derive(Clone, Eq, PartialEq)]
pub struct NewWindowRestorePlan {
    pub commit: CommitId,
    pub source_window: WindowId,
    pub owner_profile: Uuid,
    pub(crate) source_native_owner: NativeProfileOwner,
    pub runtime_profile: ProfileId,
    pub admitted_revision: u64,
    pub source_logical_window: LogicalWindowId,
    pub logical_window: LogicalWindowId,
    pub runtime_window: WindowId,
    pub pages: Vec<PlannedRestorePage>,
    pub selected_tab: TabId,
    /// Navigation scheduling only: selected first; background URLs remain
    /// deferred. This plan does not produce navigation or form events.
    pub navigation_order: Vec<TabId>,
    // Public plan fields are consumed by the native adapter, but must remain
    // exactly as admitted from the validated manifest. This private seal
    // makes accidental or downstream mutation fail closed at execution. The
    // native owner remains private and is independently revalidated exactly.
    integrity: [u8; 32],
}

impl std::fmt::Debug for NewWindowRestorePlan {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NewWindowRestorePlan")
            .field("commit", &"[redacted]")
            .field("source_window", &"[redacted]")
            .field("owner_profile", &"[profile identity]")
            .field("source_native_owner", &"[native owner]")
            .field("runtime_profile", &"[profile identity]")
            .field("admitted_revision", &"[redacted]")
            .field("source_logical_window", &"[redacted]")
            .field("logical_window", &"[redacted]")
            .field("runtime_window", &"[redacted]")
            .field("page_count", &self.pages.len())
            .field("selected_tab", &"[redacted]")
            .field("navigation_count", &self.navigation_order.len())
            .finish()
    }
}

pub struct AppliedNewWindowRestore {
    pub plan: NewWindowRestorePlan,
    pub runtime_effects: Vec<RuntimeEffect>,
}

impl std::fmt::Debug for AppliedNewWindowRestore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AppliedNewWindowRestore")
            .field("plan", &self.plan)
            .field("runtime_effects", &"[redacted]")
            .finish()
    }
}

/// A multi-page restore materialized into a new Qt window's existing blank
/// reducer window. The destination tab IDs are in manifest page order and are
/// distinct from the source runtime IDs used while planning.
pub struct AppliedPristineWindowRestore {
    pub plan: NewWindowRestorePlan,
    pub destination_window: WindowId,
    pub destination_tabs: Vec<TabId>,
    pub runtime_effects: Vec<RuntimeEffect>,
}

impl std::fmt::Debug for AppliedPristineWindowRestore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AppliedPristineWindowRestore")
            .field("plan", &self.plan)
            .field("destination_window", &"[redacted]")
            .field("destination_tab_count", &self.destination_tabs.len())
            .field("runtime_effects", &"[redacted]")
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum RestorePlanEvent {
    Ready {
        request_id: u64,
        plan: NewWindowRestorePlan,
    },
    NotFound {
        request_id: u64,
    },
    Failed {
        request_id: u64,
    },
}

impl std::fmt::Debug for RestorePlanEvent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ready { plan, .. } => formatter
                .debug_struct("RestorePlanEvent::Ready")
                .field("request_id", &"[redacted]")
                .field("plan", plan)
                .finish(),
            Self::NotFound { .. } => formatter
                .debug_struct("RestorePlanEvent::NotFound")
                .field("request_id", &"[redacted]")
                .finish(),
            Self::Failed { .. } => formatter
                .debug_struct("RestorePlanEvent::Failed")
                .field("request_id", &"[redacted]")
                .finish(),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct PendingRestorePlan {
    // INVARIANT: restore projection must not retarget a result to a later
    // native owner of the same logical profile/window.
    source_window: WindowId,
    profile_uuid: Uuid,
    native_owner: NativeProfileOwner,
    include_policy_preview: bool,
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct PendingRestoreSidecar {
    // INVARIANT: page-state bytes may be admitted only by the exact owner,
    // document, and committed URL that requested them.
    window: WindowId,
    profile_uuid: Uuid,
    native_owner: NativeProfileOwner,
    commit: CommitId,
    source_page: LogicalPageId,
    destination_tab: TabId,
    destination_target: Target,
    destination_url: String,
    allow_media_restore: bool,
    allow_editable_restore: bool,
}

pub(crate) struct PendingRestoreSidecarIntent {
    window: WindowId,
    profile_uuid: Uuid,
    native_owner: NativeProfileOwner,
    commit: CommitId,
    source_page: LogicalPageId,
    destination_tab: TabId,
    destination_target: Target,
    destination_url: String,
    allow_media_restore: bool,
    allow_editable_restore: bool,
}

#[derive(Clone, Eq, PartialEq)]
pub enum RestoreSidecarEvent {
    Ready {
        request_id: u64,
        window: WindowId,
        commit: CommitId,
        source_page: LogicalPageId,
        destination_tab: TabId,
    },
    Missing {
        request_id: u64,
        window: WindowId,
        commit: CommitId,
        source_page: LogicalPageId,
        destination_tab: TabId,
    },
    Failed {
        request_id: u64,
    },
}

impl std::fmt::Debug for RestoreSidecarEvent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ready { .. } => formatter
                .debug_struct("RestoreSidecarEvent::Ready")
                .field("request_id", &"[redacted]")
                .field("window", &"[redacted]")
                .field("commit", &"[redacted]")
                .field("source_page", &"[redacted]")
                .field("destination_tab", &"[redacted]")
                .finish(),
            Self::Missing { .. } => formatter
                .debug_struct("RestoreSidecarEvent::Missing")
                .field("request_id", &"[redacted]")
                .field("window", &"[redacted]")
                .field("commit", &"[redacted]")
                .field("source_page", &"[redacted]")
                .field("destination_tab", &"[redacted]")
                .finish(),
            Self::Failed { .. } => formatter
                .debug_struct("RestoreSidecarEvent::Failed")
                .field("request_id", &"[redacted]")
                .finish(),
        }
    }
}

pub(crate) struct StoredRestoreSidecar {
    pub(crate) window: WindowId,
    profile_uuid: Uuid,
    native_owner: NativeProfileOwner,
    pub(crate) source_page: LogicalPageId,
    pub(crate) destination_tab: TabId,
    destination_target: Target,
    destination_url: String,
    pub(crate) sidecar: PageStateSidecar,
    allow_media_restore: bool,
    allow_editable_restore: bool,
    editable_restore_confirmed: bool,
    capture_qualification: Option<RestoreCaptureQualification>,
}

struct RestoreSidecarAdmission {
    request_id: u64,
    window: WindowId,
    profile_uuid: Uuid,
    native_owner: NativeProfileOwner,
    source_page: LogicalPageId,
    destination_tab: TabId,
    destination_target: Target,
    destination_url: String,
    allow_media_restore: bool,
    allow_editable_restore: bool,
}

struct RestoreCaptureQualification {
    origin: String,
    route_fingerprint: String,
    capture_fingerprint: String,
    nonce: String,
}

impl BrowserApplication {
    /// Adopts a one-page plan into a dedicated, still-blank destination
    /// window. This is the native multi-window adapter's first safe restore
    /// path: it does not touch the source browser window or infer a POST body.
    /// Multi-page plans require a tab-capable destination and are refused.
    ///
    /// # Errors
    /// Refuses a stale or non-pristine destination before changing its tab.
    pub fn apply_single_page_restore_to_pristine_window(
        &mut self,
        plan: &NewWindowRestorePlan,
        allow_media_restore: bool,
        allow_editable_restore: bool,
    ) -> Result<Vec<RuntimeEffect>, VersioningFailure> {
        if plan.pages.len() != 1 {
            return Err(failure(VersioningReason::UnsupportedCapability));
        }
        self.apply_restore_to_pristine_window_with_options(
            plan,
            allow_media_restore,
            allow_editable_restore,
        )
        .map(|applied| applied.runtime_effects)
    }

    /// Materializes any bounded page plan into a pristine, tab-capable window
    /// and schedules optional semantic restoration for its selected safe-GET
    /// page after that exact destination URL commits. Sidecar failure cannot
    /// roll back or invalidate the structural restore.
    ///
    /// # Errors
    /// Refuses stale plans and non-pristine destinations before navigation.
    pub fn apply_restore_to_pristine_window_with_options(
        &mut self,
        plan: &NewWindowRestorePlan,
        allow_media_restore: bool,
        allow_editable_restore: bool,
    ) -> Result<AppliedPristineWindowRestore, VersioningFailure> {
        let applied = self.apply_restore_to_pristine_window(plan)?;
        let Some(selected_index) = plan
            .pages
            .iter()
            .position(|page| page.runtime_tab == plan.selected_tab)
        else {
            return Err(failure(VersioningReason::UnsafeRestoreDescriptor));
        };
        if let RestoreNavigation::SafeGet(destination_url) = &plan.pages[selected_index].navigation
            && let Some(destination_tab) = applied.destination_tabs.get(selected_index).copied()
        {
            // INVARIANT: bind the optional sidecar read to the document
            // created by this restore navigation. Do not read before the
            // selected safe URL has actually committed.
            if let Ok(intent) = self.restore_sidecar_intent(
                applied.destination_window,
                plan.commit,
                plan.pages[selected_index].source_logical_page,
                destination_tab,
                destination_url,
                RestoreSidecarOptions {
                    allow_media_restore,
                    allow_editable_restore,
                },
            ) {
                self.pending_restore_sidecar_intents
                    .insert(destination_tab, intent);
            }
        }
        Ok(applied)
    }

    /// Materializes an admitted plan into an existing blank reducer window,
    /// retaining source page order and navigating only the selected page.
    /// The caller is responsible for supplying a native tab-capable surface;
    /// this method never replaces the source window or its live tabs.
    ///
    /// # Errors
    /// Refuses a stale plan or any destination that is no longer pristine.
    pub fn apply_restore_to_pristine_window(
        &mut self,
        plan: &NewWindowRestorePlan,
    ) -> Result<AppliedPristineWindowRestore, VersioningFailure> {
        let destination_window = plan.source_window;
        let first_tab = self.pristine_restore_tab(plan)?;
        self.ensure_lazy_restore_capacity(plan)?;
        let (destination_tabs, mut runtime_effects) =
            self.allocate_pristine_restore_tabs(destination_window, first_tab, plan.pages.len())?;
        self.bind_pristine_restore_pages(
            plan,
            destination_window,
            &destination_tabs,
            &mut runtime_effects,
        )?;

        let selected_index = plan
            .pages
            .iter()
            .position(|page| page.runtime_tab == plan.selected_tab)
            .ok_or_else(|| failure(VersioningReason::UnsafeRestoreDescriptor))?;
        let selected_destination_tab = destination_tabs[selected_index];
        runtime_effects.extend(
            self.dispatch_runtime(RuntimeInput::EngineFact(Event::ActivateTab {
                window: destination_window,
                tab: selected_destination_tab,
            }))
            .map_err(|_| failure(VersioningReason::StaleOwner))?,
        );
        self.start_selected_restore_navigation(
            &plan.pages[selected_index],
            selected_destination_tab,
            &mut runtime_effects,
        )?;
        self.remember_lazy_restore_pages(plan, destination_window, &destination_tabs);

        Ok(AppliedPristineWindowRestore {
            plan: plan.clone(),
            destination_window,
            destination_tabs,
            runtime_effects,
        })
    }

    fn pristine_restore_tab(
        &self,
        plan: &NewWindowRestorePlan,
    ) -> Result<TabId, VersioningFailure> {
        self.validate_new_window_restore_plan(plan)?;
        let destination_window = plan.source_window;
        let window = self
            .windows()
            .get(&destination_window)
            .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
        if window.context.is_some() || window.tabs.len() != 1 {
            return Err(failure(VersioningReason::StaleOwner));
        }
        let first_tab = window.tabs[0];
        let first_state = self
            .tabs()
            .get(&first_tab)
            .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
        if first_state
            .url
            .as_deref()
            .is_some_and(|url| url != "about:blank")
            || self.runtime_view().logical_page(first_tab).is_some()
            || self
                .runtime_view()
                .logical_window(destination_window)
                .is_some()
        {
            return Err(failure(VersioningReason::StaleOwner));
        }
        Ok(first_tab)
    }

    fn allocate_pristine_restore_tabs(
        &mut self,
        destination_window: WindowId,
        first_tab: TabId,
        page_count: usize,
    ) -> Result<(Vec<TabId>, Vec<RuntimeEffect>), VersioningFailure> {
        let mut destination_tabs = Vec::with_capacity(page_count);
        destination_tabs.push(first_tab);
        let mut runtime_effects = Vec::new();
        for _ in 1..page_count {
            runtime_effects.extend(
                self.dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab {
                    window: destination_window,
                }))
                .map_err(|_| failure(VersioningReason::StaleOwner))?,
            );
            let tab = self.windows()[&destination_window]
                .active_tab
                .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
            if destination_tabs.contains(&tab) {
                return Err(failure(VersioningReason::StaleOwner));
            }
            destination_tabs.push(tab);
        }
        Ok((destination_tabs, runtime_effects))
    }

    fn bind_pristine_restore_pages(
        &mut self,
        plan: &NewWindowRestorePlan,
        destination_window: WindowId,
        destination_tabs: &[TabId],
        runtime_effects: &mut Vec<RuntimeEffect>,
    ) -> Result<(), VersioningFailure> {
        let mut identity_pages = Vec::with_capacity(plan.pages.len());
        let mut lineage_pages = Vec::with_capacity(plan.pages.len());
        for (page, tab) in plan.pages.iter().zip(destination_tabs) {
            for event in [
                Event::SetTabPinned {
                    tab: *tab,
                    pinned: page.pinned,
                },
                Event::SetTabMuted {
                    tab: *tab,
                    muted: page.muted,
                },
                Event::SetTabZoom {
                    tab: *tab,
                    zoom_hundredths: u32::from(page.zoom_hundredths),
                },
            ] {
                runtime_effects.extend(
                    self.dispatch_runtime(RuntimeInput::EngineFact(event))
                        .map_err(|_| failure(VersioningReason::StaleOwner))?,
                );
            }
            identity_pages.push((*tab, page.logical_page));
            lineage_pages.push((*tab, page.source_logical_page));
        }
        for event in [
            Event::Versioning(VersioningEvent::BindStructuralIdentity {
                window: destination_window,
                logical_window: plan.logical_window,
                pages: identity_pages,
            }),
            Event::Versioning(VersioningEvent::BindRestoredLineage {
                window: destination_window,
                pages: lineage_pages,
            }),
        ] {
            runtime_effects.extend(
                self.dispatch_runtime(RuntimeInput::EngineFact(event))
                    .map_err(|_| failure(VersioningReason::StaleOwner))?,
            );
        }
        Ok(())
    }

    fn start_selected_restore_navigation(
        &mut self,
        selected_page: &PlannedRestorePage,
        selected_destination_tab: TabId,
        runtime_effects: &mut Vec<RuntimeEffect>,
    ) -> Result<(), VersioningFailure> {
        if let RestoreNavigation::SafeGet(url) = &selected_page.navigation {
            let target = self
                .capture_target(selected_destination_tab)
                .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
            let url = ValidatedUrl::parse(url)
                .map_err(|_| failure(VersioningReason::UnsafeRestoreDescriptor))?;
            runtime_effects.extend(
                self.dispatch_runtime(RuntimeInput::EngineFact(Event::StartNavigation {
                    target,
                    url,
                }))
                .map_err(|_| failure(VersioningReason::StaleOwner))?,
            );
        }
        Ok(())
    }

    fn remember_lazy_restore_pages(
        &mut self,
        plan: &NewWindowRestorePlan,
        window: WindowId,
        destination_tabs: &[TabId],
    ) {
        if plan.pages.len() != destination_tabs.len() {
            return;
        }
        let Ok((profile_uuid, native_owner)) = self.structural_commit_owner(window) else {
            return;
        };
        let mut pending = Vec::new();
        for (page, destination_tab) in plan.pages.iter().zip(destination_tabs) {
            if page.runtime_tab == plan.selected_tab {
                continue;
            }
            let RestoreNavigation::SafeGet(url) = &page.navigation else {
                continue;
            };
            let Some(destination_target) = self.capture_target(*destination_tab) else {
                return;
            };
            pending.push(PendingLazyRestorePage {
                window,
                destination_tab: *destination_tab,
                destination_target,
                logical_page: page.logical_page,
                source_page: page.source_logical_page,
                profile_uuid,
                native_owner,
                url: url.clone(),
            });
        }
        if self.pending_lazy_restore_pages.len() + pending.len() > 128 {
            return;
        }
        for page in pending {
            self.pending_lazy_restore_pages
                .insert(page.destination_tab, page);
        }
    }

    fn lazy_restore_page_matches(&self, page: &PendingLazyRestorePage) -> bool {
        self.structural_commit_owner(page.window)
            .is_ok_and(|(profile_uuid, native_owner)| {
                profile_uuid == page.profile_uuid && native_owner == page.native_owner
            })
            && self
                .windows()
                .get(&page.window)
                .is_some_and(|window| window.tabs.contains(&page.destination_tab))
            && self.tabs().get(&page.destination_tab).is_some_and(|tab| {
                tab.window == page.window
                    && tab.existence == ferric_browser_core::ExistenceState::Live
                    && tab.url.as_deref().is_none_or(|url| url == "about:blank")
            })
            && self.capture_target(page.destination_tab) == Some(page.destination_target)
            && self.runtime_view().logical_page(page.destination_tab) == Some(page.logical_page)
            && self.runtime_view().lineage_parent(page.destination_tab) == Some(page.source_page)
    }

    #[must_use]
    pub fn has_lazy_restore_page(&self, tab: TabId) -> bool {
        self.pending_lazy_restore_pages
            .get(&tab)
            .is_some_and(|page| self.lazy_restore_page_matches(page))
    }

    pub(crate) fn advance_lazy_restore_pages(
        &mut self,
        effects: &[RuntimeEffect],
    ) -> Vec<RuntimeEffect> {
        if self.pending_lazy_restore_pages.is_empty() {
            return Vec::new();
        }
        let stale_tabs = self
            .pending_lazy_restore_pages
            .iter()
            .filter_map(|(tab, page)| (!self.lazy_restore_page_matches(page)).then_some(*tab))
            .collect::<Vec<_>>();
        for tab in stale_tabs {
            self.pending_lazy_restore_pages.remove(&tab);
        }
        let activated = effects
            .iter()
            .filter_map(|effect| match effect {
                RuntimeEffect::AppliedEvent(Event::ActivateTab { window, tab }) => {
                    Some((*window, *tab))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut navigation_effects = Vec::new();
        for (window, tab) in activated {
            let Some(page) = self.pending_lazy_restore_pages.remove(&tab) else {
                continue;
            };
            if page.window != window
                || !self.lazy_restore_page_matches(&page)
                || self
                    .windows()
                    .get(&window)
                    .is_none_or(|state| state.active_tab != Some(tab))
                || SafePageUrl::from_observed(Some(&page.url), true).replay_url()
                    != Some(page.url.as_str())
            {
                continue;
            }
            let Ok(url) = ValidatedUrl::parse(&page.url) else {
                continue;
            };
            if let Ok(mut effects) =
                self.dispatch_runtime(RuntimeInput::EngineFact(Event::StartNavigation {
                    target: page.destination_target,
                    url,
                }))
            {
                navigation_effects.append(&mut effects);
            }
        }
        navigation_effects
    }

    /// Executes a previously admitted new-window plan through normal reducer
    /// events. The original window is neither replaced nor navigated. Only
    /// the selected safe-GET page is navigated; background pages remain lazy.
    ///
    /// # Errors
    /// Refuses stale plans and any modified/unsafe descriptor before opening
    /// a window. A later native view failure may leave a new blank window,
    /// but cannot overwrite the original window's work.
    pub fn execute_new_window_restore(
        &mut self,
        plan: NewWindowRestorePlan,
    ) -> Result<AppliedNewWindowRestore, VersioningFailure> {
        self.validate_new_window_restore_plan(&plan)?;
        self.ensure_lazy_restore_capacity(&plan)?;

        let mut runtime_effects = self
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CreateWindow {
                profile: plan.runtime_profile,
            }))
            .map_err(|_| failure(VersioningReason::StaleOwner))?;
        if self.runtime_view().active_window() != Some(plan.runtime_window) {
            return Err(failure(VersioningReason::StaleOwner));
        }
        for page in &plan.pages {
            runtime_effects.extend(
                self.dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab {
                    window: plan.runtime_window,
                }))
                .map_err(|_| failure(VersioningReason::StaleOwner))?,
            );
            if self.windows()[&plan.runtime_window].active_tab != Some(page.runtime_tab) {
                return Err(failure(VersioningReason::StaleOwner));
            }
            for event in [
                Event::SetTabPinned {
                    tab: page.runtime_tab,
                    pinned: page.pinned,
                },
                Event::SetTabMuted {
                    tab: page.runtime_tab,
                    muted: page.muted,
                },
                Event::SetTabZoom {
                    tab: page.runtime_tab,
                    zoom_hundredths: u32::from(page.zoom_hundredths),
                },
            ] {
                runtime_effects.extend(
                    self.dispatch_runtime(RuntimeInput::EngineFact(event))
                        .map_err(|_| failure(VersioningReason::StaleOwner))?,
                );
            }
        }
        runtime_effects.extend(
            self.dispatch_runtime(RuntimeInput::EngineFact(Event::Versioning(
                VersioningEvent::BindStructuralIdentity {
                    window: plan.runtime_window,
                    logical_window: plan.logical_window,
                    pages: plan
                        .pages
                        .iter()
                        .map(|page| (page.runtime_tab, page.logical_page))
                        .collect(),
                },
            )))
            .map_err(|_| failure(VersioningReason::StaleOwner))?,
        );
        runtime_effects.extend(
            self.dispatch_runtime(RuntimeInput::EngineFact(Event::Versioning(
                VersioningEvent::BindRestoredLineage {
                    window: plan.runtime_window,
                    pages: plan
                        .pages
                        .iter()
                        .map(|page| (page.runtime_tab, page.source_logical_page))
                        .collect(),
                },
            )))
            .map_err(|_| failure(VersioningReason::StaleOwner))?,
        );
        runtime_effects.extend(
            self.dispatch_runtime(RuntimeInput::EngineFact(Event::ActivateTab {
                window: plan.runtime_window,
                tab: plan.selected_tab,
            }))
            .map_err(|_| failure(VersioningReason::StaleOwner))?,
        );
        let selected = plan
            .pages
            .iter()
            .find(|page| page.runtime_tab == plan.selected_tab)
            .ok_or_else(|| failure(VersioningReason::UnsafeRestoreDescriptor))?;
        if let RestoreNavigation::SafeGet(url) = &selected.navigation {
            let target = self
                .capture_target(plan.selected_tab)
                .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
            let url = ValidatedUrl::parse(url)
                .map_err(|_| failure(VersioningReason::UnsafeRestoreDescriptor))?;
            runtime_effects.extend(
                self.dispatch_runtime(RuntimeInput::EngineFact(Event::StartNavigation {
                    target,
                    url,
                }))
                .map_err(|_| failure(VersioningReason::StaleOwner))?,
            );
        }
        self.remember_new_window_lazy_pages(&plan);
        Ok(AppliedNewWindowRestore {
            plan,
            runtime_effects,
        })
    }

    fn remember_new_window_lazy_pages(&mut self, plan: &NewWindowRestorePlan) {
        let tabs = plan
            .pages
            .iter()
            .map(|page| page.runtime_tab)
            .collect::<Vec<_>>();
        self.remember_lazy_restore_pages(plan, plan.runtime_window, &tabs);
    }

    fn ensure_lazy_restore_capacity(
        &self,
        plan: &NewWindowRestorePlan,
    ) -> Result<(), VersioningFailure> {
        let needed = plan
            .pages
            .iter()
            .filter(|page| {
                page.runtime_tab != plan.selected_tab
                    && matches!(page.navigation, RestoreNavigation::SafeGet(_))
            })
            .count();
        if self.pending_lazy_restore_pages.len().saturating_add(needed) > 128 {
            return Err(failure(VersioningReason::QuotaExceeded));
        }
        Ok(())
    }

    fn validate_new_window_restore_plan(
        &self,
        plan: &NewWindowRestorePlan,
    ) -> Result<(), VersioningFailure> {
        if plan.integrity != new_window_restore_plan_integrity(plan) {
            return Err(failure(VersioningReason::UnsafeRestoreDescriptor));
        }
        let (owner_profile, native_owner) = self.structural_commit_owner(plan.source_window)?;
        if owner_profile != plan.owner_profile
            || native_owner != plan.source_native_owner
            || self.runtime_view().revision() != plan.admitted_revision
            || self.windows()[&plan.source_window].profile != plan.runtime_profile
            || plan.pages.is_empty()
            || plan.pages.len() > 128
        {
            return Err(failure(VersioningReason::StaleOwner));
        }
        let (predicted_window, predicted_tabs) = self
            .runtime_view()
            .preview_new_window_ids(plan.runtime_profile, plan.pages.len())
            .map_err(|_| failure(VersioningReason::StaleOwner))?;
        if predicted_window != plan.runtime_window
            || plan
                .pages
                .iter()
                .map(|page| page.runtime_tab)
                .collect::<Vec<_>>()
                != predicted_tabs
            || !plan
                .pages
                .iter()
                .any(|page| page.runtime_tab == plan.selected_tab)
            || plan.navigation_order.len() != plan.pages.len()
            || plan.navigation_order.first() != Some(&plan.selected_tab)
            || plan.logical_window == plan.source_logical_window
            || self.windows().keys().any(|window| {
                self.runtime_view().logical_window(*window) == Some(plan.logical_window)
            })
        {
            return Err(failure(VersioningReason::UnsafeRestoreDescriptor));
        }
        let mut unique_logical = std::collections::BTreeSet::new();
        let mut unique_sources = std::collections::BTreeSet::new();
        let mut unique_schedule = std::collections::BTreeSet::new();
        for page in &plan.pages {
            if !unique_logical.insert(page.logical_page)
                || !unique_sources.insert(page.source_logical_page)
                || page.logical_page == page.source_logical_page
                || self
                    .tabs()
                    .keys()
                    .any(|tab| self.runtime_view().logical_page(*tab) == Some(page.logical_page))
                || !(25..=500).contains(&page.zoom_hundredths)
            {
                return Err(failure(VersioningReason::UnsafeRestoreDescriptor));
            }
            if let RestoreNavigation::SafeGet(url) = &page.navigation {
                if SafePageUrl::from_observed(Some(url), true).replay_url() != Some(url)
                    || ValidatedUrl::parse(url).is_err()
                {
                    return Err(failure(VersioningReason::UnsafeRestoreDescriptor));
                }
            }
        }
        if plan
            .navigation_order
            .iter()
            .any(|tab| !predicted_tabs.contains(tab) || !unique_schedule.insert(*tab))
        {
            return Err(failure(VersioningReason::UnsafeRestoreDescriptor));
        }
        Ok(())
    }

    /// Reads one exact current-profile commit through the bounded metadata
    /// worker. The full manifest is consumed only by this application layer;
    /// presentation and IPC still receive redacted commit info.
    ///
    /// # Errors
    /// Refuses wrong/transient owners or excessive pending work.
    pub fn request_new_window_restore_plan(
        &mut self,
        source_window: WindowId,
        commit: CommitId,
    ) -> Result<StorageTicket, VersioningFailure> {
        self.request_restore_plan(source_window, commit, false)
    }

    /// Requests a validated allowlisted policy preview for the exact commit.
    /// No page restore or configuration mutation occurs before explicit
    /// confirmation of the resulting `PolicyRestoreEvent`.
    ///
    /// # Errors
    /// Refuses transient/wrong owners, invalid commits, and unavailable storage.
    pub fn request_policy_restore_preview(
        &mut self,
        source_window: WindowId,
        commit: CommitId,
    ) -> Result<StorageTicket, VersioningFailure> {
        self.request_restore_plan(source_window, commit, true)
    }

    fn request_restore_plan(
        &mut self,
        source_window: WindowId,
        commit: CommitId,
        include_policy_preview: bool,
    ) -> Result<StorageTicket, VersioningFailure> {
        self.pending_policy_restore_previews
            .retain(|_, preview| !preview.is_expired());
        if self.pending_restore_plans.len() >= 16 {
            return Err(failure(VersioningReason::Busy));
        }
        if include_policy_preview && self.pending_policy_restore_previews.len() >= 16 {
            return Err(failure(VersioningReason::Busy));
        }
        let (profile_uuid, native_owner) = self.structural_commit_owner(source_window)?;
        let ticket = self
            .storage
            .submit(StorageCommand::VersioningManifest {
                profile_uuid,
                id: commit,
            })
            .map_err(|_| failure(VersioningReason::Busy))?;
        self.pending_restore_plans.insert(
            ticket,
            PendingRestorePlan {
                source_window,
                profile_uuid,
                native_owner,
                include_policy_preview,
            },
        );
        Ok(ticket)
    }

    /// Reads one committed page sidecar for the selected restored tab. The
    /// worker resolves the object through the exact commit manifest; bytes
    /// remain inside the application and are parsed before planning.
    ///
    /// # Errors
    /// Refuses a stale page lineage, inactive destination, transient profile,
    /// missing storage root or excessive pending restore work.
    pub fn request_restore_sidecar(
        &mut self,
        window: WindowId,
        commit: CommitId,
        source_page: LogicalPageId,
        destination_tab: TabId,
        allow_media_restore: bool,
        allow_editable_restore: bool,
    ) -> Result<StorageTicket, VersioningFailure> {
        let current_url = self
            .tabs()
            .get(&destination_tab)
            .filter(|tab| tab.window == window)
            .and_then(|tab| tab.url.as_deref())
            .filter(|url| ValidatedUrl::parse((*url).to_owned()).is_ok())
            .ok_or_else(|| failure(VersioningReason::UnsafeRestoreDescriptor))?;
        let intent = self.restore_sidecar_intent(
            window,
            commit,
            source_page,
            destination_tab,
            current_url,
            RestoreSidecarOptions {
                allow_media_restore,
                allow_editable_restore,
            },
        )?;
        self.submit_restore_sidecar(intent)
    }

    fn restore_sidecar_intent(
        &self,
        window: WindowId,
        commit: CommitId,
        source_page: LogicalPageId,
        destination_tab: TabId,
        destination_url: &str,
        options: RestoreSidecarOptions,
    ) -> Result<PendingRestoreSidecarIntent, VersioningFailure> {
        if self.pending_restore_sidecars.len()
            + self.restore_sidecars.len()
            + self.pending_restore_sidecar_intents.len()
            >= 16
        {
            return Err(failure(VersioningReason::Busy));
        }
        if page_route_for_url(destination_url).is_none()
            || ValidatedUrl::parse(destination_url.to_owned()).is_err()
        {
            return Err(failure(VersioningReason::UnsafeRestoreDescriptor));
        }
        let (profile_uuid, native_owner) = self.structural_commit_owner(window)?;
        let destination = self
            .windows()
            .get(&window)
            .filter(|window_state| window_state.active_tab == Some(destination_tab))
            .and_then(|window_state| window_state.tabs.contains(&destination_tab).then_some(()));
        if destination.is_none()
            || self.runtime_view().lineage_parent(destination_tab) != Some(source_page)
        {
            return Err(failure(VersioningReason::StaleOwner));
        }
        let destination_target = self
            .capture_target(destination_tab)
            .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
        Ok(PendingRestoreSidecarIntent {
            window,
            profile_uuid,
            native_owner,
            commit,
            source_page,
            destination_tab,
            destination_target,
            destination_url: destination_url.to_owned(),
            allow_media_restore: options.allow_media_restore,
            allow_editable_restore: options.allow_editable_restore,
        })
    }

    fn submit_restore_sidecar(
        &mut self,
        intent: PendingRestoreSidecarIntent,
    ) -> Result<StorageTicket, VersioningFailure> {
        if self.pending_restore_sidecars.len()
            + self.restore_sidecars.len()
            + self.pending_restore_sidecar_intents.len()
            >= 16
        {
            return Err(failure(VersioningReason::Busy));
        }
        if !self.restore_sidecar_intent_context_matches(&intent) {
            return Err(failure(VersioningReason::StaleOwner));
        }
        let state_root = self
            .profile_snapshot()
            .and_then(|profile| profile.roots)
            .map(|roots| roots.state)
            .ok_or_else(|| failure(VersioningReason::PrivatePersistenceDenied))?;
        let ticket = self
            .storage
            .submit(StorageCommand::VersioningSidecarRead {
                profile_uuid: intent.profile_uuid,
                state_root,
                id: intent.commit,
                logical_page: intent.source_page,
            })
            .map_err(|_| failure(VersioningReason::Busy))?;
        self.pending_restore_sidecars.insert(
            ticket,
            PendingRestoreSidecar {
                window: intent.window,
                profile_uuid: intent.profile_uuid,
                native_owner: intent.native_owner,
                commit: intent.commit,
                source_page: intent.source_page,
                destination_tab: intent.destination_tab,
                destination_target: intent.destination_target,
                destination_url: intent.destination_url,
                allow_media_restore: intent.allow_media_restore,
                allow_editable_restore: intent.allow_editable_restore,
            },
        );
        Ok(ticket)
    }

    /// Consumes one parsed stored sidecar and creates a closed page-state
    /// request for the exact still-selected descendant page. Media targets
    /// are emitted only for a restore explicitly opted in at the UI boundary.
    ///
    /// # Errors
    /// Refuses replayed requests, changed ownership/lineage, malformed current
    /// observations, or a route that no longer matches the saved sidecar.
    pub fn build_restore_geometry_script(
        &mut self,
        window: WindowId,
        request_id: u64,
        current_sidecar_json: &[u8],
    ) -> Result<String, VersioningFailure> {
        let stored = self
            .restore_sidecars
            .remove(&request_id)
            .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
        if stored.window != window {
            return Err(failure(VersioningReason::StaleOwner));
        }
        if !self.restore_sidecar_owner_matches(window, &stored) {
            return Err(failure(VersioningReason::StaleOwner));
        }
        let window_state = self
            .windows()
            .get(&window)
            .filter(|window_state| window_state.active_tab == Some(stored.destination_tab))
            .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
        if !window_state.tabs.contains(&stored.destination_tab)
            || self.runtime_view().lineage_parent(stored.destination_tab)
                != Some(stored.source_page)
        {
            return Err(failure(VersioningReason::StaleOwner));
        }
        let current_url = self
            .tabs()
            .get(&stored.destination_tab)
            .filter(|tab| tab.window == window)
            .and_then(|tab| tab.url.as_deref())
            .filter(|url| ValidatedUrl::parse((*url).to_owned()).is_ok())
            .ok_or_else(|| failure(VersioningReason::UnsafeRestoreDescriptor))?;
        let current = PageStateSidecar::parse_json(current_sidecar_json)
            .map_err(|_| failure(VersioningReason::UnsafeRestoreDescriptor))?;
        let qualification = stored
            .capture_qualification
            .as_ref()
            .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
        if canonical_origin(current_url).as_deref() != Some(qualification.origin.as_str())
            || route_fingerprint_for_url(current_url).as_deref()
                != Some(qualification.route_fingerprint.as_str())
            || !current.qualifies_for(
                &qualification.origin,
                &qualification.route_fingerprint,
                &qualification.capture_fingerprint,
                &qualification.nonce,
            )
        {
            return Err(failure(VersioningReason::UnsafeRestoreDescriptor));
        }
        let plan = stored
            .sidecar
            .plan_geometry_restore_for_url(&current, current_url)
            .map_err(|_| failure(VersioningReason::UnsafeRestoreDescriptor))?;
        let route = page_route_for_url(current_url)
            .ok_or_else(|| failure(VersioningReason::UnsafeRestoreDescriptor))?;
        plan.script_request(
            &route,
            stored.allow_media_restore,
            stored.allow_editable_restore && stored.editable_restore_confirmed,
        )
        .map_err(|_| failure(VersioningReason::UnsafeRestoreDescriptor))
    }

    /// Creates a one-use, `ApplicationWorld` capture request for the exact
    /// selected restored descendant. The page returns bounded geometry and,
    /// only when separately opted in, bounded media observations plus hashed
    /// candidate hints; the application checks qualification again before
    /// planning any mutation.
    ///
    /// # Errors
    /// Refuses stale ownership, a repeated capture request or an unsafe route.
    pub fn build_restore_geometry_capture_request(
        &mut self,
        window: WindowId,
        request_id: u64,
        destination_tab: TabId,
    ) -> Result<String, VersioningFailure> {
        let stored = self
            .restore_sidecars
            .get(&request_id)
            .filter(|stored| stored.window == window && stored.destination_tab == destination_tab)
            .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
        if stored.capture_qualification.is_some()
            || !self.restore_sidecar_owner_matches(window, stored)
        {
            return Err(failure(VersioningReason::StaleOwner));
        }
        let source_page = stored.source_page;
        let window_state = self
            .windows()
            .get(&window)
            .filter(|window_state| window_state.active_tab == Some(destination_tab))
            .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
        if !window_state.tabs.contains(&destination_tab)
            || self.runtime_view().lineage_parent(destination_tab) != Some(source_page)
        {
            return Err(failure(VersioningReason::StaleOwner));
        }
        let current_url = self
            .tabs()
            .get(&destination_tab)
            .filter(|tab| tab.window == window)
            .and_then(|tab| tab.url.as_deref())
            .filter(|url| ValidatedUrl::parse((*url).to_owned()).is_ok())
            .ok_or_else(|| failure(VersioningReason::UnsafeRestoreDescriptor))?;
        let origin = canonical_origin(current_url)
            .ok_or_else(|| failure(VersioningReason::UnsafeRestoreDescriptor))?;
        let route = page_route_for_url(current_url)
            .filter(|route| route.len() <= 2_048)
            .ok_or_else(|| failure(VersioningReason::UnsafeRestoreDescriptor))?;
        let route_fingerprint = route_fingerprint_for_url(current_url)
            .ok_or_else(|| failure(VersioningReason::UnsafeRestoreDescriptor))?;
        let nonce = Uuid::new_v4().simple().to_string();
        let mut hasher = Sha256::new();
        hasher.update(b"ferric-versioned-geometry-capture-v1\0");
        hasher.update(window.to_string().as_bytes());
        hasher.update([0]);
        hasher.update(destination_tab.to_string().as_bytes());
        hasher.update([0]);
        hasher.update(current_url.as_bytes());
        hasher.update([0]);
        hasher.update(nonce.as_bytes());
        let capture_fingerprint = format!("{:x}", hasher.finalize());
        let request = serde_json::json!({
            "expected_origin": &origin,
            "expected_route": &route,
            "route_fingerprint": &route_fingerprint,
            "capture_fingerprint": &capture_fingerprint,
            "nonce": &nonce,
            "max_nodes": 4096,
            "max_candidates": 64,
            "max_ms": 50,
            "include_controls": false,
            "include_media": stored.allow_media_restore,
            "include_editable": stored.allow_editable_restore
                && stored.editable_restore_confirmed,
        });
        let encoded = serde_json::to_string(&request)
            .map_err(|_| failure(VersioningReason::UnsafeRestoreDescriptor))?;
        if encoded.len() > 4_096 {
            return Err(failure(VersioningReason::UnsafeRestoreDescriptor));
        }
        let qualification = RestoreCaptureQualification {
            origin,
            route_fingerprint,
            capture_fingerprint,
            nonce,
        };
        let stored = self
            .restore_sidecars
            .get_mut(&request_id)
            .filter(|stored| {
                stored.window == window
                    && stored.destination_tab == destination_tab
                    && stored.capture_qualification.is_none()
            })
            .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
        stored.capture_qualification = Some(qualification);
        Ok(encoded)
    }

    /// Returns saved native-control values only to the exact restored window
    /// after the source UI requested an editable-recovery preview. The caller
    /// must render this only as plain text in the native confirmation dialog.
    ///
    /// # Errors
    /// Refuses stale or mismatched restore-preview ownership.
    pub fn restore_editable_preview_json(
        &mut self,
        window: WindowId,
        request_id: u64,
    ) -> Result<String, VersioningFailure> {
        let stored = self
            .restore_sidecars
            .get(&request_id)
            .filter(|stored| {
                stored.window == window
                    && stored.allow_editable_restore
                    && !stored.editable_restore_confirmed
                    && stored.capture_qualification.is_none()
            })
            .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
        if !self.restore_sidecar_owner_matches(window, stored) {
            return Err(failure(VersioningReason::StaleOwner));
        }
        let window_state = self
            .windows()
            .get(&window)
            .filter(|window_state| window_state.active_tab == Some(stored.destination_tab))
            .ok_or_else(|| failure(VersioningReason::StaleOwner))?;
        if !window_state.tabs.contains(&stored.destination_tab)
            || self.runtime_view().lineage_parent(stored.destination_tab)
                != Some(stored.source_page)
        {
            return Err(failure(VersioningReason::StaleOwner));
        }
        stored
            .sidecar
            .editable_restore_preview_json()
            .map_err(|_| failure(VersioningReason::UnsafeRestoreDescriptor))
    }

    /// Captures a one-use native confirmation for the exact stored sidecar.
    /// It never accepts page-script or IPC authorization.
    pub fn confirm_restore_editable_preview(&mut self, window: WindowId, request_id: u64) -> bool {
        let Some(stored) = self.restore_sidecars.get(&request_id) else {
            return false;
        };
        let valid = stored.window == window
            && stored.allow_editable_restore
            && !stored.editable_restore_confirmed
            && stored.capture_qualification.is_none()
            && self.restore_sidecar_owner_matches(window, stored);
        if !valid {
            return false;
        }
        let Some(stored) = self.restore_sidecars.get_mut(&request_id) else {
            return false;
        };
        stored.editable_restore_confirmed = true;
        true
    }

    fn restore_sidecar_owner_matches(
        &self,
        window: WindowId,
        stored: &StoredRestoreSidecar,
    ) -> bool {
        stored.window == window
            && self
                .structural_commit_owner(window)
                .is_ok_and(|(profile_uuid, native_owner)| {
                    profile_uuid == stored.profile_uuid && native_owner == stored.native_owner
                })
            && self.restore_sidecar_context_matches(
                window,
                stored.destination_tab,
                stored.destination_target,
                &stored.destination_url,
                stored.source_page,
            )
    }

    fn restore_sidecar_intent_context_matches(&self, intent: &PendingRestoreSidecarIntent) -> bool {
        self.tabs()
            .get(&intent.destination_tab)
            .is_some_and(|tab| tab.window == intent.window)
            && self.structural_commit_owner(intent.window).is_ok_and(
                |(profile_uuid, native_owner)| {
                    profile_uuid == intent.profile_uuid && native_owner == intent.native_owner
                },
            )
            && self.restore_sidecar_context_matches(
                intent.window,
                intent.destination_tab,
                intent.destination_target,
                &intent.destination_url,
                intent.source_page,
            )
    }

    fn restore_sidecar_context_matches(
        &self,
        window: WindowId,
        destination_tab: TabId,
        destination_target: Target,
        destination_url: &str,
        source_page: LogicalPageId,
    ) -> bool {
        self.capture_target(destination_tab) == Some(destination_target)
            && self.windows().get(&window).is_some_and(|window_state| {
                window_state.active_tab == Some(destination_tab)
                    && window_state.tabs.contains(&destination_tab)
            })
            && self.runtime_view().lineage_parent(destination_tab) == Some(source_page)
            && self.tabs().get(&destination_tab).is_some_and(|tab| {
                tab.window == window && tab.url.as_deref() == Some(destination_url)
            })
    }

    fn restore_sidecar_pending_context_matches(&self, pending: &PendingRestoreSidecar) -> bool {
        self.structural_commit_owner(pending.window)
            .is_ok_and(|(profile_uuid, native_owner)| {
                profile_uuid == pending.profile_uuid && native_owner == pending.native_owner
            })
            && self.restore_sidecar_context_matches(
                pending.window,
                pending.destination_tab,
                pending.destination_target,
                &pending.destination_url,
                pending.source_page,
            )
    }

    pub(crate) fn advance_restore_sidecar_intents(&mut self, runtime_effects: &[RuntimeEffect]) {
        let commits = runtime_effects
            .iter()
            .filter_map(|effect| match effect {
                RuntimeEffect::AppliedEvent(
                    Event::CommitNavigation { target, url, .. }
                    | Event::CommitNavigationWithTransition { target, url, .. },
                ) => Some((*target, url.to_string())),
                _ => None,
            })
            .collect::<Vec<_>>();
        let invalidated_targets = runtime_effects
            .iter()
            .filter_map(|effect| match effect {
                RuntimeEffect::AppliedEvent(
                    Event::FailNavigation { target } | Event::CompleteNavigation { target },
                ) => Some(*target),
                _ => None,
            })
            .collect::<Vec<_>>();
        let observed_url_changes = runtime_effects
            .iter()
            .filter_map(|effect| match effect {
                RuntimeEffect::AppliedEvent(Event::SameDocumentNavigation { target, url }) => {
                    Some((*target, url.to_string()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if commits.is_empty()
            && invalidated_targets.is_empty()
            && observed_url_changes.is_empty()
            && self.pending_restore_sidecar_intents.is_empty()
        {
            return;
        }

        let tabs = self
            .pending_restore_sidecar_intents
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for tab in tabs {
            let Some(intent) = self.pending_restore_sidecar_intents.get(&tab) else {
                continue;
            };
            if !self.restore_sidecar_intent_owner_and_target_match(intent) {
                self.pending_restore_sidecar_intents.remove(&tab);
                continue;
            }
            // Qt reports the provisional target URL through its ordinary URL
            // change callback before the full navigation commits. The
            // adapter expresses that as SameDocumentNavigation, so retain an
            // exact-URL update only while this document is still provisional.
            // A different route or any post-load same-document change still
            // invalidates the one-use sidecar intent.
            let changed_document = observed_url_changes.iter().any(|(target, url)| {
                *target == intent.destination_target
                    && (url != &intent.destination_url
                        || self.tabs()[&tab].loading != LoadingState::Provisional)
            });
            if invalidated_targets.contains(&intent.destination_target) || changed_document {
                self.pending_restore_sidecar_intents.remove(&tab);
                continue;
            }
            let Some((_, url)) = commits
                .iter()
                .find(|(target, _)| *target == intent.destination_target)
            else {
                continue;
            };
            let intent = self
                .pending_restore_sidecar_intents
                .remove(&tab)
                .expect("restore sidecar intent remains present");
            if *url == intent.destination_url
                && self.restore_sidecar_intent_context_matches(&intent)
            {
                let _ = self.submit_restore_sidecar(intent);
            }
        }
    }

    fn restore_sidecar_intent_owner_and_target_match(
        &self,
        intent: &PendingRestoreSidecarIntent,
    ) -> bool {
        self.tabs()
            .get(&intent.destination_tab)
            .is_some_and(|tab| tab.window == intent.window)
            && self.structural_commit_owner(intent.window).is_ok_and(
                |(profile_uuid, native_owner)| {
                    profile_uuid == intent.profile_uuid && native_owner == intent.native_owner
                },
            )
            && self.capture_target(intent.destination_tab) == Some(intent.destination_target)
            && self
                .windows()
                .get(&intent.window)
                .is_some_and(|window_state| {
                    window_state.active_tab == Some(intent.destination_tab)
                        && window_state.tabs.contains(&intent.destination_tab)
                })
            && self.runtime_view().lineage_parent(intent.destination_tab)
                == Some(intent.source_page)
    }

    pub(crate) fn discard_restore_sidecars_for_owner(&mut self, owner: NativeProfileOwner) {
        self.restore_sidecars
            .retain(|_, stored| stored.native_owner != owner);
        self.pending_restore_sidecar_intents
            .retain(|_, intent| intent.native_owner != owner);
        self.pending_lazy_restore_pages
            .retain(|_, page| page.native_owner != owner);
    }

    /// Drops a ready restore sidecar when its destination view disappears
    /// before geometry planning. The payload never moves to another tab.
    pub fn discard_restore_sidecar(&mut self, request_id: u64) -> bool {
        self.restore_sidecars.remove(&request_id).is_some()
    }

    pub(crate) fn route_restore_storage_effects(
        &mut self,
        effects: Vec<StorageEffect>,
    ) -> Vec<StorageEffect> {
        let mut forwarded = Vec::with_capacity(effects.len());
        for effect in effects {
            let ticket = effect.ticket;
            if self.pending_restore_sidecars.contains_key(&ticket) {
                self.route_restore_sidecar_effect(effect);
                continue;
            }
            if self.pending_restore_plans.contains_key(&ticket) {
                self.route_restore_plan_effect(effect);
            } else {
                forwarded.push(effect);
            }
        }
        forwarded
    }

    fn route_restore_sidecar_effect(&mut self, effect: StorageEffect) {
        let Some(pending) = self.pending_restore_sidecars.remove(&effect.ticket) else {
            return;
        };
        let request_id = effect.ticket.value();
        if !self.restore_sidecar_pending_context_matches(&pending) {
            self.restore_sidecar_events
                .push_back(RestoreSidecarEvent::Failed { request_id });
            return;
        }
        let event = match effect.completion {
            StorageCompletion::VersioningSidecarRead(Ok(Some(bytes))) => {
                let admission = RestoreSidecarAdmission {
                    request_id,
                    window: pending.window,
                    profile_uuid: pending.profile_uuid,
                    native_owner: pending.native_owner,
                    source_page: pending.source_page,
                    destination_tab: pending.destination_tab,
                    destination_target: pending.destination_target,
                    destination_url: pending.destination_url.clone(),
                    allow_media_restore: pending.allow_media_restore,
                    allow_editable_restore: pending.allow_editable_restore,
                };
                if self
                    .parse_restore_sidecar(admission, &bytes.into_bytes())
                    .is_ok()
                {
                    RestoreSidecarEvent::Ready {
                        request_id,
                        window: pending.window,
                        commit: pending.commit,
                        source_page: pending.source_page,
                        destination_tab: pending.destination_tab,
                    }
                } else {
                    RestoreSidecarEvent::Failed { request_id }
                }
            }
            StorageCompletion::VersioningSidecarRead(Ok(None)) => RestoreSidecarEvent::Missing {
                request_id,
                window: pending.window,
                commit: pending.commit,
                source_page: pending.source_page,
                destination_tab: pending.destination_tab,
            },
            _ => RestoreSidecarEvent::Failed { request_id },
        };
        self.restore_sidecar_events.push_back(event);
    }

    fn parse_restore_sidecar(
        &mut self,
        admission: RestoreSidecarAdmission,
        bytes: &[u8],
    ) -> Result<(), ()> {
        let sidecar = PageStateSidecar::parse_json(bytes).map_err(|_| ())?;
        if self.restore_sidecars.len() >= 16 || !sidecar.matches_url(&admission.destination_url) {
            return Err(());
        }
        self.restore_sidecars.insert(
            admission.request_id,
            StoredRestoreSidecar {
                window: admission.window,
                profile_uuid: admission.profile_uuid,
                native_owner: admission.native_owner,
                source_page: admission.source_page,
                destination_tab: admission.destination_tab,
                destination_target: admission.destination_target,
                destination_url: admission.destination_url,
                sidecar,
                allow_media_restore: admission.allow_media_restore,
                allow_editable_restore: admission.allow_editable_restore,
                editable_restore_confirmed: false,
                capture_qualification: None,
            },
        );
        Ok(())
    }

    fn route_restore_plan_effect(&mut self, effect: StorageEffect) {
        let Some(pending) = self.pending_restore_plans.remove(&effect.ticket) else {
            return;
        };
        let request_id = effect.ticket.value();
        let owner_is_current = self
            .structural_commit_owner(pending.source_window)
            .is_ok_and(|(profile_uuid, native_owner)| {
                profile_uuid == pending.profile_uuid && native_owner == pending.native_owner
            });
        if !owner_is_current {
            if pending.include_policy_preview {
                self.policy_restore_events
                    .push_back(super::PolicyRestoreEvent::Failed { request_id });
            } else {
                self.restore_plan_events
                    .push_back(RestorePlanEvent::Failed { request_id });
            }
            return;
        }
        match effect.completion {
            StorageCompletion::VersioningManifest(Ok(Some(manifest)))
                if pending.include_policy_preview =>
            {
                let preview =
                    self.preview_allowlisted_policy_restore(pending.source_window, &manifest);
                match preview {
                    Ok(preview) if self.pending_policy_restore_previews.len() < 16 => {
                        self.pending_policy_restore_previews
                            .insert(request_id, preview.clone());
                        self.policy_restore_events.push_back(
                            super::PolicyRestoreEvent::PreviewReady {
                                request_id,
                                preview,
                            },
                        );
                    }
                    _ => self
                        .policy_restore_events
                        .push_back(super::PolicyRestoreEvent::Failed { request_id }),
                }
            }
            StorageCompletion::VersioningManifest(Ok(Some(manifest))) => {
                let event = match self.plan_new_window_restore(pending.source_window, &manifest) {
                    Ok(plan) => RestorePlanEvent::Ready { request_id, plan },
                    Err(_) => RestorePlanEvent::Failed { request_id },
                };
                self.restore_plan_events.push_back(event);
            }
            StorageCompletion::VersioningManifest(Ok(None)) if pending.include_policy_preview => {
                self.policy_restore_events
                    .push_back(super::PolicyRestoreEvent::Failed { request_id });
            }
            StorageCompletion::VersioningManifest(Ok(None)) => {
                self.restore_plan_events
                    .push_back(RestorePlanEvent::NotFound { request_id });
            }
            _ if pending.include_policy_preview => self
                .policy_restore_events
                .push_back(super::PolicyRestoreEvent::Failed { request_id }),
            _ => self
                .restore_plan_events
                .push_back(RestorePlanEvent::Failed { request_id }),
        }
    }

    #[must_use]
    pub fn poll_restore_plan_events(&mut self) -> Vec<RestorePlanEvent> {
        self.restore_plan_events.drain(..).collect()
    }

    #[must_use]
    pub fn poll_policy_restore_events(&mut self) -> Vec<super::PolicyRestoreEvent> {
        self.policy_restore_events.drain(..).collect()
    }

    /// Returns bounded results from exact commit/page sidecar reads.
    #[must_use]
    pub fn poll_restore_sidecar_events(&mut self) -> Vec<RestoreSidecarEvent> {
        self.restore_sidecar_events.drain(..).collect()
    }

    /// Validates one exact-profile manifest and predicts fresh runtime IDs by
    /// running normal reducer allocation on an isolated state clone. The plan
    /// must be revalidated against `admitted_revision` before execution.
    /// No current window or tab changes here.
    ///
    /// # Errors
    /// Refuses a foreign/transient profile, malformed manifest, context scope
    /// or unsafe purported replay URL.
    pub fn plan_new_window_restore(
        &self,
        source_window: WindowId,
        manifest: &VersionedCaptureManifest,
    ) -> Result<NewWindowRestorePlan, VersioningFailure> {
        let (profile_uuid, native_owner) = self.structural_commit_owner(source_window)?;
        if manifest.profile_uuid != profile_uuid {
            return Err(failure(VersioningReason::WrongProfile));
        }
        manifest
            .validate()
            .map_err(|_| failure(VersioningReason::UnsafeRestoreDescriptor))?;
        if manifest.scope == ManifestScope::Context || manifest.windows.len() != 1 {
            return Err(failure(VersioningReason::UnsupportedCapability));
        }
        let source = &manifest.windows[0];
        if source.context_ref.is_some() || source.pages.is_empty() {
            return Err(failure(VersioningReason::UnsupportedCapability));
        }
        let source_logical_window = source
            .logical_window_id
            .parse::<LogicalWindowId>()
            .map_err(|_| failure(VersioningReason::UnsafeRestoreDescriptor))?;
        let logical_window = LogicalWindowId::fresh();
        let commit = manifest
            .id
            .parse::<CommitId>()
            .map_err(|_| failure(VersioningReason::UnsafeRestoreDescriptor))?;
        let runtime_profile = self
            .windows()
            .get(&source_window)
            .ok_or_else(|| failure(VersioningReason::StaleOwner))?
            .profile;
        let (runtime_window, tabs) = self
            .runtime_view()
            .preview_new_window_ids(runtime_profile, source.pages.len())
            .map_err(|_| failure(VersioningReason::StaleOwner))?;
        let pages = source
            .pages
            .iter()
            .zip(tabs)
            .map(|(page, runtime_tab)| {
                let source_logical_page = page
                    .logical_page_id
                    .parse::<LogicalPageId>()
                    .map_err(|_| failure(VersioningReason::UnsafeRestoreDescriptor))?;
                let navigation = match &page.safe_url_or_placeholder {
                    ManifestSafeUrl::Replayable(url) => {
                        if SafePageUrl::from_observed(Some(url), true).replay_url()
                            != Some(url.as_str())
                        {
                            return Err(failure(VersioningReason::UnsafeRestoreDescriptor));
                        }
                        RestoreNavigation::SafeGet(url.clone())
                    }
                    ManifestSafeUrl::Placeholder(reason) => RestoreNavigation::Placeholder(*reason),
                };
                Ok(PlannedRestorePage {
                    source_logical_page,
                    logical_page: LogicalPageId::fresh(),
                    runtime_tab,
                    navigation,
                    pinned: page.pinned,
                    muted: page.muted,
                    zoom_hundredths: page.zoom_hundredths,
                })
            })
            .collect::<Result<Vec<_>, VersioningFailure>>()?;
        let selected_logical = source
            .selected_logical_page
            .as_deref()
            .map(str::parse::<LogicalPageId>)
            .transpose()
            .map_err(|_| failure(VersioningReason::UnsafeRestoreDescriptor))?;
        let selected_tab = selected_logical
            .and_then(|logical| {
                pages
                    .iter()
                    .find(|page| page.source_logical_page == logical)
                    .map(|page| page.runtime_tab)
            })
            .unwrap_or(pages[0].runtime_tab);
        let navigation_order = std::iter::once(selected_tab)
            .chain(
                pages
                    .iter()
                    .map(|page| page.runtime_tab)
                    .filter(|tab| *tab != selected_tab),
            )
            .collect();
        let mut plan = NewWindowRestorePlan {
            commit,
            source_window,
            owner_profile: profile_uuid,
            source_native_owner: native_owner,
            runtime_profile,
            admitted_revision: self.runtime_view().revision(),
            source_logical_window,
            logical_window,
            runtime_window,
            pages,
            selected_tab,
            navigation_order,
            integrity: [0; 32],
        };
        plan.integrity = new_window_restore_plan_integrity(&plan);
        Ok(plan)
    }
}

fn update_restore_integrity(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value);
}

fn update_restore_integrity_text(hasher: &mut Sha256, value: &str) {
    update_restore_integrity(hasher, value.as_bytes());
}

fn new_window_restore_plan_integrity(plan: &NewWindowRestorePlan) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"ferric-new-window-restore-plan-v1\0");
    update_restore_integrity_text(&mut hasher, &plan.commit.to_string());
    update_restore_integrity_text(&mut hasher, &plan.source_window.to_string());
    update_restore_integrity(&mut hasher, plan.owner_profile.as_bytes());
    update_restore_integrity_text(&mut hasher, &plan.runtime_profile.to_string());
    hasher.update(plan.admitted_revision.to_le_bytes());
    update_restore_integrity_text(&mut hasher, &plan.source_logical_window.to_string());
    update_restore_integrity_text(&mut hasher, &plan.logical_window.to_string());
    update_restore_integrity_text(&mut hasher, &plan.runtime_window.to_string());
    hasher.update((plan.pages.len() as u64).to_le_bytes());
    for page in &plan.pages {
        update_restore_integrity_text(&mut hasher, &page.source_logical_page.to_string());
        update_restore_integrity_text(&mut hasher, &page.logical_page.to_string());
        update_restore_integrity_text(&mut hasher, &page.runtime_tab.to_string());
        match &page.navigation {
            RestoreNavigation::SafeGet(url) => {
                update_restore_integrity(&mut hasher, b"safe-get");
                update_restore_integrity_text(&mut hasher, url);
            }
            RestoreNavigation::Placeholder(reason) => {
                update_restore_integrity(&mut hasher, b"placeholder");
                let reason = match reason {
                    ManifestUnavailable::NotObserved => b"not-observed".as_slice(),
                    ManifestUnavailable::Unsupported => b"unsupported".as_slice(),
                    ManifestUnavailable::UnsafeNavigation => b"unsafe-navigation".as_slice(),
                };
                update_restore_integrity(&mut hasher, reason);
            }
        }
        update_restore_integrity(&mut hasher, &[u8::from(page.pinned), u8::from(page.muted)]);
        hasher.update(page.zoom_hundredths.to_le_bytes());
    }
    update_restore_integrity_text(&mut hasher, &plan.selected_tab.to_string());
    hasher.update((plan.navigation_order.len() as u64).to_le_bytes());
    for tab in &plan.navigation_order {
        update_restore_integrity_text(&mut hasher, &tab.to_string());
    }
    hasher.finalize().into()
}

fn failure(reason: VersioningReason) -> VersioningFailure {
    VersioningFailure::new(reason, SensitiveCategory::Page)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProfileActivation;
    use ferric_browser_config::{Config, RuntimeOverrides};
    use ferric_browser_core::{
        CommandInvocation, CommandSource, DispatchTarget, EngineEffect, Event, NavigationContext,
        ParsedCommand, PrivacyKind, ProfileInstanceId, ScopeSelector, VersioningEngineEffect,
    };
    use ferric_browser_runtime::{RuntimeCommand, RuntimeDispatch, RuntimeInput};
    use ferric_browser_storage::{ProfileStore, RootSpec, StorageRoots, StoreMode};
    use std::{
        fs, thread,
        time::{Duration, Instant},
    };

    fn fixture() -> (
        BrowserApplication,
        StorageRoots,
        WindowId,
        VersionedCaptureManifest,
    ) {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("temporary roots");
        roots.ensure().expect("private roots");
        let profile_uuid = Uuid::new_v4();
        let directory = roots.data.join("profiles").join(profile_uuid.to_string());
        fs::create_dir_all(&directory).expect("profile directory");
        let (mut application, window, _) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "default")
                .expect("normal runtime");
        application
            .activate_profile(ProfileActivation {
                name: "default".into(),
                id: Some(profile_uuid),
                privacy: PrivacyKind::Normal,
                roots: Some(roots.clone()),
                lock: None,
                contexts: None,
                storage_path: Some(directory.join("browser.sqlite")),
            })
            .expect("durable profile");
        let profile = application.windows()[&window].profile;
        application
            .bind_native_profile(profile, ProfileInstanceId::fresh())
            .expect("native owner");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab { window }))
            .expect("second tab");
        let manifest = application
            .prepare_structural_commit(ScopeSelector::Window { window }, "before trial".into())
            .expect("structural capture")
            .manifest;
        (application, roots, window, manifest)
    }

    fn assert_lazy_activation_navigates_once(
        application: &mut BrowserApplication,
        window: WindowId,
        tab: TabId,
        expected_url: &str,
    ) {
        let activated = application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::ActivateTab { window, tab }))
            .expect("activate lazy restored tab");
        assert_eq!(
            activated
                .iter()
                .filter(|effect| matches!(
                    effect,
                    RuntimeEffect::AppliedEvent(Event::StartNavigation { .. })
                ))
                .count(),
            1,
            "the exact inactive page navigates once on selection"
        );
        let navigated_urls = activated
            .iter()
            .filter_map(|effect| match effect {
                RuntimeEffect::Engine(EngineEffect::Navigate { url, .. }) => Some(url.to_string()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(navigated_urls, [expected_url]);
        assert_eq!(
            application.tabs()[&tab].loading,
            ferric_browser_core::LoadingState::Provisional
        );
        assert!(application.pending_lazy_restore_pages.is_empty());
        let repeated = application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::ActivateTab { window, tab }))
            .expect("reselect restored tab");
        assert!(!repeated.iter().any(|effect| matches!(
            effect,
            RuntimeEffect::AppliedEvent(Event::StartNavigation { .. })
        )));
    }

    #[test]
    fn duplicate_urls_receive_distinct_fresh_tabs_and_selected_first_navigation() {
        let (mut application, roots, window, mut manifest) = fixture();
        let source_tab_ids = application.windows()[&window].tabs.clone();
        for page in &mut manifest.windows[0].pages {
            page.safe_url_or_placeholder =
                ManifestSafeUrl::Replayable("https://example.test/shared".into());
        }
        let before_revision = application.runtime_view().revision();
        let before_windows = application.windows().len();
        let plan = application
            .plan_new_window_restore(window, &manifest)
            .expect("safe plan");
        assert_eq!(plan.pages.len(), 2);
        assert_ne!(plan.pages[0].runtime_tab, plan.pages[1].runtime_tab);
        assert!(
            plan.pages
                .iter()
                .all(|page| !source_tab_ids.contains(&page.runtime_tab))
        );
        assert_ne!(plan.runtime_window, window);
        assert_eq!(plan.navigation_order[0], plan.selected_tab);
        assert_eq!(plan.admitted_revision, before_revision);
        assert_eq!(application.runtime_view().revision(), before_revision);
        assert_eq!(application.windows().len(), before_windows);
        assert!(!format!("{plan:?}").contains("example.test"));
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CreateWindow {
                profile: plan.runtime_profile,
            }))
            .expect("ordinary window allocation");
        assert_eq!(
            application.runtime_view().active_window(),
            Some(plan.runtime_window)
        );
        for page in &plan.pages {
            application
                .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab {
                    window: plan.runtime_window,
                }))
                .expect("ordinary tab allocation");
            assert_eq!(
                application.windows()[&plan.runtime_window].active_tab,
                Some(page.runtime_tab)
            );
        }
        application.close_profile();
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn unsafe_post_placeholders_never_produce_navigation_or_history_events() {
        let (mut application, roots, window, mut manifest) = fixture();
        manifest.windows[0].pages[0].safe_url_or_placeholder =
            ManifestSafeUrl::Placeholder(ManifestUnavailable::UnsafeNavigation);
        manifest.windows[0].pages[1].safe_url_or_placeholder =
            ManifestSafeUrl::Replayable("https://example.test/safe".into());
        let plan = application
            .plan_new_window_restore(window, &manifest)
            .expect("placeholder plan");
        assert!(matches!(
            plan.pages[0].navigation,
            RestoreNavigation::Placeholder(ManifestUnavailable::UnsafeNavigation)
        ));
        assert!(matches!(
            plan.pages[1].navigation,
            RestoreNavigation::SafeGet(_)
        ));
        assert_eq!(application.windows().len(), 1);
        assert_eq!(
            application.runtime_view().revision(),
            plan.admitted_revision
        );
        application.close_profile();
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn invalid_ids_owner_context_and_replay_url_are_refused() {
        let (mut application, roots, window, manifest) = fixture();
        let mut invalid = manifest.clone();
        invalid.windows[0].pages[0].logical_page_id = "not-an-id".into();
        assert_eq!(
            application
                .plan_new_window_restore(window, &invalid)
                .unwrap_err()
                .reason(),
            VersioningReason::UnsafeRestoreDescriptor
        );
        let mut foreign = manifest.clone();
        foreign.profile_uuid = Uuid::new_v4();
        assert_eq!(
            application
                .plan_new_window_restore(window, &foreign)
                .unwrap_err()
                .reason(),
            VersioningReason::WrongProfile
        );
        let mut context = manifest.clone();
        context.windows[0].context_ref = Some("work".into());
        assert_eq!(
            application
                .plan_new_window_restore(window, &context)
                .unwrap_err()
                .reason(),
            VersioningReason::UnsupportedCapability
        );
        let mut unsafe_url = manifest;
        unsafe_url.windows[0].pages[0].safe_url_or_placeholder =
            ManifestSafeUrl::Replayable("https://example.test/form?token=secret".into());
        assert_eq!(
            application
                .plan_new_window_restore(window, &unsafe_url)
                .unwrap_err()
                .reason(),
            VersioningReason::UnsafeRestoreDescriptor
        );
        application.close_profile();
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn missing_durable_profile_refuses_even_a_valid_manifest() {
        let (mut application, roots, window, manifest) = fixture();
        application.close_profile();
        assert!(
            application
                .plan_new_window_restore(window, &manifest)
                .is_err()
        );
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn exact_durable_commit_is_read_and_planned_after_worker_publication() {
        let (mut application, roots, window, _) = fixture();
        let (operation, commit, _) = application
            .begin_structural_commit(ScopeSelector::Window { window }, "durable".into())
            .expect("capture admitted");
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let _ = application.poll_storage_effects();
            if let Some(event) = application.poll_commit_events().into_iter().next() {
                assert_eq!(
                    event,
                    super::super::CommitEvent::Durable { operation, commit }
                );
                break;
            }
            assert!(Instant::now() < deadline, "commit was not published");
            thread::yield_now();
        }
        let ticket = application
            .request_new_window_restore_plan(window, commit)
            .expect("manifest read admitted");
        let deadline = Instant::now() + Duration::from_secs(3);
        let plan = loop {
            let _ = application.poll_storage_effects();
            if let Some(event) = application.poll_restore_plan_events().into_iter().next() {
                let RestorePlanEvent::Ready { request_id, plan } = event else {
                    panic!("durable plan was not ready");
                };
                assert_eq!(request_id, ticket.value());
                break plan;
            }
            assert!(Instant::now() < deadline, "manifest was not read");
            thread::yield_now();
        };
        assert_eq!(plan.commit, commit);
        assert_eq!(plan.pages.len(), 2);
        assert!(plan.pages.iter().all(|page| matches!(
            page.navigation,
            RestoreNavigation::Placeholder(ManifestUnavailable::NotObserved)
        )));
        assert_eq!(application.windows().len(), 1);
        application.close_profile();
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn execution_opens_new_reducer_window_and_navigates_only_selected_safe_page() {
        let (mut application, roots, window, mut manifest) = fixture();
        let original_tabs = application.windows()[&window].tabs.clone();
        let selected = manifest.windows[0]
            .selected_logical_page
            .clone()
            .expect("selected page");
        for page in &mut manifest.windows[0].pages {
            if page.logical_page_id == selected {
                page.safe_url_or_placeholder =
                    ManifestSafeUrl::Replayable("https://example.test/selected".into());
            } else {
                page.safe_url_or_placeholder =
                    ManifestSafeUrl::Placeholder(ManifestUnavailable::UnsafeNavigation);
            }
        }
        let plan = application
            .plan_new_window_restore(window, &manifest)
            .expect("safe plan");
        let expected_window = plan.runtime_window;
        let expected_selected = plan.selected_tab;
        let applied = application
            .execute_new_window_restore(plan)
            .expect("new reducer window");
        assert_eq!(application.windows()[&window].tabs, original_tabs);
        assert_eq!(
            application.windows()[&expected_window].active_tab,
            Some(expected_selected)
        );
        assert_eq!(application.windows()[&expected_window].tabs.len(), 2);
        assert_eq!(
            application.runtime_view().logical_page(expected_selected),
            Some(
                applied
                    .plan
                    .pages
                    .iter()
                    .find(|page| page.runtime_tab == expected_selected)
                    .unwrap()
                    .logical_page
            )
        );
        assert_eq!(
            applied
                .runtime_effects
                .iter()
                .filter(|effect| matches!(
                    effect,
                    RuntimeEffect::AppliedEvent(Event::StartNavigation { .. })
                ))
                .count(),
            1
        );
        assert!(!applied.runtime_effects.iter().any(|effect| matches!(
            effect,
            RuntimeEffect::AppliedEvent(
                Event::CommitNavigation { .. } | Event::CommitNavigationWithTransition { .. }
            )
        )));
        let inactive = applied
            .plan
            .pages
            .iter()
            .find(|page| page.runtime_tab != expected_selected)
            .unwrap();
        assert_eq!(application.tabs()[&inactive.runtime_tab].url, None);
        let next_capture = application
            .prepare_structural_commit(
                ScopeSelector::Window {
                    window: expected_window,
                },
                "after restore".into(),
            )
            .expect("restored lineage capture");
        for (stored, planned) in next_capture.manifest.windows[0]
            .pages
            .iter()
            .zip(&applied.plan.pages)
        {
            assert_eq!(stored.logical_page_id, planned.logical_page.to_string());
            assert_eq!(
                stored.lineage_parent,
                Some(planned.source_logical_page.to_string())
            );
        }
        application.close_profile();
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn stale_or_modified_plan_cannot_replace_existing_work() {
        let (mut application, roots, window, mut manifest) = fixture();
        manifest.windows[0].pages[0].safe_url_or_placeholder =
            ManifestSafeUrl::Replayable("https://example.test/first".into());
        let original_tabs = application.windows()[&window].tabs.clone();
        let mut modified = application
            .plan_new_window_restore(window, &manifest)
            .expect("plan");
        modified.pages[0].navigation =
            RestoreNavigation::SafeGet("https://example.test/?token=secret".into());
        assert_eq!(
            application
                .execute_new_window_restore(modified)
                .unwrap_err()
                .reason(),
            VersioningReason::UnsafeRestoreDescriptor
        );
        assert_eq!(application.windows().len(), 1);
        assert_eq!(
            application.windows()[&window].tabs.as_slice(),
            original_tabs.as_slice()
        );
        let mut substituted_safe_url = application
            .plan_new_window_restore(window, &manifest)
            .expect("fresh plan for URL substitution");
        let RestoreNavigation::SafeGet(url) = &mut substituted_safe_url.pages[0].navigation else {
            panic!("fixture page should be replayable");
        };
        *url = "https://example.test/a-different-safe-route".into();
        assert_eq!(
            application
                .execute_new_window_restore(substituted_safe_url)
                .unwrap_err()
                .reason(),
            VersioningReason::UnsafeRestoreDescriptor
        );
        assert_eq!(application.windows().len(), 1);
        assert_eq!(
            application.windows()[&window].tabs.as_slice(),
            original_tabs.as_slice()
        );
        let mut changed_metadata = application
            .plan_new_window_restore(window, &manifest)
            .expect("fresh plan for metadata mutation");
        changed_metadata.pages[0].muted = !changed_metadata.pages[0].muted;
        assert_eq!(
            application
                .execute_new_window_restore(changed_metadata)
                .unwrap_err()
                .reason(),
            VersioningReason::UnsafeRestoreDescriptor
        );
        assert_eq!(application.windows().len(), 1);
        assert_eq!(
            application.windows()[&window].tabs.as_slice(),
            original_tabs.as_slice()
        );
        let mut changed_pinned = application
            .plan_new_window_restore(window, &manifest)
            .expect("fresh plan for pinned metadata mutation");
        changed_pinned.pages[0].pinned = !changed_pinned.pages[0].pinned;
        assert_eq!(
            application
                .execute_new_window_restore(changed_pinned)
                .unwrap_err()
                .reason(),
            VersioningReason::UnsafeRestoreDescriptor
        );
        assert_eq!(application.windows().len(), 1);
        assert_eq!(
            application.windows()[&window].tabs.as_slice(),
            original_tabs.as_slice()
        );
        let mut changed_zoom = application
            .plan_new_window_restore(window, &manifest)
            .expect("fresh plan for zoom metadata mutation");
        changed_zoom.pages[0].zoom_hundredths ^= 1;
        assert_eq!(
            application
                .execute_new_window_restore(changed_zoom)
                .unwrap_err()
                .reason(),
            VersioningReason::UnsafeRestoreDescriptor
        );
        assert_eq!(application.windows().len(), 1);
        assert_eq!(
            application.windows()[&window].tabs.as_slice(),
            original_tabs.as_slice()
        );
        let mut changed_order = application
            .plan_new_window_restore(window, &manifest)
            .expect("fresh plan for navigation-order mutation");
        let selected_tab = changed_order.selected_tab;
        changed_order.navigation_order.push(selected_tab);
        assert_eq!(
            application
                .execute_new_window_restore(changed_order)
                .unwrap_err()
                .reason(),
            VersioningReason::UnsafeRestoreDescriptor
        );
        assert_eq!(application.windows().len(), 1);
        assert_eq!(
            application.windows()[&window].tabs.as_slice(),
            original_tabs.as_slice()
        );
        let stale = application
            .plan_new_window_restore(window, &manifest)
            .expect("fresh plan");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab { window }))
            .expect("later mutation");
        assert_eq!(
            application
                .execute_new_window_restore(stale)
                .unwrap_err()
                .reason(),
            VersioningReason::StaleOwner
        );
        assert_eq!(application.windows().len(), 1);
        assert_eq!(
            &application.windows()[&window].tabs[..original_tabs.len()],
            original_tabs
        );
        application.close_profile();
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn pristine_destination_applies_one_page_without_touching_source_window() {
        let (mut application, roots, source_window, mut manifest) = fixture();
        let original_tabs = application.windows()[&source_window].tabs.clone();
        let selected = manifest.windows[0]
            .selected_logical_page
            .clone()
            .expect("selected page");
        manifest.windows[0]
            .pages
            .retain(|page| page.logical_page_id == selected);
        manifest.scope = ManifestScope::Tab;
        manifest.windows[0].pages[0].safe_url_or_placeholder =
            ManifestSafeUrl::Replayable("https://example.test/safe".into());
        let profile = application.windows()[&source_window].profile;
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CreateWindow { profile }))
            .expect("fresh destination");
        let destination = application.runtime_view().active_window().unwrap();
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab {
                window: destination,
            }))
            .expect("blank destination tab");
        let plan = application
            .plan_new_window_restore(destination, &manifest)
            .expect("destination plan");
        let target = application.windows()[&destination].active_tab.unwrap();
        let effects = application
            .apply_single_page_restore_to_pristine_window(&plan, false, false)
            .expect("one-page restore");
        assert_eq!(application.windows()[&source_window].tabs, original_tabs);
        assert_eq!(application.windows()[&destination].tabs, vec![target]);
        assert_eq!(
            application.runtime_view().logical_page(target),
            Some(plan.pages[0].logical_page)
        );
        assert_eq!(
            application.runtime_view().lineage_parent(target),
            Some(plan.pages[0].source_logical_page)
        );
        assert_eq!(
            effects
                .iter()
                .filter(|effect| matches!(
                    effect,
                    RuntimeEffect::AppliedEvent(Event::StartNavigation { .. })
                ))
                .count(),
            1
        );
        application.close_profile();
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn ordinary_restore_preserves_current_policy_and_permissions() {
        let (mut application, roots, source_window, mut manifest) = fixture();
        let database_path = roots
            .data
            .join("profiles")
            .join(manifest.profile_uuid.to_string())
            .join("browser.sqlite");
        // Profile activation starts its storage worker asynchronously. Wait
        // until that worker finishes schema creation before opening a second
        // read/write fixture connection to the same disposable profile.
        let deadline = Instant::now() + Duration::from_secs(3);
        let permission_store = loop {
            match ProfileStore::open(&database_path, StoreMode::Normal) {
                Ok(store) => break store,
                Err(error) => {
                    assert!(Instant::now() < deadline, "permission store: {error}");
                    thread::sleep(Duration::from_millis(10));
                }
            }
        };
        permission_store
            .set_permission_rule("https://example.test", "notifications", "allow", None, 1)
            .expect("current permission grant");
        let original_permissions = permission_store
            .permission_rules(None)
            .expect("current permission rules");
        manifest.policy_snapshot = Some(application.allowlisted_policy_snapshot());
        assert!(
            manifest
                .policy_snapshot
                .as_ref()
                .expect("captured policy")
                .javascript_enabled
        );
        let mut disabled = RuntimeOverrides::default();
        disabled
            .set_setting("content.javascript", toml::Value::Boolean(false))
            .expect("valid current setting");
        application
            .apply_configuration_layer(&disabled)
            .expect("change current policy");
        let current_revision = application.configuration_revision();
        assert!(!application.configuration().content.javascript);

        let profile = application.windows()[&source_window].profile;
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CreateWindow { profile }))
            .expect("fresh destination window");
        let destination = application.runtime_view().active_window().unwrap();
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab {
                window: destination,
            }))
            .expect("blank destination tab");
        let plan = application
            .plan_new_window_restore(destination, &manifest)
            .expect("ordinary structural plan");
        application
            .apply_restore_to_pristine_window_with_options(&plan, false, false)
            .expect("ordinary structural restore");
        assert!(!application.configuration().content.javascript);
        assert_eq!(application.configuration_revision(), current_revision);
        assert_eq!(
            permission_store
                .permission_rules(None)
                .expect("rules after ordinary restore"),
            original_permissions
        );

        drop(permission_store);
        application.close_profile();
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn pristine_destination_applies_all_pages_in_order_and_navigates_only_selected() {
        let (mut application, roots, source_window, mut manifest) = fixture();
        let original_source_tabs = application.windows()[&source_window].tabs.clone();
        let original_source_pages = original_source_tabs
            .iter()
            .map(|tab| application.runtime_view().logical_page(*tab))
            .collect::<Vec<_>>();
        manifest.windows[0].pages[0].safe_url_or_placeholder =
            ManifestSafeUrl::Replayable("https://example.test/first".into());
        manifest.windows[0].pages[1].safe_url_or_placeholder =
            ManifestSafeUrl::Replayable("https://example.test/second".into());

        let profile = application.windows()[&source_window].profile;
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CreateWindow { profile }))
            .expect("fresh destination window");
        let destination_window = application.runtime_view().active_window().unwrap();
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab {
                window: destination_window,
            }))
            .expect("initial blank destination tab");
        let initial_destination_tab = application.windows()[&destination_window].tabs[0];
        let plan = application
            .plan_new_window_restore(destination_window, &manifest)
            .expect("multi-page plan");
        let selected_index = plan
            .pages
            .iter()
            .position(|page| page.runtime_tab == plan.selected_tab)
            .unwrap();

        let applied = application
            .apply_restore_to_pristine_window_with_options(&plan, false, false)
            .expect("materialize into blank tab-capable destination");
        let restored_tabs = &applied.destination_tabs;
        assert_eq!(applied.destination_window, destination_window);
        assert_eq!(restored_tabs.len(), 2);
        assert_eq!(restored_tabs[0], initial_destination_tab);
        assert_ne!(restored_tabs[0], restored_tabs[1]);
        assert_eq!(
            application.windows()[&destination_window].tabs,
            *restored_tabs
        );
        assert_eq!(
            application.windows()[&destination_window].active_tab,
            Some(restored_tabs[selected_index])
        );
        for (index, (page, tab)) in plan.pages.iter().zip(restored_tabs).enumerate() {
            assert_eq!(
                application.runtime_view().logical_page(*tab),
                Some(page.logical_page),
                "destination page order changed at {index}"
            );
            assert_eq!(
                application.runtime_view().lineage_parent(*tab),
                Some(page.source_logical_page)
            );
        }
        let inactive_tab = restored_tabs[1 - selected_index];
        assert_eq!(
            application.tabs()[&inactive_tab].url,
            None,
            "inactive restored page must remain lazy"
        );
        assert_eq!(application.pending_lazy_restore_pages.len(), 1);
        assert!(application.has_lazy_restore_page(inactive_tab));
        assert_eq!(
            applied
                .runtime_effects
                .iter()
                .filter(|effect| matches!(
                    effect,
                    RuntimeEffect::AppliedEvent(Event::StartNavigation { .. })
                ))
                .count(),
            1
        );
        assert_eq!(
            application.windows()[&source_window].tabs,
            original_source_tabs
        );
        assert_eq!(
            original_source_tabs
                .iter()
                .map(|tab| application.runtime_view().logical_page(*tab))
                .collect::<Vec<_>>(),
            original_source_pages
        );
        assert_lazy_activation_navigates_once(
            &mut application,
            destination_window,
            inactive_tab,
            "https://example.test/first",
        );
        assert!(!application.has_lazy_restore_page(inactive_tab));
        application.close_profile();
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn lazy_restore_capacity_refuses_before_mutating_pristine_destination() {
        let (mut application, roots, source_window, mut manifest) = fixture();
        for (index, page) in manifest.windows[0].pages.iter_mut().enumerate() {
            page.safe_url_or_placeholder =
                ManifestSafeUrl::Replayable(format!("https://example.test/page-{index}"));
        }
        let profile = application.windows()[&source_window].profile;
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CreateWindow { profile }))
            .expect("fresh destination window");
        let window = application.runtime_view().active_window().unwrap();
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab { window }))
            .expect("blank destination tab");
        let blank_tab = application.windows()[&window].tabs[0];
        let plan = application
            .plan_new_window_restore(window, &manifest)
            .expect("two-page safe plan");
        let (profile_uuid, native_owner) = application.structural_commit_owner(window).unwrap();
        let seed = PendingLazyRestorePage {
            window,
            destination_tab: blank_tab,
            destination_target: application.capture_target(blank_tab).unwrap(),
            logical_page: plan.pages[0].logical_page,
            source_page: plan.pages[0].source_logical_page,
            profile_uuid,
            native_owner,
            url: "https://example.test/pending".into(),
        };
        for raw in 10_000..10_128 {
            let tab = TabId::from_display(&format!("tabid-{raw}")).unwrap();
            application
                .pending_lazy_restore_pages
                .insert(tab, seed.clone());
        }
        let revision = application.runtime_view().revision();
        assert_eq!(
            application
                .apply_restore_to_pristine_window(&plan)
                .expect_err("capacity must be checked before tab allocation")
                .reason(),
            VersioningReason::QuotaExceeded
        );
        assert_eq!(application.runtime_view().revision(), revision);
        assert_eq!(application.windows()[&window].tabs, vec![blank_tab]);

        application.pending_lazy_restore_pages.clear();
        assert!(application.apply_restore_to_pristine_window(&plan).is_ok());
        application.close_profile();
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn lazy_restored_page_cannot_replace_an_intervening_navigation() {
        let (mut application, roots, source_window, mut manifest) = fixture();
        manifest.windows[0].pages[0].safe_url_or_placeholder =
            ManifestSafeUrl::Replayable("https://example.test/first".into());
        manifest.windows[0].pages[1].safe_url_or_placeholder =
            ManifestSafeUrl::Replayable("https://example.test/second".into());
        let profile = application.windows()[&source_window].profile;
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CreateWindow { profile }))
            .expect("fresh destination window");
        let window = application.runtime_view().active_window().unwrap();
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab { window }))
            .expect("blank destination tab");
        let plan = application
            .plan_new_window_restore(window, &manifest)
            .expect("safe restore plan");
        let applied = application
            .apply_restore_to_pristine_window_with_options(&plan, false, false)
            .expect("restore pages");
        let inactive = applied
            .destination_tabs
            .into_iter()
            .find(|tab| *tab != plan.selected_tab)
            .expect("inactive restored tab");
        assert_eq!(application.pending_lazy_restore_pages.len(), 1);

        let target = application.capture_target(inactive).expect("blank target");
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::StartNavigation {
                target,
                url: ValidatedUrl::parse("https://example.test/user-choice").unwrap(),
            }))
            .expect("independent navigation");
        assert!(application.pending_lazy_restore_pages.is_empty());
        let activated = application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::ActivateTab {
                window,
                tab: inactive,
            }))
            .expect("activate independently navigated tab");
        assert!(
            !activated.iter().any(|effect| matches!(
                effect,
                RuntimeEffect::Engine(EngineEffect::Navigate { .. })
            ))
        );
        application.close_profile();
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn lazy_restored_page_is_discarded_on_native_owner_rebind() {
        let (mut application, roots, source_window, mut manifest) = fixture();
        manifest.windows[0].pages[0].safe_url_or_placeholder =
            ManifestSafeUrl::Replayable("https://example.test/first".into());
        manifest.windows[0].pages[1].safe_url_or_placeholder =
            ManifestSafeUrl::Replayable("https://example.test/second".into());
        let profile = application.windows()[&source_window].profile;
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CreateWindow { profile }))
            .expect("fresh destination window");
        let window = application.runtime_view().active_window().unwrap();
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab { window }))
            .expect("blank destination tab");
        let plan = application
            .plan_new_window_restore(window, &manifest)
            .expect("safe restore plan");
        let applied = application
            .apply_restore_to_pristine_window_with_options(&plan, false, false)
            .expect("restore pages");
        let inactive = applied
            .destination_tabs
            .into_iter()
            .find(|tab| *tab != plan.selected_tab)
            .expect("inactive restored tab");
        assert_eq!(application.pending_lazy_restore_pages.len(), 1);

        application
            .bind_native_profile(profile, ProfileInstanceId::fresh())
            .expect("replace native owner");
        assert!(application.pending_lazy_restore_pages.is_empty());
        let activated = application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::ActivateTab {
                window,
                tab: inactive,
            }))
            .expect("activate after owner rebind");
        assert!(
            !activated.iter().any(|effect| matches!(
                effect,
                RuntimeEffect::Engine(EngineEffect::Navigate { .. })
            ))
        );
        application.close_profile();
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn late_restore_plan_results_fail_after_native_owner_rebind() {
        let (mut application, roots, window, manifest) = fixture();
        let stale_plan = application
            .plan_new_window_restore(window, &manifest)
            .expect("validated plan captures the exact source owner");
        let private_ids = [
            stale_plan.commit.to_string(),
            stale_plan.source_window.to_string(),
            stale_plan.source_logical_window.to_string(),
            stale_plan.logical_window.to_string(),
            stale_plan.runtime_window.to_string(),
            stale_plan.selected_tab.to_string(),
        ];
        let plan_debug = format!("{stale_plan:?}");
        assert!(plan_debug.contains("[native owner]"));
        assert!(!plan_debug.contains("profile_instance"));
        assert!(!plan_debug.contains(stale_plan.owner_profile.to_string().as_str()));
        assert!(!plan_debug.contains(stale_plan.runtime_profile.to_string().as_str()));
        for secret in &private_ids {
            assert!(!plan_debug.contains(secret.as_str()));
        }
        let restore_ticket = application
            .request_new_window_restore_plan(window, CommitId::fresh())
            .expect("restore plan request");
        let policy_ticket = application
            .request_policy_restore_preview(window, CommitId::fresh())
            .expect("policy preview request");
        let profile = application.windows()[&window].profile;
        application
            .bind_native_profile(profile, ProfileInstanceId::fresh())
            .expect("new native owner generation");
        assert!(application.execute_new_window_restore(stale_plan).is_err());
        assert_eq!(application.windows().len(), 1);

        let forwarded = application.route_restore_storage_effects(vec![
            crate::StorageEffect {
                ticket: restore_ticket,
                completion: StorageCompletion::VersioningManifest(Ok(None)),
            },
            crate::StorageEffect {
                ticket: policy_ticket,
                completion: StorageCompletion::VersioningManifest(Ok(None)),
            },
        ]);
        assert!(forwarded.is_empty());
        assert_eq!(
            application.poll_restore_plan_events(),
            vec![RestorePlanEvent::Failed {
                request_id: restore_ticket.value(),
            }]
        );
        assert_eq!(
            application.poll_policy_restore_events(),
            vec![crate::versioning::PolicyRestoreEvent::Failed {
                request_id: policy_ticket.value(),
            }]
        );

        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn late_restore_sidecar_result_fails_after_native_owner_rebind() {
        let (mut application, roots, window, _) = fixture();
        let (profile_uuid, native_owner) = application
            .structural_commit_owner(window)
            .expect("current native owner");
        let destination_tab = application.windows()[&window].tabs[0];
        let destination_target = application
            .capture_target(destination_tab)
            .expect("live source tab target");
        let commit = CommitId::fresh();
        let source_page = LogicalPageId::fresh();
        let state_root = application
            .profile_snapshot()
            .and_then(|profile| profile.roots)
            .map(|roots| roots.state)
            .expect("durable sidecar state root");
        let ticket = application
            .storage
            .submit(StorageCommand::VersioningSidecarRead {
                profile_uuid,
                state_root,
                id: commit,
                logical_page: source_page,
            })
            .expect("ticket for exact-sidecar callback");
        application.pending_restore_sidecars.insert(
            ticket,
            PendingRestoreSidecar {
                window,
                profile_uuid,
                native_owner,
                commit,
                source_page,
                destination_tab,
                destination_target,
                destination_url: "https://example.test/restored".into(),
                allow_media_restore: false,
                allow_editable_restore: false,
            },
        );
        let profile = application.windows()[&window].profile;
        application
            .bind_native_profile(profile, ProfileInstanceId::fresh())
            .expect("new native owner generation");

        let forwarded = application.route_restore_storage_effects(vec![crate::StorageEffect {
            ticket,
            completion: StorageCompletion::VersioningSidecarRead(Ok(None)),
        }]);
        assert!(forwarded.is_empty());
        assert_eq!(
            application.poll_restore_sidecar_events(),
            vec![RestoreSidecarEvent::Failed {
                request_id: ticket.value(),
            }]
        );
        assert!(application.restore_sidecars.is_empty());

        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn post_commit_restore_sidecar_read_rejects_same_document_change() {
        let (mut application, roots, source_window, mut manifest) = fixture();
        let selected = manifest.windows[0]
            .selected_logical_page
            .clone()
            .expect("selected source page");
        manifest.windows[0]
            .pages
            .retain(|page| page.logical_page_id == selected);
        manifest.windows[0].pages[0].safe_url_or_placeholder =
            ManifestSafeUrl::Replayable("https://example.test/restored".into());

        let profile = application.windows()[&source_window].profile;
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CreateWindow { profile }))
            .expect("fresh pristine destination");
        let destination_window = application.runtime_view().active_window().unwrap();
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab {
                window: destination_window,
            }))
            .expect("blank destination tab");
        let plan = application
            .plan_new_window_restore(destination_window, &manifest)
            .expect("safe restore plan");
        let applied = application
            .apply_restore_to_pristine_window_with_options(&plan, false, true)
            .expect("restore selected page");
        let destination_tab = applied.destination_tabs[0];
        let destination_target = application
            .capture_target(destination_tab)
            .expect("restore navigation document");
        assert!(application.pending_restore_sidecars.is_empty());
        assert!(
            application
                .pending_restore_sidecar_intents
                .contains_key(&destination_tab)
        );

        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::SameDocumentNavigation {
                target: destination_target,
                url: ValidatedUrl::parse("https://example.test/restored")
                    .expect("provisional URL update"),
            }))
            .expect("Qt reports the exact provisional URL before load success");
        assert!(
            application
                .pending_restore_sidecar_intents
                .contains_key(&destination_tab),
            "the exact provisional URL must not discard the pending sidecar"
        );

        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CommitNavigation {
                target: destination_target,
                url: ValidatedUrl::parse("https://example.test/restored")
                    .expect("validated restore URL"),
                title: "Restored".into(),
            }))
            .expect("exact selected navigation commits");
        assert!(
            !application
                .pending_restore_sidecar_intents
                .contains_key(&destination_tab)
        );
        assert_eq!(application.pending_restore_sidecars.len(), 1);
        let (ticket, _) = application
            .pending_restore_sidecars
            .iter()
            .next()
            .expect("post-commit sidecar read");
        let ticket = *ticket;

        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::SameDocumentNavigation {
                target: destination_target,
                url: ValidatedUrl::parse("https://example.test/restored#changed")
                    .expect("post-load same-document URL change"),
            }))
            .expect("same-document mutation after load");
        let forwarded = application.route_restore_storage_effects(vec![crate::StorageEffect {
            ticket,
            completion: StorageCompletion::VersioningSidecarRead(Ok(None)),
        }]);
        assert!(forwarded.is_empty());
        assert_eq!(
            application.poll_restore_sidecar_events(),
            vec![RestoreSidecarEvent::Failed {
                request_id: ticket.value(),
            }]
        );

        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn restore_sidecar_intent_rejects_a_different_provisional_route() {
        let (mut application, roots, source_window, mut manifest) = fixture();
        let selected = manifest.windows[0]
            .selected_logical_page
            .clone()
            .expect("selected source page");
        manifest.windows[0]
            .pages
            .retain(|page| page.logical_page_id == selected);
        manifest.windows[0].pages[0].safe_url_or_placeholder =
            ManifestSafeUrl::Replayable("https://example.test/restored".into());

        let profile = application.windows()[&source_window].profile;
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CreateWindow { profile }))
            .expect("fresh destination window");
        let destination_window = application.runtime_view().active_window().unwrap();
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab {
                window: destination_window,
            }))
            .expect("blank destination tab");
        let plan = application
            .plan_new_window_restore(destination_window, &manifest)
            .expect("safe restore plan");
        let applied = application
            .apply_restore_to_pristine_window_with_options(&plan, false, false)
            .expect("restore selected page");
        let destination_tab = applied.destination_tabs[0];
        let destination_target = application
            .capture_target(destination_tab)
            .expect("restore navigation document");
        assert!(
            application
                .pending_restore_sidecar_intents
                .contains_key(&destination_tab)
        );

        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::SameDocumentNavigation {
                target: destination_target,
                url: ValidatedUrl::parse("https://example.test/other")
                    .expect("changed provisional route"),
            }))
            .expect("changed provisional URL");
        assert!(
            !application
                .pending_restore_sidecar_intents
                .contains_key(&destination_tab)
        );
        application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::CommitNavigation {
                target: destination_target,
                url: ValidatedUrl::parse("https://example.test/restored")
                    .expect("original restore URL"),
                title: "Restored".into(),
            }))
            .expect("later matching URL cannot revive the stale intent");
        assert!(application.pending_restore_sidecars.is_empty());

        application.close_profile();
        drop(application);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    fn default_restore_command_only_queues_validated_source_plan() {
        let (mut application, roots, window, _) = fixture();
        let (operation, commit, _) = application
            .begin_structural_commit(ScopeSelector::Window { window }, "before".into())
            .expect("commit admission");
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let _ = application.poll_storage_effects();
            if let Some(event) = application.poll_commit_events().into_iter().next() {
                assert_eq!(
                    event,
                    super::super::CommitEvent::Durable { operation, commit }
                );
                break;
            }
            assert!(Instant::now() < deadline, "commit publication timed out");
            thread::yield_now();
        }
        let original_windows = application.windows().len();
        let context = application.capture_command_context(
            DispatchTarget::Window(window),
            CommandSource::Keyboard,
            1,
            None,
        );
        let effects = application
            .dispatch_runtime(RuntimeInput::Command(RuntimeCommand::Dispatch(Box::new(
                RuntimeDispatch {
                    invocation: CommandInvocation::new(ParsedCommand {
                        name: "restore".into(),
                        arguments: vec![commit.to_string()],
                    })
                    .with_context(context),
                    navigation: NavigationContext::default(),
                    target: DispatchTarget::Window(window),
                },
            ))))
            .expect("restore route");
        let request_id = effects
            .iter()
            .find_map(|effect| match effect {
                RuntimeEffect::Engine(EngineEffect::Versioning(
                    VersioningEngineEffect::RestoreQueued { request_id },
                )) => Some(*request_id),
                _ => None,
            })
            .expect("pending restore ticket");
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let _ = application.poll_storage_effects();
            if let Some(event) = application.poll_restore_plan_events().into_iter().next() {
                let RestorePlanEvent::Ready {
                    request_id: ready,
                    plan,
                } = event
                else {
                    panic!("default restore was not planned");
                };
                assert_eq!(ready, request_id);
                assert_eq!(plan.commit, commit);
                break;
            }
            assert!(Instant::now() < deadline, "restore plan timed out");
            thread::yield_now();
        }
        assert_eq!(application.windows().len(), original_windows);
        application.close_profile();
        roots.cleanup().expect("temporary roots cleanup");
    }
}

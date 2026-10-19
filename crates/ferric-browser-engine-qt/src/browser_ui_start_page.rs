//! Native start-page requests and typed presentation projections.

use super::{
    CxxQtType, Event, Pin, QString, StorageRequest, Uuid, ValidatedUrl, Value, qobject,
    unix_timestamp,
};
use ferric_browser_core::InternalPage;
use ferric_browser_storage::{
    StartPageMutation, StartPageSnapshot, StartPageTile, StartPageView, StorageOperationError,
    site_key,
};
use std::hash::{Hash, Hasher};

#[derive(Default)]
pub(super) struct PrivateStartState {
    pub(super) snapshot: StartPageSnapshot,
    pub(super) visits: Vec<ferric_browser_storage::VisitInput>,
    pub(super) icon_directory: Option<std::path::PathBuf>,
}
impl Drop for PrivateStartState {
    fn drop(&mut self) {
        if let Some(path) = self.icon_directory.take() {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}

type PrivateOwnerMap =
    std::collections::BTreeMap<String, std::sync::Weak<std::sync::Mutex<PrivateStartState>>>;
static PRIVATE_OWNERS: std::sync::OnceLock<std::sync::Mutex<PrivateOwnerMap>> =
    std::sync::OnceLock::new();

fn parse_view(value: &str) -> Option<StartPageView> {
    match value {
        "top-sites" => Some(StartPageView::TopSites),
        "favourites" => Some(StartPageView::Favourites),
        "guide" => Some(StartPageView::Guide),
        _ => None,
    }
}

impl qobject::BrowserUi {
    pub(super) fn set_private_start_owner(mut self: Pin<&mut Self>, token: &QString) {
        if !self.as_ref().active_profile_is_transient() {
            return;
        }
        let token = token.to_string();
        if token.is_empty() || token.len() > 256 {
            return;
        }
        let Ok(mut owners) = PRIVATE_OWNERS.get_or_init(Default::default).lock() else {
            return;
        };
        owners.retain(|_, owner| owner.strong_count() > 0);
        let state = owners
            .get(&token)
            .and_then(std::sync::Weak::upgrade)
            .unwrap_or_else(|| {
                let state = std::sync::Arc::clone(&self.as_ref().rust().private_start_state);
                owners.insert(token.clone(), std::sync::Arc::downgrade(&state));
                state
            });
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .private_start_state = state;
        self.as_mut()
            .set_start_page_owner_token(QString::from(token));
        self.refresh_start_page_metadata();
    }

    pub(super) fn refresh_start_page_metadata(mut self: Pin<&mut Self>) {
        if self.as_ref().active_profile_is_transient() {
            let snapshot = self
                .as_ref()
                .rust()
                .private_start_state
                .lock()
                .ok()
                .map(|state| state.snapshot.clone());
            if let Some(snapshot) = snapshot {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .library
                    .start_snapshot = snapshot;
                self.as_mut().set_start_page_ready(true);
                self.publish_start_page();
            }
        } else {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .library
                .start_refresh_requested = true;
            self.flush_start_page_refresh();
        }
    }

    pub(super) fn publish_start_page(mut self: Pin<&mut Self>) {
        let snapshot = self.as_ref().rust().library.start_snapshot.clone();
        if self.as_ref().active_profile_is_transient() {
            if let Ok(mut state) = self.as_ref().rust().private_start_state.lock() {
                state.snapshot = snapshot.clone();
            }
        }
        self.as_mut().set_start_page_eligible_sites(
            snapshot
                .eligible_sites
                .iter()
                .map(|site| QString::from(site))
                .collect(),
        );
        self.as_mut().set_start_page_eligible_urls(
            snapshot
                .eligible_urls
                .iter()
                .map(|url| QString::from(url))
                .collect(),
        );
        let mut rows: Vec<(&str, usize, StartPageTile)> = snapshot
            .top_sites
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, tile)| ("top", index + 1, tile))
            .collect();
        rows.extend(
            snapshot
                .favourites
                .iter()
                .enumerate()
                .filter_map(|(index, tile)| {
                    tile.clone().map(|tile| ("favourite", index + 1, tile))
                }),
        );
        rows.extend(
            snapshot
                .bookmarks
                .iter()
                .cloned()
                .map(|tile| ("bookmark", 0, tile)),
        );
        rows.extend(snapshot.dismissed.iter().map(|site| {
            (
                "dismissed",
                0,
                StartPageTile {
                    site: site.clone(),
                    ..StartPageTile::default()
                },
            )
        }));
        self.as_mut().set_start_page_kinds(
            rows.iter()
                .map(|(kind, _, _)| QString::from(*kind))
                .collect(),
        );
        self.as_mut().set_start_page_slots(
            rows.iter()
                .map(|(_, slot, _)| QString::from(slot.to_string()))
                .collect(),
        );
        self.as_mut().set_start_page_ids(
            rows.iter()
                .map(|(_, _, tile)| QString::from(&tile.id))
                .collect(),
        );
        self.as_mut().set_start_page_urls(
            rows.iter()
                .map(|(_, _, tile)| QString::from(&tile.url))
                .collect(),
        );
        self.as_mut().set_start_page_titles(
            rows.iter()
                .map(|(_, _, tile)| QString::from(&tile.title))
                .collect(),
        );
        self.as_mut().set_start_page_sites(
            rows.iter()
                .map(|(_, _, tile)| QString::from(&tile.site))
                .collect(),
        );
        self.as_mut()
            .set_start_page_view(QString::from(snapshot.view.as_str()));
        if snapshot.ranked {
            let revision = self
                .as_ref()
                .rust()
                .start_page_rank_revision
                .wrapping_add(1);
            self.as_mut().set_start_page_rank_revision(revision);
        }
        let revision = self.as_ref().rust().start_page_revision.wrapping_add(1);
        self.set_start_page_revision(revision);
    }

    pub(super) fn refresh_start_page(mut self: Pin<&mut Self>) -> bool {
        if self.as_ref().active_profile_is_transient() {
            let shared = self
                .as_ref()
                .rust()
                .private_start_state
                .lock()
                .ok()
                .map(|state| (state.snapshot.clone(), state.visits.clone()));
            if let Some((snapshot, _)) = &shared {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .library
                    .start_snapshot = snapshot.clone();
            }
            let mut ranked = std::collections::BTreeMap::<String, (f64, i64, StartPageTile)>::new();
            let now = unix_timestamp();
            for record in &shared
                .as_ref()
                .map(|(_, visits)| visits.clone())
                .unwrap_or_default()
            {
                let Some(site) = site_key(&record.url) else {
                    continue;
                };
                if self
                    .as_ref()
                    .rust()
                    .library
                    .start_snapshot
                    .dismissed
                    .contains(&site)
                {
                    continue;
                }
                let entry =
                    ranked
                        .entry(site.clone())
                        .or_insert((0.0, i64::MIN, StartPageTile::default()));
                entry.0 += (-(f64::from(
                    u32::try_from(now.saturating_sub(record.timestamp).max(0)).unwrap_or(u32::MAX),
                )) / (14.0 * 86400.0))
                    .exp2();
                if record.timestamp > entry.1
                    || (record.timestamp == entry.1 && record.url < entry.2.url)
                {
                    entry.1 = record.timestamp;
                    entry.2 = StartPageTile {
                        id: site.clone(),
                        site,
                        url: record.url.clone(),
                        title: record.title.clone(),
                    };
                }
            }
            let mut rows: Vec<_> = ranked.into_values().collect();
            rows.sort_by(|left, right| {
                right
                    .0
                    .total_cmp(&left.0)
                    .then_with(|| right.1.cmp(&left.1))
                    .then_with(|| left.2.site.cmp(&right.2.site))
            });
            let mut rust = self.as_mut().rust_mut();
            let snapshot = &mut rust.as_mut().get_mut().library.start_snapshot;
            snapshot.eligible_urls = shared
                .as_ref()
                .map(|(_, visits)| visits.iter().map(|visit| visit.url.clone()).collect())
                .unwrap_or_default();
            snapshot.eligible_sites = rows.iter().map(|(_, _, tile)| tile.site.clone()).collect();
            snapshot.ranked = true;
            snapshot.top_sites = rows.into_iter().take(8).map(|(_, _, tile)| tile).collect();
            drop(rust);
            self.as_mut().set_start_page_ready(true);
            self.as_mut().publish_start_page();
            return true;
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .start_rank_requested = true;
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .library
            .start_refresh_requested = true;
        self.as_mut().set_start_page_error(QString::default());
        self.as_mut().flush_start_page_refresh();
        true
    }

    pub(super) fn flush_start_page_refresh(mut self: Pin<&mut Self>) {
        if self.as_ref().active_profile_is_transient()
            || self.as_ref().rust().library.start_read_pending
            || self.as_ref().rust().library.start_write_pending
            || !self.as_ref().rust().library.start_refresh_requested
        {
            return;
        }
        let now = unix_timestamp();
        let days = self
            .as_ref()
            .rust()
            .config
            .get("history")
            .and_then(|history| history.get("retention_days"))
            .and_then(Value::as_i64)
            .unwrap_or(90);
        let cutoff = now.saturating_sub(days.saturating_mul(86400));
        let rank = self.as_ref().rust().start_rank_requested;
        match self.as_mut().submit_storage(StorageRequest::StartPage {
            now,
            retention_before: cutoff,
            rank,
        }) {
            Ok(()) => {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .library
                    .start_read_pending = true;
                {
                    let mut rust = self.as_mut().rust_mut();
                    let this = rust.as_mut().get_mut();
                    this.library.start_refresh_requested = false;
                    this.start_rank_requested = false;
                }
                self.set_start_page_busy(true);
            }
            Err(error) => {
                if !matches!(
                    error,
                    ferric_browser_storage::StorageWorkerError::Busy
                        | ferric_browser_storage::StorageWorkerError::QueueFull
                ) {
                    self.as_mut()
                        .rust_mut()
                        .as_mut()
                        .get_mut()
                        .library
                        .start_refresh_requested = false;
                    self.as_mut().set_start_page_busy(false);
                    self.set_start_page_error(QString::from(error.to_string()));
                }
            }
        }
    }

    pub(super) fn complete_start_page_read(
        mut self: Pin<&mut Self>,
        result: Result<StartPageSnapshot, StorageOperationError>,
    ) {
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.library.start_read_pending = false;
            if let Ok(snapshot) = &result {
                let old_top = this.library.start_snapshot.top_sites.clone();
                this.library.start_snapshot = snapshot.clone();
                if !snapshot.ranked {
                    this.library.start_snapshot.top_sites = old_top;
                }
            }
        }
        self.as_mut().set_start_page_busy(false);
        match result {
            Ok(_) => {
                self.as_mut().set_start_page_ready(true);
                self.as_mut().set_start_page_error(QString::default());
                self.publish_start_page();
            }
            Err(error) => self.set_start_page_error(QString::from(error.message)),
        }
    }

    pub(super) fn complete_start_page_write(
        mut self: Pin<&mut Self>,
        result: Result<(), StorageOperationError>,
    ) {
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .library
            .start_write_pending = false;
        match result {
            Ok(()) => {
                self.as_ref().rust().library.storage_library_dirty.set(true);
                self.as_mut().refresh_start_page_metadata();
            }
            Err(error) => {
                self.as_mut().set_start_page_busy(false);
                self.set_start_page_error(QString::from(error.message));
            }
        }
    }

    pub(super) fn start_page_action(
        mut self: Pin<&mut Self>,
        action: &QString,
        slot: i32,
        value: &QString,
        title: &QString,
    ) -> bool {
        let action = action.to_string();
        let value = value.to_string();
        let title = super::sanitize_untrusted_title(&title.to_string());
        let target = u8::try_from(slot)
            .ok()
            .filter(|slot| (1..=8).contains(slot));
        let mutation = match action.as_str() {
            "view" => parse_view(&value).map(StartPageMutation::View),
            "pin" => target.and_then(|slot| {
                ValidatedUrl::parse(&value)
                    .ok()
                    .filter(|url| site_key(url.as_str()).is_some())
                    .map(|url| StartPageMutation::Pin {
                        slot,
                        url: url.as_str().into(),
                        title: title.clone(),
                        bookmark_id: None,
                    })
            }),
            "bookmark" => target.and_then(|slot| {
                self.as_ref()
                    .rust()
                    .library
                    .start_snapshot
                    .bookmarks
                    .iter()
                    .find(|entry| entry.id == value)
                    .map(|entry| StartPageMutation::Pin {
                        slot,
                        url: entry.url.clone(),
                        title: entry.title.clone(),
                        bookmark_id: Some(entry.id.clone()),
                    })
            }),
            "edit" => target.and_then(|slot| {
                ValidatedUrl::parse(&value)
                    .ok()
                    .filter(|url| site_key(url.as_str()).is_some())
                    .map(|url| StartPageMutation::Edit {
                        slot,
                        url: url.as_str().into(),
                        title: title.clone(),
                    })
            }),
            "unpin" => target.map(StartPageMutation::Unpin),
            "move" => target.and_then(|from| {
                value
                    .parse::<u8>()
                    .ok()
                    .filter(|to| (1..=8).contains(to))
                    .map(|to| StartPageMutation::Move { from, to })
            }),
            "dismiss" | "restore" => (!value.is_empty()).then(|| StartPageMutation::Dismiss {
                site: value.clone(),
                dismissed: action == "dismiss",
            }),
            _ => None,
        };
        let Some(mutation) = mutation else {
            self.set_start_page_error(QString::from("Invalid start-page action or stale bookmark"));
            return false;
        };
        if self.as_ref().active_profile_is_transient() {
            let latest = self
                .as_ref()
                .rust()
                .private_start_state
                .lock()
                .ok()
                .map(|state| state.snapshot.clone());
            if let Some(snapshot) = latest {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .library
                    .start_snapshot = snapshot;
            }
            let mut rust = self.as_mut().rust_mut();
            let snapshot = &mut rust.as_mut().get_mut().library.start_snapshot;
            match mutation {
                StartPageMutation::View(view) => snapshot.view = view,
                StartPageMutation::Pin {
                    slot,
                    url,
                    title,
                    bookmark_id,
                } => {
                    let existing = snapshot
                        .bookmarks
                        .iter()
                        .find(|entry| {
                            bookmark_id.as_ref().is_some_and(|id| &entry.id == id)
                                || entry.url == url
                        })
                        .cloned();
                    let entry = existing.unwrap_or_else(|| StartPageTile {
                        id: Uuid::new_v4().to_string(),
                        site: site_key(&url).unwrap_or_default(),
                        url,
                        title,
                    });
                    for slot in &mut snapshot.favourites {
                        if slot
                            .as_ref()
                            .is_some_and(|candidate| candidate.id == entry.id)
                        {
                            *slot = None;
                        }
                    }
                    if !snapshot
                        .bookmarks
                        .iter()
                        .any(|candidate| candidate.id == entry.id)
                    {
                        snapshot.bookmarks.push(entry.clone());
                    }
                    snapshot.favourites[usize::from(slot - 1)] = Some(entry);
                }
                StartPageMutation::Edit { slot, url, title } => {
                    let Some(entry) = snapshot.favourites[usize::from(slot - 1)].as_mut() else {
                        return false;
                    };
                    entry.site = site_key(&url).unwrap_or_default();
                    entry.url = url;
                    entry.title = title;
                    if let Some(bookmark) = snapshot
                        .bookmarks
                        .iter_mut()
                        .find(|bookmark| bookmark.id == entry.id)
                    {
                        *bookmark = entry.clone();
                    }
                }
                StartPageMutation::Unpin(slot) => snapshot.favourites[usize::from(slot - 1)] = None,
                StartPageMutation::Move { from, to } => {
                    if snapshot.favourites[usize::from(from - 1)].is_none() {
                        return false;
                    }
                    snapshot
                        .favourites
                        .swap(usize::from(from - 1), usize::from(to - 1));
                }
                StartPageMutation::Dismiss { site, dismissed } => {
                    snapshot.dismissed.retain(|candidate| candidate != &site);
                    if dismissed {
                        snapshot.dismissed.push(site.clone());
                        snapshot.top_sites.retain(|entry| entry.site != site);
                    }
                }
            }
            drop(rust);
            self.as_mut().set_start_page_error(QString::default());
            self.publish_start_page();
            return true;
        }
        if self.as_ref().rust().library.start_write_pending {
            self.set_start_page_error(QString::from(
                "A change is still being saved; retry when it finishes",
            ));
            return false;
        }
        match self
            .as_mut()
            .submit_storage(StorageRequest::StartPageWrite {
                mutation,
                timestamp: unix_timestamp(),
            }) {
            Ok(()) => {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .library
                    .start_write_pending = true;
                self.as_mut().set_start_page_error(QString::default());
                self.set_start_page_busy(true);
                true
            }
            Err(error) => {
                self.set_start_page_error(QString::from(error.to_string()));
                false
            }
        }
    }

    pub(super) fn dispatch_start_page_command(
        mut self: Pin<&mut Self>,
        command: &super::ParsedCommand,
    ) -> Option<bool> {
        if !matches!(
            command.name.as_str(),
            "tab-new" | "start-page" | "start-page-view" | "start-page-cycle" | "favourite-add"
        ) {
            return None;
        }
        if let Err(error) = self
            .as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
        {
            self.set_status_text(QString::from(error.to_string()));
            return Some(false);
        }
        match command.name.as_str() {
            "tab-new" => {
                if !command.arguments.is_empty() {
                    return Some(false);
                }
                let url = self
                    .as_ref()
                    .rust()
                    .config
                    .get("navigation")
                    .and_then(|navigation| navigation.get("new_tab"))
                    .and_then(Value::as_str)
                    .unwrap_or("rb://start")
                    .to_owned();
                self.execute_parsed_commands(vec![super::ParsedCommand {
                    name: "tab-open".into(),
                    arguments: vec![url],
                }])
                .into()
            }
            "start-page" => {
                if !command.arguments.is_empty() {
                    return Some(false);
                }
                self.navigate(&QString::from("rb://start"));
                Some(true)
            }
            "start-page-view" => {
                let [view] = command.arguments.as_slice() else {
                    self.set_status_text(QString::from(
                        "start-page-view requires top-sites, favourites, or guide",
                    ));
                    return Some(false);
                };
                if parse_view(view).is_none() {
                    return Some(false);
                }
                self.as_mut().navigate(&QString::from("rb://start"));
                self.as_mut().queue_pending_engine_action();
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_engine_action = Some(format!("start-page-view\t{view}"));
                Some(true)
            }
            "start-page-cycle" => {
                if self.as_ref().rust().current_url.to_string() != "rb://start"
                    || !command.arguments.is_empty()
                {
                    self.set_status_text(QString::from(
                        "View cycling is available on the start page",
                    ));
                    return Some(false);
                }
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_engine_action = Some("start-page-cycle".into());
                Some(true)
            }
            "favourite-add" => {
                let url = self.as_ref().rust().current_url.to_string();
                if site_key(&url).is_none() || !command.arguments.is_empty() {
                    self.set_status_text(QString::from(
                        "Open a safe web page before adding a favourite",
                    ));
                    return Some(false);
                }
                let title = self.as_ref().rust().page_title.to_string();
                self.as_mut().navigate(&QString::from("rb://start"));
                self.as_mut().queue_pending_engine_action();
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_engine_action = Some(format!("start-page-add\t{url}\t{title}"));
                Some(true)
            }
            _ => None,
        }
    }

    pub(super) fn start_page_pick_bookmark(mut self: Pin<&mut Self>, id: &QString) -> bool {
        let id = id.to_string();
        let tile = self
            .as_ref()
            .rust()
            .library
            .storage_library
            .as_ref()
            .and_then(|library| library.bookmarks.iter().find(|bookmark| bookmark.id == id))
            .cloned();
        let Some(tile) = tile else {
            self.set_status_text(QString::from("Bookmark is no longer available"));
            return false;
        };
        if site_key(&tile.url).is_none() {
            return false;
        }
        self.as_mut().navigate(&QString::from("rb://start"));
        self.as_mut().queue_pending_engine_action();
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(format!(
            "start-page-bookmark\t{}\t{}\t{}",
            tile.id, tile.url, tile.title
        ));
        true
    }

    pub(super) fn start_page_key_available(self: Pin<&mut Self>) -> bool {
        self.as_ref()
            .rust()
            .bindings
            .as_ref()
            .is_none_or(|bindings| !bindings.is_pending())
    }

    pub(super) fn content_navigated(
        mut self: Pin<&mut Self>,
        index: i32,
        url: &QString,
        engine_index: i32,
        traversal: bool,
    ) {
        let Some(tab) = self.as_ref().tab_for_index(index) else {
            return;
        };
        let url = url.to_string();
        if ValidatedUrl::parse(&url).is_err() {
            return;
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .navigation
            .content_histories
            .entry(tab)
            .or_default()
            .record(url.clone(), engine_index, traversal);
        let revision = self
            .as_ref()
            .rust()
            .content_history_revision
            .wrapping_add(1);
        self.as_mut().set_content_history_revision(revision);
        if InternalPage::from_url(&url).is_some() {
            if let Some(target) = self.as_ref().target_for_index(index) {
                if let Ok(parsed) = ValidatedUrl::parse(&url) {
                    let _ = self.as_mut().reduce_event(Event::CommitNavigation {
                        target,
                        url: parsed,
                        title: if url == "rb://start" {
                            "Start page".into()
                        } else {
                            "Blank".into()
                        },
                    });
                    if let Some(target) = self.as_ref().target_for_index(index) {
                        let _ = self
                            .as_mut()
                            .reduce_event(Event::CompleteNavigation { target });
                    }
                }
            }
            if index == self.as_ref().rust().active_tab_index {
                self.as_mut().update_current_url(QString::from(&url));
                self.as_mut().set_initial_url(QString::from(&url));
                self.as_mut()
                    .set_page_title(QString::from(if url == "rb://start" {
                        "Start page"
                    } else {
                        "Blank"
                    }));
                self.as_mut().set_load_state(QString::from("complete"));
            }
        }
    }

    pub(super) fn content_engine_traversed(
        mut self: Pin<&mut Self>,
        index: i32,
        url: &QString,
        engine_index: i32,
    ) -> bool {
        let Some(tab) = self.as_ref().tab_for_index(index) else {
            return false;
        };
        let url = url.to_string();
        if ValidatedUrl::parse(&url).is_err() {
            return false;
        }
        let changed = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .navigation
            .content_histories
            .entry(tab)
            .or_default()
            .observe_engine_traversal(url, engine_index);
        if changed {
            let revision = self
                .as_ref()
                .rust()
                .content_history_revision
                .wrapping_add(1);
            self.as_mut().set_content_history_revision(revision);
        }
        changed
    }

    pub(super) fn content_history_available(self: Pin<&mut Self>, index: i32, offset: i32) -> bool {
        self.as_ref()
            .tab_for_index(index)
            .and_then(|tab| {
                self.as_ref()
                    .rust()
                    .navigation
                    .content_histories
                    .get(&tab)
                    .map(|history| history.can_traverse(offset))
            })
            .unwrap_or(false)
    }

    pub(super) fn content_history_export(self: Pin<&mut Self>, index: i32) -> QString {
        let Some(tab) = self.as_ref().tab_for_index(index) else {
            return QString::default();
        };
        let binding = self.as_ref();
        let rust = binding.rust();
        let Some(history) = rust.navigation.content_histories.get(&tab) else {
            return QString::default();
        };
        let (entries, cursor) = history.snapshot();
        let entries: Vec<_> = entries
            .iter()
            .map(|entry| serde_json::json!({"url": entry.url, "engine_index": entry.engine_index}))
            .collect();
        QString::from(serde_json::json!({"entries": entries, "cursor": cursor}).to_string())
    }

    pub(super) fn content_history_import(
        mut self: Pin<&mut Self>,
        index: i32,
        snapshot: &QString,
    ) -> bool {
        let Some(tab) = self.as_ref().tab_for_index(index) else {
            return false;
        };
        let text = snapshot.to_string();
        if text.len() > 4 * 1024 * 1024 {
            return false;
        }
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            return false;
        };
        let Some(cursor) = value["cursor"]
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
        else {
            return false;
        };
        let Some(rows) = value["entries"]
            .as_array()
            .filter(|rows| rows.len() <= 4096)
        else {
            return false;
        };
        let entries: Option<Vec<_>> = rows
            .iter()
            .map(|row| {
                let url = row["url"].as_str()?;
                ValidatedUrl::parse(url).ok()?;
                let engine_index = i32::try_from(row["engine_index"].as_i64()?).ok()?;
                Some(ferric_browser_core::ContentEntry {
                    url: url.into(),
                    engine_index,
                })
            })
            .collect();
        let Some(history) = entries.and_then(|entries| {
            ferric_browser_core::ContentHistory::from_snapshot(entries, cursor)
        }) else {
            return false;
        };
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .navigation
            .content_histories
            .insert(tab, history);
        true
    }

    pub(super) fn content_traverse(
        mut self: Pin<&mut Self>,
        index: i32,
        offset: i32,
        engine_index: i32,
    ) -> QString {
        let Some(tab) = self.as_ref().tab_for_index(index) else {
            return QString::default();
        };
        let entry = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .navigation
            .content_histories
            .get_mut(&tab)
            .and_then(|history| history.traverse(offset));
        let Some(entry) = entry else {
            return QString::default();
        };
        let revision = self
            .as_ref()
            .rust()
            .content_history_revision
            .wrapping_add(1);
        self.as_mut().set_content_history_revision(revision);
        QString::from(format!(
            "{}\t{}",
            entry.engine_index.saturating_sub(engine_index),
            entry.url
        ))
    }

    pub(super) fn start_page_icon_path(
        self: Pin<&mut Self>,
        url: &QString,
        create: bool,
    ) -> QString {
        let Some(site) = site_key(&url.to_string()) else {
            return QString::default();
        };
        let directory = if self.as_ref().active_profile_is_transient() {
            let binding = self.as_ref();
            let rust = binding.rust();
            let Ok(mut state) = rust.private_start_state.lock() else {
                return QString::default();
            };
            if state.icon_directory.is_none() && create {
                let directory =
                    std::env::temp_dir().join(format!("ferric-private-icons-{}", Uuid::new_v4()));
                if std::fs::create_dir(&directory).is_err() {
                    return QString::default();
                }
                if super::set_private_directory_permissions(&directory).is_err() {
                    let _ = std::fs::remove_dir(&directory);
                    return QString::default();
                }
                state.icon_directory = Some(directory);
            }
            let Some(directory) = state.icon_directory.clone() else {
                return QString::default();
            };
            directory
        } else {
            let Some(roots) = self.as_ref().rust().storage_roots.clone() else {
                return QString::default();
            };
            roots
                .cache
                .join("start-page-icons")
                .join(&self.as_ref().rust().profile_name)
        };
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        site.hash(&mut hasher);
        let path = directory.join(format!("{:016x}.png", hasher.finish()));
        if create {
            if std::fs::create_dir_all(&directory).is_err() {
                return QString::default();
            }
            if let Ok(entries) = std::fs::read_dir(&directory) {
                let mut files: Vec<_> = entries
                    .flatten()
                    .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
                    .collect();
                files.sort_by_key(|entry| {
                    entry
                        .metadata()
                        .ok()
                        .and_then(|metadata| metadata.modified().ok())
                });
                let remove = files.len().saturating_sub(127);
                for entry in files.into_iter().take(remove) {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        } else if !path.is_file() {
            return QString::default();
        }
        QString::from(path.to_string_lossy().as_ref())
    }
}

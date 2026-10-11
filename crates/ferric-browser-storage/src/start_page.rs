//! Profile-local start-page state. All SQL is executed by the metadata worker.

use std::collections::BTreeMap;

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::{ProfileStore, StoreError, bounded_stored_text, is_safe_history_url};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StartPageView {
    TopSites,
    Favourites,
    #[default]
    Guide,
}

impl StartPageView {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TopSites => "top-sites",
            Self::Favourites => "favourites",
            Self::Guide => "guide",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StartPageTile {
    pub id: String,
    pub url: String,
    pub title: String,
    pub site: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StartPageSnapshot {
    pub view: StartPageView,
    pub favourites: [Option<StartPageTile>; 8],
    pub top_sites: Vec<StartPageTile>,
    pub dismissed: Vec<String>,
    pub bookmarks: Vec<StartPageTile>,
    pub eligible_sites: Vec<String>,
    pub eligible_urls: Vec<String>,
    pub ranked: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StartPageMutation {
    View(StartPageView),
    Pin {
        slot: u8,
        url: String,
        title: String,
        bookmark_id: Option<String>,
    },
    Edit {
        slot: u8,
        url: String,
        title: String,
    },
    Unpin(u8),
    Move {
        from: u8,
        to: u8,
    },
    Dismiss {
        site: String,
        dismissed: bool,
    },
}

/// Uses normalized HTTP(S) hosts; default ports do not split a site.
#[must_use]
pub fn site_key(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    if !matches!(scheme, "http" | "https") || !is_safe_history_url(url) {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    let authority = authority.to_ascii_lowercase();
    let (host, port) = if authority.starts_with('[') {
        let bracket = authority.find(']')?;
        let host = &authority[..=bracket];
        let suffix = &authority[bracket + 1..];
        (
            host,
            if suffix.is_empty() {
                None
            } else {
                Some(suffix.strip_prefix(':')?.parse::<u16>().ok()?)
            },
        )
    } else if let Some((host, port)) = authority.rsplit_once(':') {
        (host, Some(port.parse::<u16>().ok()?))
    } else {
        (authority.as_str(), None)
    };
    let host = host.trim_end_matches('.');
    let host = host.strip_prefix("www.").unwrap_or(host);
    if host.is_empty() {
        return None;
    }
    let default_port = if scheme == "https" { 443 } else { 80 };
    Some(match port {
        Some(port) if port != default_port => format!("{host}:{port}"),
        _ => host.to_owned(),
    })
}

impl ProfileStore {
    pub fn start_page_snapshot(
        &self,
        now: i64,
        retention_before: i64,
    ) -> Result<StartPageSnapshot, StoreError> {
        self.start_page_snapshot_with_ranking(now, retention_before, true)
    }

    // Keep the snapshot reads together so their SQL and presentation mapping remain auditable.
    #[allow(clippy::too_many_lines)]
    pub fn start_page_snapshot_with_ranking(
        &self,
        now: i64,
        retention_before: i64,
        rank: bool,
    ) -> Result<StartPageSnapshot, StoreError> {
        let view: Option<String> = self
            .connection
            .query_row(
                "SELECT view FROM start_page_preferences WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let mut snapshot = StartPageSnapshot {
            view: match view.as_deref() {
                Some("top-sites") => StartPageView::TopSites,
                Some("favourites") => StartPageView::Favourites,
                _ => StartPageView::Guide,
            },
            ..StartPageSnapshot::default()
        };
        let mut statement = self.connection.prepare(
            "SELECT b.id, b.url, b.title, f.slot FROM bookmarks b
             LEFT JOIN start_page_favourites f ON f.bookmark_id = b.id ORDER BY b.created_at, b.id",
        )?;
        for row in statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<u8>>(3)?,
            ))
        })? {
            let (id, url, label, slot) = row?;
            if let Some(site) = site_key(&url) {
                let tile = StartPageTile {
                    id,
                    url,
                    title: label,
                    site,
                };
                if let Some(slot) = slot {
                    snapshot.favourites[usize::from(slot - 1)] = Some(tile.clone());
                }
                snapshot.bookmarks.push(tile);
            }
        }
        let mut dismissed = self
            .connection
            .prepare("SELECT site_key FROM start_page_dismissed ORDER BY site_key")?;
        snapshot.dismissed = dismissed
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        let mut eligible = self.connection.prepare("SELECT normalized_url FROM pages WHERE EXISTS (SELECT 1 FROM visits WHERE page_id = pages.id AND committed_at >= ?1)")?;
        for url in eligible.query_map([retention_before], |row| row.get::<_, String>(0))? {
            let url = url?;
            if let Some(site) = site_key(&url) {
                if snapshot.dismissed.binary_search(&site).is_err() {
                    snapshot.eligible_sites.push(site);
                    snapshot.eligible_urls.push(url);
                }
            }
        }
        snapshot.eligible_sites.sort();
        snapshot.eligible_sites.dedup();
        snapshot.ranked = rank;
        if !rank {
            return Ok(snapshot);
        }
        // Aggregate retained visits, not a truncated history or lifetime page counter.
        let mut visits = self.connection.prepare(
            "SELECT p.normalized_url, p.safe_title, v.committed_at, COUNT(*) FROM visits v
             JOIN pages p ON p.id = v.page_id WHERE v.committed_at >= ?1
             GROUP BY p.id, v.committed_at ORDER BY v.committed_at, p.id",
        )?;
        let mut sites: BTreeMap<String, (f64, i64, StartPageTile)> = BTreeMap::new();
        for row in visits.query_map([retention_before], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, u32>(3)?,
            ))
        })? {
            let (url, title, timestamp, count) = row?;
            let Some(site) = site_key(&url) else { continue };
            if snapshot.dismissed.binary_search(&site).is_ok() {
                continue;
            }
            let weight = (-(f64::from(
                u32::try_from(now.saturating_sub(timestamp).max(0)).unwrap_or(u32::MAX),
            )) / (14.0 * 86400.0))
                .exp2()
                * f64::from(count);
            let entry = sites
                .entry(site.clone())
                .or_insert_with(|| (0.0, i64::MIN, StartPageTile::default()));
            entry.0 += weight;
            if timestamp > entry.1 || (timestamp == entry.1 && url < entry.2.url) {
                entry.1 = timestamp;
                entry.2 = StartPageTile {
                    id: site.clone(),
                    site,
                    url,
                    title,
                };
            }
        }
        let mut ranked: Vec<_> = sites.into_values().collect();
        ranked.sort_by(|left, right| {
            right
                .0
                .total_cmp(&left.0)
                .then_with(|| right.1.cmp(&left.1))
                .then_with(|| left.2.site.cmp(&right.2.site))
        });
        snapshot.top_sites = ranked
            .into_iter()
            .take(8)
            .map(|(_, _, tile)| tile)
            .collect();
        Ok(snapshot)
    }

    // Each mutation shares the same transaction and validation boundary.
    #[allow(clippy::too_many_lines)]
    pub fn mutate_start_page(
        &self,
        mutation: &StartPageMutation,
        timestamp: i64,
    ) -> Result<(), StoreError> {
        let slot = |value: u8| -> Result<(), StoreError> {
            if (1..=8).contains(&value) {
                Ok(())
            } else {
                Err(StoreError::InvalidInput("favourite slot must be 1 to 8"))
            }
        };
        let transaction = self.connection.unchecked_transaction()?;
        match mutation {
            StartPageMutation::View(view) => {
                transaction.execute("INSERT INTO start_page_preferences(id, view) VALUES (1, ?1) ON CONFLICT(id) DO UPDATE SET view = excluded.view", [view.as_str()])?;
            }
            StartPageMutation::Pin {
                slot: target,
                url,
                title,
                bookmark_id,
            } => {
                slot(*target)?;
                if site_key(url).is_none() || !bounded_stored_text(title, 4096) {
                    return Err(StoreError::InvalidInput(
                        "favourite needs a safe HTTP(S) URL and bounded title",
                    ));
                }
                let existing: Option<String> = if let Some(id) = bookmark_id {
                    transaction
                        .query_row(
                            "SELECT id FROM bookmarks WHERE id = ?1 AND url = ?2",
                            params![id, url],
                            |row| row.get(0),
                        )
                        .optional()?
                } else {
                    transaction.query_row("SELECT b.id FROM bookmarks b LEFT JOIN start_page_favourites f ON f.bookmark_id = b.id WHERE b.url = ?1 ORDER BY f.slot IS NULL, b.created_at, b.id LIMIT 1", [url], |row| row.get(0)).optional()?
                };
                if bookmark_id.is_some() && existing.is_none() {
                    return Err(StoreError::InvalidInput("bookmark is stale"));
                }
                let id = existing.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                transaction.execute("INSERT OR IGNORE INTO bookmarks(id, url, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)", params![id, url, title, timestamp])?;
                transaction.execute(
                    "DELETE FROM start_page_favourites WHERE slot = ?1 OR bookmark_id = ?2",
                    params![target, id],
                )?;
                transaction.execute(
                    "INSERT INTO start_page_favourites(slot, bookmark_id) VALUES (?1, ?2)",
                    params![target, id],
                )?;
            }
            StartPageMutation::Edit {
                slot: target,
                url,
                title,
            } => {
                slot(*target)?;
                if site_key(url).is_none() || !bounded_stored_text(title, 4096) {
                    return Err(StoreError::InvalidInput("invalid favourite URL or title"));
                }
                if transaction.execute("UPDATE bookmarks SET url = ?2, title = ?3, updated_at = ?4 WHERE id = (SELECT bookmark_id FROM start_page_favourites WHERE slot = ?1)", params![target, url, title, timestamp])? == 0 { return Err(StoreError::InvalidInput("favourite is stale")); }
            }
            StartPageMutation::Unpin(target) => {
                slot(*target)?;
                transaction.execute(
                    "DELETE FROM start_page_favourites WHERE slot = ?1",
                    [target],
                )?;
            }
            StartPageMutation::Move { from, to } => {
                slot(*from)?;
                slot(*to)?;
                let source: Option<String> = transaction
                    .query_row(
                        "SELECT bookmark_id FROM start_page_favourites WHERE slot = ?1",
                        [from],
                        |row| row.get(0),
                    )
                    .optional()?;
                let destination: Option<String> = transaction
                    .query_row(
                        "SELECT bookmark_id FROM start_page_favourites WHERE slot = ?1",
                        [to],
                        |row| row.get(0),
                    )
                    .optional()?;
                let Some(source) = source else {
                    return Err(StoreError::InvalidInput("source favourite is empty"));
                };
                transaction.execute(
                    "DELETE FROM start_page_favourites WHERE slot IN (?1, ?2)",
                    params![from, to],
                )?;
                transaction.execute(
                    "INSERT INTO start_page_favourites(slot, bookmark_id) VALUES (?1, ?2)",
                    params![to, source],
                )?;
                if let Some(destination) = destination {
                    transaction.execute(
                        "INSERT INTO start_page_favourites(slot, bookmark_id) VALUES (?1, ?2)",
                        params![from, destination],
                    )?;
                }
            }
            StartPageMutation::Dismiss { site, dismissed } => {
                if site.is_empty() || site.len() > 1024 || site.chars().any(char::is_control) {
                    return Err(StoreError::InvalidInput("invalid site key"));
                }
                if *dismissed {
                    transaction.execute(
                        "INSERT OR IGNORE INTO start_page_dismissed(site_key) VALUES (?1)",
                        [site],
                    )?;
                } else {
                    transaction.execute(
                        "DELETE FROM start_page_dismissed WHERE site_key = ?1",
                        [site],
                    )?;
                }
            }
        }
        transaction.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::StoreMode;
    use super::*;

    fn store() -> ProfileStore {
        let path =
            std::env::temp_dir().join(format!("ferric-start-test-{}.sqlite", uuid::Uuid::new_v4()));
        ProfileStore::open(path, StoreMode::Normal).unwrap()
    }

    #[test]
    fn bookmarks_back_stable_slots_and_mutations_are_atomic() {
        let store = store();
        let pin = |slot, url: &str| StartPageMutation::Pin {
            slot,
            url: url.into(),
            title: url.into(),
            bookmark_id: None,
        };
        store
            .mutate_start_page(&pin(1, "https://a.test"), 10)
            .unwrap();
        store
            .mutate_start_page(&pin(8, "https://b.test"), 11)
            .unwrap();
        store
            .mutate_start_page(&StartPageMutation::Move { from: 1, to: 8 }, 12)
            .unwrap();
        let snapshot = store.start_page_snapshot(12, 0).unwrap();
        assert_eq!(
            snapshot.favourites[0].as_ref().unwrap().url,
            "https://b.test"
        );
        assert_eq!(
            snapshot.favourites[7].as_ref().unwrap().url,
            "https://a.test"
        );
        store
            .mutate_start_page(
                &StartPageMutation::Edit {
                    slot: 8,
                    url: "https://c.test".into(),
                    title: "C".into(),
                },
                13,
            )
            .unwrap();
        let edited = store.start_page_snapshot(13, 0).unwrap();
        assert!(
            edited
                .bookmarks
                .iter()
                .any(|tile| tile.url == "https://c.test" && tile.title == "C")
        );
        assert!(
            store
                .mutate_start_page(&pin(8, "javascript:alert(1)"), 14)
                .is_err()
        );
        assert_eq!(
            store.start_page_snapshot(14, 0).unwrap().favourites,
            edited.favourites
        );
        store
            .mutate_start_page(&StartPageMutation::Unpin(8), 15)
            .unwrap();
        let snapshot = store.start_page_snapshot(15, 0).unwrap();
        assert!(snapshot.favourites[7].is_none());
        assert_eq!(snapshot.bookmarks.len(), 2);
        store
            .delete_bookmark(&snapshot.favourites[0].as_ref().unwrap().id)
            .unwrap();
        assert!(store.start_page_snapshot(15, 0).unwrap().favourites[0].is_none());
    }

    #[test]
    fn ranking_groups_sites_weights_frequency_and_recency_and_dismisses() {
        let store = store();
        let now = 2_000_000;
        store
            .record_visit_with_retention(
                "https://www.a.test/one",
                "A",
                "navigate",
                now - 86400,
                Some(0),
            )
            .unwrap();
        store
            .record_visit_with_retention("http://a.test/two", "A2", "navigate", now, Some(0))
            .unwrap();
        store
            .record_visit_with_retention("https://b.test", "B", "navigate", now, Some(0))
            .unwrap();
        let ranked = store.start_page_snapshot(now, 0).unwrap();
        assert_eq!(ranked.top_sites.len(), 2);
        assert_eq!(ranked.top_sites[0].site, "a.test");
        assert_eq!(ranked.top_sites[0].url, "http://a.test/two");
        store
            .mutate_start_page(
                &StartPageMutation::Dismiss {
                    site: "a.test".into(),
                    dismissed: true,
                },
                now,
            )
            .unwrap();
        assert_eq!(
            store.start_page_snapshot(now, 0).unwrap().top_sites.len(),
            1
        );
        store
            .mutate_start_page(
                &StartPageMutation::Dismiss {
                    site: "a.test".into(),
                    dismissed: false,
                },
                now,
            )
            .unwrap();
        assert_eq!(
            store.start_page_snapshot(now, 0).unwrap().top_sites.len(),
            2
        );
        assert_eq!(
            store.start_page_snapshot(now, now + 1).unwrap().top_sites,
            Vec::<StartPageTile>::new()
        );
        assert_eq!(
            site_key("https://www.a.test:443/x").as_deref(),
            Some("a.test")
        );
        assert_ne!(site_key("https://a.test:8080"), site_key("https://a.test"));
    }

    #[test]
    fn reuse_pinned_bookmark_and_keep_only_one_slot_per_bookmark() {
        let store = store();
        store
            .add_bookmark("old", "https://a.test", "Old", 1)
            .unwrap();
        store
            .add_bookmark("new", "https://a.test", "New", 2)
            .unwrap();
        store
            .mutate_start_page(
                &StartPageMutation::Pin {
                    slot: 3,
                    url: "https://a.test".into(),
                    title: "ignored".into(),
                    bookmark_id: Some("new".into()),
                },
                3,
            )
            .unwrap();
        store
            .mutate_start_page(
                &StartPageMutation::Pin {
                    slot: 1,
                    url: "https://a.test".into(),
                    title: "ignored".into(),
                    bookmark_id: None,
                },
                4,
            )
            .unwrap();
        let snapshot = store.start_page_snapshot(4, 0).unwrap();
        assert_eq!(snapshot.favourites[0].as_ref().unwrap().id, "new");
        assert!(snapshot.favourites[2].is_none());
        store
            .mutate_start_page(&StartPageMutation::View(StartPageView::Favourites), 5)
            .unwrap();
        assert_eq!(
            store.start_page_snapshot(5, 0).unwrap().view,
            StartPageView::Favourites
        );
    }

    #[test]
    fn schema_three_upgrade_preserves_data_and_creates_backup() {
        let store = store();
        store
            .add_bookmark("keep", "https://keep.test", "Keep", 1)
            .unwrap();
        store.connection.execute_batch("DELETE FROM schema_migrations WHERE version = 4; DROP TABLE start_page_preferences; DROP TABLE start_page_favourites; DROP TABLE start_page_dismissed;").unwrap();
        let path = store.path.clone();
        drop(store);
        let upgraded = ProfileStore::open(&path, StoreMode::Normal).unwrap();
        assert_eq!(upgraded.schema_version(), 4);
        assert_eq!(
            upgraded.start_page_snapshot(2, 0).unwrap().bookmarks[0].id,
            "keep"
        );
        let backup =
            rusqlite::Connection::open(path.with_extension("sqlite.pre-start-page-v3")).unwrap();
        assert_eq!(
            backup
                .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| row
                    .get::<_, i64>(
                    0
                ))
                .unwrap(),
            3
        );
        assert_eq!(
            backup
                .query_row("SELECT title FROM bookmarks WHERE id = 'keep'", [], |row| {
                    row.get::<_, String>(0)
                })
                .unwrap(),
            "Keep"
        );
    }
    #[test]
    fn canonical_site_keys_preserve_nondefault_ports_and_ipv6() {
        assert_eq!(
            site_key("https://WWW.Example.test.:443/path").as_deref(),
            Some("example.test")
        );
        assert_eq!(
            site_key("https://example.test.:8443").as_deref(),
            Some("example.test:8443")
        );
        assert_eq!(site_key("http://[::1]:80/path").as_deref(), Some("[::1]"));
        assert_eq!(
            site_key("https://[::1]:8443").as_deref(),
            Some("[::1]:8443")
        );
        assert!(site_key("https://example.test:wrong").is_none());
        assert!(site_key("rb://start").is_none());
    }
}

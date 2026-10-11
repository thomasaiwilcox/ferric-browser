//! Native tab surfaces and their positions relative to `WebEngine` history.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InternalPage {
    Start,
    Blank,
}

impl InternalPage {
    #[must_use]
    pub fn from_url(url: &str) -> Option<Self> {
        match url {
            "rb://start" => Some(Self::Start),
            "rb://blank" => Some(Self::Blank),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentEntry {
    pub url: String,
    pub engine_index: i32,
}

#[derive(Clone, Debug, Default)]
pub struct ContentHistory {
    entries: Vec<ContentEntry>,
    cursor: usize,
}

impl ContentHistory {
    pub fn record(&mut self, url: String, engine_index: i32, traversal: bool) {
        if traversal {
            if let Some(current) = self.entries.get_mut(self.cursor) {
                current.url = url;
                if InternalPage::from_url(&current.url).is_none() {
                    current.engine_index = engine_index;
                }
            }
            return;
        }
        if self.entries.get(self.cursor).is_some_and(|entry| {
            entry.url == url
                && (entry.engine_index == engine_index || InternalPage::from_url(&url).is_some())
        }) {
            return;
        }
        self.entries.truncate(self.cursor.saturating_add(1));
        self.entries.push(ContentEntry { url, engine_index });
        if self.entries.len() > 4096 {
            self.entries.remove(0);
        }
        self.cursor = self.entries.len() - 1;
    }

    #[must_use]
    pub fn snapshot(&self) -> (&[ContentEntry], usize) {
        (&self.entries, self.cursor)
    }

    #[must_use]
    pub fn from_snapshot(entries: Vec<ContentEntry>, cursor: usize) -> Option<Self> {
        if entries.len() > 4096
            || (!entries.is_empty() && cursor >= entries.len())
            || (entries.is_empty() && cursor != 0)
        {
            return None;
        }
        Some(Self { entries, cursor })
    }

    #[must_use]
    pub fn can_traverse(&self, offset: i32) -> bool {
        self.cursor
            .checked_add_signed(offset as isize)
            .is_some_and(|cursor| cursor < self.entries.len())
    }

    pub fn traverse(&mut self, offset: i32) -> Option<ContentEntry> {
        let next = self.cursor.checked_add_signed(offset as isize)?;
        let entry = self.entries.get(next)?.clone();
        self.cursor = next;
        Some(entry)
    }

    /// Align the logical cursor with a traversal initiated by the engine.
    pub fn observe_engine_traversal(&mut self, url: String, engine_index: i32) -> bool {
        if engine_index < 0 || InternalPage::from_url(&url).is_some() {
            return false;
        }
        let existing = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                entry.engine_index == engine_index && InternalPage::from_url(&entry.url).is_none()
            })
            .min_by_key(|(index, entry)| (entry.url != url, index.abs_diff(self.cursor)))
            .map(|(index, _)| index);
        if let Some(index) = existing {
            self.entries[index].url = url;
            self.cursor = index;
        } else {
            let index = self
                .entries
                .iter()
                .position(|entry| {
                    InternalPage::from_url(&entry.url).is_none()
                        && entry.engine_index > engine_index
                })
                .unwrap_or(self.entries.len());
            self.entries
                .insert(index, ContentEntry { url, engine_index });
            self.cursor = index;
            if self.entries.len() > 4096 {
                if self.cursor == 0 {
                    self.entries.pop();
                } else {
                    self.entries.remove(0);
                    self.cursor -= 1;
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_entries_share_anchors_without_losing_web_history() {
        let mut history = ContentHistory::default();
        history.record("rb://start".into(), 0, false);
        history.record("https://a.test".into(), 0, false);
        history.record("rb://start".into(), 0, false);
        history.record("https://b.test".into(), 1, false);
        assert_eq!(history.traverse(-1).unwrap().url, "rb://start");
        assert_eq!(history.traverse(-1).unwrap().url, "https://a.test");
        assert_eq!(history.traverse(2).unwrap().engine_index, 1);
        assert!(history.traverse(1).is_none());
        history.traverse(-1);
        history.record("https://c.test".into(), 1, false);
        assert!(history.traverse(1).is_none());
    }
    #[test]
    fn transferred_history_preserves_cursor_and_rejects_invalid_positions() {
        let mut history = ContentHistory::default();
        history.record("https://a.test".into(), 0, false);
        history.record("rb://start".into(), 0, false);
        history.record("https://b.test".into(), 1, false);
        history.traverse(-1);
        let (entries, cursor) = history.snapshot();
        let mut adopted = ContentHistory::from_snapshot(entries.to_vec(), cursor).unwrap();
        assert!(adopted.can_traverse(-1) && adopted.can_traverse(1));
        assert_eq!(adopted.traverse(1).unwrap().url, "https://b.test");
        assert!(ContentHistory::from_snapshot(entries.to_vec(), entries.len()).is_none());
    }
    #[test]
    fn native_traversal_keeps_its_anchor_until_the_engine_moves() {
        let mut history = ContentHistory::default();
        history.record("https://a.test".into(), 0, false);
        history.record("rb://start".into(), 0, false);
        history.record("https://b.test".into(), 1, false);
        history.traverse(-1);
        history.record("rb://start".into(), 1, true);
        history.record("rb://start".into(), 1, false);
        assert_eq!(history.snapshot().0.len(), 3);
        history.traverse(1);
        assert_eq!(history.traverse(-1).unwrap().engine_index, 0);
        assert_eq!(history.traverse(-1).unwrap().url, "https://a.test");
    }

    #[test]
    fn engine_traversal_preserves_native_entries_and_the_forward_branch() {
        let mut history = ContentHistory::default();
        history.record("https://a.test".into(), 0, false);
        history.record("rb://start".into(), 0, false);
        history.record("https://b.test".into(), 1, false);
        history.record("https://c.test".into(), 2, false);
        assert!(history.observe_engine_traversal("https://b.test".into(), 1));
        assert_eq!(history.snapshot().0.len(), 4);
        assert_eq!(history.traverse(-1).unwrap().url, "rb://start");
        assert_eq!(history.traverse(2).unwrap().url, "https://c.test");
        assert!(history.observe_engine_traversal("https://a.test".into(), 0));
        assert_eq!(history.traverse(3).unwrap().url, "https://c.test");
    }

    #[test]
    fn engine_traversal_distinguishes_same_url_states_and_recovers_missing_entries() {
        let mut history = ContentHistory::default();
        history.record("https://a.test".into(), 0, false);
        history.record("https://b.test".into(), 1, false);
        history.record("https://b.test".into(), 2, false);
        assert!(history.observe_engine_traversal("https://b.test".into(), 1));
        assert_eq!(history.traverse(1).unwrap().engine_index, 2);
        history.record("https://d.test".into(), 4, false);
        assert!(history.observe_engine_traversal("https://c.test".into(), 3));
        assert_eq!(history.traverse(-1).unwrap().engine_index, 2);
        assert_eq!(history.traverse(2).unwrap().url, "https://d.test");
        let previous = history.snapshot().1;
        assert!(!history.observe_engine_traversal("rb://start".into(), 4));
        assert!(!history.observe_engine_traversal("https://a.test".into(), -1));
        assert_eq!(history.snapshot().1, previous);
    }

    #[test]
    fn recovering_the_oldest_engine_entry_keeps_the_selected_entry_within_the_bound() {
        let entries = (1..=4096)
            .map(|engine_index| ContentEntry {
                url: format!("https://a.test/{engine_index}"),
                engine_index,
            })
            .collect();
        let mut history = ContentHistory::from_snapshot(entries, 4095).unwrap();
        assert!(history.observe_engine_traversal("https://a.test/0".into(), 0));
        let (entries, cursor) = history.snapshot();
        assert_eq!(entries.len(), 4096);
        assert_eq!(cursor, 0);
        assert_eq!(entries[cursor].url, "https://a.test/0");
        assert_eq!(history.traverse(1).unwrap().engine_index, 1);
    }
}

//! Process-local order for stable pages, including reserved unpresented pages.
//! Native host creation order must not reorder a delayed opening or restoration.

use ferric_browser_core::TabId;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(super) struct PageOrder {
    pages: Vec<TabId>,
    ranks: HashMap<TabId, usize>,
}

impl PageOrder {
    pub(super) fn observe(&mut self, pages: impl IntoIterator<Item = TabId>) {
        for page in pages {
            if let std::collections::hash_map::Entry::Vacant(entry) = self.ranks.entry(page) {
                entry.insert(self.pages.len());
                self.pages.push(page);
            }
        }
    }

    pub(super) fn rank(&self, page: TabId) -> usize {
        self.ranks.get(&page).copied().unwrap_or(usize::MAX)
    }

    pub(super) fn swap(&mut self, first: TabId, second: TabId) -> bool {
        let (Some(&left), Some(&right)) = (self.ranks.get(&first), self.ranks.get(&second)) else {
            return false;
        };
        self.pages.swap(left, right);
        self.ranks.insert(first, right);
        self.ranks.insert(second, left);
        true
    }

    pub(super) fn restore(&mut self, pages: &[TabId]) {
        self.forget(pages);
        self.observe(pages.iter().copied());
    }

    pub(super) fn forget(&mut self, pages: &[TabId]) {
        if pages.is_empty() {
            return;
        }
        let removed: HashSet<_> = pages.iter().copied().collect();
        self.pages.retain(|page| !removed.contains(page));
        self.ranks.clear();
        self.ranks.extend(
            self.pages
                .iter()
                .copied()
                .enumerate()
                .map(|(rank, page)| (page, rank)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn page(value: u64) -> TabId {
        TabId::from_display(&format!("tabid-{value}")).unwrap()
    }

    #[test]
    fn reservations_survive_delayed_host_registration_and_scoped_moves() {
        let mut order = PageOrder::default();
        order.observe([page(1), page(2), page(3)]);
        // The third host registers before the second; a foreign-scope page
        // can sit between group members without being moved itself.
        order.observe([page(3), page(4)]);
        order.observe([page(2)]);
        assert_eq!(order.rank(page(2)), 1);
        assert!(order.swap(page(1), page(3)));
        assert_eq!(
            (
                order.rank(page(3)),
                order.rank(page(2)),
                order.rank(page(1))
            ),
            (0, 1, 2)
        );
        assert_eq!(order.rank(page(4)), 3);
        order.forget(&[page(2)]);
        assert_eq!(order.rank(page(2)), usize::MAX);
        assert_eq!(order.rank(page(4)), 2);
    }

    #[test]
    fn restoration_reorders_reserved_pages_without_reordering_surviving_groups() {
        let mut order = PageOrder::default();
        order.observe([page(1), page(3), page(2), page(4)]);
        order.restore(&[page(2), page(3)]);
        assert_eq!(
            [page(1), page(4), page(2), page(3)].map(|id| order.rank(id)),
            [0, 1, 2, 3]
        );
        order.forget(&[page(1), page(2), page(3), page(4)]);
        assert_eq!(order.pages, Vec::new());
        assert!(order.ranks.is_empty());
    }
}

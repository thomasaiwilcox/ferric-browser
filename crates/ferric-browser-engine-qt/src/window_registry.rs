//! Typed live-window registry handoff from QML to the IPC projection.
//!
//! QML reports its live windows as parallel typed lists. This module validates
//! the bounded records before retaining them in Rust, so IPC never needs to
//! consume presentation-assembled JSON.

use super::{CxxQtType, Pin, QStringList, is_bounded_untrusted_text, qobject};

#[derive(Clone, Debug)]
pub(super) struct LiveWindowRegistryEntry {
    pub(super) id: String,
    pub(super) owner_token: String,
    pub(super) profile: String,
    pub(super) private: bool,
    pub(super) ephemeral: bool,
    pub(super) tab_count: i32,
}

pub(super) fn decode_live_window_registry(
    ids: &[String],
    owner_tokens: &[String],
    profiles: &[String],
    private_flags: &[String],
    ephemeral_flags: &[String],
    tab_counts: &[String],
) -> Option<Vec<LiveWindowRegistryEntry>> {
    let field_lengths = [
        ids.len(),
        owner_tokens.len(),
        profiles.len(),
        private_flags.len(),
        ephemeral_flags.len(),
        tab_counts.len(),
    ];
    let &length = field_lengths.first()?;
    if length > 64 || field_lengths.iter().any(|candidate| *candidate != length) {
        return None;
    }

    let mut registry = Vec::with_capacity(length);
    for index in 0..length {
        let id = ids[index].clone();
        let owner_token = owner_tokens[index].clone();
        let profile = profiles[index].clone();
        let private = match private_flags[index].as_str() {
            "true" => true,
            "false" => false,
            _ => return None,
        };
        let ephemeral = match ephemeral_flags[index].as_str() {
            "true" => true,
            "false" => false,
            _ => return None,
        };
        let Ok(tab_count) = tab_counts[index].parse::<i32>() else {
            return None;
        };
        if (!id.is_empty() && !is_bounded_untrusted_text(&id))
            || !is_bounded_untrusted_text(&owner_token)
            || !is_bounded_untrusted_text(&profile)
            || !(0..=1_000_000).contains(&tab_count)
        {
            return None;
        }
        registry.push(LiveWindowRegistryEntry {
            id,
            owner_token,
            profile,
            private,
            ephemeral,
            tab_count,
        });
    }
    Some(registry)
}

impl qobject::BrowserUi {
    pub(super) fn publish_live_window_registry(
        mut self: Pin<&mut Self>,
        ids: &QStringList,
        owner_tokens: &QStringList,
        profiles: &QStringList,
        private_flags: &QStringList,
        ephemeral_flags: &QStringList,
        tab_counts: &QStringList,
    ) -> bool {
        let ids = ids.iter().map(ToString::to_string).collect::<Vec<_>>();
        let owner_tokens = owner_tokens
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let profiles = profiles.iter().map(ToString::to_string).collect::<Vec<_>>();
        let private_flags = private_flags
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let ephemeral_flags = ephemeral_flags
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let tab_counts = tab_counts
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let Some(registry) = decode_live_window_registry(
            &ids,
            &owner_tokens,
            &profiles,
            &private_flags,
            &ephemeral_flags,
            &tab_counts,
        ) else {
            return false;
        };
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .live_window_registry = registry;
        true
    }
}

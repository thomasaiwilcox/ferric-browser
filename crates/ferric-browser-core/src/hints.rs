use crate::{DocumentId, HintSessionId, IdSource, TabId, switcher_rank, tokenize_switcher_query};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

const DEFAULT_HINT_CHARS: &str = "asdfghjkl";
pub const MAX_HINT_CANDIDATES: usize = 5_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HintKind {
    Link,
    Button,
    Input,
    Select,
    Textarea,
    ContentEditable,
    Image,
    Media,
    Scrollable,
    Aria,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum HintAutoFollow {
    Always,
    UniqueMatch,
    #[default]
    FullMatch,
    Never,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum HintUnmatchedPolicy {
    #[default]
    Hide,
    Dim,
    Show,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum HintInputMode {
    #[default]
    Label,
    Text,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HintGeometry {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl HintGeometry {
    fn is_valid(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width > 0.0
            && self.height > 0.0
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct HintTarget {
    pub tab: TabId,
    pub generation: u64,
    pub document: DocumentId,
}

impl fmt::Debug for HintTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HintTarget([redacted])")
    }
}

#[derive(Clone, PartialEq)]
pub struct HintCandidate {
    pub element_id: u32,
    pub kind: HintKind,
    pub frame_path: String,
    pub text: String,
    pub href: Option<String>,
    pub geometry: HintGeometry,
}

impl fmt::Debug for HintCandidate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HintCandidate")
            .field("kind", &self.kind)
            .field("has_href", &self.href.is_some())
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub struct LabeledHint {
    pub label: String,
    pub candidate: HintCandidate,
}

impl fmt::Debug for LabeledHint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LabeledHint")
            .field("label", &"[redacted]")
            .field("candidate", &self.candidate)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum HintError {
    TooManyCandidates { count: usize, maximum: usize },
    InvalidGeometry { index: usize },
    EmptyLabel,
    UnknownLabel(String),
    StaleTarget,
    WrongFrame,
    CandidateNotVisible,
    InvalidAlphabet,
    InvalidMinimumWidth,
}

impl fmt::Debug for HintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyCandidates { .. } => formatter.write_str("HintError::TooManyCandidates"),
            Self::InvalidGeometry { .. } => formatter.write_str("HintError::InvalidGeometry"),
            Self::EmptyLabel => formatter.write_str("HintError::EmptyLabel"),
            Self::UnknownLabel(_) => formatter.write_str("HintError::UnknownLabel([redacted])"),
            Self::StaleTarget => formatter.write_str("HintError::StaleTarget"),
            Self::WrongFrame => formatter.write_str("HintError::WrongFrame"),
            Self::CandidateNotVisible => formatter.write_str("HintError::CandidateNotVisible"),
            Self::InvalidAlphabet => formatter.write_str("HintError::InvalidAlphabet"),
            Self::InvalidMinimumWidth => formatter.write_str("HintError::InvalidMinimumWidth"),
        }
    }
}

impl std::fmt::Display for HintError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooManyCandidates { .. } => formatter.write_str("too many hint candidates"),
            Self::InvalidGeometry { .. } => {
                formatter.write_str("hint candidate geometry is invalid")
            }
            Self::EmptyLabel => formatter.write_str("hint label is empty"),
            Self::UnknownLabel(_) => formatter.write_str("unknown hint label"),
            Self::StaleTarget => formatter.write_str("hint target is stale"),
            Self::WrongFrame => formatter.write_str("hint candidate belongs to another frame"),
            Self::CandidateNotVisible => formatter.write_str("hint candidate is no longer visible"),
            Self::InvalidAlphabet => formatter.write_str("hint alphabet is invalid"),
            Self::InvalidMinimumWidth => formatter.write_str("hint minimum width is invalid"),
        }
    }
}

impl std::error::Error for HintError {}

#[derive(Clone, PartialEq)]
pub struct HintSession {
    pub id: HintSessionId,
    pub target: HintTarget,
    pub hints: Vec<LabeledHint>,
}

impl fmt::Debug for HintSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HintSession")
            .field("id", &"[redacted]")
            .field("target", &"[redacted]")
            .field("hint_count", &self.hints.len())
            .finish()
    }
}

impl HintSession {
    #[must_use]
    pub fn new(ids: &mut IdSource, target: HintTarget, hints: Vec<LabeledHint>) -> Self {
        Self {
            id: ids.hint_session(),
            target,
            hints,
        }
    }

    /// Validates a selection against the captured document and current frame
    /// facts. The engine adapter supplies the fresh visibility/geometry facts.
    ///
    /// # Errors
    ///
    /// Returns a stale-target, frame, visibility, or unknown-label error.
    pub fn select(
        &self,
        label: &str,
        target: HintTarget,
        frame_path: &str,
        visible: bool,
        geometry: HintGeometry,
    ) -> Result<&HintCandidate, HintError> {
        if label.is_empty() {
            return Err(HintError::EmptyLabel);
        }
        if self.target != target {
            return Err(HintError::StaleTarget);
        }
        let hint = self
            .hints
            .iter()
            .find(|hint| hint.label == label)
            .ok_or_else(|| HintError::UnknownLabel(label.to_owned()))?;
        if hint.candidate.frame_path != frame_path {
            return Err(HintError::WrongFrame);
        }
        if !visible || !geometry.is_valid() {
            return Err(HintError::CandidateNotVisible);
        }
        let captured = hint.candidate.geometry;
        let captured_center = (
            captured.x + captured.width / 2.0,
            captured.y + captured.height / 2.0,
        );
        let fresh_center = (
            geometry.x + geometry.width / 2.0,
            geometry.y + geometry.height / 2.0,
        );
        let x_tolerance = captured.width.max(16.0);
        let y_tolerance = captured.height.max(16.0);
        if (captured_center.0 - fresh_center.0).abs() > x_tolerance
            || (captured_center.1 - fresh_center.1).abs() > y_tolerance
        {
            return Err(HintError::CandidateNotVisible);
        }
        Ok(&hint.candidate)
    }
}

/// Assigns stable labels in viewport reading order. All labels in one result
/// have the same length, making the set prefix-free even when it is filtered
/// down to one candidate. Callers must retain the result for the hint session.
///
/// # Errors
///
/// Returns an error when the hard candidate cap or geometry validation fails.
pub fn assign_labels(mut candidates: Vec<HintCandidate>) -> Result<Vec<LabeledHint>, HintError> {
    assign_labels_with_options(std::mem::take(&mut candidates), DEFAULT_HINT_CHARS, 1)
}

/// Assigns fixed-width, prefix-free labels using a validated custom alphabet.
///
/// # Errors
///
/// Returns an error for invalid configuration, candidate count, or geometry.
pub fn assign_labels_with_options(
    mut candidates: Vec<HintCandidate>,
    alphabet: &str,
    minimum_width: usize,
) -> Result<Vec<LabeledHint>, HintError> {
    validate_label_options(alphabet, minimum_width)?;
    if candidates.len() > MAX_HINT_CANDIDATES {
        return Err(HintError::TooManyCandidates {
            count: candidates.len(),
            maximum: MAX_HINT_CANDIDATES,
        });
    }
    for (index, candidate) in candidates.iter().enumerate() {
        if !candidate.geometry.is_valid() {
            return Err(HintError::InvalidGeometry { index });
        }
    }
    sort_candidates(&mut candidates);
    let width = label_width(candidates.len(), alphabet.len(), minimum_width);
    Ok(candidates
        .into_iter()
        .enumerate()
        .map(|(index, candidate)| LabeledHint {
            label: encode_label(index, width, alphabet.as_bytes()),
            candidate,
        })
        .collect())
}

/// Relabels a refreshed candidate set while preserving stable element labels
/// whenever the required fixed label width has not changed.
///
/// # Errors
///
/// Returns an error for invalid configuration, candidate count, or geometry.
pub fn refresh_labels(
    mut candidates: Vec<HintCandidate>,
    previous: &[LabeledHint],
    alphabet: &str,
    minimum_width: usize,
) -> Result<Vec<LabeledHint>, HintError> {
    validate_label_options(alphabet, minimum_width)?;
    if candidates.len() > MAX_HINT_CANDIDATES {
        return Err(HintError::TooManyCandidates {
            count: candidates.len(),
            maximum: MAX_HINT_CANDIDATES,
        });
    }
    for (index, candidate) in candidates.iter().enumerate() {
        if !candidate.geometry.is_valid() {
            return Err(HintError::InvalidGeometry { index });
        }
    }
    sort_candidates(&mut candidates);
    let width = label_width(candidates.len(), alphabet.len(), minimum_width);
    let previous_width = previous.first().map_or(width, |hint| hint.label.len());
    if previous_width != width {
        return assign_labels_with_options(candidates, alphabet, minimum_width);
    }

    let alphabet_bytes = alphabet.as_bytes();
    let previous_labels = previous
        .iter()
        .filter(|hint| {
            hint.label.len() == width
                && hint
                    .label
                    .bytes()
                    .all(|byte| alphabet_bytes.contains(&byte))
        })
        .map(|hint| {
            (
                (
                    hint.candidate.frame_path.as_str(),
                    hint.candidate.element_id,
                ),
                hint.label.as_str(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut used = BTreeSet::new();
    let mut labels = vec![String::new(); candidates.len()];
    for (index, candidate) in candidates.iter().enumerate() {
        if let Some(label) =
            previous_labels.get(&(candidate.frame_path.as_str(), candidate.element_id))
            && used.insert((*label).to_owned())
        {
            (*label).clone_into(&mut labels[index]);
        }
    }
    let mut next = 0;
    for label in &mut labels {
        if !label.is_empty() {
            continue;
        }
        loop {
            let candidate_label = encode_label(next, width, alphabet_bytes);
            next += 1;
            if used.insert(candidate_label.clone()) {
                *label = candidate_label;
                break;
            }
        }
    }
    Ok(candidates
        .into_iter()
        .zip(labels)
        .map(|(candidate, label)| LabeledHint { label, candidate })
        .collect())
}

fn validate_label_options(alphabet: &str, minimum_width: usize) -> Result<(), HintError> {
    let mut unique = BTreeSet::new();
    if !(2..=32).contains(&alphabet.len())
        || !alphabet.is_ascii()
        || alphabet
            .bytes()
            .any(|byte| !byte.is_ascii_graphic() || byte == b'/' || !unique.insert(byte))
    {
        return Err(HintError::InvalidAlphabet);
    }
    if !(1..=8).contains(&minimum_width) {
        return Err(HintError::InvalidMinimumWidth);
    }
    Ok(())
}

fn sort_candidates(candidates: &mut [HintCandidate]) {
    candidates.sort_by(|left, right| {
        left.geometry
            .y
            .total_cmp(&right.geometry.y)
            .then_with(|| left.geometry.x.total_cmp(&right.geometry.x))
            .then_with(|| left.frame_path.cmp(&right.frame_path))
            .then_with(|| left.element_id.cmp(&right.element_id))
            .then_with(|| left.text.cmp(&right.text))
    });
}

fn label_width(count: usize, base: usize, minimum_width: usize) -> usize {
    let mut capacity = base.saturating_pow(u32::try_from(minimum_width).unwrap_or(u32::MAX));
    let mut width = minimum_width;
    while capacity < count {
        capacity = capacity.saturating_mul(base);
        width += 1;
    }
    width
}

fn encode_label(mut index: usize, width: usize, alphabet: &[u8]) -> String {
    let base = alphabet.len();
    let mut bytes = vec![alphabet[0]; width];
    for slot in bytes.iter_mut().rev() {
        *slot = alphabet[index % base];
        index /= base;
    }
    String::from_utf8(bytes).expect("hint alphabet is ASCII")
}

#[derive(Clone, Eq, PartialEq)]
pub enum HintInteractionInput {
    Character(char),
    EnterTextMode,
    Backspace,
    Clear,
    Next,
    Previous,
    Activate,
    RotateCollision,
    SelectLabel(String),
    Cancel,
}

impl fmt::Debug for HintInteractionInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Character(_) => {
                formatter.write_str("HintInteractionInput::Character([redacted])")
            }
            Self::SelectLabel(_) => {
                formatter.write_str("HintInteractionInput::SelectLabel([redacted])")
            }
            Self::EnterTextMode => formatter.write_str("HintInteractionInput::EnterTextMode"),
            Self::Backspace => formatter.write_str("HintInteractionInput::Backspace"),
            Self::Clear => formatter.write_str("HintInteractionInput::Clear"),
            Self::Next => formatter.write_str("HintInteractionInput::Next"),
            Self::Previous => formatter.write_str("HintInteractionInput::Previous"),
            Self::Activate => formatter.write_str("HintInteractionInput::Activate"),
            Self::RotateCollision => formatter.write_str("HintInteractionInput::RotateCollision"),
            Self::Cancel => formatter.write_str("HintInteractionInput::Cancel"),
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum HintInteractionOutcome {
    Updated,
    Activate(String),
    InvalidCharacter(char),
    NoMatches,
    RotateCollision,
    Cancel,
}

impl fmt::Debug for HintInteractionOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Activate(_) => {
                formatter.write_str("HintInteractionOutcome::Activate([redacted])")
            }
            Self::InvalidCharacter(_) => {
                formatter.write_str("HintInteractionOutcome::InvalidCharacter([redacted])")
            }
            Self::Updated => formatter.write_str("HintInteractionOutcome::Updated"),
            Self::NoMatches => formatter.write_str("HintInteractionOutcome::NoMatches"),
            Self::RotateCollision => formatter.write_str("HintInteractionOutcome::RotateCollision"),
            Self::Cancel => formatter.write_str("HintInteractionOutcome::Cancel"),
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct HintInteractionSnapshot {
    pub mode: HintInputMode,
    pub prefix: String,
    pub query: String,
    pub matching_labels: Vec<String>,
    pub active_label: Option<String>,
    pub remaining: usize,
    pub total: usize,
}

impl fmt::Debug for HintInteractionSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HintInteractionSnapshot")
            .field("mode", &self.mode)
            .field("prefix", &"[redacted]")
            .field("query", &"[redacted]")
            .field("matching_label_count", &self.matching_labels.len())
            .field("has_active_label", &self.active_label.is_some())
            .field("remaining", &self.remaining)
            .field("total", &self.total)
            .finish()
    }
}

/// Testable keyboard interaction state for an immutable set of hint labels.
/// Filtering never mutates or reassigns labels.
#[derive(Clone, PartialEq)]
pub struct HintInteraction {
    hints: Vec<LabeledHint>,
    alphabet: String,
    auto_follow: HintAutoFollow,
    mode: HintInputMode,
    prefix: String,
    query: String,
    matches: Vec<usize>,
    active: usize,
}

impl fmt::Debug for HintInteraction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HintInteraction")
            .field("hint_count", &self.hints.len())
            .field("auto_follow", &self.auto_follow)
            .field("mode", &self.mode)
            .field("match_count", &self.matches.len())
            .field("active_index", &self.active)
            .finish()
    }
}

impl HintInteraction {
    #[must_use]
    pub fn new(hints: Vec<LabeledHint>, alphabet: String, auto_follow: HintAutoFollow) -> Self {
        let matches = (0..hints.len()).collect();
        Self {
            hints,
            alphabet,
            auto_follow,
            mode: HintInputMode::Label,
            prefix: String::new(),
            query: String::new(),
            matches,
            active: 0,
        }
    }

    #[must_use]
    pub fn initial_activation(&self) -> Option<&str> {
        (self.auto_follow == HintAutoFollow::Always && self.matches.len() == 1)
            .then(|| self.active_label())
            .flatten()
    }

    #[must_use]
    pub fn snapshot(&self) -> HintInteractionSnapshot {
        HintInteractionSnapshot {
            mode: self.mode,
            prefix: self.prefix.clone(),
            query: self.query.clone(),
            matching_labels: self
                .matches
                .iter()
                .map(|index| self.hints[*index].label.clone())
                .collect(),
            active_label: self.active_label().map(str::to_owned),
            remaining: self.matches.len(),
            total: self.hints.len(),
        }
    }

    pub fn handle(&mut self, input: HintInteractionInput) -> HintInteractionOutcome {
        match input {
            HintInteractionInput::Cancel => HintInteractionOutcome::Cancel,
            HintInteractionInput::RotateCollision => HintInteractionOutcome::RotateCollision,
            HintInteractionInput::SelectLabel(label) => {
                if let Some(index) = self
                    .matches
                    .iter()
                    .position(|hint| self.hints[*hint].label == label)
                {
                    self.active = index;
                    HintInteractionOutcome::Updated
                } else {
                    HintInteractionOutcome::NoMatches
                }
            }
            HintInteractionInput::EnterTextMode if self.mode == HintInputMode::Label => {
                self.mode = HintInputMode::Text;
                self.query.clear();
                self.rank_text_matches();
                HintInteractionOutcome::Updated
            }
            HintInteractionInput::EnterTextMode => {
                self.push_text('/');
                HintInteractionOutcome::Updated
            }
            HintInteractionInput::Character(character) => self.handle_character(character),
            HintInteractionInput::Backspace => self.backspace(),
            HintInteractionInput::Clear => {
                if self.mode == HintInputMode::Label {
                    self.prefix.clear();
                    self.filter_label_matches();
                } else {
                    self.query.clear();
                    self.rank_text_matches();
                }
                HintInteractionOutcome::Updated
            }
            HintInteractionInput::Next => {
                if !self.matches.is_empty() {
                    self.active = (self.active + 1) % self.matches.len();
                }
                HintInteractionOutcome::Updated
            }
            HintInteractionInput::Previous => {
                if !self.matches.is_empty() {
                    self.active = (self.active + self.matches.len() - 1) % self.matches.len();
                }
                HintInteractionOutcome::Updated
            }
            HintInteractionInput::Activate => self
                .active_label()
                .map_or(HintInteractionOutcome::NoMatches, |label| {
                    HintInteractionOutcome::Activate(label.to_owned())
                }),
        }
    }

    fn handle_character(&mut self, character: char) -> HintInteractionOutcome {
        if self.mode == HintInputMode::Text {
            self.push_text(character);
            return HintInteractionOutcome::Updated;
        }
        if !character.is_ascii() || !self.alphabet.contains(character) {
            return HintInteractionOutcome::InvalidCharacter(character);
        }
        let old_prefix = self.prefix.clone();
        self.prefix.push(character);
        self.filter_label_matches();
        if self.matches.is_empty() {
            self.prefix = old_prefix;
            self.filter_label_matches();
            return HintInteractionOutcome::NoMatches;
        }
        let should_activate = match self.auto_follow {
            HintAutoFollow::Always | HintAutoFollow::UniqueMatch => self.matches.len() == 1,
            HintAutoFollow::FullMatch => self
                .active_label()
                .is_some_and(|label| label == self.prefix),
            HintAutoFollow::Never => false,
        };
        if should_activate {
            return HintInteractionOutcome::Activate(
                self.active_label().expect("one active hint").to_owned(),
            );
        }
        HintInteractionOutcome::Updated
    }

    fn backspace(&mut self) -> HintInteractionOutcome {
        if self.mode == HintInputMode::Text {
            if self.query.pop().is_none() {
                self.mode = HintInputMode::Label;
                self.filter_label_matches();
            } else {
                self.rank_text_matches();
            }
        } else {
            self.prefix.pop();
            self.filter_label_matches();
        }
        HintInteractionOutcome::Updated
    }

    fn push_text(&mut self, character: char) {
        if !character.is_control() && self.query.chars().count() < 256 {
            self.query.push(character);
            self.rank_text_matches();
        }
    }

    fn filter_label_matches(&mut self) {
        self.matches = self
            .hints
            .iter()
            .enumerate()
            .filter_map(|(index, hint)| hint.label.starts_with(&self.prefix).then_some(index))
            .collect();
        self.active = 0;
    }

    fn rank_text_matches(&mut self) {
        let query = tokenize_switcher_query(&self.query);
        let mut ranked = self
            .hints
            .iter()
            .enumerate()
            .filter_map(|(index, hint)| {
                if query.is_empty() {
                    return Some((0, index));
                }
                let text_rank = switcher_rank(&query, "", &hint.candidate.text, &[]);
                let url_rank = hint
                    .candidate
                    .href
                    .as_deref()
                    .and_then(|url| switcher_rank(&query, "", url, &[]))
                    .map(|rank| rank.saturating_sub(200));
                text_rank.max(url_rank).map(|rank| (rank, index))
            })
            .collect::<Vec<_>>();
        ranked.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
        self.matches = ranked.into_iter().map(|(_, index)| index).collect();
        self.active = 0;
    }

    fn active_label(&self) -> Option<&str> {
        self.matches
            .get(self.active)
            .map(|index| self.hints[*index].label.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(x: f64, y: f64, text: &str) -> HintCandidate {
        HintCandidate {
            element_id: 1,
            kind: HintKind::Link,
            frame_path: "0".into(),
            text: text.into(),
            href: None,
            geometry: HintGeometry {
                x,
                y,
                width: 10.0,
                height: 10.0,
            },
        }
    }

    #[test]
    fn labels_are_reading_order_and_prefix_free() {
        let hints = assign_labels(vec![
            candidate(20.0, 0.0, "second"),
            candidate(0.0, 0.0, "first"),
            candidate(0.0, 20.0, "third"),
        ])
        .expect("labels");
        assert_eq!(
            hints
                .iter()
                .map(|hint| hint.label.as_str())
                .collect::<Vec<_>>(),
            ["a", "s", "d"]
        );
        for left in &hints {
            for right in &hints {
                assert!(!(left.label != right.label && right.label.starts_with(&left.label)));
            }
        }
    }

    #[test]
    fn larger_sets_use_fixed_width_labels() {
        let candidates = (0..10)
            .map(|index| candidate(f64::from(index), 0.0, &index.to_string()))
            .collect();
        let hints = assign_labels(candidates).expect("labels");
        assert!(hints.iter().all(|hint| hint.label.len() == 2));
        assert_eq!(hints[0].label, "aa");
        assert_eq!(hints[9].label, "sa");
    }

    #[test]
    fn configurable_alphabet_and_minimum_width_remain_prefix_free() {
        let hints = assign_labels_with_options(
            (0..5)
                .map(|index| candidate(f64::from(index), 0.0, &index.to_string()))
                .collect(),
            "zx",
            3,
        )
        .expect("custom labels");
        assert_eq!(hints[0].label, "zzz");
        assert_eq!(hints[4].label, "xzz");
        assert!(hints.iter().all(|hint| hint.label.len() == 3));
        assert!(matches!(
            assign_labels_with_options(vec![], "aa", 1),
            Err(HintError::InvalidAlphabet)
        ));
    }

    #[test]
    fn hint_debug_redacts_page_text_urls_and_search_query() {
        let sentinel = "private-hint-sentinel-7d0a";
        let candidate = HintCandidate {
            element_id: 41,
            kind: HintKind::Link,
            frame_path: sentinel.into(),
            text: sentinel.into(),
            href: Some(format!("https://example.test/?q={sentinel}")),
            geometry: HintGeometry {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
        };
        let hint = LabeledHint {
            label: "a".into(),
            candidate: candidate.clone(),
        };
        let mut ids = crate::IdSource::new();
        let target = HintTarget {
            tab: ids.tab(),
            generation: 1,
            document: ids.document(),
        };
        let session = HintSession {
            id: ids.hint_session(),
            target,
            hints: vec![hint.clone()],
        };
        let outcome = HintInteractionOutcome::Activate(candidate.href.clone().unwrap());
        let snapshot = HintInteractionSnapshot {
            mode: HintInputMode::Text,
            prefix: "a".into(),
            query: sentinel.into(),
            matching_labels: vec!["a".into()],
            active_label: Some("a".into()),
            remaining: 1,
            total: 1,
        };
        let interaction =
            HintInteraction::new(vec![hint.clone()], "asdf".into(), HintAutoFollow::Never);
        let error = HintError::UnknownLabel(sentinel.into());

        for projection in [
            format!("{candidate:?}"),
            format!("{hint:?}"),
            format!("{target:?}"),
            format!("{session:?}"),
            format!("{outcome:?}"),
            format!("{snapshot:?}"),
            format!("{interaction:?}"),
            format!("{error:?}"),
            format!("{error}"),
        ] {
            assert!(!projection.contains(sentinel), "{projection}");
        }
    }

    #[test]
    fn hint_error_projections_redact_page_derived_counts_and_positions() {
        let marker = 987_654_321;
        let errors = [
            HintError::TooManyCandidates {
                count: marker,
                maximum: marker - 1,
            },
            HintError::InvalidGeometry { index: marker },
        ];
        let marker_text = marker.to_string();

        for error in errors {
            for projection in [format!("{error:?}"), format!("{error}")] {
                assert!(!projection.contains(marker_text.as_str()), "{projection}");
            }
        }
    }

    #[test]
    fn stable_ids_reuse_labels_until_width_changes() {
        let mut initial_candidates = (0..3)
            .map(|index| {
                let mut value = candidate(f64::from(index), 0.0, &index.to_string());
                value.element_id = index + 1;
                value
            })
            .collect::<Vec<_>>();
        let initial = assign_labels_with_options(initial_candidates.clone(), "as", 2).unwrap();
        initial_candidates.swap(0, 2);
        initial_candidates[0].geometry.x = 30.0;
        initial_candidates[2].geometry.x = 0.0;
        let refreshed = refresh_labels(initial_candidates.clone(), &initial, "as", 2).unwrap();
        for hint in &refreshed {
            let old = initial
                .iter()
                .find(|old| old.candidate.element_id == hint.candidate.element_id)
                .unwrap();
            assert_eq!(hint.label, old.label);
        }

        initial_candidates.push({
            let mut value = candidate(40.0, 0.0, "new");
            value.element_id = 4;
            value
        });
        initial_candidates.push({
            let mut value = candidate(50.0, 0.0, "width change");
            value.element_id = 5;
            value
        });
        let relabeled = refresh_labels(initial_candidates, &initial, "as", 2).unwrap();
        assert!(relabeled.iter().all(|hint| hint.label.len() == 3));
        assert_eq!(relabeled[0].label, "aaa");
    }

    #[test]
    fn interaction_edits_prefix_cycles_and_honors_auto_follow() {
        let hints = assign_labels_with_options(
            (0..3)
                .map(|index| candidate(f64::from(index), 0.0, &index.to_string()))
                .collect(),
            "as",
            2,
        )
        .unwrap();
        let mut model = HintInteraction::new(hints, "as".into(), HintAutoFollow::UniqueMatch);
        assert_eq!(
            model.handle(HintInteractionInput::Character('a')),
            HintInteractionOutcome::Updated
        );
        assert_eq!(model.snapshot().remaining, 2);
        let before = model.snapshot();
        assert_eq!(
            model.handle(HintInteractionInput::Character('x')),
            HintInteractionOutcome::InvalidCharacter('x')
        );
        assert_eq!(model.snapshot(), before);
        assert_eq!(
            model.handle(HintInteractionInput::Character('s')),
            HintInteractionOutcome::Activate("as".into())
        );
    }

    #[test]
    fn every_auto_follow_policy_has_distinct_activation_timing() {
        let hints = assign_labels_with_options(
            (0..3)
                .map(|index| {
                    let mut value = candidate(f64::from(index), 0.0, &index.to_string());
                    value.element_id = index + 1;
                    value
                })
                .collect(),
            "as",
            2,
        )
        .unwrap();

        let mut always = HintInteraction::new(hints.clone(), "as".into(), HintAutoFollow::Always);
        assert_eq!(
            always.handle(HintInteractionInput::Character('s')),
            HintInteractionOutcome::Activate("sa".into())
        );
        let sole =
            HintInteraction::new(vec![hints[0].clone()], "as".into(), HintAutoFollow::Always);
        assert_eq!(sole.initial_activation(), Some("aa"));

        let mut unique =
            HintInteraction::new(hints.clone(), "as".into(), HintAutoFollow::UniqueMatch);
        assert!(unique.initial_activation().is_none());
        assert_eq!(
            unique.handle(HintInteractionInput::Character('s')),
            HintInteractionOutcome::Activate("sa".into())
        );

        let mut full = HintInteraction::new(hints.clone(), "as".into(), HintAutoFollow::FullMatch);
        assert_eq!(
            full.handle(HintInteractionInput::Character('s')),
            HintInteractionOutcome::Updated
        );
        assert_eq!(
            full.handle(HintInteractionInput::Character('a')),
            HintInteractionOutcome::Activate("sa".into())
        );

        let mut never = HintInteraction::new(hints, "as".into(), HintAutoFollow::Never);
        assert_eq!(
            never.handle(HintInteractionInput::Character('s')),
            HintInteractionOutcome::Updated
        );
        assert_eq!(
            never.handle(HintInteractionInput::Character('a')),
            HintInteractionOutcome::Updated
        );
        assert_eq!(
            never.handle(HintInteractionInput::Activate),
            HintInteractionOutcome::Activate("sa".into())
        );
    }

    #[test]
    fn custom_alphabet_matching_preserves_case_and_editing_state() {
        let hints = assign_labels_with_options(
            (0..3)
                .map(|index| {
                    let mut value = candidate(f64::from(index), 0.0, &index.to_string());
                    value.element_id = index + 1;
                    value
                })
                .collect(),
            "AS",
            2,
        )
        .unwrap();
        let mut model = HintInteraction::new(hints, "AS".into(), HintAutoFollow::Never);
        assert_eq!(
            model.handle(HintInteractionInput::Character('A')),
            HintInteractionOutcome::Updated
        );
        let first = model.snapshot().active_label;
        assert_eq!(
            model.handle(HintInteractionInput::Next),
            HintInteractionOutcome::Updated
        );
        assert_ne!(model.snapshot().active_label, first);
        assert_eq!(
            model.handle(HintInteractionInput::Backspace),
            HintInteractionOutcome::Updated
        );
        assert_eq!(model.snapshot().remaining, 3);
        assert_eq!(
            model.handle(HintInteractionInput::Character('a')),
            HintInteractionOutcome::InvalidCharacter('a')
        );
    }

    #[test]
    fn text_mode_ranks_accessible_text_before_url_and_never_auto_activates() {
        let mut text = candidate(0.0, 0.0, "Rust docs");
        text.element_id = 1;
        text.href = Some("https://example.test/other".into());
        let mut url = candidate(20.0, 0.0, "Other");
        url.element_id = 2;
        url.href = Some("https://rust.example/docs".into());
        let hints = assign_labels(vec![url, text]).unwrap();
        let text_label = hints
            .iter()
            .find(|hint| hint.candidate.element_id == 1)
            .unwrap()
            .label
            .clone();
        let mut model = HintInteraction::new(hints, "asdfghjkl".into(), HintAutoFollow::Always);
        assert_eq!(
            model.handle(HintInteractionInput::EnterTextMode),
            HintInteractionOutcome::Updated
        );
        for character in "rust docs".chars() {
            assert_eq!(
                model.handle(HintInteractionInput::Character(character)),
                HintInteractionOutcome::Updated
            );
        }
        assert_eq!(
            model.snapshot().active_label.as_deref(),
            Some(text_label.as_str())
        );
        assert_eq!(
            model.handle(HintInteractionInput::Clear),
            HintInteractionOutcome::Updated
        );
        assert_eq!(
            model.handle(HintInteractionInput::Backspace),
            HintInteractionOutcome::Updated
        );
        assert_eq!(model.snapshot().mode, HintInputMode::Label);
    }

    #[test]
    fn text_mode_handles_unicode_multi_token_queries_and_stable_ties() {
        let mut first = candidate(0.0, 0.0, "Crème Rust documentation");
        first.element_id = 1;
        let mut second = candidate(20.0, 0.0, "Crème Rust documentation");
        second.element_id = 2;
        let hints = assign_labels(vec![second, first]).unwrap();
        let expected = hints[0].label.clone();
        let mut model = HintInteraction::new(hints, "asdfghjkl".into(), HintAutoFollow::Never);
        model.handle(HintInteractionInput::EnterTextMode);
        for character in "crème rust".chars() {
            assert_eq!(
                model.handle(HintInteractionInput::Character(character)),
                HintInteractionOutcome::Updated
            );
        }
        assert_eq!(model.snapshot().remaining, 2);
        assert_eq!(
            model.snapshot().active_label.as_deref(),
            Some(expected.as_str())
        );
    }

    #[test]
    fn cap_geometry_and_stale_target_are_enforced() {
        let too_many = (0..=MAX_HINT_CANDIDATES)
            .map(|_| candidate(0.0, 0.0, "x"))
            .collect();
        assert!(matches!(
            assign_labels(too_many),
            Err(HintError::TooManyCandidates { .. })
        ));
        let mut invalid = candidate(0.0, 0.0, "bad");
        invalid.geometry.width = f64::NAN;
        assert!(matches!(
            assign_labels(vec![invalid]),
            Err(HintError::InvalidGeometry { index: 0 })
        ));

        let mut ids = IdSource::new();
        let target = HintTarget {
            tab: ids.tab(),
            generation: 1,
            document: ids.document(),
        };
        let hints = assign_labels(vec![candidate(0.0, 0.0, "link")]).expect("labels");
        let session = HintSession::new(&mut ids, target, hints);
        let wrong_target = HintTarget {
            generation: 2,
            ..target
        };
        assert!(matches!(
            session.select(
                "a",
                wrong_target,
                "0",
                true,
                candidate(0.0, 0.0, "x").geometry
            ),
            Err(HintError::StaleTarget)
        ));
        assert!(matches!(
            session.select("a", target, "0", false, candidate(0.0, 0.0, "x").geometry),
            Err(HintError::CandidateNotVisible)
        ));
        assert!(matches!(
            session.select(
                "a",
                target,
                "0",
                true,
                candidate(100.0, 100.0, "x").geometry
            ),
            Err(HintError::CandidateNotVisible)
        ));
    }
}

use crate::{DocumentId, HintSessionId, IdSource, TabId};

pub const DEFAULT_HINT_ALPHABET: &[u8] = b"asdfghjkl";
pub const MAX_HINT_CANDIDATES: usize = 5_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HintKind {
    Link,
    Button,
    Input,
    Select,
    Textarea,
    ContentEditable,
    Aria,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HintTarget {
    pub tab: TabId,
    pub generation: u64,
    pub document: DocumentId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HintCandidate {
    pub element_id: u32,
    pub kind: HintKind,
    pub frame_path: String,
    pub text: String,
    pub href: Option<String>,
    pub geometry: HintGeometry,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LabeledHint {
    pub label: String,
    pub candidate: HintCandidate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HintError {
    TooManyCandidates { count: usize, maximum: usize },
    InvalidGeometry { index: usize },
    EmptyLabel,
    UnknownLabel(String),
    StaleTarget,
    WrongFrame,
    CandidateNotVisible,
}

impl std::fmt::Display for HintError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooManyCandidates { count, maximum } => {
                write!(formatter, "hint candidate count {count} exceeds {maximum}")
            }
            Self::InvalidGeometry { index } => {
                write!(formatter, "hint candidate {index} has invalid geometry")
            }
            Self::EmptyLabel => formatter.write_str("hint label is empty"),
            Self::UnknownLabel(label) => write!(formatter, "unknown hint label: {label}"),
            Self::StaleTarget => formatter.write_str("hint target is stale"),
            Self::WrongFrame => formatter.write_str("hint candidate belongs to another frame"),
            Self::CandidateNotVisible => formatter.write_str("hint candidate is no longer visible"),
        }
    }
}

impl std::error::Error for HintError {}

#[derive(Clone, Debug, PartialEq)]
pub struct HintSession {
    pub id: HintSessionId,
    pub target: HintTarget,
    pub hints: Vec<LabeledHint>,
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
    candidates.sort_by(|left, right| {
        left.geometry
            .y
            .total_cmp(&right.geometry.y)
            .then_with(|| left.geometry.x.total_cmp(&right.geometry.x))
            .then_with(|| left.frame_path.cmp(&right.frame_path))
            .then_with(|| left.text.cmp(&right.text))
    });
    let width = label_width(candidates.len());
    Ok(candidates
        .into_iter()
        .enumerate()
        .map(|(index, candidate)| LabeledHint {
            label: encode_label(index, width),
            candidate,
        })
        .collect())
}

fn label_width(count: usize) -> usize {
    if count <= 1 {
        return 1;
    }
    let mut capacity = DEFAULT_HINT_ALPHABET.len();
    let mut width = 1;
    while capacity < count {
        capacity = capacity.saturating_mul(DEFAULT_HINT_ALPHABET.len());
        width += 1;
    }
    width
}

fn encode_label(mut index: usize, width: usize) -> String {
    let base = DEFAULT_HINT_ALPHABET.len();
    let mut bytes = vec![DEFAULT_HINT_ALPHABET[0]; width];
    for slot in bytes.iter_mut().rev() {
        *slot = DEFAULT_HINT_ALPHABET[index % base];
        index /= base;
    }
    String::from_utf8(bytes).expect("hint alphabet is ASCII")
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

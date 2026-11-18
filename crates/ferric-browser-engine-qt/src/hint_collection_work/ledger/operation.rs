//! Fixed identities for collection probes. These are dispatch slots, not
//! additional budgets. Metadata remains closed in the production graph profile.
//! Positions must come from the captured native catalogue; an ordinal grants
//! neither frame association nor authority to select a different native target.
use serde::Deserialize;

use super::{Binding, NativePhase};

const DOCUMENTS: usize = 9;
const EDGES: usize = DOCUMENTS - 1;
const CHILDREN: usize = 64;
const CHECKPOINTS: usize = Checkpoint::Count as usize;
const DOCUMENT_BASE: usize = 2 * CHILDREN;
const FOCUS_BASE: usize = DOCUMENT_BASE + CHECKPOINTS * DOCUMENTS * 2;
const TERMINAL_BASE: usize = FOCUS_BASE + CHECKPOINTS * DOCUMENTS * 2 * CHILDREN;
const CAPTURE_BASE: usize = TERMINAL_BASE + CHECKPOINTS * 3;
const ANCESTRY_BASE: usize = CAPTURE_BASE + EDGES;
const GRAPH_BASE: usize = ANCESTRY_BASE + CHECKPOINTS * EDGES * 2;
const VALIDATION_BASE: usize = GRAPH_BASE + 2 * 4;
const GEOMETRY_BASE: usize = VALIDATION_BASE + DOCUMENTS * 2;
const SLOT_COUNT: usize = GEOMETRY_BASE + 2;
const _: () = assert!(SLOT_COUNT <= 65_536);

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AttachmentPass {
    Cached,
    Initialize,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[repr(u8)]
pub(crate) enum Checkpoint {
    Provider,
    BindTop,
    BindSelected,
    FocusedBeforeFirst,
    FocusedBeforeSecond,
    CaptureAncestry,
    CaptureFinal,
    FallbackTop,
    TopCaptureBefore,
    TopCaptureAfter,
    TopCaptureFocused,
    TopCaptureEntered,
    TopResealBefore,
    TopResealAfter,
    TopResealFocused,
    TopResealEntered,
    TopDependency,
    SelectedCaptureBefore,
    SelectedCaptureAfter,
    SelectedCaptureFocused,
    SelectedCaptureEntered,
    SelectedResealBefore,
    SelectedResealAfter,
    SelectedResealFocused,
    SelectedResealEntered,
    SelectedDependency,
    #[serde(skip)]
    Count,
}

impl Checkpoint {
    fn index(self) -> usize {
        usize::from(self as u8)
    }

    fn matches(self, binding: &Binding) -> bool {
        match self {
            Self::Count => false,
            Self::Provider => binding.phase == NativePhase::Preflight,
            Self::BindTop | Self::FallbackTop => {
                binding.phase == NativePhase::Preflight && binding.owner.is_empty()
            }
            Self::BindSelected
            | Self::FocusedBeforeFirst
            | Self::FocusedBeforeSecond
            | Self::CaptureAncestry
            | Self::CaptureFinal => {
                binding.phase == NativePhase::Preflight && !binding.owner.is_empty()
            }
            _ => {
                binding.phase == NativePhase::Policy
                    && binding.owner.is_empty()
                        == (self.index() < Self::SelectedCaptureBefore.index())
            }
        }
    }

    fn has_ancestry(self) -> bool {
        matches!(self, Self::CaptureFinal)
            || self.index() >= Self::TopCaptureFocused.index()
                && !matches!(self, Self::TopResealBefore | Self::TopResealAfter)
    }

    fn has_focus_scan(self) -> bool {
        self.has_ancestry()
            || matches!(
                self,
                Self::Provider | Self::FocusedBeforeFirst | Self::CaptureAncestry
            )
    }

    fn terminal_passes(self) -> usize {
        match self {
            Self::FocusedBeforeFirst => 3,
            Self::TopCaptureFocused
            | Self::TopResealFocused
            | Self::SelectedCaptureFocused
            | Self::SelectedResealFocused => 2,
            _ => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum Operation {
    Attachment {
        frame: u8,
        pass: AttachmentPass,
    },
    DocumentFacts {
        checkpoint: Checkpoint,
        document: u8,
        pass: u8,
    },
    ChildFocus {
        checkpoint: Checkpoint,
        parent: u8,
        pass: u8,
        child: u8,
    },
    TerminalFocus {
        checkpoint: Checkpoint,
        pass: u8,
    },
    AncestryCapture {
        edge: u8,
    },
    AncestryVerify {
        checkpoint: Checkpoint,
        edge: u8,
        pass: u8,
    },
    Graph {
        pass: u8,
    },
    DocumentValidation {
        document: u8,
        pass: u8,
    },
    Geometry,
}

impl Operation {
    pub(super) fn is_geometry(self) -> bool {
        matches!(self, Self::Geometry)
    }
    pub(super) fn is_graph(self) -> bool {
        matches!(self, Self::Graph { .. })
    }

    // Only these positions belong to the proposed document-probe subtotal.
    // Provider/entered/ancestry work must not obtain funding through it.
    pub(super) fn is_document_probe(self) -> bool {
        match self {
            Self::DocumentValidation { .. } => true,
            Self::DocumentFacts {
                checkpoint,
                document,
                ..
            } => match checkpoint {
                Checkpoint::BindTop
                | Checkpoint::FallbackTop
                | Checkpoint::TopCaptureBefore
                | Checkpoint::TopCaptureAfter
                | Checkpoint::TopResealBefore
                | Checkpoint::TopResealAfter => document == 0,
                Checkpoint::BindSelected
                | Checkpoint::FocusedBeforeFirst
                | Checkpoint::FocusedBeforeSecond
                | Checkpoint::CaptureFinal
                | Checkpoint::TopCaptureFocused
                | Checkpoint::TopResealFocused
                | Checkpoint::TopDependency
                | Checkpoint::SelectedCaptureBefore
                | Checkpoint::SelectedCaptureAfter
                | Checkpoint::SelectedCaptureFocused
                | Checkpoint::SelectedResealBefore
                | Checkpoint::SelectedResealAfter
                | Checkpoint::SelectedResealFocused
                | Checkpoint::SelectedDependency => true,
                _ => false,
            },
            _ => false,
        }
    }

    pub(super) fn id(self, binding: &Binding) -> Option<u16> {
        if !binding.valid() {
            return None;
        }
        let id = match self {
            Self::Attachment { frame, pass }
                if binding.phase == NativePhase::Preflight && usize::from(frame) < CHILDREN =>
            {
                2 * usize::from(frame) + usize::from(matches!(pass, AttachmentPass::Initialize))
            }
            Self::DocumentFacts {
                checkpoint,
                document,
                pass,
            } if checkpoint.matches(binding)
                && !matches!(
                    checkpoint,
                    Checkpoint::Provider | Checkpoint::CaptureAncestry
                )
                && usize::from(document) < DOCUMENTS
                && pass < 2 =>
            {
                DOCUMENT_BASE
                    + (checkpoint.index() * DOCUMENTS + usize::from(document)) * 2
                    + usize::from(pass)
            }
            Self::ChildFocus {
                checkpoint,
                parent,
                pass,
                child,
            } if checkpoint.matches(binding)
                && checkpoint.has_focus_scan()
                && usize::from(parent) < DOCUMENTS
                && pass < 2
                && usize::from(child) < CHILDREN =>
            {
                FOCUS_BASE
                    + ((checkpoint.index() * DOCUMENTS + usize::from(parent)) * 2
                        + usize::from(pass))
                        * CHILDREN
                    + usize::from(child)
            }
            Self::TerminalFocus { checkpoint, pass }
                if checkpoint.matches(binding)
                    && usize::from(pass) < checkpoint.terminal_passes() =>
            {
                TERMINAL_BASE + checkpoint.index() * 3 + usize::from(pass)
            }
            Self::AncestryCapture { edge }
                if binding.phase == NativePhase::Preflight
                    && !binding.owner.is_empty()
                    && usize::from(edge) < EDGES =>
            {
                CAPTURE_BASE + usize::from(edge)
            }
            Self::AncestryVerify {
                checkpoint,
                edge,
                pass,
            } if checkpoint.matches(binding)
                && checkpoint.has_ancestry()
                && usize::from(edge) < EDGES
                && pass < 2 =>
            {
                ANCESTRY_BASE
                    + (checkpoint.index() * EDGES + usize::from(edge)) * 2
                    + usize::from(pass)
            }
            Self::Graph { pass } if binding.phase == NativePhase::Policy && pass < 4 => {
                GRAPH_BASE + usize::from(!binding.owner.is_empty()) * 4 + usize::from(pass)
            }
            Self::DocumentValidation { document, pass }
                if binding.phase == NativePhase::Validation
                    && usize::from(document) < DOCUMENTS
                    && pass < 2 =>
            {
                VALIDATION_BASE + usize::from(document) * 2 + usize::from(pass)
            }
            Self::Geometry if binding.phase == NativePhase::Policy => {
                GEOMETRY_BASE + usize::from(!binding.owner.is_empty())
            }
            _ => return None,
        };
        u16::try_from(id).ok()
    }
}

#[cfg(test)]
mod tests;

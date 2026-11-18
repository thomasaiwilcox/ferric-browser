//! One original-request budget. A native grant reserves capacity before
//! dispatch; only native-established outcomes may return unused capacity.
use std::collections::BTreeMap;

use ferric_browser_config::MAX_INTERACTION_PREDICATE_STEPS;
use ferric_browser_core::{MAX_HINT_CANDIDATES, MAX_HINT_INSPECTIONS};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::interaction_proof::MAX_INTERACTION_ANNOTATION_BYTES;

mod operation;
pub(crate) use operation::Operation;

// Reviewed graph/decode SUBTOTAL, not the complete native schedule or the
// semantic predicate allowance: two worlds, one structural decode and four
// native graph probes each, preserving their existing local 100k safeguards.
// Geometry verification and the derived merge consume this same remaining
// pool; adding either stage never creates another allowance.
pub(crate) const GRAPH_DECODE_VERIFICATION_SUBTOTAL: usize =
    2 * (1 + 2 * 2) * MAX_INTERACTION_PREDICATE_STEPS;
// Document dispatch inventory only, not a complete native work bound.
const DOCUMENT_PROBE_SLOTS: usize = 76 + 62 + 126 + 18;
const MAX_PENDING_NATIVE: usize = 128;

#[derive(Debug, Default)]
pub(crate) struct Used {
    pub(crate) inspections: usize,
    pub(crate) candidates: usize,
    pub(crate) predicate_steps: usize,
    pub(crate) annotation_bytes: usize,
    pub(crate) verification_steps: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeBudget {
    pub(crate) inspections: usize,
    pub(crate) verification_steps: usize,
    pub(crate) annotation_bytes: usize,
}

impl NativeBudget {
    fn fits(self, limit: Self) -> bool {
        self.inspections <= limit.inspections
            && self.verification_steps <= limit.verification_steps
            && self.annotation_bytes <= limit.annotation_bytes
    }

    fn is_empty(self) -> bool {
        self == Self::default()
    }
}

/// Fixed browser schedule input, never a renderer/configuration setting.
/// The current production constructor covers graph/decode only. Native
/// preflight cannot be granted until its complete protocol bounds are supplied.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Schedule {
    pub(crate) verification_limit: usize,
    pub(crate) preflight: NativeBudget,
    profile: Profile,
    document_probes: usize,
}

#[derive(Clone, Copy, Debug)]
enum Profile {
    GraphDecode,
    DocumentProbes,
    #[cfg(test)]
    Prototype,
}

#[cfg(test)]
mod tests;

impl Schedule {
    pub(crate) const fn graph_decode() -> Self {
        Self {
            profile: Profile::GraphDecode,
            document_probes: 0,
            verification_limit: GRAPH_DECODE_VERIFICATION_SUBTOTAL,
            preflight: NativeBudget {
                inspections: 0,
                verification_steps: 0,
                annotation_bytes: 0,
            },
        }
    }

    #[cfg(test)]
    pub(crate) const fn testing(verification_limit: usize, preflight: NativeBudget) -> Self {
        Self {
            verification_limit,
            preflight,
            profile: Profile::Prototype,
            document_probes: 0,
        }
    }

    // Graph ticks plus document dispatch units only. Other native work and
    // scalar/string costs require their own admission and bounds.
    pub(crate) const fn graph_documents() -> Self {
        let mut schedule = Self::graph_decode();
        schedule.preflight = NativeBudget {
            inspections: 76,
            verification_steps: 0,
            annotation_bytes: 0,
        };
        schedule.profile = Profile::DocumentProbes;
        schedule.document_probes = DOCUMENT_PROBE_SLOTS;
        schedule
    }
    #[cfg(test)]
    pub(crate) const fn testing_document_probes() -> Self {
        Self::graph_documents()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum NativePhase {
    Preflight,
    Policy,
    Projection,
    Validation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Binding {
    pub(crate) phase: NativePhase,
    pub(crate) stage: u8,
    pub(crate) owner: String,
}

impl Binding {
    pub(crate) fn valid(&self) -> bool {
        let owner_valid = self.owner.is_empty()
            || self.owner.strip_prefix("frame-").is_some_and(|id| {
                id.len() == 36 && Uuid::parse_str(id).is_ok_and(|value| value.to_string() == id)
            });
        owner_valid
            && match self.phase {
                NativePhase::Preflight | NativePhase::Projection | NativePhase::Validation => {
                    self.stage == 0
                }
                NativePhase::Policy => (1..=4).contains(&self.stage),
            }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Token {
    ledger: Uuid,
    request: String,
    operation: u16,
    binding: Binding,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Grant {
    pub(crate) token: Token,
    pub(crate) limit: NativeBudget,
}

pub(crate) enum NativeOutcome {
    Known(NativeBudget),
    NoScript,
    Unknown,
}

#[derive(Debug)]
pub(crate) struct Ledger {
    identity: Uuid,
    request: u64,
    schedule: Schedule,
    pub(crate) used: Used,
    preflight_used: NativeBudget,
    // Unissued fixed document slots retain headroom inside the SAME 50k pool.
    // A NoScript refund releases its unit, but never makes its slot reusable.
    document_probes_left: usize,
    // A dense, bounded issuance bitmap avoids keeping completed receipts.
    // An operation ID is never reused, even after a proven no-script result.
    issued: Vec<u64>,
    pending: BTreeMap<u16, (Token, NativeBudget)>,
}

impl Ledger {
    pub(crate) fn new(request: u64, schedule: Schedule) -> Option<Box<Self>> {
        if request == 0
            || schedule.verification_limit <= schedule.preflight.verification_steps
            || schedule.preflight.inspections >= MAX_HINT_INSPECTIONS
            || schedule.preflight.annotation_bytes > MAX_INTERACTION_ANNOTATION_BYTES
            || schedule.document_probes >= MAX_HINT_INSPECTIONS
        {
            return None;
        }
        Some(Box::new(Self {
            identity: Uuid::new_v4(),
            request,
            schedule,
            used: Used::default(),
            preflight_used: NativeBudget::default(),
            document_probes_left: schedule.document_probes,
            issued: Vec::new(),
            pending: BTreeMap::new(),
        }))
    }

    pub(crate) fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    pub(crate) fn remaining_native(&self) -> NativeBudget {
        NativeBudget {
            inspections: MAX_HINT_INSPECTIONS - self.used.inspections,
            verification_steps: self.schedule.verification_limit - self.used.verification_steps,
            annotation_bytes: MAX_INTERACTION_ANNOTATION_BYTES - self.used.annotation_bytes,
        }
    }

    pub(crate) fn remaining_producer_inspections(&self) -> usize {
        self.remaining_native()
            .inspections
            .saturating_sub(self.document_probes_left)
    }

    pub(crate) fn reserve_operation(
        &mut self,
        operation: Operation,
        binding: Binding,
        limit: NativeBudget,
    ) -> Option<Grant> {
        match self.schedule.profile {
            Profile::GraphDecode
                if !operation.is_graph()
                    || limit.inspections != 0
                    || limit.annotation_bytes != 0
                    || limit.verification_steps > MAX_INTERACTION_PREDICATE_STEPS =>
            {
                return None;
            }
            Profile::GraphDecode => {}
            Profile::DocumentProbes => {
                let permitted = if operation.is_document_probe() {
                    limit
                        == NativeBudget {
                            inspections: 1,
                            verification_steps: 0,
                            annotation_bytes: 0,
                        }
                } else {
                    (operation.is_graph() || operation.is_geometry())
                        && limit.inspections == 0
                        && limit.annotation_bytes == 0
                        && limit.verification_steps <= MAX_INTERACTION_PREDICATE_STEPS
                };
                if !permitted {
                    return None;
                }
            }
            #[cfg(test)]
            Profile::Prototype => {}
        }
        let document_probe = self.schedule.document_probes != 0 && operation.is_document_probe();
        if document_probe && self.document_probes_left == 0 {
            return None;
        }
        let grant = self.reserve_slot(operation.id(&binding)?, binding, limit)?;
        if document_probe {
            self.document_probes_left -= 1;
        }
        Some(grant)
    }

    // Only tests exercise arbitrary bitmap positions. Production callers must
    // derive the slot through the fixed typed operation boundary.
    #[cfg(test)]
    pub(super) fn reserve(
        &mut self,
        operation: u16,
        binding: Binding,
        limit: NativeBudget,
    ) -> Option<Grant> {
        self.reserve_slot(operation, binding, limit)
    }

    fn reserve_slot(
        &mut self,
        operation: u16,
        binding: Binding,
        limit: NativeBudget,
    ) -> Option<Grant> {
        let word = usize::from(operation) / 64;
        let bit = 1_u64 << (operation % 64);
        if !binding.valid()
            || limit.is_empty()
            || !limit.fits(self.remaining_native())
            || self.pending.len() == MAX_PENDING_NATIVE
            || self.issued.get(word).is_some_and(|value| value & bit != 0)
        {
            return None;
        }
        if binding.phase == NativePhase::Preflight {
            let available = NativeBudget {
                inspections: self.schedule.preflight.inspections - self.preflight_used.inspections,
                verification_steps: self.schedule.preflight.verification_steps
                    - self.preflight_used.verification_steps,
                annotation_bytes: self.schedule.preflight.annotation_bytes
                    - self.preflight_used.annotation_bytes,
            };
            if !limit.fits(available) {
                return None;
            }
            self.preflight_used.inspections += limit.inspections;
            self.preflight_used.verification_steps += limit.verification_steps;
            self.preflight_used.annotation_bytes += limit.annotation_bytes;
        }
        self.issued.resize(self.issued.len().max(word + 1), 0);
        self.issued[word] |= bit;
        self.used.inspections += limit.inspections;
        self.used.verification_steps += limit.verification_steps;
        self.used.annotation_bytes += limit.annotation_bytes;
        let token = Token {
            ledger: self.identity,
            request: self.request.to_string(),
            operation,
            binding,
        };
        self.pending.insert(operation, (token.clone(), limit));
        Some(Grant { token, limit })
    }

    /// Wrong identity/binding leaves the live grant untouched. A matching
    /// malformed/unknown report burns its reservation; late replies cannot
    /// refund it or open a completed operation again.
    pub(crate) fn complete(
        &mut self,
        token: &Token,
        binding: &Binding,
        outcome: NativeOutcome,
    ) -> bool {
        let Some((original, limit)) = self.pending.get(&token.operation) else {
            return false;
        };
        if original != token || &token.binding != binding {
            return false;
        }
        let limit = *limit;
        self.pending.remove(&token.operation);
        let spent = match outcome {
            NativeOutcome::Known(spent) if spent.fits(limit) => spent,
            NativeOutcome::NoScript => NativeBudget::default(),
            NativeOutcome::Known(_) | NativeOutcome::Unknown => return false,
        };
        let unused = NativeBudget {
            inspections: limit.inspections - spent.inspections,
            verification_steps: limit.verification_steps - spent.verification_steps,
            annotation_bytes: limit.annotation_bytes - spent.annotation_bytes,
        };
        self.used.inspections -= unused.inspections;
        self.used.verification_steps -= unused.verification_steps;
        self.used.annotation_bytes -= unused.annotation_bytes;
        if binding.phase == NativePhase::Preflight {
            self.preflight_used.inspections -= unused.inspections;
            self.preflight_used.verification_steps -= unused.verification_steps;
            self.preflight_used.annotation_bytes -= unused.annotation_bytes;
        }
        true
    }

    pub(crate) fn charge_verification(&mut self, steps: usize) -> bool {
        if steps > self.remaining_native().verification_steps {
            self.used.verification_steps = self.schedule.verification_limit;
            return false;
        }
        self.used.verification_steps += steps;
        true
    }

    pub(crate) fn burn_remaining(&mut self, candidates: bool) {
        self.document_probes_left = 0;
        self.used.inspections = MAX_HINT_INSPECTIONS;
        self.used.predicate_steps = MAX_INTERACTION_PREDICATE_STEPS;
        self.used.annotation_bytes = MAX_INTERACTION_ANNOTATION_BYTES;
        self.used.verification_steps = self.schedule.verification_limit;
        if candidates {
            self.used.candidates = MAX_HINT_CANDIDATES;
        }
        self.pending.clear();
    }
}

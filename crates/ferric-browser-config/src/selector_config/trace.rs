//! Opt-in observations of the selector composer's actual operations. These
//! records do not select rules, compile a second policy or confer authority.
use super::{HintSelectorFamily, HintSelectorPolicy, SelectorOverlayEvent};
use crate::PolicyFieldOrigin;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HintSelectorSeedOrigins {
    pub mode: Arc<PolicyFieldOrigin>,
    /// None means the base's empty list came from the setting's typed default.
    pub selectors: Option<Arc<PolicyFieldOrigin>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HintSelectorInputField {
    Mode,
    Selectors,
    Selector(usize),
    DefaultSelectors,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HintSelectorValueOrigin {
    pub source: Arc<PolicyFieldOrigin>,
    pub input: HintSelectorInputField,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HintSelectorFieldOrigins {
    pub mode: Option<HintSelectorValueOrigin>,
    pub selectors: Vec<HintSelectorValueOrigin>,
    /// The cause of a configured empty custom list. Absent for built-in-only
    /// policy and for a populated composed list; mode separately controls use
    /// of the built-in selectors.
    pub empty_list: Option<HintSelectorValueOrigin>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HintSelectorContributionOutcome {
    Applied,
    EffectiveOverflow(String),
    BlockedByEarlierOverflow,
}

impl HintSelectorContributionOutcome {
    pub(super) fn observed(blocked: bool, overflow: Option<&str>) -> Self {
        if blocked {
            Self::BlockedByEarlierOverflow
        } else if let Some(error) = overflow {
            Self::EffectiveOverflow(error.into())
        } else {
            Self::Applied
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HintSelectorDuplicate {
    pub input_index: usize,
    /// Index at this step, before a later replacement can retire it.
    pub retained_index: usize,
    pub retained_origin: HintSelectorValueOrigin,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HintSelectorContribution {
    pub source: Arc<PolicyFieldOrigin>,
    pub policy: HintSelectorPolicy,
    pub selectors_explicit: bool,
    pub outcome: HintSelectorContributionOutcome,
    pub duplicates: Vec<HintSelectorDuplicate>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HintSelectorFamilyTrace {
    pub seed: HintSelectorFieldOrigins,
    pub fields: HintSelectorFieldOrigins,
    pub contributions: Vec<HintSelectorContribution>,
}

pub type HintSelectorCompositionTrace = BTreeMap<HintSelectorFamily, HintSelectorFamilyTrace>;

impl HintSelectorValueOrigin {
    fn new(source: &Arc<PolicyFieldOrigin>, input: HintSelectorInputField) -> Self {
        Self {
            source: Arc::clone(source),
            input,
        }
    }
}

impl HintSelectorFieldOrigins {
    pub(super) fn seed(policy: &HintSelectorPolicy, origins: &HintSelectorSeedOrigins) -> Self {
        let list_source = origins.selectors.as_ref().unwrap_or(&origins.mode);
        Self {
            mode: Some(HintSelectorValueOrigin::new(
                &origins.mode,
                HintSelectorInputField::Mode,
            )),
            selectors: policy
                .selectors
                .iter()
                .enumerate()
                .map(|(index, _)| {
                    HintSelectorValueOrigin::new(
                        list_source,
                        HintSelectorInputField::Selector(index),
                    )
                })
                .collect(),
            empty_list: policy.selectors.is_empty().then(|| {
                HintSelectorValueOrigin::new(
                    list_source,
                    if origins.selectors.is_some() {
                        HintSelectorInputField::Selectors
                    } else {
                        HintSelectorInputField::DefaultSelectors
                    },
                )
            }),
        }
    }

    pub(super) fn observe(
        &mut self,
        event: SelectorOverlayEvent,
        source: &Arc<PolicyFieldOrigin>,
        selectors_explicit: bool,
        duplicates: &mut Vec<HintSelectorDuplicate>,
    ) {
        let empty = || {
            HintSelectorValueOrigin::new(
                source,
                if selectors_explicit {
                    HintSelectorInputField::Selectors
                } else {
                    HintSelectorInputField::DefaultSelectors
                },
            )
        };
        match event {
            SelectorOverlayEvent::Replace { count } => {
                self.mode = Some(HintSelectorValueOrigin::new(
                    source,
                    HintSelectorInputField::Mode,
                ));
                self.selectors = (0..count)
                    .map(|index| {
                        HintSelectorValueOrigin::new(
                            source,
                            HintSelectorInputField::Selector(index),
                        )
                    })
                    .collect();
                self.empty_list = (count == 0).then(empty);
            }
            SelectorOverlayEvent::Extend { inherited: false } => {
                self.mode = Some(HintSelectorValueOrigin::new(
                    source,
                    HintSelectorInputField::Mode,
                ));
                self.empty_list = Some(empty());
            }
            SelectorOverlayEvent::Extend { inherited: true } => {}
            SelectorOverlayEvent::Append { input_index } => {
                self.selectors.push(HintSelectorValueOrigin::new(
                    source,
                    HintSelectorInputField::Selector(input_index),
                ));
                self.empty_list = None;
            }
            SelectorOverlayEvent::Duplicate {
                input_index,
                retained_index,
            } => {
                duplicates.push(HintSelectorDuplicate {
                    input_index,
                    retained_index,
                    retained_origin: self.selectors[retained_index].clone(),
                });
            }
        }
    }
}

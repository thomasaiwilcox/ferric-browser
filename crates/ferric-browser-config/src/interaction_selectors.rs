//! Source selector enumeration for browser-engine syntax checks. This walks
//! authored data without applying enablement, URL matching or ID shadowing.
use crate::{InteractionPolicyPatch, InteractionRootStep};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionSelectorSource<'a> {
    pub path: String,
    pub selector: &'a str,
}

impl InteractionPolicyPatch {
    #[must_use]
    pub fn selector_sources(&self) -> Vec<InteractionSelectorSource<'_>> {
        let mut result = Vec::new();
        for (index, anchor) in self
            .anchors
            .as_deref()
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            let path = format!("anchors[{index}]");
            if let Some(root) = &anchor.root {
                append_steps(&mut result, &format!("{path}.root.steps"), &root.steps);
            }
            for (field, selector) in [
                ("scope_selector", &anchor.scope_selector),
                ("target_selector", &anchor.target_selector),
            ] {
                if let Some(selector) = selector {
                    result.push(InteractionSelectorSource {
                        path: format!("{path}.{field}"),
                        selector,
                    });
                }
            }
        }
        for (index, collection) in self
            .collections
            .as_deref()
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            let path = format!("collections[{index}]");
            if let Some(root) = &collection.root {
                append_steps(&mut result, &format!("{path}.root.steps"), &root.steps);
            }
            if let Some(selector) = &collection.container_selector {
                result.push(InteractionSelectorSource {
                    path: format!("{path}.container_selector"),
                    selector,
                });
            }
            result.push(InteractionSelectorSource {
                path: format!("{path}.item_selector"),
                selector: &collection.item_selector,
            });
            append_steps(
                &mut result,
                &format!("{path}.primary_root_steps"),
                collection.primary_root_steps.as_deref().unwrap_or_default(),
            );
            if let Some(selector) = &collection.primary_selector {
                result.push(InteractionSelectorSource {
                    path: format!("{path}.primary_selector"),
                    selector,
                });
            }
        }
        result
    }
}

fn append_steps<'a>(
    output: &mut Vec<InteractionSelectorSource<'a>>,
    path: &str,
    steps: &'a [InteractionRootStep],
) {
    for (index, step) in steps.iter().enumerate() {
        for (field, selector) in [
            ("host_selector", &step.host_selector),
            ("assigned_selector", &step.assigned_selector),
        ] {
            if let Some(selector) = selector {
                output.push(InteractionSelectorSource {
                    path: format!("{path}[{index}].{field}"),
                    selector,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disabled_policy_enumerates_each_selector_position_with_its_path() {
        let source = r"
enabled=false
[[anchors]]
id='nav'
code='m'
families=['links']
root={start='top-document',steps=[{kind='open-shadow',host_selector='nav-shell'},{kind='assigned-slot',host_selector='slot.nav',assigned_selector='.nav-item'}]}
scope_selector='.nav-scope'
target_selector='a.nav-target'
[[collections]]
id='cards'
families=['buttons']
codes=['1']
root={start='top-document',steps=[{kind='same-origin-frame',host_selector='iframe.cards'}]}
container_selector='.cards'
item_selector='card-item'
primary_root_steps=[{kind='assigned-slot',host_selector='slot.primary',assigned_selector='.primary-item'}]
primary_selector='button.primary'
order='dom'
";
        let policy: InteractionPolicyPatch = toml::from_str(source).unwrap();
        let parsed = policy.validate_authored().unwrap();
        let selectors = policy.selector_sources();
        assert_eq!(selectors.len(), 11);
        assert_eq!(selectors.len(), parsed.selectors.len());
        let paths: Vec<_> = selectors.iter().map(|item| item.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "anchors[0].root.steps[0].host_selector",
                "anchors[0].root.steps[1].host_selector",
                "anchors[0].root.steps[1].assigned_selector",
                "anchors[0].scope_selector",
                "anchors[0].target_selector",
                "collections[0].root.steps[0].host_selector",
                "collections[0].container_selector",
                "collections[0].item_selector",
                "collections[0].primary_root_steps[0].host_selector",
                "collections[0].primary_root_steps[0].assigned_selector",
                "collections[0].primary_selector"
            ]
        );
    }
}

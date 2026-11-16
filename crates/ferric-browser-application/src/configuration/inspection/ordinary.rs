//! Ordinary settings keep their existing global-then-site priority contract.
//! Values come from typed configuration resolution; explanation observes its
//! actual assignments rather than resolving values from a second raw loop.
use super::{
    ConfigurationInspectionInputs,
    model::{
        InspectedSettingContribution, InspectedSettingValue, InspectionBoundary, InspectionOrigin,
        SettingExplanation, SettingInspectionRequest, SettingInspectionValidation,
    },
    service::{ComposedSetting, SemanticInput, SemanticOrigin, SettingMeaning},
};
use ferric_browser_config::{
    Config, PolicyFieldOrigin, PolicySourceIdentity, RuntimeOverrides, apply_runtime_overrides,
    apply_runtime_overrides_traced, matching_site_rules,
};
use std::{collections::BTreeMap, sync::Arc};

type Fields = BTreeMap<Vec<String>, InspectionOrigin>;

pub(super) fn compose(
    inputs: &ConfigurationInspectionInputs,
    request: &SettingInspectionRequest,
    desired: bool,
) -> Result<ComposedSetting, String> {
    match compose_checked(inputs, request, desired) {
        Err(diagnostic) if !desired => known_active(inputs, request, diagnostic),
        result => result,
    }
}

fn known_active(
    inputs: &ConfigurationInspectionInputs,
    request: &SettingInspectionRequest,
    diagnostic: String,
) -> Result<ComposedSetting, String> {
    let mut config = inputs.active.effective.clone();
    if let Some(url) = &request.url {
        let winner = matching_site_rules(&config, url.as_str())
            .into_iter()
            .filter_map(|rule| rule.set.get(&request.key))
            .next_back()
            .cloned();
        if let Some(raw) = winner {
            assign_site_value(&mut config, &request.key, raw)?;
        }
    }
    let value = value_at(&encode(&config)?, &setting_path(&request.key)).cloned();
    Ok(ComposedSetting {
        value: InspectedSettingValue {
            value,
            validation: SettingInspectionValidation::unknown(),
            provenance_diagnostic: Some(diagnostic),
            explanation: None,
        },
        meaning: None,
    })
}

fn compose_checked(
    inputs: &ConfigurationInspectionInputs,
    request: &SettingInspectionRequest,
    desired: bool,
) -> Result<ComposedSetting, String> {
    let boundary = if desired {
        inputs.desired_boundary()
    } else {
        match request.metadata.apply_time {
            "reload" => InspectionBoundary::Reload,
            "startup" | "restart" => InspectionBoundary::Startup,
            _ => InspectionBoundary::Desired,
        }
    };
    let path = setting_path(&request.key);
    let (resolved, mut fields, mut contributions) = resolve_globals(inputs, boundary, &path)?;
    let mut config = if desired {
        resolved.clone()
    } else {
        inputs.active.effective.clone()
    };
    let resolved_value = value_at(&encode(&resolved)?, &path).cloned();
    let actual_value = value_at(&encode(&config)?, &path).cloned();
    let provenance_known = resolved_value == actual_value;
    let selection = apply_sites(
        inputs,
        request,
        desired,
        &mut config,
        &mut fields,
        &mut contributions,
    )?;
    let value = value_at(&encode(&config)?, &path).cloned();
    let mut relative = BTreeMap::new();
    if let Some(value) = &value {
        for leaf in leaves(value, path.clone()) {
            // Typed normalization can turn an atomic raw value into a tagged
            // table, for example a plain downloads.directory path. Its fields
            // retain the assigning ancestor's origin.
            let origin = (0..=leaf.len())
                .rev()
                .find_map(|length| fields.get(&leaf[..length]));
            relative.insert(leaf[path.len()..].to_vec(), origin.cloned());
        }
    }
    let meaning = SettingMeaning {
        fields: relative
            .iter()
            .map(|(path, origin)| {
                (
                    path.clone(),
                    origin.as_ref().map(SemanticOrigin::from),
                    None,
                )
            })
            .collect(),
        inputs: contributions
            .iter()
            .map(|contribution| SemanticInput {
                source: SemanticOrigin::from(&contribution.origin),
                path: contribution.path.clone(),
                input: contribution.input.clone(),
                selectors_explicit: None,
            })
            .collect(),
    };
    Ok(ComposedSetting {
        value: InspectedSettingValue {
            value,
            validation: SettingInspectionValidation::composed(),
            provenance_diagnostic: (!provenance_known)
                .then(|| "captured source composition does not explain the active value".into()),
            explanation: provenance_known.then_some(SettingExplanation::Ordinary {
                fields: relative,
                contributions,
                selection,
            }),
        },
        meaning: provenance_known.then_some(meaning),
    })
}

fn resolve_globals(
    inputs: &ConfigurationInspectionInputs,
    boundary: InspectionBoundary,
    path: &[String],
) -> Result<(Config, Fields, Vec<InspectedSettingContribution>), String> {
    let layers = inputs.layers(boundary);
    layers
        .base_authored
        .validate_base_settings(&layers.base)
        .map_err(|error| error.to_string())?;
    super::super::policy_compilation::checked(layers, inputs.revision_for(boundary))
        .map_err(|error| error.to_string())?;
    let mut config = layers.base.clone();
    let base = encode(&config)?;
    let mut fields = Fields::new();
    let mut contributions = Vec::new();
    seed(
        inputs,
        boundary,
        path,
        &base,
        &mut fields,
        &mut contributions,
    )?;
    let profile = inputs
        .profile()
        .ok_or("inspection profile is unavailable")?;
    for (source, overrides) in [
        (
            PolicySourceIdentity::Profile {
                name: profile.name.clone(),
            },
            &layers.profile,
        ),
        (PolicySourceIdentity::Generated, &layers.runtime),
        (PolicySourceIdentity::CommandLine, &layers.command_line),
        (PolicySourceIdentity::Temporary, &layers.temporary),
    ] {
        let applied = apply_runtime_overrides_traced(&config, overrides)
            .map_err(|error| error.to_string())?;
        for assignment in applied.assignments {
            if !overlaps(path, &assignment.path) {
                continue;
            }
            let origin = inputs.global_origin(boundary, source.clone())?;
            fields.retain(|field, _| !field.starts_with(&assignment.path));
            for leaf in leaves(&assignment.assigned, assignment.path.clone()) {
                fields.insert(leaf, origin.clone());
            }
            contributions.push(InspectedSettingContribution {
                origin,
                path: assignment.path,
                input: assignment.input,
            });
        }
        config = applied.configuration;
    }
    Ok((config, fields, contributions))
}

fn seed(
    inputs: &ConfigurationInspectionInputs,
    boundary: InspectionBoundary,
    path: &[String],
    base: &toml::Value,
    fields: &mut Fields,
    contributions: &mut Vec<InspectedSettingContribution>,
) -> Result<(), String> {
    let catalogue = &inputs.layers(boundary).base_authored;
    if catalogue.documents().is_empty() {
        if let Some(value) = value_at(base, path) {
            let origin = inputs.global_origin(boundary, PolicySourceIdentity::InMemory)?;
            for leaf in leaves(value, path.to_vec()) {
                fields.insert(leaf, origin.clone());
            }
            contributions.push(InspectedSettingContribution {
                origin,
                path: path.to_vec(),
                input: value.clone(),
            });
        }
        return Ok(());
    }
    // Attribute only actual merged leaves to the last explicit raw leaf. Empty
    // include tables cannot erase an earlier nonempty table's leaf origins.
    if let Some(value) = value_at(base, path) {
        for leaf in leaves(value, path.to_vec()) {
            for (index, document) in catalogue.documents().iter().enumerate().rev() {
                if authored_field_value(document.settings(), &leaf).is_some() {
                    let document_id =
                        u32::try_from(index).map_err(|_| "source catalogue is too large")?;
                    fields.insert(
                        leaf,
                        inputs.global_origin(
                            boundary,
                            PolicySourceIdentity::Authored { document_id },
                        )?,
                    );
                    break;
                }
            }
        }
    }
    for (index, document) in catalogue.documents().iter().enumerate() {
        if let Some(value) = authored_value(document.settings(), path) {
            let document_id = u32::try_from(index).map_err(|_| "source catalogue is too large")?;
            contributions.push(InspectedSettingContribution {
                origin: inputs
                    .global_origin(boundary, PolicySourceIdentity::Authored { document_id })?,
                path: path.to_vec(),
                input: value.clone(),
            });
        }
    }
    Ok(())
}

fn apply_sites(
    inputs: &ConfigurationInspectionInputs,
    request: &SettingInspectionRequest,
    desired: bool,
    config: &mut Config,
    fields: &mut Fields,
    contributions: &mut Vec<InspectedSettingContribution>,
) -> Result<Option<ferric_browser_config::SitePolicySelectionTrace>, String> {
    let Some(url) = &request.url else {
        return Ok(None);
    };
    let boundary = if desired {
        inputs.desired_boundary()
    } else {
        InspectionBoundary::Reload
    };
    let layers = inputs.layers(boundary);
    let checked = super::super::policy_compilation::checked(layers, inputs.revision_for(boundary))
        .map_err(|error| error.to_string())?;
    let (selected, selection) = checked.rules.select_with_trace(url.as_str());
    let path = setting_path(&request.key);
    let mut winner = None;
    // Ordinary consumers retain the existing priority-then-flat-array order.
    // Common selection supplies identities and whole-ID shadow diagnostics.
    for rule in matching_site_rules(config, url.as_str()) {
        if let Some(raw) = rule.set.get(&request.key) {
            let source = selected
                .selected()
                .iter()
                .find(|selected| selected.rule() == rule)
                .ok_or("active ordinary site rule does not belong to the captured source feed")?;
            let origin = inputs.origin(boundary, Arc::new(source.origin().clone()))?;
            contributions.push(InspectedSettingContribution {
                origin: origin.clone(),
                path: path.clone(),
                input: raw.clone(),
            });
            winner = Some((origin, raw.clone()));
        }
    }
    if let Some((origin, raw)) = winner {
        assign_site_value(config, &request.key, raw)?;
        fields.retain(|field, _| !field.starts_with(&path));
        if let Some(value) = value_at(&encode(config)?, &path) {
            for leaf in leaves(value, path) {
                fields.insert(leaf, origin.clone());
            }
        }
    }
    Ok(Some(selection))
}

fn assign_site_value(config: &mut Config, key: &str, raw: toml::Value) -> Result<(), String> {
    let mut overrides = RuntimeOverrides::default();
    overrides
        .set_setting(key, raw)
        .map_err(|error| error.to_string())?;
    *config = apply_runtime_overrides(config, &overrides).map_err(|error| error.to_string())?;
    Ok(())
}

impl ConfigurationInspectionInputs {
    fn global_origin(
        &self,
        boundary: InspectionBoundary,
        source: PolicySourceIdentity,
    ) -> Result<InspectionOrigin, String> {
        self.origin(
            boundary,
            Arc::new(PolicyFieldOrigin::shared(
                source,
                self.revision_for(boundary),
            )?),
        )
    }

    pub(super) fn revision_for(&self, boundary: InspectionBoundary) -> u64 {
        match boundary {
            InspectionBoundary::Desired => self.sources.revision(),
            InspectionBoundary::Reload => self.sources.reload_revision(),
            InspectionBoundary::Startup => self.sources.startup_revision(),
            InspectionBoundary::Candidate => self
                .candidate
                .as_ref()
                .expect("candidate boundary requires captured candidate")
                .revision(),
        }
    }
}

fn encode(config: &Config) -> Result<toml::Value, String> {
    toml::Value::try_from(config).map_err(|error| error.to_string())
}

pub(super) fn setting_path(key: &str) -> Vec<String> {
    if key.starts_with("bindings.") {
        key.splitn(3, '.').map(str::to_owned).collect()
    } else {
        key.split('.').map(str::to_owned).collect()
    }
}

fn value_at<'a>(value: &'a toml::Value, path: &[String]) -> Option<&'a toml::Value> {
    path.iter().try_fold(value, |value, part| value.get(part))
}

fn authored_value<'a>(
    settings: &'a BTreeMap<String, toml::Value>,
    path: &[String],
) -> Option<&'a toml::Value> {
    settings.iter().find_map(|(key, value)| {
        let prefix = setting_path(key);
        path.strip_prefix(prefix.as_slice())
            .and_then(|tail| value_at(value, tail))
    })
}

fn authored_field_value<'a>(
    settings: &'a BTreeMap<String, toml::Value>,
    path: &[String],
) -> Option<&'a toml::Value> {
    settings.iter().find_map(|(key, value)| {
        let prefix = setting_path(key);
        path.strip_prefix(prefix.as_slice()).and_then(|tail| {
            let mut current = value;
            for part in tail {
                if let Some(table) = current.as_table() {
                    current = table.get(part)?;
                } else {
                    return Some(current);
                }
            }
            Some(current)
        })
    })
}

fn overlaps(left: &[String], right: &[String]) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

fn leaves(value: &toml::Value, path: Vec<String>) -> Vec<Vec<String>> {
    if let Some(table) = value.as_table().filter(|table| !table.is_empty()) {
        table
            .iter()
            .flat_map(|(key, value)| {
                let mut child = path.clone();
                child.push(key.clone());
                leaves(value, child)
            })
            .collect()
    } else {
        vec![path]
    }
}

//! Optional observations of the existing runtime assignment operations. No
//! partial history escapes when raw or final typed validation rejects a layer.
use super::Config;

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeOverrideAssignment {
    /// Literal table components. Binding keychains may themselves contain dots.
    pub path: Vec<String>,
    pub input: toml::Value,
    /// Value assigned by the operation, before final Config deserialization.
    /// Consumers obtain the final typed value from `configuration` instead.
    pub assigned: toml::Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AppliedRuntimeOverrides {
    pub configuration: Config,
    pub assignments: Vec<RuntimeOverrideAssignment>,
}

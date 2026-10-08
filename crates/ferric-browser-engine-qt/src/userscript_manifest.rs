use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u64,
    pub name: String,
    pub executable: String,
    #[serde(default)]
    pub argv: Vec<String>,
    #[serde(default)]
    pub context_fields: Vec<String>,
    #[serde(default)]
    pub allowed_results: Vec<String>,
    #[serde(default)]
    pub allowed_commands: Vec<String>,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    #[serde(default)]
    pub allow_private: bool,
    #[serde(default)]
    pub actions: Vec<ActionManifest>,
    #[serde(default, alias = "match")]
    pub matches: Vec<String>,
    #[serde(default, alias = "exclude")]
    pub excludes: Vec<String>,
    #[serde(default)]
    pub run_at: RunAt,
    #[serde(default)]
    pub frames: FrameScope,
    #[serde(default)]
    pub page_world: bool,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub source: Option<String>,
}

impl std::fmt::Debug for Manifest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Manifest")
            .field("schema_version", &self.schema_version)
            .field("timeout_seconds", &self.timeout_seconds)
            .field("allow_private", &self.allow_private)
            .field("run_at", &self.run_at)
            .field("frames", &self.frames)
            .field("page_world", &self.page_world)
            .field("enabled", &self.enabled)
            .field("action_count", &self.actions.len())
            .field("match_count", &self.matches.len())
            .field("exclude_count", &self.excludes.len())
            .field("context_field_count", &self.context_fields.len())
            .field("allowed_result_count", &self.allowed_results.len())
            .field("allowed_command_count", &self.allowed_commands.len())
            .field("has_source", &self.source.is_some())
            .finish()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RunAt {
    #[serde(rename = "document_start")]
    Start,
    #[serde(rename = "document_end")]
    End,
    #[default]
    #[serde(rename = "document_idle")]
    Idle,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FrameScope {
    #[default]
    Top,
    All,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActionManifest {
    pub id: String,
    pub subject: String,
    pub verb: String,
    pub label: String,
    #[serde(default)]
    pub required_fields: Vec<String>,
}

impl std::fmt::Debug for ActionManifest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ActionManifest")
            .field("identity_and_label", &"[REDACTED]")
            .field("required_field_count", &self.required_fields.len())
            .finish()
    }
}

#[derive(Clone)]
pub struct LoadedManifest {
    pub manifest: Manifest,
    pub path: PathBuf,
    pub executable: PathBuf,
}

impl std::fmt::Debug for LoadedManifest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LoadedManifest")
            .field("manifest", &self.manifest)
            .field("path", &"[REDACTED]")
            .field("executable", &"[REDACTED]")
            .finish()
    }
}

const fn default_timeout() -> u64 {
    30
}

const fn default_enabled() -> bool {
    true
}

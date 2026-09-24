use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActionManifest {
    pub id: String,
    pub subject: String,
    pub verb: String,
    pub label: String,
    #[serde(default)]
    pub required_fields: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct LoadedManifest {
    pub manifest: Manifest,
    pub path: PathBuf,
    pub executable: PathBuf,
}

const fn default_timeout() -> u64 {
    30
}

const fn default_enabled() -> bool {
    true
}

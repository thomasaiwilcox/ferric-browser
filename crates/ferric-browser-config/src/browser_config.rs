//! Browser behavior configuration records.

use super::{
    CollisionPolicy, Deserialize, EntryMode, ExternalLinks, LastClose, LogLevel,
    PermissionDecision, PortalMode, Serialize, SessionRestore, default_builtin, default_checkpoint,
    default_command_limit, default_count_limit, default_keychain_timeout, default_languages,
    default_lists, default_log_size, default_new_tab, default_overlay_delay,
    default_related_position, default_retained_files, default_retention, default_search_engine,
    default_start_pages, default_true, default_undo_limit, default_update_interval, default_zoom,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputConfig {
    #[serde(default)]
    pub entry_mode: EntryMode,
    #[serde(default = "default_true")]
    pub auto_insert: bool,
    #[serde(default = "default_keychain_timeout")]
    pub keychain_timeout_ms: u64,
    #[serde(default = "default_count_limit")]
    pub count_limit: u32,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            entry_mode: EntryMode::default(),
            auto_insert: true,
            keychain_timeout_ms: default_keychain_timeout(),
            count_limit: default_count_limit(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryConfig {
    #[serde(default = "default_true")]
    pub keychain_overlay: bool,
    #[serde(default = "default_overlay_delay")]
    pub keychain_overlay_delay_ms: u64,
    #[serde(default)]
    pub learning_mode: bool,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            keychain_overlay: true,
            keychain_overlay_delay_ms: default_overlay_delay(),
            learning_mode: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NavigationConfig {
    #[serde(default = "default_search_engine")]
    pub default_search: String,
    #[serde(default = "default_start_pages")]
    pub start_pages: Vec<String>,
    #[serde(default = "default_new_tab")]
    pub new_tab: String,
    #[serde(default)]
    pub external_links: ExternalLinks,
}

impl Default for NavigationConfig {
    fn default() -> Self {
        Self {
            default_search: default_search_engine(),
            start_pages: default_start_pages(),
            new_tab: default_new_tab(),
            external_links: ExternalLinks::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkCleaningConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_builtin")]
    pub rules: String,
    #[serde(default = "default_true")]
    pub confirm_navigation: bool,
    /// Optional explicit HTTPS source for a signed-by-configuration rule
    /// manifest. It is never contacted unless the user invokes the update
    /// command.
    #[serde(default)]
    pub update_source: Option<String>,
    /// SHA-256 of the exact manifest bytes expected from `update_source`.
    #[serde(default)]
    pub update_sha256: Option<String>,
}

impl Default for LinkCleaningConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            rules: default_builtin(),
            confirm_navigation: true,
            update_source: None,
            update_sha256: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinksConfig {
    #[serde(default)]
    pub cleaning: LinkCleaningConfig,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TabsConfig {
    #[serde(default)]
    pub last_close: LastClose,
    #[serde(default = "default_related_position")]
    pub related_position: String,
    #[serde(default = "default_undo_limit")]
    pub undo_limit: u32,
    #[serde(default)]
    pub auto_discard: bool,
}

impl Default for TabsConfig {
    fn default() -> Self {
        Self {
            last_close: LastClose::default(),
            related_position: default_related_position(),
            undo_limit: default_undo_limit(),
            auto_discard: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionConfig {
    #[serde(default)]
    pub restore: SessionRestore,
    #[serde(default = "default_true")]
    pub lazy_restore: bool,
    #[serde(default = "default_checkpoint")]
    pub checkpoint_seconds: u64,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            restore: SessionRestore::default(),
            lazy_restore: true,
            checkpoint_seconds: default_checkpoint(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwitcherConfig {
    #[serde(default = "default_undo_limit")]
    pub max_results: u32,
    #[serde(default)]
    pub include_private: bool,
}

impl Default for SwitcherConfig {
    fn default() -> Self {
        Self {
            max_results: default_undo_limit(),
            include_private: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryConfig {
    #[serde(default = "default_retention")]
    pub retention_days: u32,
    #[serde(default = "default_command_limit")]
    pub command_limit: u32,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            retention_days: default_retention(),
            command_limit: default_command_limit(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Autoplay {
    #[default]
    EngineDefault,
    RequireGesture,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentConfig {
    #[serde(default = "default_true")]
    pub javascript: bool,
    #[serde(default = "default_true")]
    pub images: bool,
    #[serde(default)]
    pub force_dark: bool,
    #[serde(default)]
    pub autoplay: Autoplay,
    #[serde(default = "default_zoom")]
    pub zoom: f64,
}

impl Default for ContentConfig {
    fn default() -> Self {
        Self {
            javascript: true,
            images: true,
            force_dark: false,
            autoplay: Autoplay::default(),
            zoom: default_zoom(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivacyConfig {
    #[serde(default)]
    pub remote_suggestions: bool,
    #[serde(default)]
    pub push_service: bool,
    #[serde(default)]
    pub private_history_suggestions: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PermissionsConfig {
    #[serde(default)]
    pub camera: PermissionDecision,
    #[serde(default)]
    pub microphone: PermissionDecision,
    #[serde(default)]
    pub screen_capture: PermissionDecision,
    #[serde(default)]
    pub notifications: PermissionDecision,
    #[serde(default)]
    pub geolocation: PermissionDecision,
    #[serde(default)]
    pub clipboard: PermissionDecision,
    #[serde(default)]
    pub local_fonts: PermissionDecision,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockingConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub network_filtering: bool,
    #[serde(default)]
    pub cosmetic_filtering: bool,
    #[serde(default = "default_update_interval")]
    pub update_interval_hours: u32,
    #[serde(default = "default_lists")]
    pub lists: Vec<String>,
    /// Exact hosts or one-label wildcard suffixes that bypass network
    /// blocking for this profile. This is a generated/runtime-friendly
    /// durable setting; it never grants permissions or weakens TLS.
    #[serde(default)]
    pub bypass_sites: Vec<String>,
    /// Exact hosts or one-label wildcard suffixes for security-sensitive
    /// network denial. These rules cannot be bypassed by site exceptions.
    #[serde(default)]
    pub security_deny_hosts: Vec<String>,
}

impl Default for BlockingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            network_filtering: true,
            cosmetic_filtering: false,
            update_interval_hours: default_update_interval(),
            lists: default_lists(),
            bypass_sites: Vec::new(),
            security_deny_hosts: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DownloadDirectory {
    #[default]
    XdgDownloads,
    Path(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DownloadsConfig {
    #[serde(default)]
    pub directory: DownloadDirectory,
    #[serde(default = "default_true")]
    pub ask_destination: bool,
    #[serde(default)]
    pub collision: CollisionPolicy,
    #[serde(default)]
    pub open_when_complete: bool,
}

impl Default for DownloadsConfig {
    fn default() -> Self {
        Self {
            directory: DownloadDirectory::default(),
            ask_destination: true,
            collision: CollisionPolicy::default(),
            open_when_complete: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpellcheckConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_languages")]
    pub languages: Vec<String>,
}

impl Default for SpellcheckConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            languages: default_languages(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesktopConfig {
    #[serde(default)]
    pub portals: PortalMode,
    #[serde(default = "default_true")]
    pub notifications: bool,
    #[serde(default = "default_true")]
    pub media_keys: bool,
}

impl Default for DesktopConfig {
    fn default() -> Self {
        Self {
            portals: PortalMode::default(),
            notifications: true,
            media_keys: true,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TriState {
    #[default]
    Auto,
    On,
    Off,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HyprlandConfig {
    #[serde(default)]
    pub enabled: TriState,
    #[serde(default)]
    pub workspace_routing: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SiteDoctorConfig {
    #[serde(default = "default_true")]
    pub temporary_experiments: bool,
}

impl Default for SiteDoctorConfig {
    fn default() -> Self {
        Self {
            temporary_experiments: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IpcConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub private_queries: bool,
}

impl Default for IpcConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            private_queries: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionTargetConfig {
    #[serde(default)]
    pub subject_types: Vec<String>,
    pub executable: String,
    #[serde(default)]
    pub argv: Vec<String>,
    #[serde(default)]
    pub detach: bool,
    #[serde(default)]
    pub allow_private: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolsConfig {
    /// Direct argv for trusted external editing. The complete argument
    /// `{file}` is replaced with the managed file path at invocation time.
    #[serde(default)]
    pub editor: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoggingConfig {
    #[serde(default)]
    pub level: LogLevel,
    #[serde(default = "default_log_size")]
    pub max_file_mib: u32,
    #[serde(default = "default_retained_files")]
    pub retained_files: u32,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::default(),
            max_file_mib: default_log_size(),
            retained_files: default_retained_files(),
        }
    }
}

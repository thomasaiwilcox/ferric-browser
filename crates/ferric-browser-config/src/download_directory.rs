//! One download directory representation for configuration and runtime edits.
use crate::ConfigError;
use serde::{Deserialize, Serialize, de::Error};
use std::path::Path;

const MAX_DIRECTORY_BYTES: usize = 16 * 1024;
const EXPECTED: &str = "downloads.directory must be xdg-downloads or an absolute folder path without control characters";

/// Use the desktop Downloads folder or an explicitly selected absolute folder.
/// Serialization preserves the existing tagged path format. Plain path strings
/// are accepted by both startup configuration and runtime editors.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DownloadDirectory {
    #[default]
    XdgDownloads,
    Path(String),
}

impl DownloadDirectory {
    pub(crate) fn validate(&self) -> Result<(), ConfigError> {
        if let Self::Path(path) = self
            && (path.len() > MAX_DIRECTORY_BYTES
                || !Path::new(path).is_absolute()
                || path.chars().any(char::is_control))
        {
            return Err(ConfigError::Validation(EXPECTED.into()));
        }
        Ok(())
    }
}
impl<'de> Deserialize<'de> for DownloadDirectory {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct CustomPath {
            path: String,
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Input {
            Text(String),
            Custom(CustomPath),
        }
        let directory =
            match Input::deserialize(deserializer).map_err(|_| D::Error::custom(EXPECTED))? {
                Input::Text(value) if value == "xdg-downloads" => Self::XdgDownloads,
                Input::Text(value) => Self::Path(value),
                Input::Custom(value) => Self::Path(value.path),
            };
        directory.validate().map_err(D::Error::custom)?;
        Ok(directory)
    }
}

/// Keep generated overrides canonical while limiting structured values to this setting.
pub(crate) fn normalize(value: toml::Value) -> Result<toml::Value, ConfigError> {
    let directory: DownloadDirectory = value
        .try_into()
        .map_err(|_| ConfigError::Validation(EXPECTED.into()))?;
    toml::Value::try_from(directory).map_err(|_| ConfigError::Validation(EXPECTED.into()))
}

#[cfg(test)]
#[path = "download_directory_tests.rs"]
mod tests;

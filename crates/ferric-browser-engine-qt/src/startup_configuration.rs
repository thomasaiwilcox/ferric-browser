//! Native startup ownership. The `QVariant` carries only an opaque shared handle;
//! source paths and authored policy never become QML properties or JSON data.
use std::{pin::Pin, sync::Arc};

use cxx_qt::CxxQtType;
use cxx_qt_lib::{QString, QVariant};
use ferric_browser_config::{Config, ConfigurationSources};

use crate::qobject;

pub(crate) struct StartupConfiguration {
    base: Config,
    authored: Arc<ConfigurationSources>,
}

/// Captures the already-loaded base and its exact source catalogue for GUI
/// construction. Copies of the returned value share ownership; no file is read.
#[must_use]
pub fn startup_configuration_value(base: Config, authored: Arc<ConfigurationSources>) -> QVariant {
    qobject::ferric_startup_configuration_value(Box::new(StartupConfiguration { base, authored }))
}

/// Equivalent typed capture for native embedders before QML construction.
/// The main Rust executable supplies its existing `LoadedConfig` instead.
pub(crate) fn load_startup_configuration(path: &QString) -> Result<QVariant, String> {
    let loaded =
        ferric_browser_config::load(path.to_string()).map_err(|error| error.to_string())?;
    Ok(startup_configuration_value(loaded.config, loaded.authored))
}

pub(crate) fn startup_base_json(captured: &StartupConfiguration) -> QString {
    QString::from(serde_json::to_string(&captured.base).unwrap_or_default())
}

pub(crate) fn install_startup_configuration(
    captured: &StartupConfiguration,
    mut ui: Pin<&mut qobject::BrowserUi>,
    base_json: &QString,
) -> bool {
    let Ok(base) = serde_json::from_str::<Config>(&base_json.to_string()) else {
        return false;
    };
    if base != captured.base {
        return false;
    }
    ui.as_mut().rust_mut().get_mut().base_authored_input = captured.authored.clone();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_value_owns_the_exact_catalogue_until_its_last_copy_drops() {
        let sources = Arc::new(ConfigurationSources::default());
        let weak = Arc::downgrade(&sources);
        let value = startup_configuration_value(Config::default(), sources);
        let copy = value.clone();
        assert_eq!(weak.strong_count(), 1);
        drop(value);
        assert_eq!(weak.strong_count(), 1);
        drop(copy);
        assert!(weak.upgrade().is_none());
    }
}

use super::{Path, PathBuf, QString, QUrl, qobject};

impl qobject::BrowserUi {
    // Qt invokables require a receiver even for pure URL conversion.
    #[allow(clippy::unused_self)]
    pub(super) fn local_file_path(&self, url: &QUrl) -> QString {
        qobject::ferric_browser_local_file_path(url)
    }

    // A suggested name is a filesystem path, not an already encoded URL.
    // Resolve bare names in the chooser's folder without inspecting the disk.
    #[allow(clippy::unused_self)]
    pub(super) fn file_dialog_default_url(&self, name: &QString, folder: &QUrl) -> QUrl {
        let name = name.to_string();
        if name.is_empty() || name.contains('\0') {
            return QUrl::default();
        }
        let name = Path::new(&name);
        let path = if name.is_absolute() {
            name.to_path_buf()
        } else {
            let folder = qobject::ferric_browser_local_file_path(folder).to_string();
            if folder.is_empty() {
                return QUrl::default();
            }
            Path::new(&folder).join(name)
        };
        // Linux repeated leading slashes remain a local path, not a UNC host.
        let normalized = path.components().collect::<PathBuf>();
        let Some(path) = normalized.to_str() else {
            return QUrl::default();
        };
        QUrl::from_local_file(&QString::from(path))
    }
}

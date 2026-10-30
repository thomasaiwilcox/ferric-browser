//! GUI bootstrap retains the `QGuiApplication` API while enabling native dialogs.
use cxx_qt_lib::{QByteArray, QGuiApplication, QVector};

#[must_use]
pub fn new_gui_application() -> cxx::UniquePtr<QGuiApplication> {
    new_gui_application_with_network_policy(false)
}

/// Declares the process-wide route before any profile or `WebContents` exists.
/// Direct mode rejects conflicting proxy environment/Chromium options.
#[must_use]
pub fn new_gui_application_with_network_policy(
    direct_network: bool,
) -> cxx::UniquePtr<QGuiApplication> {
    let mut arguments = QVector::<QByteArray>::default();
    for argument in std::env::args_os() {
        #[cfg(unix)]
        use std::os::unix::ffi::OsStrExt;
        #[cfg(not(unix))]
        let argument = argument.to_string_lossy();
        arguments.append(QByteArray::from(argument.as_bytes()));
    }
    let application = super::qobject::ferric_browser_gui_application(&arguments);
    if !application.is_null() && !super::qobject::capture_credential_network_policy(direct_network)
    {
        return cxx::UniquePtr::null();
    }
    application
}

//! GUI bootstrap retains the `QGuiApplication` API while enabling native dialogs.
use cxx_qt_lib::{QByteArray, QGuiApplication, QVector};

#[must_use]
pub fn new_gui_application() -> cxx::UniquePtr<QGuiApplication> {
    let mut arguments = QVector::<QByteArray>::default();
    for argument in std::env::args_os() {
        #[cfg(unix)]
        use std::os::unix::ffi::OsStrExt;
        #[cfg(not(unix))]
        let argument = argument.to_string_lossy();
        arguments.append(QByteArray::from(argument.as_bytes()));
    }
    super::qobject::ferric_browser_gui_application(&arguments)
}

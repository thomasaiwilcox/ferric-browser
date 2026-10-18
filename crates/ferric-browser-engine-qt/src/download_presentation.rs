//! Typed presentation boundary for profile-session download rows.
use cxx_qt_lib::{QList, QMap, QMapPair_QString_QVariant, QString, QVariant};
use ferric_browser_application::DownloadRow;

pub(super) fn project(rows: &[DownloadRow]) -> QVariant {
    let mut values = QList::<QVariant>::default();
    for row in rows {
        let mut value = QMap::<QMapPair_QString_QVariant>::default();
        for (key, text) in [
            ("id", row.record.id.as_str()),
            ("state", row.record.state.as_str()),
            ("destination", row.record.destination.as_str()),
            ("displayName", row.display_name.as_str()),
            ("bytes", &row.record.bytes_received.to_string()),
            ("total", &row.total_bytes.to_string()),
            (
                "reason",
                row.failure
                    .as_ref()
                    .map_or("", ferric_browser_application::DownloadFailure::message),
            ),
        ] {
            value.insert(QString::from(key), QVariant::from(&QString::from(text)));
        }
        for (key, flag) in [
            ("active", row.active),
            ("fileReady", row.file_ready),
            ("finishing", row.finishing),
            (
                "retryAvailable",
                row.retry == ferric_browser_application::DownloadRetryState::Idle,
            ),
            (
                "retryPending",
                matches!(
                    row.retry,
                    ferric_browser_application::DownloadRetryState::Preparing
                        | ferric_browser_application::DownloadRetryState::Running
                ),
            ),
        ] {
            value.insert(QString::from(key), QVariant::from(&flag));
        }
        values.append(QVariant::from(&value));
    }
    QVariant::from(&values)
}

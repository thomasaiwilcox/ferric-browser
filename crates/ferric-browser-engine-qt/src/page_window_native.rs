//! Mapping-time identity read from the actual Qt window, never presentation data.
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct NativeWindowIdentity {
    pub(super) tag: String,
    pub(super) app_id: String,
    pub(super) pid: u32,
}

impl NativeWindowIdentity {
    pub(super) fn capture(ui: &crate::qobject::BrowserUi) -> Option<Self> {
        let values = crate::qobject::ferric_browser_native_page_window_identity(ui)
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let [tag, app_id] = values.as_slice() else {
            return None;
        };
        Self::checked(tag, app_id, std::process::id())
    }

    pub(super) fn checked(tag: &str, app_id: &str, pid: u32) -> Option<Self> {
        let uuid = crate::Uuid::parse_str(tag.strip_prefix("ferric-page-")?).ok()?;
        if uuid.is_nil()
            || tag.len() != 48
            || pid == 0
            || app_id.is_empty()
            || app_id.len() > 256
            || !app_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            return None;
        }
        Some(Self {
            tag: tag.into(),
            app_id: app_id.into(),
            pid,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_bounded_native_marker_and_desktop_id_shapes_are_admitted() {
        let tag = format!("ferric-page-{}", crate::Uuid::new_v4());
        assert!(
            NativeWindowIdentity::checked(&tag, "io.github.ferricbrowser.FerricBrowser", 7)
                .is_some()
        );
        assert!(NativeWindowIdentity::checked(&tag, "application_tests", 7).is_some());
        for invalid in [
            "",
            "page-title",
            "ferric-page-00000000-0000-0000-0000-000000000000",
            "ferric-page-7",
        ] {
            assert!(NativeWindowIdentity::checked(invalid, "application_tests", 7).is_none());
        }
        assert!(NativeWindowIdentity::checked(&tag, "bad\nidentity", 7).is_none());
        assert!(NativeWindowIdentity::checked(&tag, "app; execute()", 7).is_none());
        assert!(NativeWindowIdentity::checked(&tag, "app", 0).is_none());
    }
}

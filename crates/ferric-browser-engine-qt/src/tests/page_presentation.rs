use ferric_browser_application::{PageOpenIntent, PageOpenPrivacy};
use ferric_browser_config::TabsPresentation;

use crate::{IpcOpenTarget, browser_ui_ipc::page_open_intent};

#[test]
fn t103_open_targets_map_once_and_keep_background_inactive() {
    let windows = [
        (IpcOpenTarget::Current, PageOpenIntent::CurrentPage),
        (
            IpcOpenTarget::Tab,
            PageOpenIntent::NewPage { foreground: true },
        ),
        (
            IpcOpenTarget::BackgroundTab,
            PageOpenIntent::NewPage { foreground: false },
        ),
        (
            IpcOpenTarget::Window,
            PageOpenIntent::NewWindow {
                privacy: PageOpenPrivacy::Normal,
            },
        ),
        (
            IpcOpenTarget::PrivateWindow,
            PageOpenIntent::NewWindow {
                privacy: PageOpenPrivacy::Private,
            },
        ),
    ];

    for (destination, expected) in windows {
        assert_eq!(
            page_open_intent(&TabsPresentation::Windows, destination),
            expected,
            "destination {destination:?} must map once in windows presentation"
        );
    }

    for destination in [
        IpcOpenTarget::Current,
        IpcOpenTarget::Tab,
        IpcOpenTarget::BackgroundTab,
        IpcOpenTarget::Window,
        IpcOpenTarget::PrivateWindow,
    ] {
        assert_eq!(
            page_open_intent(&TabsPresentation::Internal, destination),
            PageOpenIntent::ExistingPresentation,
            "internal presentation must preserve existing effects for {destination:?}"
        );
    }

    assert_eq!(
        page_open_intent(&TabsPresentation::Windows, IpcOpenTarget::BackgroundTab),
        PageOpenIntent::NewPage { foreground: false },
        "background intent must never be promoted to foreground"
    );
}

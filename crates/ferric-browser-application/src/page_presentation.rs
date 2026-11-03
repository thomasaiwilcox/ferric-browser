//! Typed page-open presentation policy, separate from runtime ownership.

use ferric_browser_config::TabsPresentation;

/// An explicit destination carried by a page-open request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageOpenDestination {
    Current,
    Tab,
    BackgroundTab,
    Window,
    PrivateWindow,
}

/// The privacy mode requested for a newly presented native window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageOpenPrivacy {
    Normal,
    Private,
}

/// A typed result for presenting a page-open request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageOpenIntent {
    /// Keep the existing internal-tab dispatch path and its effects unchanged.
    ExistingPresentation,
    /// Navigate the captured current page.
    CurrentPage,
    /// Create a new page host with the requested activation intent.
    NewPage { foreground: bool },
    /// Create a standalone page host with the requested privacy mode.
    NewWindow { privacy: PageOpenPrivacy },
}

/// Plans an explicit destination without changing the current runtime model.
#[must_use]
pub fn plan_page_open(
    presentation: &TabsPresentation,
    destination: PageOpenDestination,
) -> PageOpenIntent {
    if presentation == &TabsPresentation::Internal {
        return PageOpenIntent::ExistingPresentation;
    }

    match destination {
        PageOpenDestination::Current => PageOpenIntent::CurrentPage,
        PageOpenDestination::Tab => PageOpenIntent::NewPage { foreground: true },
        PageOpenDestination::BackgroundTab => PageOpenIntent::NewPage { foreground: false },
        PageOpenDestination::Window => PageOpenIntent::NewWindow {
            privacy: PageOpenPrivacy::Normal,
        },
        PageOpenDestination::PrivateWindow => PageOpenIntent::NewWindow {
            privacy: PageOpenPrivacy::Private,
        },
    }
}

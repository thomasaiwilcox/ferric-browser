//! Captured document work for native adapters. No Qt object enters this layer.
use crate::BrowserApplication;
use ferric_browser_core::{FailureKind, TabId, Target, ValidatedUrl};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}
impl ScrollDirection {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Left => "left",
            Self::Right => "right",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerticalDirection {
    Up,
    Down,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScrollEdge {
    Top,
    Bottom,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScrollTargetPolicy {
    Auto,
    Document,
    Status,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScriptWorld {
    Application,
    Page,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewCommand {
    Scroll {
        direction: ScrollDirection,
        count: u32,
    },
    ScrollPage {
        direction: VerticalDirection,
        half: bool,
        count: u32,
    },
    ScrollTo {
        edge: ScrollEdge,
    },
    ScrollTarget(ScrollTargetPolicy),
    Zoom {
        hundredths: u32,
    },
    PrintPdf {
        path: PathBuf,
    },
    SavePage {
        path: PathBuf,
    },
    ViewSource {
        url: ValidatedUrl,
    },
    DeveloperTools {
        detach: bool,
    },
    Print,
    Evaluate {
        script: String,
        world: ScriptWorld,
    },
    SiteDoctorReload {
        experiment_id: String,
    },
    SiteDoctorFreshView {
        experiment_id: String,
        url: ValidatedUrl,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewRequest {
    pub target: Target,
    pub command: ViewCommand,
}

impl BrowserApplication {
    /// Captures document identity before a queued native action can outlive a
    /// tab switch, reorder, navigation or owner retirement.
    /// # Errors
    /// Rejects invalid payload bounds and unavailable target documents.
    pub fn capture_view_request(
        &self,
        command: ViewCommand,
        tab: TabId,
    ) -> Result<ViewRequest, FailureKind> {
        match &command {
            ViewCommand::Scroll { count, .. } | ViewCommand::ScrollPage { count, .. }
                if !(1..=9_999).contains(count) =>
            {
                return Err(FailureKind::InvalidArgument);
            }
            ViewCommand::Zoom { hundredths } if !(25..=500).contains(hundredths) => {
                return Err(FailureKind::InvalidArgument);
            }
            ViewCommand::Evaluate { script, .. }
                if script.is_empty() || script.len() > 64 * 1024 || script.contains('\0') =>
            {
                return Err(FailureKind::InvalidArgument);
            }
            ViewCommand::PrintPdf { path } | ViewCommand::SavePage { path }
                if path.as_os_str().is_empty() || path.as_os_str().len() > 4096 =>
            {
                return Err(FailureKind::InvalidArgument);
            }
            ViewCommand::SiteDoctorReload { experiment_id }
            | ViewCommand::SiteDoctorFreshView { experiment_id, .. }
                if experiment_id.is_empty() || experiment_id.len() > 128 =>
            {
                return Err(FailureKind::InvalidArgument);
            }
            _ => {}
        }
        let target = self.capture_target(tab).ok_or(FailureKind::NotFound)?;
        Ok(ViewRequest { target, command })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferric_browser_config::Config;
    use ferric_browser_core::{Event, PrivacyKind};
    use ferric_browser_runtime::RuntimeInput;

    #[test]
    fn payload_and_document_identity_survive_later_commands() {
        let (mut app, window, tab) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "view-request")
                .unwrap();
        let first = app
            .capture_view_request(
                ViewCommand::Evaluate {
                    script: "first()".into(),
                    world: ScriptWorld::Page,
                },
                tab,
            )
            .unwrap();
        app.dispatch_runtime(RuntimeInput::Lifecycle(Event::OpenTab { window }))
            .unwrap();
        assert_ne!(app.active_tab().unwrap().id, first.target.tab);
        let second = app
            .capture_view_request(
                ViewCommand::Evaluate {
                    script: "second()".into(),
                    world: ScriptWorld::Application,
                },
                tab,
            )
            .unwrap();
        assert_eq!(first.target, second.target);
        assert!(
            matches!(first.command, ViewCommand::Evaluate { ref script, world: ScriptWorld::Page } if script == "first()")
        );
        app.dispatch_runtime(RuntimeInput::UserDecision(Event::StartNavigation {
            target: first.target,
            url: ValidatedUrl::parse("https://example.test/new").unwrap(),
        }))
        .unwrap();
        assert_ne!(app.capture_target(tab), Some(first.target));
        assert_eq!(
            app.capture_view_request(ViewCommand::Zoom { hundredths: 0 }, tab),
            Err(FailureKind::InvalidArgument)
        );
    }
}

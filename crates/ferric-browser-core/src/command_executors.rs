//! Registry metadata for commands with one shared native executor.
use crate::CommandDefinition;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommonCommandExecutor {
    TabUndo,
    TabClone,
    ReopenWindow,
    TabDetach,
    TabGive,
    Search,
    SearchNext,
    Scroll,
    ScrollTarget,
    CleanLink,
    Yank,
    PrintPdf,
    SavePage,
    ViewSource,
    DevTools,
    Print,
    DownloadDesktop,
    Spawn,
    ScriptRun,
    JavaScript,
    Send,
}

impl CommandDefinition {
    /// Identifies the common executor after registry alias resolution. UI,
    /// keyboard, action and IPC entry points use the same command handlers.
    #[must_use]
    pub fn common_executor(&self) -> Option<CommonCommandExecutor> {
        Some(match self.name.as_str() {
            "tab-undo" => CommonCommandExecutor::TabUndo,
            "tab-clone" => CommonCommandExecutor::TabClone,
            "reopen-in-window" => CommonCommandExecutor::ReopenWindow,
            "tab-detach" => CommonCommandExecutor::TabDetach,
            "tab-give" => CommonCommandExecutor::TabGive,
            "search" => CommonCommandExecutor::Search,
            "search-next" => CommonCommandExecutor::SearchNext,
            "scroll" | "scroll-page" | "scroll-to" => CommonCommandExecutor::Scroll,
            "scroll-target" => CommonCommandExecutor::ScrollTarget,
            "url-clean" | "url-explain" => CommonCommandExecutor::CleanLink,
            "yank" => CommonCommandExecutor::Yank,
            "print-pdf" => CommonCommandExecutor::PrintPdf,
            "save-page" => CommonCommandExecutor::SavePage,
            "view-source" => CommonCommandExecutor::ViewSource,
            "devtools" => CommonCommandExecutor::DevTools,
            "print" => CommonCommandExecutor::Print,
            "download-open" | "download-show" => CommonCommandExecutor::DownloadDesktop,
            "spawn" => CommonCommandExecutor::Spawn,
            "script-run" => CommonCommandExecutor::ScriptRun,
            "jseval" => CommonCommandExecutor::JavaScript,
            "send" => CommonCommandExecutor::Send,
            _ => return None,
        })
    }
}

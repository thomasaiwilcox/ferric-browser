//! The shared command executor for UI, bindings, actions and IPC.
use super::{CxxQtType, ParsedCommand, Pin, PublicError, Value, qobject};
use ferric_browser_core::CommonCommandExecutor;

impl qobject::BrowserUi {
    pub(super) fn execute_common_command(
        self: Pin<&mut Self>,
        executor: CommonCommandExecutor,
        command: &ParsedCommand,
        operation_id: Option<&str>,
    ) -> Result<Value, PublicError> {
        self.as_ref()
            .rust()
            .registry
            // Entry points own interactive-mode eligibility. Native actions
            // such as sending an active selection also run from Insert mode.
            .resolve(&command.name)
            .map_err(|error| {
                PublicError::new(
                    ferric_browser_ipc::ErrorCode::InvalidArgument,
                    error.to_string(),
                    error.to_string(),
                )
            })?;
        match executor {
            CommonCommandExecutor::TabUndo => self.execute_tab_undo_command(command),
            CommonCommandExecutor::TabClone => self.execute_tab_clone_command(command),
            CommonCommandExecutor::ReopenWindow => self.execute_reopen_in_window_command(command),
            CommonCommandExecutor::TabDetach => {
                self.execute_tab_detach_command(command, operation_id)
            }
            CommonCommandExecutor::TabGive => self.execute_tab_give_command(command, operation_id),
            CommonCommandExecutor::Search => self.execute_search_command(command),
            CommonCommandExecutor::SearchNext => self.execute_search_next_command(command),
            CommonCommandExecutor::Scroll => self.execute_scroll_command(command),
            CommonCommandExecutor::ScrollTarget => self.execute_scroll_target_command(command),
            CommonCommandExecutor::CleanLink => self.execute_link_clean_command(command),
            CommonCommandExecutor::Yank => self.execute_yank_command(command),
            CommonCommandExecutor::PrintPdf => self.execute_print_pdf_command(command),
            CommonCommandExecutor::SavePage => self.execute_save_page_command(command),
            CommonCommandExecutor::ViewSource => self.execute_view_source_command(command),
            CommonCommandExecutor::DevTools => self.execute_devtools_command(command),
            CommonCommandExecutor::Print => self.execute_print_command(command),
            CommonCommandExecutor::DownloadDesktop => {
                self.execute_download_desktop_command(command)
            }
            CommonCommandExecutor::Spawn => self.execute_spawn_command(command, operation_id),
            CommonCommandExecutor::ScriptRun => {
                self.execute_script_run_command(command, operation_id)
            }
            CommonCommandExecutor::JavaScript => self.execute_jseval_command(command),
            CommonCommandExecutor::Send => {
                self.execute_send_command_with_operation(command, operation_id)
            }
        }
        .map_err(PublicError::engine)
    }
}

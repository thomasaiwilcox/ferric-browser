use super::{
    CxxQtType, PathBuf, Pin, QString, QStringList, UserscriptManagerRequest, Uuid,
    captured_target_is_current, cleanup_print_artifact, fs, qobject, userscript,
    validate_pdf_output_path, validate_print_pdf_path, validate_save_page_path,
};

impl qobject::BrowserUi {
    pub(super) fn select_matching_page_scripts(
        mut self: Pin<&mut Self>,
        url: &QString,
        private_profile: bool,
    ) -> bool {
        let Some(roots) = self.as_ref().rust().userscript_roots.clone() else {
            self.as_mut()
                .set_page_userscript_names(QStringList::default());
            self.as_mut()
                .set_page_userscript_sources(QStringList::default());
            self.as_mut()
                .set_page_userscript_run_at(QStringList::default());
            self.as_mut()
                .set_page_userscript_runs_on_sub_frames(QStringList::default());
            return true;
        };
        match userscript::matching_page_scripts(&roots.config, &url.to_string(), private_profile) {
            Ok(scripts) => {
                self.as_mut().set_page_userscript_names(
                    scripts
                        .iter()
                        .map(|script| QString::from(&script.name))
                        .collect(),
                );
                self.as_mut().set_page_userscript_sources(
                    scripts
                        .iter()
                        .map(|script| QString::from(&script.source))
                        .collect(),
                );
                self.as_mut().set_page_userscript_run_at(
                    scripts
                        .iter()
                        .map(|script| QString::from(script.run_at))
                        .collect(),
                );
                self.as_mut().set_page_userscript_runs_on_sub_frames(
                    scripts
                        .iter()
                        .map(|script| QString::from(script.runs_on_sub_frames.to_string()))
                        .collect(),
                );
                true
            }
            Err(error) => {
                self.as_mut()
                    .set_page_userscript_names(QStringList::default());
                self.as_mut()
                    .set_page_userscript_sources(QStringList::default());
                self.as_mut()
                    .set_page_userscript_run_at(QStringList::default());
                self.as_mut()
                    .set_page_userscript_runs_on_sub_frames(QStringList::default());
                self.set_status_text(QString::from(format!(
                    "Page userscripts unavailable: {error}"
                )));
                false
            }
        }
    }

    pub(super) fn refresh_userscript_inventory(mut self: Pin<&mut Self>) -> bool {
        if self.as_ref().active_profile_is_transient() {
            self.as_mut().clear_userscript_inventory_rows();
            return true;
        }
        let Some(root) = self
            .as_ref()
            .rust()
            .userscript_roots
            .as_ref()
            .map(|roots| roots.config.clone())
        else {
            self.as_mut().clear_userscript_inventory_rows();
            self.as_mut().set_status_text(QString::from(
                "Userscripts are unavailable without configured storage",
            ));
            return false;
        };
        let result = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .userscript_manager_worker
            .as_mut()
            .ok_or_else(|| "userscript manager is unavailable".to_owned())
            .and_then(|worker| worker.request(UserscriptManagerRequest::Refresh { root }));
        if let Err(error) = result {
            self.as_mut()
                .set_userscript_install_state(QString::from(format!("error:{error}")));
            self.as_mut().set_status_text(QString::from(format!(
                "Userscript inventory unavailable: {error}"
            )));
            return false;
        }
        true
    }

    pub(super) fn set_userscript_enabled(
        mut self: Pin<&mut Self>,
        name: &QString,
        enabled: bool,
    ) -> bool {
        let name = name.to_string();
        if self.as_ref().active_profile_is_transient() {
            self.as_mut().set_status_text(QString::from(
                "Userscript changes are unavailable in private or ephemeral profiles",
            ));
            return false;
        }
        if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
            self.as_mut()
                .set_status_text(QString::from("Userscript name is empty or invalid"));
            return false;
        }
        let Some(root) = self
            .as_ref()
            .rust()
            .userscript_roots
            .as_ref()
            .map(|roots| roots.config.clone())
        else {
            self.as_mut().set_status_text(QString::from(
                "Userscripts are unavailable without configured storage",
            ));
            return false;
        };
        let result = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .userscript_manager_worker
            .as_mut()
            .ok_or_else(|| "userscript manager is unavailable".to_owned())
            .and_then(|worker| {
                worker.request(UserscriptManagerRequest::SetEnabled {
                    root,
                    name,
                    enabled,
                })
            });
        if let Err(error) = result {
            self.as_mut().set_status_text(QString::from(format!(
                "Userscript change rejected: {error}"
            )));
            return false;
        }
        self.as_mut()
            .set_userscript_install_state(QString::from("idle"));
        self.as_mut()
            .set_status_text(QString::from("Updating userscript enabled state…"));
        true
    }

    pub(super) fn remove_userscript(mut self: Pin<&mut Self>, name: &QString) -> bool {
        if self.as_ref().active_profile_is_transient() {
            self.as_mut().set_status_text(QString::from(
                "Userscript removal is unavailable in private or ephemeral profiles",
            ));
            return false;
        }
        let name = name.to_string();
        if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
            self.as_mut()
                .set_status_text(QString::from("Userscript name is empty or invalid"));
            return false;
        }
        let Some(root) = self
            .as_ref()
            .rust()
            .userscript_roots
            .as_ref()
            .map(|roots| roots.config.clone())
        else {
            self.as_mut().set_status_text(QString::from(
                "Userscripts are unavailable without configured storage",
            ));
            return false;
        };
        let result = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .userscript_manager_worker
            .as_mut()
            .ok_or_else(|| "userscript manager is unavailable".to_owned())
            .and_then(|worker| worker.request(UserscriptManagerRequest::Remove { root, name }));
        if let Err(error) = result {
            self.as_mut()
                .set_userscript_install_state(QString::from(format!("error:{error}")));
            self.as_mut().set_status_text(QString::from(format!(
                "Userscript removal rejected: {error}"
            )));
            return false;
        }
        self.as_mut()
            .set_userscript_install_state(QString::from("removing"));
        self.as_mut()
            .set_status_text(QString::from("Removing userscript…"));
        true
    }

    pub(super) fn prepare_print_pdf(self: Pin<&mut Self>, path: &QString) -> QString {
        match validate_print_pdf_path(&path.to_string()) {
            Ok(path) => QString::from(path.to_string_lossy().as_ref()),
            Err(error) => {
                self.set_status_text(QString::from(error));
                QString::default()
            }
        }
    }

    pub(super) fn prepare_save_page(mut self: Pin<&mut Self>, path: &QString) -> QString {
        match validate_save_page_path(&path.to_string()) {
            Ok(path) => {
                let path_string = path.to_string_lossy().into_owned();
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_save_page = Some(path.clone());
                QString::from(path_string)
            }
            Err(error) => {
                self.set_status_text(QString::from(error));
                QString::default()
            }
        }
    }

    pub(super) fn take_save_page_path(mut self: Pin<&mut Self>) -> QString {
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_save_page
            .take()
            .map_or_else(QString::default, |path| {
                QString::from(path.to_string_lossy().as_ref())
            })
    }

    pub(super) fn finish_print_pdf(self: Pin<&mut Self>, path: &QString, succeeded: bool) -> bool {
        let path_string = path.to_string();
        let result = if succeeded {
            validate_pdf_output_path(&path_string).and_then(|path| {
                let metadata = fs::symlink_metadata(&path)
                    .map_err(|error| format!("generated PDF is unavailable: {error}"))?;
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    return Err("generated PDF is not a regular file".into());
                }
                Ok(())
            })
        } else {
            Err("PDF generation failed; no completed PDF was produced".to_owned())
        };
        match result {
            Ok(()) => {
                self.set_status_text(QString::from("PDF generated"));
                true
            }
            Err(error) => {
                self.set_status_text(QString::from(error));
                false
            }
        }
    }

    pub(super) fn prepare_print_job(mut self: Pin<&mut Self>) -> QString {
        if self.as_ref().rust().pending_print.is_some() {
            self.set_status_text(QString::from("A print job is already pending"));
            return QString::default();
        }
        let (directory, private_temporary) =
            self.as_ref().rust().storage_roots.as_ref().map_or_else(
                || {
                    (
                        std::env::temp_dir().join(format!(
                            "ferric-browser-print-{}",
                            self.as_ref().rust().session_id
                        )),
                        true,
                    )
                },
                |roots| (roots.runtime.join("print"), false),
            );
        if let Err(error) = fs::create_dir_all(&directory) {
            self.set_status_text(QString::from(format!(
                "Print staging directory is unavailable: {error}"
            )));
            return QString::default();
        }
        #[cfg(unix)]
        if let Err(error) = fs::set_permissions(
            &directory,
            std::os::unix::fs::PermissionsExt::from_mode(0o700),
        ) {
            self.set_status_text(QString::from(format!(
                "Print staging directory permissions could not be secured: {error}"
            )));
            return QString::default();
        }
        let path = directory.join(format!("{}.pdf", Uuid::new_v4()));
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.pending_print = Some(path.clone());
            this.pending_print_private = private_temporary;
        }
        if private_temporary {
            self.set_status_text(QString::from(
                "Rendering a private temporary PDF for desktop printing",
            ));
        }
        QString::from(path.to_string_lossy().as_ref())
    }

    pub(super) fn finish_print_job(
        mut self: Pin<&mut Self>,
        path: &QString,
        succeeded: bool,
    ) -> bool {
        let path = PathBuf::from(path.to_string());
        let (requested, private_temporary) = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            (this.pending_print.clone(), this.pending_print_private)
        };
        if requested.as_ref() != Some(&path) {
            self.set_status_text(QString::from("Print request became stale"));
            return false;
        }
        if !succeeded {
            self.as_mut().rust_mut().as_mut().get_mut().pending_print = None;
            cleanup_print_artifact(&path, private_temporary);
            self.set_status_text(QString::from("Print failed: PDF generation failed"));
            return false;
        }
        let path = match validate_pdf_output_path(path.to_string_lossy().as_ref()) {
            Ok(path) => path,
            Err(error) => {
                self.as_mut().rust_mut().as_mut().get_mut().pending_print = None;
                cleanup_print_artifact(&path, private_temporary);
                self.set_status_text(QString::from(format!("Print failed: {error}")));
                return false;
            }
        };
        let request = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .print_worker
            .as_mut()
            .ok_or_else(|| "print worker is unavailable".to_owned())
            .and_then(|worker| worker.request(path.clone()));
        if let Err(error) = request {
            self.as_mut().rust_mut().as_mut().get_mut().pending_print = None;
            cleanup_print_artifact(&path, private_temporary);
            self.set_status_text(QString::from(format!("Print failed: {error}")));
            return false;
        }
        self.set_status_text(QString::from("Print job queued"));
        true
    }

    pub(super) fn take_download_request(mut self: Pin<&mut Self>) -> bool {
        self.as_mut().set_download_request_token(QString::default());
        self.as_mut().set_download_request_url(QString::default());
        let request_target = self
            .as_ref()
            .rust()
            .pending_download
            .as_ref()
            .map(|request| request.target);
        let request_is_stale = request_target.is_some_and(|target| {
            !captured_target_is_current(self.as_ref().rust().state.as_ref(), target)
        });
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        let Some(request) = this.pending_download.as_mut() else {
            return false;
        };
        if request_is_stale {
            this.pending_download = None;
            self.set_status_text(QString::from("Download request is stale"));
            return false;
        }
        if request.issued {
            return false;
        }
        request.issued = true;
        let token = request.token.clone();
        let url = request.url.clone();
        drop(rust);
        self.as_mut()
            .set_download_request_token(QString::from(token));
        self.as_mut().set_download_request_url(QString::from(url));
        true
    }

    pub(super) fn complete_download_request(
        mut self: Pin<&mut Self>,
        token: &QString,
        succeeded: bool,
    ) -> bool {
        let token = token.to_string();
        let request = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(request) = this.pending_download.as_ref() else {
                self.set_status_text(QString::from("Download request expired"));
                return false;
            };
            if request.token != token {
                self.set_status_text(QString::from("Download request is stale"));
                return false;
            }
            this.pending_download.take()
        };
        let Some(request) = request else {
            return false;
        };
        if !captured_target_is_current(self.as_ref().rust().state.as_ref(), request.target) {
            self.set_status_text(QString::from("Download request target is stale"));
            return false;
        }
        if succeeded {
            self.set_status_text(QString::from("Download activation requested"));
            true
        } else {
            self.set_status_text(QString::from("Download activation failed"));
            false
        }
    }
}

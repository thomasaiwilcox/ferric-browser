use super::*;
use crate::{DownloadRetryState, ProfileActivation};
use ferric_browser_config::Config;
use ferric_browser_core::PrivacyKind;
use ferric_browser_runtime::BrowserRuntime;
use ferric_browser_storage::{ProfileStore, RootSpec, StorageRoots, StoreMode};

struct Roots(StorageRoots);
impl Roots {
    fn new() -> Self {
        let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
        roots.ensure().unwrap();
        Self(roots)
    }
}
impl Drop for Roots {
    fn drop(&mut self) {
        self.0.clone().cleanup().unwrap();
    }
}
fn app(roots: &Roots, privacy: PrivacyKind, id: Uuid) -> BrowserApplication {
    let (runtime, _, _) = BrowserRuntime::bootstrap(privacy, "same label").unwrap();
    let mut app = BrowserApplication::from_runtime(runtime, Config::default());
    app.activate_profile(ProfileActivation {
        name: "same label".into(),
        id: Some(id),
        privacy,
        roots: Some(roots.0.clone()),
        lock: None,
        contexts: None,
        storage_path: (privacy == PrivacyKind::Normal)
            .then(|| roots.0.data.join("downloads.sqlite")),
    })
    .unwrap();
    app
}
fn child(source: &BrowserApplication, privacy: PrivacyKind) -> BrowserApplication {
    let (runtime, _, _) = BrowserRuntime::bootstrap(privacy, "peer").unwrap();
    source.inherit_profile(runtime).unwrap()
}
fn offer(app: &mut BrowserApplication) -> String {
    let id = Uuid::new_v4();
    app.offer_session_download(id, "https://example.test/report", "Report.pdf", 10)
        .unwrap();
    id.to_string()
}

#[test]
fn private_completed_downloads_follow_the_shared_session_and_never_saved_history() {
    for privacy in [PrivacyKind::Private, PrivacyKind::Ephemeral] {
        let roots = Roots::new();
        let profile = Uuid::new_v4();
        let mut source = app(&roots, privacy, profile);
        assert!(source.session_downloads_available());
        let id = offer(&mut source);
        let mut peer = child(&source, privacy);
        let mut unrelated = app(&roots, privacy, profile);
        assert!(source.shares_permission_session(&peer));
        assert!(!source.shares_permission_session(&unrelated));
        assert!(unrelated.session_download_rows(&[]).unwrap().is_empty());
        source
            .set_session_download_destination(&id, &roots.0.data.join("Report.pdf"))
            .unwrap();
        assert_eq!(
            peer.update_session_download(&id, DownloadState::Cancelled, 0, true, None),
            Err(DownloadSessionError::WrongOwner)
        );
        assert_eq!(
            peer.finalize_session_download(&id),
            Err(DownloadSessionError::WrongOwner)
        );
        source
            .update_session_download(&id, DownloadState::Completed, 2048, false, None)
            .unwrap();
        source.finalize_session_download(&id).unwrap();
        source
            .update_session_download(&id, DownloadState::Completed, 2048, true, Some(20))
            .unwrap();
        let row = peer.session_download_rows(&[]).unwrap().remove(0);
        assert!(row.file_ready);
        assert!(!row.active);
        assert_eq!(row.display_name, "Report.pdf");
        assert_eq!(row.record.bytes_received, 2048);
        let mut unrelated_saved = row.record.clone();
        unrelated_saved.id = Uuid::new_v4().to_string();
        assert_eq!(
            peer.session_download_rows(&[unrelated_saved])
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            peer.download_history_status(),
            DownloadHistoryStatus::NotStored
        );
        source.close_profile();
        assert!(!source.session_downloads_available());
        assert!(peer.session_downloads_available());
        assert_eq!(peer.session_download_rows(&[]).unwrap().len(), 1);
        peer.close_profile();
        unrelated.close_profile();
        let mut reopened = app(&roots, privacy, profile);
        assert!(reopened.session_download_rows(&[]).unwrap().is_empty());
        assert!(!roots.0.data.join("downloads.sqlite").exists());
        reopened.close_profile();
    }
}

#[test]
fn unavailable_history_does_not_reject_transfer_and_explicit_retry_saves_latest_record() {
    let roots = Roots::new();
    let mut app = app(&roots, PrivacyKind::Normal, Uuid::new_v4());
    app.close_storage();
    assert!(app.session_downloads_available());
    let id = offer(&mut app);
    assert_eq!(
        app.download_history_status(),
        DownloadHistoryStatus::Unavailable
    );
    app.set_session_download_destination(&id, &roots.0.data.join("Report.pdf"))
        .unwrap();
    app.update_session_download(&id, DownloadState::Paused, 16384, false, None)
        .unwrap();
    let row = app.session_download_rows(&[]).unwrap().remove(0);
    assert!(row.active);
    assert_eq!(row.record.state, DownloadState::Paused);
    app.update_session_download(&id, DownloadState::Cancelled, 16384, true, None)
        .unwrap();
    app.restart_storage().unwrap();
    // Failures are retried by an explicit user action, not on every timer tick.
    assert_eq!(
        app.download_history_status(),
        DownloadHistoryStatus::Unavailable
    );
    app.retry_download_history();
    app.close_profile();
    let store =
        ProfileStore::open(roots.0.data.join("downloads.sqlite"), StoreMode::Normal).unwrap();
    let saved = store.download(&id).unwrap().unwrap();
    assert_eq!(saved.state, DownloadState::Cancelled);
    assert_eq!(saved.bytes_received, 16384);
    assert_eq!(
        saved.destination,
        roots.0.data.join("Report.pdf").to_str().unwrap()
    );
}

#[test]
fn shutdown_drains_newer_progress_after_an_already_admitted_older_write() {
    let roots = Roots::new();
    let mut app = app(&roots, PrivacyKind::Normal, Uuid::new_v4());
    let id = offer(&mut app);
    app.set_session_download_destination(&id, &roots.0.data.join("Report.pdf"))
        .unwrap();
    for bytes in [1, 64, 1024, 65536] {
        app.update_session_download(&id, DownloadState::InProgress, bytes, false, None)
            .unwrap();
    }
    app.update_session_download(&id, DownloadState::Completed, 65536, false, None)
        .unwrap();
    app.finalize_session_download(&id).unwrap();
    app.update_session_download(&id, DownloadState::Completed, 65536, true, Some(20))
        .unwrap();
    app.close_profile();
    let store =
        ProfileStore::open(roots.0.data.join("downloads.sqlite"), StoreMode::Normal).unwrap();
    let saved = store.download(&id).unwrap().unwrap();
    assert_eq!(saved.state, DownloadState::Completed);
    assert_eq!(saved.bytes_received, 65536);
    assert_eq!(saved.completed_at, Some(20));
}

#[test]
fn unfinished_files_cannot_be_opened_and_replaced_profiles_reject_old_callbacks() {
    let roots = Roots::new();
    let mut app = app(&roots, PrivacyKind::Private, Uuid::new_v4());
    let id = offer(&mut app);
    app.update_session_download(&id, DownloadState::Completed, 128, false, None)
        .unwrap();
    let row = app.session_download_rows(&[]).unwrap().remove(0);
    assert!(row.finishing);
    assert!(!row.file_ready);
    assert_eq!(
        app.update_session_download(&id, DownloadState::Completed, 128, true, Some(20)),
        Err(DownloadSessionError::FileNotFinalized)
    );
    for bad in ["relative/file", "/tmp/bad\nfile"] {
        assert_eq!(
            app.set_session_download_destination(&id, Path::new(bad)),
            Err(DownloadSessionError::InvalidDestination)
        );
    }
    app.close_profile();
    assert_eq!(
        app.update_session_download(&id, DownloadState::Cancelled, 128, true, None),
        Err(DownloadSessionError::ProfileUnavailable)
    );
}

#[test]
fn retry_reservations_are_shared_released_on_drop_and_bound_to_owned_transport() {
    for privacy in [PrivacyKind::Normal, PrivacyKind::Private] {
        let roots = Roots::new();
        let mut owner = app(&roots, privacy, Uuid::new_v4());
        let peer = child(&owner, privacy);
        let source = offer(&mut owner);
        owner
            .update_session_download(&source, DownloadState::Cancelled, 0, true, None)
            .unwrap();
        let reserved = owner.reserve_session_download_retry(&source).unwrap();
        assert_eq!(
            peer.reserve_session_download_retry(&source).unwrap_err(),
            DownloadSessionError::RetryPending
        );
        assert_eq!(
            peer.session_download_rows(&[])
                .unwrap()
                .iter()
                .find(|row| row.record.id == source)
                .unwrap()
                .retry,
            DownloadRetryState::Preparing
        );
        drop(reserved);
        let mut reserved = owner.reserve_session_download_retry(&source).unwrap();
        let retry = offer(&mut owner);
        reserved.bind(&retry).unwrap();
        drop(reserved);
        assert_eq!(
            peer.reserve_session_download_retry(&source).unwrap_err(),
            DownloadSessionError::RetryPending
        );
        owner
            .update_session_download(&retry, DownloadState::Cancelled, 0, true, None)
            .unwrap();
        assert_eq!(
            peer.session_download_rows(&[])
                .unwrap()
                .iter()
                .find(|row| row.record.id == source)
                .unwrap()
                .retry,
            DownloadRetryState::Idle
        );
        assert!(peer.reserve_session_download_retry(&source).is_ok());
    }
}

#[test]
fn retry_permit_cannot_bind_another_owners_offer_or_block_an_unrelated_private_session() {
    let roots = Roots::new();
    let mut owner = app(&roots, PrivacyKind::Private, Uuid::new_v4());
    let mut peer = child(&owner, PrivacyKind::Private);
    let unrelated = app(&roots, PrivacyKind::Private, Uuid::new_v4());
    let source = offer(&mut owner);
    owner
        .update_session_download(&source, DownloadState::Cancelled, 0, true, None)
        .unwrap();
    let mut reserved = owner.reserve_session_download_retry(&source).unwrap();
    assert!(unrelated.reserve_session_download_retry(&source).is_ok());
    let foreign = offer(&mut peer);
    assert_eq!(
        reserved.bind(&foreign),
        Err(DownloadSessionError::WrongOwner)
    );
    assert!(!owner.owns_session_download_offer(&foreign));
    drop(reserved);
    assert!(peer.reserve_session_download_retry(&source).is_ok());
    assert!(
        peer.session_download_rows(&[])
            .unwrap()
            .iter()
            .find(|row| row.record.id == foreign)
            .unwrap()
            .active
    );
}

#[test]
fn retry_projection_requires_an_eligible_state_and_reusable_http_source() {
    let roots = Roots::new();
    let mut owner = app(&roots, PrivacyKind::Private, Uuid::new_v4());
    let id = Uuid::new_v4();
    owner
        .offer_session_download(id, "blob:https://example.test/expired", "report", 10)
        .unwrap();
    owner
        .update_session_download(&id.to_string(), DownloadState::Cancelled, 0, true, None)
        .unwrap();
    let rows = owner.session_download_rows(&[]).unwrap();
    assert_eq!(rows[0].retry, DownloadRetryState::Unavailable);
    let http = offer(&mut owner);
    assert_eq!(
        owner
            .session_download_rows(&[])
            .unwrap()
            .iter()
            .find(|row| row.record.id == http)
            .unwrap()
            .retry,
        DownloadRetryState::Unavailable
    );
    owner
        .update_session_download(&http, DownloadState::Cancelled, 0, true, None)
        .unwrap();
    assert_eq!(
        owner
            .session_download_rows(&[])
            .unwrap()
            .iter()
            .find(|row| row.record.id == http)
            .unwrap()
            .retry,
        DownloadRetryState::Idle
    );
}

#[test]
fn file_publication_requires_current_owner_destination_and_unrevoked_completion() {
    let roots = Roots::new();
    let mut source = app(&roots, PrivacyKind::Private, Uuid::new_v4());
    let mut peer = child(&source, PrivacyKind::Private);
    let id = offer(&mut source);
    let path = roots.0.data.join("Report.pdf");
    source.set_session_download_destination(&id, &path).unwrap();
    assert!(!source.session_download_finalization_current(&id, &path));
    assert_eq!(
        source.finalize_session_download(&id),
        Err(DownloadSessionError::FileNotFinalized)
    );
    source
        .update_session_download(&id, DownloadState::Completed, 512, false, None)
        .unwrap();
    assert!(source.session_download_finalization_current(&id, &path));
    assert!(!source.session_download_finalization_current(&id, &path.with_extension("other")));
    assert!(!peer.session_download_finalization_current(&id, &path));
    assert_eq!(
        peer.finalize_session_download(&id),
        Err(DownloadSessionError::WrongOwner)
    );
    source
        .note_session_download_metrics(
            &id,
            1024,
            Some(crate::DownloadFailure::Save(
                crate::DownloadSaveFailure::Unavailable,
            )),
        )
        .unwrap();
    source
        .update_session_download(&id, DownloadState::Cancelled, 512, true, None)
        .unwrap();
    assert!(!source.session_download_finalization_current(&id, &path));
    assert_eq!(
        source.finalize_session_download(&id),
        Err(DownloadSessionError::FileNotFinalized)
    );
    let row = peer.session_download_rows(&[]).unwrap().remove(0);
    assert_eq!(row.total_bytes, 1024);
    assert_eq!(
        row.failure,
        Some(crate::DownloadFailure::Save(
            crate::DownloadSaveFailure::Unavailable
        ))
    );
    assert!(!row.file_ready);
}

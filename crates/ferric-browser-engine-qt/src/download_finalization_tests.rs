use super::super::download_files::stage_download_path;
use super::*;
use std::time::{Duration, Instant};

struct Directory(PathBuf);
impl Directory {
    fn new(parent: &Path) -> Self {
        let path = parent.join(format!("ferric-file-save-{}", Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn stage(&self, name: &str, bytes: &[u8]) -> StagedDownload {
        let staged = stage_download_path(None, Uuid::new_v4(), &self.0.join(name)).unwrap();
        fs::write(staged.staging_path(), bytes).unwrap();
        staged
    }
    fn files(&self) -> usize {
        fs::read_dir(&self.0).unwrap().count()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn prepared_file_is_hidden_complete_and_exclusively_published() {
    let directory = Directory::new(&std::env::temp_dir());
    let payload: Vec<u8> = (0..=255).cycle().take(262_144).collect();
    let staged = directory.stage("Report.bin", &payload);
    let prepared = prepare(&staged, &AtomicBool::new(false)).unwrap();
    assert!(!staged.final_path().exists());
    assert_eq!(directory.files(), 1);
    let hidden = fs::read_dir(&directory.0)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(
        hidden
            .file_name()
            .unwrap()
            .as_bytes()
            .starts_with(b".ferric-download-")
    );
    assert_eq!(fs::read(hidden).unwrap(), payload);
    prepared.publish().unwrap();
    assert_eq!(fs::read(staged.final_path()).unwrap(), payload);
    assert_eq!(directory.files(), 1);
    cleanup_staged_download(&staged);
}

#[test]
fn publication_preserves_a_file_created_after_preparation() {
    let directory = Directory::new(&std::env::temp_dir());
    let staged = directory.stage("Report.bin", b"download");
    let prepared = prepare(&staged, &AtomicBool::new(false)).unwrap();
    fs::write(staged.final_path(), b"existing file").unwrap();
    assert!(matches!(prepared.publish(), Err(SaveError::Publish(_))));
    assert_eq!(fs::read(staged.final_path()).unwrap(), b"existing file");
    assert_eq!(directory.files(), 1);
    cleanup_staged_download(&staged);
}

#[test]
fn replaced_destination_directory_cannot_redirect_publication() {
    let directory = Directory::new(&std::env::temp_dir());
    let staged = directory.stage("Report.bin", b"download");
    let prepared = prepare(&staged, &AtomicBool::new(false)).unwrap();
    let moved = directory.0.with_extension("moved");
    fs::rename(&directory.0, &moved).unwrap();
    fs::create_dir(&directory.0).unwrap();
    assert!(matches!(
        prepared.publish(),
        Err(SaveError::DestinationChanged)
    ));
    assert_eq!(directory.files(), 0);
    assert_eq!(fs::read_dir(&moved).unwrap().count(), 0);
    fs::remove_dir(moved).unwrap();
    cleanup_staged_download(&staged);
}

#[test]
fn symlink_source_is_refused_without_touching_its_target() {
    let directory = Directory::new(&std::env::temp_dir());
    let staged = directory.stage("Report.bin", b"download");
    fs::remove_file(staged.staging_path()).unwrap();
    let outside = directory.0.join("outside");
    fs::write(&outside, b"preserved").unwrap();
    std::os::unix::fs::symlink(&outside, staged.staging_path()).unwrap();
    assert!(matches!(
        prepare(&staged, &AtomicBool::new(false)),
        Err(SaveError::Source(_))
    ));
    assert_eq!(fs::read(outside).unwrap(), b"preserved");
    assert!(!staged.final_path().exists());
    cleanup_staged_download(&staged);
}

#[test]
fn queued_or_prepared_cancellation_cannot_publish_and_releases_owned_files() {
    let directory = Directory::new(&std::env::temp_dir());
    let staged = directory.stage("Report.bin", &vec![7; 65_536]);
    let source = staged.staging_path().to_owned();
    let destination = staged.final_path().to_owned();
    let mut worker = DownloadFinalizer::spawn().unwrap();
    worker
        .submit("owned".into(), Uuid::new_v4(), staged)
        .unwrap();
    assert!(worker.cancel("owned"));
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(completion) = worker.poll().pop() {
            assert!(matches!(completion.result, Err(SaveError::Cancelled)));
            break;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(2));
    }
    assert!(!source.exists());
    assert!(!destination.exists());
    assert_eq!(directory.files(), 0);
    assert!(!worker.contains("owned"));
}

#[test]
fn real_cross_filesystem_copy_publishes_all_bytes() {
    let directory = Directory::new(Path::new("/dev/shm"));
    let bytes: Vec<u8> = (0..=255).cycle().take(131_072).collect();
    let staged = directory.stage("Report.bin", &bytes);
    assert_ne!(
        fs::metadata(staged.staging_path()).unwrap().dev(),
        fs::metadata(&directory.0).unwrap().dev()
    );
    prepare(&staged, &AtomicBool::new(false))
        .unwrap()
        .publish()
        .unwrap();
    assert_eq!(fs::read(staged.final_path()).unwrap(), bytes);
    cleanup_staged_download(&staged);
}

//! One serialization domain per session path, shared by independent metadata
//! workers. An intent keeps its ordering state alive while queued or executing.
use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicU64, Ordering},
    },
};

#[derive(Clone, Debug)]
pub struct SessionSaveOrder {
    path: PathBuf,
    sequence: Option<u64>,
    state: Option<Arc<Mutex<u64>>>,
}

impl PartialEq for SessionSaveOrder {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
            && self.sequence == other.sequence
            && match (&self.state, &other.state) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
    }
}

fn path_key(path: &Path) -> Option<PathBuf> {
    let mut key = PathBuf::new();
    for component in std::path::absolute(path).ok()?.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                key.pop();
                if key.as_os_str().is_empty() {
                    key.push("/");
                }
            }
            other => key.push(other.as_os_str()),
        }
    }
    Some(key)
}

impl SessionSaveOrder {
    pub(crate) fn capture(path: &Path) -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(1);
        static PATHS: OnceLock<Mutex<HashMap<PathBuf, Weak<Mutex<u64>>>>> = OnceLock::new();
        let sequence = SEQUENCE
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                value.checked_add(1)
            })
            .ok();
        let key = path_key(path);
        let state = key.as_ref().and_then(|key| {
            let mut paths = PATHS.get_or_init(Default::default).lock().ok()?;
            paths.retain(|_, state| state.strong_count() > 0);
            if let Some(state) = paths.get(key).and_then(Weak::upgrade) {
                return Some(state);
            }
            let state = Arc::new(Mutex::new(0));
            paths.insert(key.clone(), Arc::downgrade(&state));
            Some(state)
        });
        Self {
            path: key.unwrap_or_default(),
            sequence,
            state,
        }
    }

    pub(crate) fn matches_path(&self, path: &Path) -> bool {
        self.sequence.is_some()
            && self.state.is_some()
            && path_key(path).as_ref() == Some(&self.path)
    }

    pub(crate) fn save(
        &self,
        path: &Path,
        snapshot: &crate::SessionSnapshot,
    ) -> Result<(), crate::SessionError> {
        if !self.matches_path(path) {
            return Err(crate::SessionError::Invalid(
                "session save order is unavailable or belongs to another path".into(),
            ));
        }
        let mut latest = self
            .state
            .as_ref()
            .expect("validated state")
            .lock()
            .map_err(|_| crate::SessionError::Invalid("session writer is unavailable".into()))?;
        let sequence = self.sequence.expect("validated sequence");
        // A newer complete aggregate already includes this older intent.
        if sequence <= *latest {
            return Ok(());
        }
        crate::sessions::save_session_atomic_unordered(path, snapshot)?;
        *latest = sequence;
        Ok(())
    }
}

use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

macro_rules! opaque_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(u64);

        #[allow(dead_code)]
        impl $name {
            pub(crate) const fn from_raw(value: u64) -> Self {
                Self(value)
            }

            /// Parses the stable display form emitted by [`fmt::Display`].
            ///
            /// Runtime IDs are intentionally opaque to callers; parsing only
            /// permits routing a previously observed ID back to its owner.
            #[must_use]
            pub fn from_display(value: &str) -> Option<Self> {
                if value.len() > 64 {
                    return None;
                }
                let (kind, number) = value.rsplit_once('-')?;
                if !kind.eq_ignore_ascii_case(stringify!($name)) {
                    return None;
                }
                Some(Self::from_raw(number.parse().ok()?))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(
                    formatter,
                    "{}-{}",
                    stringify!($name).to_ascii_lowercase(),
                    self.0
                )
            }
        }
    };
}

opaque_id!(DocumentId);
opaque_id!(ContextId);
opaque_id!(ActionId);
opaque_id!(ProfileId);
opaque_id!(RequestId);
opaque_id!(DownloadId);
opaque_id!(SessionId);
opaque_id!(SpatialSessionId);
opaque_id!(SpatialRequestId);
opaque_id!(JourneyNodeId);
opaque_id!(HintSessionId);
opaque_id!(TabId);
opaque_id!(WindowId);

/// Monotonic runtime identity allocation.
///
/// IDs are intentionally never reused while an application instance is live.
/// Durable UUID-backed IDs will be added at the storage boundary; the core
/// only relies on identity opacity and uniqueness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdSource {
    allocation: Allocation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Allocation {
    Local(u64),
    Shared(SharedIdAllocator),
}

/// Identity authority shared by runtimes belonging to one live application.
/// Clones allocate from the same counter, including after an owner closes.
/// Equality compares allocator identity, not its changing counter value.
#[derive(Clone, Debug)]
pub struct SharedIdAllocator(Arc<AtomicU64>);

impl Default for SharedIdAllocator {
    fn default() -> Self {
        Self(Arc::new(AtomicU64::new(1)))
    }
}

impl PartialEq for SharedIdAllocator {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for SharedIdAllocator {}

impl Default for IdSource {
    fn default() -> Self {
        Self {
            allocation: Allocation::Local(1),
        }
    }
}

impl IdSource {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Uses an application-owned allocator instead of an isolated counter.
    #[must_use]
    pub fn from_shared(allocator: SharedIdAllocator) -> Self {
        Self {
            allocation: Allocation::Shared(allocator),
        }
    }

    pub(crate) fn document(&mut self) -> DocumentId {
        DocumentId::from_raw(self.allocate())
    }

    #[allow(dead_code)]
    pub(crate) fn context(&mut self) -> ContextId {
        ContextId::from_raw(self.allocate())
    }

    #[allow(dead_code)]
    pub(crate) fn action(&mut self) -> ActionId {
        ActionId::from_raw(self.allocate())
    }

    pub(crate) fn profile(&mut self) -> ProfileId {
        ProfileId::from_raw(self.allocate())
    }

    #[allow(dead_code)]
    pub(crate) fn request(&mut self) -> RequestId {
        RequestId::from_raw(self.allocate())
    }

    #[allow(dead_code)]
    pub(crate) fn download(&mut self) -> DownloadId {
        DownloadId::from_raw(self.allocate())
    }

    #[allow(dead_code)]
    pub(crate) fn session(&mut self) -> SessionId {
        SessionId::from_raw(self.allocate())
    }

    pub(crate) fn spatial_session(&mut self) -> SpatialSessionId {
        SpatialSessionId::from_raw(self.allocate())
    }

    pub(crate) fn spatial_request(&mut self) -> SpatialRequestId {
        SpatialRequestId::from_raw(self.allocate())
    }

    #[allow(dead_code)]
    pub(crate) fn journey_node(&mut self) -> JourneyNodeId {
        JourneyNodeId::from_raw(self.allocate())
    }

    pub(crate) fn hint_session(&mut self) -> HintSessionId {
        HintSessionId::from_raw(self.allocate())
    }

    pub(crate) fn tab(&mut self) -> TabId {
        TabId::from_raw(self.allocate())
    }

    pub(crate) fn window(&mut self) -> WindowId {
        WindowId::from_raw(self.allocate())
    }

    fn allocate(&mut self) -> u64 {
        match &mut self.allocation {
            Allocation::Local(next) => {
                let id = *next;
                *next = next.checked_add(1).expect("runtime ID space exhausted");
                id
            }
            Allocation::Shared(allocator) => {
                // fetch_update retains the workspace's Rust 1.85 support.
                #[allow(deprecated)]
                allocator
                    .0
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                        next.checked_add(1)
                    })
                    .expect("runtime ID space exhausted")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_every_runtime_identity_without_reuse() {
        let mut source = IdSource::new();
        let document = source.document();
        let context = source.context();
        let action = source.action();
        let profile = source.profile();
        let request = source.request();
        let download = source.download();
        let session = source.session();
        let journey_node = source.journey_node();
        let tab = source.tab();
        let window = source.window();

        assert_eq!(document.to_string(), "documentid-1");
        assert_eq!(context.to_string(), "contextid-2");
        assert_eq!(action.to_string(), "actionid-3");
        assert_eq!(profile.to_string(), "profileid-4");
        assert_eq!(request.to_string(), "requestid-5");
        assert_eq!(download.to_string(), "downloadid-6");
        assert_eq!(session.to_string(), "sessionid-7");
        assert_eq!(journey_node.to_string(), "journeynodeid-8");
        assert_eq!(tab.to_string(), "tabid-9");
        assert_eq!(window.to_string(), "windowid-10");
    }

    #[test]
    fn parses_only_the_bounded_display_form() {
        let window = WindowId::from_display("windowid-42").expect("display ID");
        assert_eq!(window.to_string(), "windowid-42");
        assert!(WindowId::from_display("window-42").is_none());
        assert!(WindowId::from_display("windowid-0x2a").is_none());
        assert!(WindowId::from_display("windowid-").is_none());
        assert!(WindowId::from_display(&format!("windowid-{}", "9".repeat(65))).is_none());
    }

    #[test]
    fn shared_runtime_ids_survive_clones_and_owner_replacement() {
        let allocator = SharedIdAllocator::default();
        let mut first = IdSource::from_shared(allocator.clone());
        let mut second = first.clone();
        let old = first.window();
        let other = second.window();
        drop(first);
        let mut replacement = IdSource::from_shared(allocator);
        let next = replacement.window();
        assert_ne!(old, other);
        assert_ne!(old, next);
        assert_ne!(other, next);
        assert_eq!(WindowId::from_display(&next.to_string()), Some(next));
    }

    #[test]
    fn isolated_sources_remain_deterministic() {
        let mut first = IdSource::new();
        let mut second = first.clone();
        assert_eq!(first.tab(), second.tab());
        assert_eq!(first, second);
    }

    #[test]
    fn shared_sources_allocate_uniquely_across_threads() {
        let allocator = SharedIdAllocator::default();
        let owners = (0..8)
            .map(|_| {
                let allocator = allocator.clone();
                std::thread::spawn(move || {
                    let mut ids = IdSource::from_shared(allocator);
                    (0..32).map(|_| ids.tab()).collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>();
        let ids = owners
            .into_iter()
            .flat_map(|owner| owner.join().expect("owner"))
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(ids.len(), 256);
    }
}

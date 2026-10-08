use std::{
    fmt,
    sync::atomic::{AtomicU64, Ordering},
};

static PROCESS_RUNTIME_ID: AtomicU64 = AtomicU64::new(1);

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
    next: u64,
    process_shared: bool,
}

impl Default for IdSource {
    fn default() -> Self {
        Self {
            next: 1,
            process_shared: false,
        }
    }
}

impl IdSource {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocates opaque runtime IDs across every participating owner in this
    /// process. Browser windows use this source so independently bootstrapped
    /// runtimes cannot issue the same window or tab identity.
    #[must_use]
    pub fn process_shared() -> Self {
        Self {
            next: 1,
            process_shared: true,
        }
    }

    /// Forks an allocator for read-only reducer projections. A process-shared
    /// preview must not consume the global counter merely to predict IDs.
    /// Callers still have to reject a stale prediction if another owner
    /// allocates before the planned events execute.
    pub(crate) fn preview_fork(&self) -> Self {
        Self {
            next: if self.process_shared {
                PROCESS_RUNTIME_ID.load(Ordering::Relaxed)
            } else {
                self.next
            },
            process_shared: false,
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
        if self.process_shared {
            return PROCESS_RUNTIME_ID
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
                .expect("process runtime ID space exhausted");
        }
        let id = self.next;
        self.next = self
            .next
            .checked_add(1)
            .expect("runtime ID space exhausted");
        id
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
    fn process_shared_sources_do_not_reuse_window_or_tab_ids() {
        let mut first = IdSource::process_shared();
        let mut second = IdSource::process_shared();
        let first_window = first.window();
        let second_window = second.window();
        let first_tab = first.tab();
        let second_tab = second.tab();
        assert_ne!(first_window, second_window);
        assert_ne!(first_tab, second_tab);
        assert_ne!(first_window.0, second_tab.0);
    }

    #[test]
    fn process_shared_preview_uses_a_local_counter() {
        let source = IdSource::process_shared();
        let mut preview = source.preview_fork();
        let first = preview.window();
        let second = preview.tab();
        assert!(!preview.process_shared);
        assert_eq!(second.0, first.0 + 1);
        assert!(source.process_shared);
        assert_eq!(source.next, 1);
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
}

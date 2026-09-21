use std::fmt;

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
}

impl Default for IdSource {
    fn default() -> Self {
        Self { next: 1 }
    }
}

impl IdSource {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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
    fn parses_only_the_bounded_display_form() {
        let window = WindowId::from_display("windowid-42").expect("display ID");
        assert_eq!(window.to_string(), "windowid-42");
        assert!(WindowId::from_display("window-42").is_none());
        assert!(WindowId::from_display("windowid-0x2a").is_none());
        assert!(WindowId::from_display("windowid-").is_none());
        assert!(WindowId::from_display(&format!("windowid-{}", "9".repeat(65))).is_none());
    }
}

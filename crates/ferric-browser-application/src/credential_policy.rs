//! Closed browser-owned authority and page-supplied credential options.
//!
//! The only values accepted from a page are the operation and optional RP ID.
//! Origin and document ownership are kept in opaque types and are not
//! deserializable. Production origin and lease construction will be supplied
//! only by the qualified engine boundary.

use ferric_browser_core::{DocumentId, PrivacyKind, ProfileId, TabId, WindowId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialAdmissionError {
    UnsupportedContext,
    RpIdMismatch,
    InvalidRequest,
}

/// A syntactically valid HTTPS origin paired with the host used for RP checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedOrigin {
    origin: String,
    rp_id: String,
}

impl VerifiedOrigin {
    /// Test-only origin parsing for synthetic policy fixtures. Parsing a string
    /// does not establish engine provenance, so production code cannot call it.
    #[cfg(test)]
    pub(crate) fn synthetic_from_security_origin(
        value: &str,
    ) -> Result<Self, CredentialAdmissionError> {
        let authority = value
            .strip_prefix("https://")
            .ok_or(CredentialAdmissionError::UnsupportedContext)?;
        if value.len() > 2048
            || !value.is_ascii()
            || authority.is_empty()
            || authority.contains(['/', '?', '#', '@', '\\'])
            || authority.bytes().any(|byte| byte.is_ascii_whitespace())
        {
            return Err(CredentialAdmissionError::UnsupportedContext);
        }

        let (host, port) = match authority.rsplit_once(':') {
            Some((host, port)) if !host.contains(':') => (host, Some(port)),
            Some(_) => return Err(CredentialAdmissionError::UnsupportedContext),
            None => (authority, None),
        };
        if !valid_host(host) {
            return Err(CredentialAdmissionError::UnsupportedContext);
        }
        let port = port
            .map(|value| {
                if value.is_empty() || value.len() > 5 || !value.bytes().all(|b| b.is_ascii_digit())
                {
                    return Err(CredentialAdmissionError::UnsupportedContext);
                }
                value
                    .parse::<u16>()
                    .ok()
                    .filter(|port| *port != 0)
                    .ok_or(CredentialAdmissionError::UnsupportedContext)
            })
            .transpose()?;
        let origin = match port {
            None | Some(443) => format!("https://{host}"),
            Some(port) => format!("https://{host}:{port}"),
        };
        Ok(Self {
            origin,
            rp_id: host.to_owned(),
        })
    }

    #[must_use]
    pub fn origin(&self) -> &str {
        &self.origin
    }

    #[must_use]
    pub fn rp_id(&self) -> &str {
        &self.rp_id
    }
}

fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && !host.starts_with('.')
        && !host.ends_with('.')
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

/// Browser-derived ownership for one ready top-level document.
///
/// No production constructor is exposed until the engine boundary can prove
/// the source of each fact. Tests use an explicit synthetic constructor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialDocumentLease {
    origin: VerifiedOrigin,
    profile: ProfileId,
    profile_generation: u64,
    window: WindowId,
    tab: TabId,
    document: DocumentId,
    renderer_generation: u64,
    view_incarnation: u64,
    privacy: PrivacyKind,
    main_frame: bool,
    secure_context: bool,
    permissions_policy_allowed: bool,
    foreground: bool,
}

impl CredentialDocumentLease {
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn synthetic(
        origin: VerifiedOrigin,
        profile: ProfileId,
        profile_generation: u64,
        window: WindowId,
        tab: TabId,
        document: DocumentId,
        renderer_generation: u64,
        view_incarnation: u64,
        privacy: PrivacyKind,
        main_frame: bool,
        secure_context: bool,
        permissions_policy_allowed: bool,
        foreground: bool,
    ) -> Self {
        Self {
            origin,
            profile,
            profile_generation,
            window,
            tab,
            document,
            renderer_generation,
            view_incarnation,
            privacy,
            main_frame,
            secure_context,
            permissions_policy_allowed,
            foreground,
        }
    }

    fn admission_error(&self) -> Option<CredentialAdmissionError> {
        if !self.main_frame
            || !self.secure_context
            || !self.permissions_policy_allowed
            || !self.foreground
            || self.profile_generation == 0
            || self.renderer_generation == 0
            || self.view_incarnation == 0
        {
            Some(CredentialAdmissionError::UnsupportedContext)
        } else {
            None
        }
    }

    #[must_use]
    pub fn origin(&self) -> &VerifiedOrigin {
        &self.origin
    }

    #[must_use]
    pub const fn profile(&self) -> ProfileId {
        self.profile
    }

    #[must_use]
    pub const fn profile_generation(&self) -> u64 {
        self.profile_generation
    }

    #[must_use]
    pub const fn window(&self) -> WindowId {
        self.window
    }

    #[must_use]
    pub const fn tab(&self) -> TabId {
        self.tab
    }

    #[must_use]
    pub const fn document(&self) -> DocumentId {
        self.document
    }

    #[must_use]
    pub const fn renderer_generation(&self) -> u64 {
        self.renderer_generation
    }

    #[must_use]
    pub const fn view_incarnation(&self) -> u64 {
        self.view_incarnation
    }

    #[must_use]
    pub const fn privacy(&self) -> PrivacyKind {
        self.privacy
    }
}

/// The page-facing subset of credential request data. It carries no origin,
/// profile, document, or result destination that could replace browser state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialRequestOptions {
    operation: CredentialOperation,
    requested_rp_id: Option<String>,
}

impl CredentialRequestOptions {
    /// # Errors
    /// Rejects an empty, oversized, or malformed requested RP ID.
    pub fn new(
        operation: CredentialOperation,
        requested_rp_id: Option<&str>,
    ) -> Result<Self, CredentialAdmissionError> {
        let requested_rp_id = requested_rp_id
            .map(|rp_id| {
                if !valid_rp_id(rp_id) {
                    Err(CredentialAdmissionError::InvalidRequest)
                } else {
                    Ok(rp_id.to_owned())
                }
            })
            .transpose()?;
        Ok(Self {
            operation,
            requested_rp_id,
        })
    }

    #[must_use]
    pub const fn operation(&self) -> CredentialOperation {
        self.operation
    }
}

fn valid_rp_id(rp_id: &str) -> bool {
    !rp_id.is_empty()
        && rp_id.len() <= 253
        && rp_id.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialOperation {
    Create,
    Get,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CredentialRequestId(u64);

impl CredentialRequestId {
    // The subsequent ceremony-state slice allocates these IDs.
    #[allow(dead_code)]
    pub(crate) const fn from_sequence(sequence: u64) -> Self {
        Self(sequence)
    }

    #[must_use]
    pub const fn sequence(self) -> u64 {
        self.0
    }
}

/// Immutable return authority captured before any provider work begins.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialRequestOwner {
    id: CredentialRequestId,
    lease: CredentialDocumentLease,
}

impl CredentialRequestOwner {
    // The subsequent ceremony-state slice captures this owner at admission.
    #[allow(dead_code)]
    pub(crate) const fn capture(id: CredentialRequestId, lease: CredentialDocumentLease) -> Self {
        Self { id, lease }
    }

    #[must_use]
    pub const fn id(&self) -> CredentialRequestId {
        self.id
    }

    #[must_use]
    pub const fn lease(&self) -> &CredentialDocumentLease {
        &self.lease
    }
}

// Used by this slice's T052 tests and by the later ceremony-state admission.
#[allow(dead_code)]
pub(crate) fn validate_request_for_lease(
    request: &CredentialRequestOptions,
    lease: &CredentialDocumentLease,
) -> Result<(), CredentialAdmissionError> {
    if let Some(error) = lease.admission_error() {
        return Err(error);
    }
    if request
        .requested_rp_id
        .as_deref()
        .is_some_and(|rp_id| rp_id != lease.origin.rp_id)
    {
        return Err(CredentialAdmissionError::RpIdMismatch);
    }
    Ok(())
}

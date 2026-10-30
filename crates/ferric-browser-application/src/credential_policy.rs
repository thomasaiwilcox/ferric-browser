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
    InvalidRequest,
}

/// Browser-verified origin serialization, constructed only through the
/// application capture boundary from trusted engine facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedOrigin {
    origin: String,
}

impl VerifiedOrigin {
    /// Validates the engine's serialized security origin. Parsing alone does
    /// not establish provenance; the private native challenge supplies that.
    fn from_engine_security_origin(value: &str) -> Result<Self, CredentialAdmissionError> {
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
        Ok(Self { origin })
    }

    #[cfg(test)]
    pub(crate) fn synthetic_from_security_origin(
        value: &str,
    ) -> Result<Self, CredentialAdmissionError> {
        Self::from_engine_security_origin(value)
    }

    #[must_use]
    pub fn origin(&self) -> &str {
        &self.origin
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
/// Production adapters capture this through the application, which supplies
/// actual Rust ownership and live session identity. Tests also have an explicit
/// synthetic constructor that cannot establish engine qualification.
#[derive(Clone, Debug, Eq, PartialEq)]
// Keep each browser-derived eligibility fact explicit; combining these facts
// would make the captured lease less auditable at this security boundary.
#[allow(clippy::struct_excessive_bools)]
pub struct CredentialDocumentLease {
    origin: VerifiedOrigin,
    profile: ProfileId,
    profile_instance: crate::ProfileInstanceId,
    target_generation: u64,
    engine_document_generation: u64,
    host_generation: u64,
    presentation_generation: u64,
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
    // The fixture keeps each independent browser eligibility fact explicit.
    #[allow(clippy::fn_params_excessive_bools, clippy::too_many_arguments)]
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
            profile_instance: crate::ProfileInstanceId::synthetic_for_profile(profile),
            target_generation: 1,
            engine_document_generation: renderer_generation,
            host_generation: view_incarnation,
            presentation_generation: view_incarnation,
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
            || self.engine_document_generation == 0
            || self.host_generation == 0
            || self.presentation_generation == 0
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
    pub const fn profile_instance(&self) -> crate::ProfileInstanceId {
        self.profile_instance
    }

    #[must_use]
    pub const fn target_generation(&self) -> u64 {
        self.target_generation
    }

    #[must_use]
    pub const fn engine_document_generation(&self) -> u64 {
        self.engine_document_generation
    }

    #[must_use]
    pub const fn host_generation(&self) -> u64 {
        self.host_generation
    }

    #[must_use]
    pub const fn presentation_generation(&self) -> u64 {
        self.presentation_generation
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
    /// Keeps the optional RP ID raw for the pinned helper's full IDNA and PSL
    /// validation. Only a byte bound is applied at this boundary.
    ///
    /// # Errors
    /// Rejects an RP ID larger than [`MAX_REQUESTED_RP_ID_BYTES`].
    pub fn new(
        operation: CredentialOperation,
        requested_rp_id: Option<&str>,
    ) -> Result<Self, CredentialAdmissionError> {
        let requested_rp_id = requested_rp_id
            .map(|rp_id| {
                if rp_id.len() > MAX_REQUESTED_RP_ID_BYTES {
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

    #[must_use]
    pub fn requested_rp_id(&self) -> Option<&str> {
        self.requested_rp_id.as_deref()
    }
}

/// Maximum raw RP ID size carried from the page to P06's pinned validator.
pub const MAX_REQUESTED_RP_ID_BYTES: usize = 1024;

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

/// Explicitly records that a candidate has not received helper PSL validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RpValidationStatus {
    Pending,
}

/// Bounded raw page options captured against one immutable native owner.
///
/// This candidate does not authorize an RP or admit a ceremony. Only P06's
/// pinned helper preparation with maintained PSL data may make that decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OriginBoundRpCandidate {
    owner: CredentialRequestOwner,
    operation: CredentialOperation,
    requested_rp_id: Option<String>,
    status: RpValidationStatus,
}

impl OriginBoundRpCandidate {
    pub(crate) fn into_owner(self) -> CredentialRequestOwner {
        self.owner
    }

    #[must_use]
    pub fn owner(&self) -> &CredentialRequestOwner {
        &self.owner
    }

    #[must_use]
    pub const fn operation(&self) -> CredentialOperation {
        self.operation
    }

    #[must_use]
    pub fn requested_rp_id(&self) -> Option<&str> {
        self.requested_rp_id.as_deref()
    }

    #[must_use]
    pub const fn status(&self) -> RpValidationStatus {
        self.status
    }
}

// The state manager binds immutable native ownership before handing only the
// bounded raw candidate to the helper. This performs no RP validation.
#[allow(dead_code)]
pub(crate) fn bind_request_to_owner(
    request: CredentialRequestOptions,
    owner: CredentialRequestOwner,
) -> Result<OriginBoundRpCandidate, CredentialAdmissionError> {
    if let Some(error) = owner.lease.admission_error() {
        return Err(error);
    }
    Ok(OriginBoundRpCandidate {
        owner,
        operation: request.operation,
        requested_rp_id: request.requested_rp_id,
        status: RpValidationStatus::Pending,
    })
}

/// Facts from the private engine main-frame challenge. This is deliberately not
/// deserializable. Adapters must construct it from the challenged isolated
/// world and native owner, never from a page or `WebChannel` message. Application
/// page/profile/window identity is captured separately from live Rust state.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct EngineCredentialFacts {
    pub security_origin: String,
    pub profile_generation: u64,
    pub page_generation: u64,
    pub document_generation: u64,
    pub host_generation: u64,
    pub presentation_generation: u64,
    pub main_frame: bool,
    pub secure_context: bool,
    pub permissions_policy_allowed: bool,
    pub foreground: bool,
}

impl crate::BrowserApplication {
    /// Captures a credential lease from current application ownership and
    /// already-verified private engine facts. This does not enable a provider.
    ///
    /// # Errors
    /// Rejects missing/closed ownership or unsupported engine/security facts.
    pub fn capture_credential_document(
        &self,
        tab: TabId,
        facts: &EngineCredentialFacts,
    ) -> Result<CredentialDocumentLease, CredentialAdmissionError> {
        let target = self
            .capture_target(tab)
            .ok_or(CredentialAdmissionError::UnsupportedContext)?;
        let page = self
            .tabs()
            .get(&tab)
            .ok_or(CredentialAdmissionError::UnsupportedContext)?;
        let profile = self
            .profiles()
            .get(&page.profile)
            .ok_or(CredentialAdmissionError::UnsupportedContext)?;
        let session = self
            .profile_instance_id()
            .ok_or(CredentialAdmissionError::UnsupportedContext)?;
        if self
            .profile_snapshot()
            .is_none_or(|active| active.privacy != profile.privacy)
            || self
                .windows()
                .get(&page.window)
                .is_none_or(|window| window.profile != page.profile)
        {
            return Err(CredentialAdmissionError::UnsupportedContext);
        }
        let lease = CredentialDocumentLease {
            origin: VerifiedOrigin::from_engine_security_origin(&facts.security_origin)?,
            profile: page.profile,
            profile_instance: session,
            target_generation: target.generation,
            engine_document_generation: facts.document_generation,
            host_generation: facts.host_generation,
            presentation_generation: facts.presentation_generation,
            profile_generation: facts.profile_generation,
            window: page.window,
            tab,
            document: target.document,
            // The native document fence advances on renderer loss as well as
            // document replacement. It is intentionally conservative.
            renderer_generation: facts.document_generation,
            view_incarnation: facts.page_generation,
            privacy: profile.privacy,
            main_frame: facts.main_frame,
            secure_context: facts.secure_context,
            permissions_policy_allowed: facts.permissions_policy_allowed,
            foreground: facts.foreground,
        };
        if let Some(error) = lease.admission_error() {
            return Err(error);
        }
        Ok(lease)
    }
}

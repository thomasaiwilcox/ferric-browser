//! T052 tests use synthetic authority facts only. They do not establish Qt or
//! engine provenance or authorize an RP for production.

use super::{
    CredentialAdmissionError, CredentialDocumentLease, CredentialOperation, CredentialRequestId,
    CredentialRequestOptions, CredentialRequestOwner, OriginBoundRpCandidate, RpValidationStatus,
    VerifiedOrigin,
};
use ferric_browser_core::{DocumentId, PrivacyKind, ProfileId, TabId, WindowId};

fn synthetic_lease(foreground: bool) -> CredentialDocumentLease {
    CredentialDocumentLease::synthetic(
        VerifiedOrigin::synthetic_from_security_origin("https://login.example.com")
            .expect("synthetic secure origin"),
        ProfileId::from_display("profileid-2").expect("profile ID"),
        7,
        WindowId::from_display("windowid-11").expect("window ID"),
        TabId::from_display("tabid-13").expect("tab ID"),
        DocumentId::from_display("documentid-5").expect("document ID"),
        19,
        23,
        PrivacyKind::Normal,
        true,
        true,
        true,
        foreground,
    )
}

fn synthetic_owner(foreground: bool) -> CredentialRequestOwner {
    CredentialRequestOwner::capture(
        CredentialRequestId::from_sequence(1),
        synthetic_lease(foreground),
    )
}

fn options(rp_id: Option<&str>) -> CredentialRequestOptions {
    CredentialRequestOptions::new(CredentialOperation::Create, rp_id)
        .expect("bounded raw request options")
}

fn bind(
    rp_id: Option<&str>,
    owner: CredentialRequestOwner,
) -> Result<OriginBoundRpCandidate, CredentialAdmissionError> {
    super::credential_policy::bind_request_to_owner(options(rp_id), owner)
}

#[test]
fn t052_candidate_is_pending_and_keeps_captured_owner_immutable() {
    assert!(VerifiedOrigin::synthetic_from_security_origin("http://login.example.com").is_err());
    assert!(
        VerifiedOrigin::synthetic_from_security_origin("https://login.example.com/path").is_err()
    );

    let owner = synthetic_owner(true);
    let captured_owner = owner.clone();
    let candidate = bind(Some("login.example.test"), owner).expect("lease-bound candidate");

    assert_eq!(candidate.status(), RpValidationStatus::Pending);
    assert_eq!(candidate.owner(), &captured_owner);
    assert_eq!(candidate.operation(), CredentialOperation::Create);
    assert_eq!(candidate.requested_rp_id(), Some("login.example.test"));
    assert_eq!(candidate.owner().id().sequence(), 1);
    assert_eq!(
        candidate.owner().lease().origin().origin(),
        "https://login.example.com"
    );
    assert_eq!(
        candidate.owner().lease().document(),
        DocumentId::from_display("documentid-5").expect("document ID")
    );
    assert_eq!(candidate.owner(), &captured_owner);
}

#[test]
fn t052_parent_sibling_public_suffix_and_idn_rp_strings_stay_raw_and_pending() {
    let owner = synthetic_owner(true);
    for rp_id in [
        "example.com",         // possible parent RP; P06 decides with its PSL
        "account.example.com", // unrelated sibling RP
        "com",                 // public suffix
        "bücher.example",      // IDN input for helper normalization
        "BÜCHER.Example",      // preserve page spelling for helper validation
        "",                    // malformed option; P06 rejects before side effects
        "login.example.com",   // exact-host candidate is pending too
    ] {
        let candidate = bind(Some(rp_id), owner.clone()).expect("bounded candidate");
        assert_eq!(candidate.requested_rp_id(), Some(rp_id));
        assert_eq!(candidate.status(), RpValidationStatus::Pending);
        assert_eq!(candidate.owner(), &owner);
    }

    let default_rp_candidate = bind(None, owner.clone()).expect("default RP candidate");
    assert_eq!(default_rp_candidate.requested_rp_id(), None);
    assert_eq!(default_rp_candidate.status(), RpValidationStatus::Pending);
}

#[test]
fn t052_bounds_raw_rp_data_and_rejects_ineligible_owner_context() {
    let oversized = "x".repeat(super::credential_policy::MAX_REQUESTED_RP_ID_BYTES + 1);
    assert_eq!(
        CredentialRequestOptions::new(CredentialOperation::Get, Some(&oversized)),
        Err(CredentialAdmissionError::InvalidRequest)
    );

    assert_eq!(
        bind(Some("example.com"), synthetic_owner(false)),
        Err(CredentialAdmissionError::UnsupportedContext)
    );
}

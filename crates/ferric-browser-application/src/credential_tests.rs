//! T052 tests use synthetic authority facts only. They do not establish Qt or
//! engine provenance for production documents.

use super::{
    CredentialAdmissionError, CredentialDocumentLease, CredentialOperation, CredentialRequestId,
    CredentialRequestOptions, CredentialRequestOwner, VerifiedOrigin,
};
use ferric_browser_core::{DocumentId, PrivacyKind, ProfileId, TabId, WindowId};

fn synthetic_lease(foreground: bool) -> CredentialDocumentLease {
    CredentialDocumentLease::synthetic(
        VerifiedOrigin::synthetic_from_security_origin("https://login.example.test")
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

fn options(rp_id: Option<&str>) -> CredentialRequestOptions {
    CredentialRequestOptions::new(CredentialOperation::Create, rp_id)
        .expect("well-formed request options")
}

#[test]
fn t052_options_cannot_overwrite_owner_authority_and_rp_matches_verified_origin() {
    assert!(VerifiedOrigin::synthetic_from_security_origin("http://login.example.test").is_err());
    assert!(
        VerifiedOrigin::synthetic_from_security_origin("https://login.example.test/path").is_err()
    );

    let lease = synthetic_lease(true);
    let owner = CredentialRequestOwner::capture(CredentialRequestId::from_sequence(1), lease);
    let captured_owner = owner.clone();
    let page_options = options(Some("attacker.test"));
    assert_eq!(page_options.operation(), CredentialOperation::Create);
    assert_eq!(
        super::credential_policy::validate_request_for_lease(&page_options, owner.lease()),
        Err(CredentialAdmissionError::RpIdMismatch)
    );
    assert_eq!(owner, captured_owner);

    super::credential_policy::validate_request_for_lease(
        &options(Some("login.example.test")),
        owner.lease(),
    )
    .expect("same-origin RP is admitted");
    assert_eq!(owner.id().sequence(), 1);
    assert_eq!(
        owner.lease().origin().origin(),
        "https://login.example.test"
    );
    assert_eq!(
        owner.lease().document(),
        DocumentId::from_display("documentid-5").expect("document ID")
    );
}

#[test]
fn t052_rejects_invalid_rp_ids_and_ineligible_synthetic_lease() {
    for rp_id in [
        "",
        ".example.test",
        "example..test",
        "example.test.",
        "ExAmPlE.test",
    ] {
        assert_eq!(
            CredentialRequestOptions::new(CredentialOperation::Get, Some(rp_id)),
            Err(CredentialAdmissionError::InvalidRequest),
            "accepted invalid RP ID {rp_id:?}"
        );
    }

    let lease = synthetic_lease(false);
    assert_eq!(
        super::credential_policy::validate_request_for_lease(&options(None), &lease),
        Err(CredentialAdmissionError::UnsupportedContext)
    );
}

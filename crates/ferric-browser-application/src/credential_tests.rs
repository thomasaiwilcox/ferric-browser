//! T052 tests use synthetic authority facts only. They do not establish Qt or
//! engine provenance or authorize an RP for production.

use super::{
    CredentialAdmissionError, CredentialDocumentLease, CredentialOperation, CredentialRequestId,
    CredentialRequestOptions, CredentialRequestOwner, OriginBoundRpCandidate, RpValidationStatus,
    VerifiedOrigin,
};
use ferric_browser_core::{DocumentId, PrivacyKind, ProfileId, TabId, WindowId};

fn synthetic_lease_for(foreground: bool, document: &str, host: &str) -> CredentialDocumentLease {
    CredentialDocumentLease::synthetic(
        VerifiedOrigin::synthetic_from_security_origin(&format!("https://{host}"))
            .expect("synthetic secure origin"),
        ProfileId::from_display("profileid-2").expect("profile ID"),
        7,
        WindowId::from_display("windowid-11").expect("window ID"),
        TabId::from_display("tabid-13").expect("tab ID"),
        DocumentId::from_display(document).expect("document ID"),
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
    synthetic_owner_with(foreground, 1, "documentid-5", "login.example.com")
}

fn synthetic_owner_with(
    foreground: bool,
    request_id: u64,
    document: &str,
    host: &str,
) -> CredentialRequestOwner {
    CredentialRequestOwner::capture(
        CredentialRequestId::from_sequence(request_id),
        synthetic_lease_for(foreground, document, host),
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

fn candidate_for(owner: CredentialRequestOwner) -> OriginBoundRpCandidate {
    bind(Some("login.example.com"), owner).expect("pending RP candidate")
}

#[test]
fn t053_abort_helper_and_stale_result_permutations_emit_once_for_captured_owner() {
    use super::{
        CredentialCeremonyState, CredentialTerminalReason, HelperCompletion, TransitionOutcome,
    };

    let owner = synthetic_owner_with(true, 11, "documentid-11", "first-login.example.com");
    let mut state = CredentialCeremonyState::synthetic_with_capacity(4);
    let token = state
        .admit(candidate_for(owner.clone()), 10, 50)
        .expect("admission reserves a terminal slot");
    assert_eq!(
        state.admit(
            candidate_for(synthetic_owner_with(
                true,
                12,
                "documentid-12",
                "busy-login.example.net",
            )),
            10,
            50,
        ),
        Err(super::CeremonyAdmissionError::Busy),
        "a concurrent request cannot replace the active owner"
    );
    assert_eq!(state.abort(token, 11), TransitionOutcome::TerminalEmitted);
    assert_eq!(
        state.helper_result(token, HelperCompletion::Success, 12),
        TransitionOutcome::StaleOrAlreadyTerminal
    );
    assert_eq!(
        state.queued_terminal_count(),
        1,
        "late helper result cannot enqueue a second effect"
    );
    let terminal = state.drain_one().expect("accepted request terminal effect");
    assert_eq!(terminal.owner(), &owner);
    assert_eq!(terminal.reason(), CredentialTerminalReason::Aborted);

    let second_owner = synthetic_owner_with(true, 22, "documentid-22", "second-login.example.net");
    let second = state
        .admit(candidate_for(second_owner.clone()), 20, 50)
        .expect("slot was released by drain");
    assert_eq!(
        state.helper_result(token, HelperCompletion::Success, 71),
        TransitionOutcome::StaleOrAlreadyTerminal,
        "a stale prior token cannot mutate a fresh owner, even past its deadline"
    );
    assert!(state.has_active_ceremony());
    assert_eq!(state.expire(second, 71), TransitionOutcome::TerminalEmitted);
    assert_eq!(
        state.abort(second, 72),
        TransitionOutcome::StaleOrAlreadyTerminal
    );
    let terminal = state.drain_one().expect("successor terminal effect");
    assert_eq!(terminal.owner(), &second_owner);
    assert_eq!(terminal.reason(), CredentialTerminalReason::DeadlineExpired);
}

#[test]
fn t053_navigation_and_deadline_transitions_preserve_original_owner() {
    use super::credential_state::SyntheticCredentialClock;
    use super::{
        CredentialCeremonyState, CredentialTerminalReason, HelperCompletion, TransitionOutcome,
    };

    let owner = synthetic_owner_with(true, 40, "documentid-40", "current.example.com");
    let unrelated_owner = synthetic_owner_with(false, 40, "documentid-41", "other.example.net");
    let mut clock = SyntheticCredentialClock::default();
    let mut state = CredentialCeremonyState::synthetic_with_capacity(3);
    let token = state
        .admit(candidate_for(owner.clone()), clock.now_ms(), 25)
        .expect("admission");
    clock.advance(5);
    assert_eq!(
        state.invalidate_document(token, &unrelated_owner, clock.now_ms()),
        TransitionOutcome::StaleOrAlreadyTerminal
    );
    assert!(state.has_active_ceremony());
    assert_eq!(
        state.invalidate_document(token, &owner, clock.now_ms()),
        TransitionOutcome::TerminalEmitted
    );
    assert_eq!(
        state.helper_result(token, HelperCompletion::Success, clock.now_ms()),
        TransitionOutcome::StaleOrAlreadyTerminal
    );
    let terminal = state.drain_one().expect("navigation terminal effect");
    assert_eq!(terminal.owner(), &owner);
    assert_eq!(
        terminal.reason(),
        CredentialTerminalReason::DocumentInvalidated
    );

    let deadline_owner = synthetic_owner_with(true, 41, "documentid-42", "deadline.example.org");
    let deadline_token = state
        .admit(candidate_for(deadline_owner.clone()), clock.now_ms(), 10)
        .expect("admission after drain");
    assert_eq!(
        state.expire(deadline_token, clock.now_ms()),
        TransitionOutcome::DeadlineNotReached
    );
    clock.set(15);
    assert_eq!(
        state.expire(deadline_token, clock.now_ms()),
        TransitionOutcome::TerminalEmitted
    );
    assert_eq!(
        state.helper_result(deadline_token, HelperCompletion::Success, clock.now_ms()),
        TransitionOutcome::StaleOrAlreadyTerminal
    );
    let terminal = state.drain_one().expect("deadline terminal effect");
    assert_eq!(terminal.owner(), &deadline_owner);
    assert_eq!(terminal.reason(), CredentialTerminalReason::DeadlineExpired);
    assert_eq!(
        state.expire(deadline_token, clock.now_ms()),
        TransitionOutcome::StaleOrAlreadyTerminal
    );
}

#[test]
fn t053_first_terminal_event_at_or_after_deadline_is_timeout_for_each_event_kind() {
    use super::{
        CredentialCeremonyState, CredentialTerminalReason, HelperCompletion, TransitionOutcome,
    };

    #[derive(Clone, Copy)]
    enum Event {
        Abort,
        HelperSuccess,
        HelperFailure,
        Navigation,
    }

    for (index, (event, event_time)) in [
        (Event::Abort, 110),
        (Event::HelperSuccess, 111),
        (Event::HelperFailure, 110),
        (Event::Navigation, 111),
    ]
    .into_iter()
    .enumerate()
    {
        let sequence = 60 + u64::try_from(index).expect("small test index");
        let document = format!("documentid-{sequence}");
        let host = format!("late{sequence}.example.com");
        let owner = synthetic_owner_with(true, sequence, &document, &host);
        let mut state = CredentialCeremonyState::synthetic_with_capacity(2);
        let token = state
            .admit(candidate_for(owner.clone()), 10, 100)
            .expect("deadline-bound admission");

        let outcome = match event {
            Event::Abort => state.abort(token, event_time),
            Event::HelperSuccess => {
                state.helper_result(token, HelperCompletion::Success, event_time)
            }
            Event::HelperFailure => {
                state.helper_result(token, HelperCompletion::Failure, event_time)
            }
            Event::Navigation => state.invalidate_document(token, &owner, event_time),
        };
        assert_eq!(outcome, TransitionOutcome::TerminalEmitted);
        assert!(!state.has_active_ceremony());
        assert_eq!(state.queued_terminal_count(), 1);

        assert_eq!(
            state.abort(token, event_time + 1),
            TransitionOutcome::StaleOrAlreadyTerminal
        );
        assert_eq!(
            state.helper_result(token, HelperCompletion::Success, event_time + 1),
            TransitionOutcome::StaleOrAlreadyTerminal
        );
        assert_eq!(
            state.helper_result(token, HelperCompletion::Failure, event_time + 1),
            TransitionOutcome::StaleOrAlreadyTerminal
        );
        assert_eq!(
            state.invalidate_document(token, &owner, event_time + 1),
            TransitionOutcome::StaleOrAlreadyTerminal
        );
        assert_eq!(
            state.expire(token, event_time + 1),
            TransitionOutcome::StaleOrAlreadyTerminal
        );
        assert_eq!(state.queued_terminal_count(), 1);

        let terminal = state.drain_one().expect("one reserved timeout effect");
        assert_eq!(terminal.owner(), &owner);
        assert_eq!(terminal.reason(), CredentialTerminalReason::DeadlineExpired);
        assert!(state.drain_one().is_none());
    }
}

#[test]
fn t053_helper_success_before_deadline_completes_once_for_original_owner() {
    use super::{
        CredentialCeremonyState, CredentialTerminalReason, HelperCompletion, TransitionOutcome,
    };

    let owner = synthetic_owner_with(true, 80, "documentid-80", "success.example.org");
    let mut state = CredentialCeremonyState::synthetic_with_capacity(2);
    let token = state
        .admit(candidate_for(owner.clone()), 100, 50)
        .expect("deadline-bound admission");
    assert_eq!(
        state.helper_result(token, HelperCompletion::Success, 149),
        TransitionOutcome::TerminalEmitted
    );
    assert_eq!(
        state.abort(token, 149),
        TransitionOutcome::StaleOrAlreadyTerminal
    );
    assert_eq!(
        state.helper_result(token, HelperCompletion::Failure, 149),
        TransitionOutcome::StaleOrAlreadyTerminal
    );
    assert_eq!(state.queued_terminal_count(), 1);
    let terminal = state.drain_one().expect("single helper completion effect");
    assert_eq!(terminal.owner(), &owner);
    assert_eq!(terminal.reason(), CredentialTerminalReason::HelperCompleted);
}

#[test]
fn t053_terminal_outbox_reserves_capacity_before_admission_and_recovers_on_drain() {
    use super::{
        CeremonyAdmissionError, CredentialCeremonyState, CredentialTerminalReason,
        TransitionOutcome,
    };

    let first_owner = synthetic_owner_with(true, 51, "documentid-51", "first.example.com");
    let second_owner = synthetic_owner_with(true, 52, "documentid-52", "second.example.net");
    let third_owner = synthetic_owner_with(true, 53, "documentid-53", "third.example.org");
    let mut state = CredentialCeremonyState::synthetic_with_capacity(2);

    let first = state
        .admit(candidate_for(first_owner.clone()), 0, 100)
        .expect("first request reserves slot one");
    assert_eq!(state.abort(first, 1), TransitionOutcome::TerminalEmitted);
    let second = state
        .admit(candidate_for(second_owner.clone()), 2, 100)
        .expect("second request reserves remaining slot");
    assert_eq!(state.abort(second, 3), TransitionOutcome::TerminalEmitted);
    assert_eq!(state.queued_terminal_count(), 2);
    assert_eq!(
        state.admit(candidate_for(third_owner.clone()), 4, 100),
        Err(CeremonyAdmissionError::TerminalOutboxFull),
        "a full outbox rejects before capturing a new owner"
    );
    assert!(!state.has_active_ceremony());

    let first_terminal = state.drain_one().expect("first accepted terminal retained");
    assert_eq!(first_terminal.owner(), &first_owner);
    assert_eq!(first_terminal.reason(), CredentialTerminalReason::Aborted);
    let third = state
        .admit(candidate_for(third_owner.clone()), 5, 100)
        .expect("drain releases capacity");
    assert_eq!(state.abort(third, 6), TransitionOutcome::TerminalEmitted);
    let second_terminal = state
        .drain_one()
        .expect("second terminal retained in order");
    assert_eq!(second_terminal.owner(), &second_owner);
    let third_terminal = state
        .drain_one()
        .expect("third terminal retained after recovery");
    assert_eq!(third_terminal.owner(), &third_owner);
}

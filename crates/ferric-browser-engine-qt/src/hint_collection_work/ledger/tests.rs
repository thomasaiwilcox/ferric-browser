use super::*;

fn binding(phase: NativePhase, stage: u8) -> Binding {
    Binding {
        phase,
        stage,
        owner: String::new(),
    }
}

fn limits(inspections: usize, verification_steps: usize, annotation_bytes: usize) -> NativeBudget {
    NativeBudget {
        inspections,
        verification_steps,
        annotation_bytes,
    }
}

fn ledger() -> Box<Ledger> {
    Ledger::new(7, Schedule::testing(100, limits(10, 20, 30))).unwrap()
}

#[test]
fn reservations_are_visible_before_dispatch_and_only_unused_capacity_returns() {
    let mut ledger = ledger();
    let binding = binding(NativePhase::Preflight, 0);
    let grant = ledger
        .reserve(0, binding.clone(), limits(10, 20, 30))
        .unwrap();
    assert_eq!(
        ledger.remaining_native(),
        limits(49990, 80, MAX_INTERACTION_ANNOTATION_BYTES - 30)
    );
    assert!(ledger.complete(
        &grant.token,
        &binding,
        NativeOutcome::Known(limits(3, 4, 5))
    ));
    assert_eq!(
        ledger.remaining_native(),
        limits(49997, 96, MAX_INTERACTION_ANNOTATION_BYTES - 5)
    );
    assert_eq!(ledger.preflight_used, limits(3, 4, 5));
    assert!(!ledger.has_pending());
    assert!(ledger.reserve(0, binding, limits(1, 1, 1)).is_none());
}

#[test]
fn native_preflight_cannot_consume_unissued_ordinary_or_verification_capacity() {
    let mut ledger = ledger();
    let binding = binding(NativePhase::Preflight, 0);
    for limit in [limits(11, 0, 0), limits(0, 21, 0), limits(0, 0, 31)] {
        assert!(ledger.reserve(0, binding.clone(), limit).is_none());
        assert_eq!(
            ledger.remaining_native(),
            limits(MAX_HINT_INSPECTIONS, 100, MAX_INTERACTION_ANNOTATION_BYTES)
        );
    }
    let grant = ledger
        .reserve(0, binding.clone(), limits(10, 20, 30))
        .unwrap();
    assert!(!ledger.complete(&grant.token, &binding, NativeOutcome::Unknown));
    assert!(ledger.reserve(1, binding, limits(1, 0, 0)).is_none());
    assert_eq!(
        ledger.remaining_native(),
        limits(49990, 80, MAX_INTERACTION_ANNOTATION_BYTES - 30)
    );
}

#[test]
fn no_script_refunds_capacity_but_cannot_repeat_the_same_scheduled_operation() {
    let mut ledger = ledger();
    let binding = binding(NativePhase::Preflight, 0);
    let grant = ledger
        .reserve(0, binding.clone(), limits(10, 20, 30))
        .unwrap();
    assert!(ledger.complete(&grant.token, &binding, NativeOutcome::NoScript));
    assert_eq!(
        ledger.remaining_native(),
        limits(MAX_HINT_INSPECTIONS, 100, MAX_INTERACTION_ANNOTATION_BYTES)
    );
    assert!(
        ledger
            .reserve(0, binding.clone(), limits(10, 20, 30))
            .is_none()
    );
    assert!(ledger.reserve(1, binding, limits(10, 20, 30)).is_some());
}

#[test]
fn unknown_or_out_of_grant_reports_burn_and_late_reports_cannot_refund() {
    for outcome in [
        NativeOutcome::Unknown,
        NativeOutcome::Known(limits(11, 1, 1)),
        NativeOutcome::Known(limits(1, 21, 1)),
        NativeOutcome::Known(limits(1, 1, 31)),
    ] {
        let mut ledger = ledger();
        let binding = binding(NativePhase::Preflight, 0);
        let grant = ledger
            .reserve(0, binding.clone(), limits(10, 20, 30))
            .unwrap();
        assert!(!ledger.complete(&grant.token, &binding, outcome));
        assert!(!ledger.complete(&grant.token, &binding, NativeOutcome::NoScript));
        assert_eq!(
            ledger.remaining_native(),
            limits(49990, 80, MAX_INTERACTION_ANNOTATION_BYTES - 30)
        );
    }
}

#[test]
fn request_world_stage_and_kind_mismatches_leave_the_live_grant_intact() {
    let mut ledger = ledger();
    let original = binding(NativePhase::Policy, 3);
    let grant = ledger
        .reserve(0, original.clone(), limits(1, 10, 10))
        .unwrap();
    for changed in [
        binding(NativePhase::Preflight, 0),
        binding(NativePhase::Policy, 4),
        Binding {
            owner: "frame-22222222-2222-4222-8222-222222222222".into(),
            ..original.clone()
        },
    ] {
        assert!(!ledger.complete(&grant.token, &changed, NativeOutcome::NoScript));
        assert!(ledger.has_pending());
    }
    let mut other = Ledger::new(7, Schedule::graph_decode()).unwrap();
    let second = other
        .reserve(0, original.clone(), limits(1, 10, 10))
        .unwrap();
    assert!(!ledger.complete(&second.token, &original, NativeOutcome::NoScript));
    assert!(ledger.complete(
        &grant.token,
        &original,
        NativeOutcome::Known(limits(1, 10, 10))
    ));
}

#[test]
fn exact_global_boundaries_and_highest_operation_id_do_not_wrap() {
    let mut ledger = ledger();
    let binding = binding(NativePhase::Policy, 3);
    for limit in [
        limits(MAX_HINT_INSPECTIONS + 1, 0, 0),
        limits(0, 101, 0),
        limits(0, 0, MAX_INTERACTION_ANNOTATION_BYTES + 1),
    ] {
        assert!(ledger.reserve(0, binding.clone(), limit).is_none());
    }
    let limit = limits(MAX_HINT_INSPECTIONS, 100, MAX_INTERACTION_ANNOTATION_BYTES);
    let grant = ledger.reserve(u16::MAX, binding.clone(), limit).unwrap();
    assert_eq!(ledger.issued.len(), 1024);
    assert_eq!(ledger.remaining_native(), NativeBudget::default());
    assert!(ledger.complete(&grant.token, &binding, NativeOutcome::Known(limit)));
    assert!(ledger.reserve(u16::MAX, binding, limits(1, 0, 0)).is_none());
    assert!(Ledger::new(0, Schedule::graph_decode()).is_none());
}

#[test]
fn pending_capacity_is_bounded_and_proven_no_script_releases_a_slot() {
    let mut ledger = Ledger::new(7, Schedule::graph_decode()).unwrap();
    let binding = binding(NativePhase::Policy, 3);
    let grants = (0..128)
        .map(|operation| {
            ledger
                .reserve(operation, binding.clone(), limits(0, 1, 0))
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert!(
        ledger
            .reserve(128, binding.clone(), limits(0, 1, 0))
            .is_none()
    );
    assert!(ledger.complete(&grants[0].token, &binding, NativeOutcome::NoScript));
    assert!(ledger.reserve(128, binding, limits(0, 1, 0)).is_some());
}

#[test]
fn rejection_cancels_refunds_and_never_steals_semantic_predicate_capacity() {
    let mut ledger = ledger();
    let binding = binding(NativePhase::Policy, 3);
    let grant = ledger
        .reserve(0, binding.clone(), limits(1, 90, 1))
        .unwrap();
    assert_eq!(ledger.used.predicate_steps, 0);
    assert!(ledger.charge_verification(10));
    assert!(!ledger.charge_verification(1));
    assert_eq!(ledger.used.predicate_steps, 0);
    ledger.burn_remaining(false);
    assert!(!ledger.complete(&grant.token, &binding, NativeOutcome::NoScript));
    assert_eq!(ledger.remaining_native(), NativeBudget::default());
}

#[test]
fn graph_schedule_does_not_pretend_to_cover_native_preflight() {
    assert_eq!(GRAPH_DECODE_VERIFICATION_SUBTOTAL, 1_000_000);
    let mut ledger = Ledger::new(7, Schedule::graph_decode()).unwrap();
    assert!(
        ledger
            .reserve(0, binding(NativePhase::Preflight, 0), limits(1, 1, 0))
            .is_none()
    );
    assert_eq!(ledger.used.predicate_steps, 0);
}

use super::{
    ClaimResult, EnvironmentKey, EvictionReason, HotPageCoordinator, PrepareDecision,
    PresentationKind, State,
};
use std::sync::{Arc, Barrier};

fn key(host_window: u64) -> EnvironmentKey {
    EnvironmentKey {
        profile_identity: 1,
        profile_generation: 2,
        partition_identity: 3,
        context_isolation: 4,
        settings_generation: 5,
        presentation: PresentationKind::Internal,
        host_window: Some(host_window),
    }
}

fn started(decision: PrepareDecision) -> u64 {
    match decision {
        PrepareDecision::Start { token } => token,
        other => panic!("expected preparation permit, got {other:?}"),
    }
}

#[test]
fn t025_many_windows_and_requests_grant_only_one_spare_then_refill_after_claim() {
    let policy = HotPageCoordinator::default();
    let first_env = key(6);
    let first_token = started(policy.request_prepare(first_env.clone(), true, 10));
    assert_eq!(policy.claim(&first_env, true), ClaimResult::ColdMiss);
    for _ in 0..100 {
        assert_eq!(
            policy.request_prepare(first_env.clone(), true, 11),
            PrepareDecision::AlreadyPreparing
        );
    }
    assert!(policy.take_replenish_request());
    assert!(!policy.take_replenish_request());

    // A newly active window replaces the old preparation incarnation; the
    // old callback cannot make a second spare ready.
    let second_env = key(7);
    let second_token = started(policy.request_prepare(second_env.clone(), true, 12));
    assert_ne!(first_token, second_token);
    assert!(!policy.mark_ready(first_token, 12));
    assert!(policy.mark_ready(second_token, 12));
    assert_eq!(
        policy.claim(&second_env, true),
        ClaimResult::ReadyHit {
            token: second_token
        }
    );

    let third_env = key(8);
    assert_eq!(
        policy.request_prepare(third_env.clone(), false, 13),
        PrepareDecision::Ineligible,
        "an ineligible refill cannot release a claimed permit"
    );
    assert_eq!(
        policy.request_prepare(third_env.clone(), true, 13),
        PrepareDecision::Suppressed,
        "the claimed spare retains the sole permit until adoption completes"
    );
    let malformed_env = EnvironmentKey {
        host_window: None,
        ..third_env.clone()
    };
    assert_eq!(
        policy.request_prepare(malformed_env, true, 13),
        PrepareDecision::Ineligible,
        "a malformed refill cannot release a claimed permit"
    );
    assert_eq!(
        policy.request_prepare(third_env.clone(), true, 13),
        PrepareDecision::Suppressed
    );
    assert!(!policy.fail_preparation(second_token, EvictionReason::RendererFailure));
    assert!(policy.finish_claim(second_token));
    let refill_token = started(policy.request_prepare(third_env.clone(), true, 14));
    assert_ne!(refill_token, second_token);
    assert!(policy.mark_ready(refill_token, 14));
    assert_eq!(
        policy.claim(&third_env, true),
        ClaimResult::ReadyHit {
            token: refill_token
        }
    );
}

#[test]
fn t025_claimed_permit_survives_evictions_until_matching_finish() {
    let policy = HotPageCoordinator::default();
    let env = key(6);
    let token = started(policy.request_prepare(env.clone(), true, 0));
    assert!(policy.mark_ready(token, 0));
    assert_eq!(policy.claim(&env, true), ClaimResult::ReadyHit { token });

    let reasons = [
        EvictionReason::Timeout,
        EvictionReason::IncompatibleEnvironment,
        EvictionReason::ProfileClosed,
        EvictionReason::HostClosed,
        EvictionReason::ConfigurationChanged,
        EvictionReason::PrivacyChanged,
        EvictionReason::PartitionChanged,
        EvictionReason::RendererFailure,
        EvictionReason::GraphicsFailure,
        EvictionReason::MemoryPressure,
        EvictionReason::Shutdown,
        EvictionReason::Contaminated,
    ];
    for reason in reasons {
        policy.evict(reason);
        assert_eq!(
            policy.request_prepare(env.clone(), true, 1),
            PrepareDecision::Suppressed,
            "{reason:?} cannot release a claimed permit"
        );
    }

    assert!(!policy.finish_claim(token.wrapping_add(1)));
    assert_eq!(
        policy.request_prepare(env.clone(), true, 1),
        PrepareDecision::Suppressed,
        "a stale finish cannot release the claimed permit"
    );
    assert!(policy.finish_claim(token));

    let refill = started(policy.request_prepare(env.clone(), true, 2));
    assert_ne!(refill, token);
    assert_eq!(
        policy.request_prepare(env, true, 2),
        PrepareDecision::AlreadyPreparing,
        "matching finish releases exactly one preparation permit"
    );
}

#[test]
fn t026_preparing_open_is_immediate_cold_miss_without_state_change() {
    let policy = HotPageCoordinator::default();
    let env = key(6);
    let token = started(policy.request_prepare(env.clone(), true, 0));

    assert_eq!(policy.claim(&env, true), ClaimResult::ColdMiss);
    assert!(matches!(
        policy.0.lock().expect("hot-page policy mutex poisoned").state,
        State::Preparing {
            token: current,
            started_ms: 0
        } if current == token
    ));
    assert_eq!(
        policy.request_prepare(env, true, 1),
        PrepareDecision::AlreadyPreparing,
        "a cold open neither waits for nor cancels preparation"
    );
}

#[test]
fn t026_racing_opens_have_exactly_one_ready_hit_and_one_cold_miss() {
    let policy = Arc::new(HotPageCoordinator::default());
    let env = key(6);
    let token = started(policy.request_prepare(env.clone(), true, 0));
    assert!(policy.mark_ready(token, 0));

    let barrier = Arc::new(Barrier::new(3));
    let workers: Vec<_> = (0..2)
        .map(|_| {
            let policy = Arc::clone(&policy);
            let barrier = Arc::clone(&barrier);
            let env = env.clone();
            std::thread::spawn(move || {
                barrier.wait();
                policy.claim(&env, true)
            })
        })
        .collect();
    barrier.wait();

    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().expect("claim worker panicked"))
        .collect();
    let mut ready_hits = 0;
    let mut cold_misses = 0;
    for result in results {
        match result {
            ClaimResult::ReadyHit { token: claimed } => {
                assert_eq!(claimed, token);
                ready_hits += 1;
            }
            ClaimResult::ColdMiss => cold_misses += 1,
        }
    }
    assert_eq!(ready_hits, 1);
    assert_eq!(cold_misses, 1);
    assert!(matches!(
        policy.0.lock().expect("hot-page policy mutex poisoned").state,
        State::Claimed { token: current } if current == token
    ));

    assert!(!policy.fail_preparation(token, EvictionReason::RendererFailure));
    assert!(!policy.finish_claim(token.wrapping_add(1)));
    assert_eq!(
        policy.request_prepare(env.clone(), true, 1),
        PrepareDecision::Suppressed,
        "a stale finish cannot release the adopted page"
    );
    assert!(policy.finish_claim(token));

    let refill = started(policy.request_prepare(env.clone(), true, 2));
    assert_ne!(refill, token);
    assert_eq!(
        policy.request_prepare(env, true, 2),
        PrepareDecision::AlreadyPreparing,
        "matching completion permits exactly one refill"
    );
}

#[test]
fn preparation_expiry_allows_two_retries_then_suppresses_stale_callbacks() {
    let policy = HotPageCoordinator::default();
    let env = key(6);
    let first = started(policy.request_prepare(env.clone(), true, 100));
    assert_eq!(
        policy.request_prepare(env.clone(), true, 5_099),
        PrepareDecision::AlreadyPreparing
    );

    let second = started(policy.request_prepare(env.clone(), true, 5_100));
    assert_ne!(first, second);
    assert!(!policy.mark_ready(first, 5_100));
    let third = started(policy.request_prepare(env.clone(), true, 10_100));
    assert_ne!(second, third);
    assert!(!policy.mark_ready(second, 10_100));
    assert_eq!(
        policy.request_prepare(env, true, 15_100),
        PrepareDecision::Suppressed,
        "the initial attempt plus two retries are the full timeout budget"
    );
    assert!(!policy.mark_ready(third, 15_100));
}

#[test]
fn retryable_failure_reasons_share_the_two_retry_cap() {
    let policy = HotPageCoordinator::default();
    let env = key(6);
    let reasons = [
        EvictionReason::RendererFailure,
        EvictionReason::GraphicsFailure,
        EvictionReason::Contaminated,
    ];
    let mut stale_token = None;
    for (attempt, reason) in reasons.into_iter().enumerate() {
        let token = started(policy.request_prepare(env.clone(), true, attempt as u64));
        if let Some(stale) = stale_token {
            assert!(!policy.fail_preparation(stale, reason));
        }
        assert!(policy.fail_preparation(token, reason));
        stale_token = Some(token);
    }
    assert_eq!(
        policy.request_prepare(env, true, 4),
        PrepareDecision::Suppressed
    );
}

#[test]
fn ready_spare_failures_consume_two_retries_for_the_same_environment() {
    let policy = HotPageCoordinator::default();
    let env = key(6);
    let reasons = [
        EvictionReason::RendererFailure,
        EvictionReason::GraphicsFailure,
        EvictionReason::Contaminated,
    ];
    let mut previous_token = None;
    for (attempt, reason) in reasons.into_iter().enumerate() {
        let token = started(policy.request_prepare(env.clone(), true, attempt as u64));
        assert!(policy.mark_ready(token, attempt as u64));
        if let Some(stale) = previous_token {
            assert!(!policy.fail_preparation(stale, reason));
        }
        assert!(policy.fail_preparation(token, reason));
        previous_token = Some(token);
    }
    assert_eq!(
        policy.request_prepare(env, true, 4),
        PrepareDecision::Suppressed,
        "Ready failures count toward the same initial-attempt plus two-retry budget"
    );
}

#[test]
fn ready_completion_is_rejected_at_or_after_deadline_and_spends_retry() {
    let policy = HotPageCoordinator::default();
    let env = key(6);

    let before_deadline = started(policy.request_prepare(env.clone(), true, 100));
    assert!(policy.mark_ready(before_deadline, 5_099));
    assert_eq!(
        policy.claim(&env, true),
        ClaimResult::ReadyHit {
            token: before_deadline
        }
    );
    assert!(policy.finish_claim(before_deadline));

    let at_deadline = started(policy.request_prepare(env.clone(), true, 10_000));
    assert!(!policy.mark_ready(at_deadline, 15_000));
    assert!(!policy.mark_ready(at_deadline, 15_001));
    let retry = started(policy.request_prepare(env.clone(), true, 15_001));
    assert_ne!(retry, at_deadline);
    assert!(!policy.mark_ready(retry, 20_002));
    let final_retry = started(policy.request_prepare(env.clone(), true, 20_002));
    assert_ne!(final_retry, retry);
    assert!(!policy.mark_ready(final_retry, 25_003));
    assert_eq!(
        policy.request_prepare(env, true, 25_003),
        PrepareDecision::Suppressed,
        "late ready callbacks consume the same initial-attempt plus two-retry budget"
    );
}

#[test]
fn stale_completion_cannot_expire_the_current_preparation() {
    let policy = HotPageCoordinator::default();
    let old = key(6);
    let old_token = started(policy.request_prepare(old, true, 0));
    let current = key(7);
    let current_token = started(policy.request_prepare(current, true, 1_000));

    assert!(!policy.mark_ready(old_token, 6_001));
    assert!(matches!(
        policy.0.lock().expect("hot-page policy mutex poisoned").state,
        State::Preparing { token, .. } if token == current_token
    ));

    assert!(!policy.mark_ready(current_token, 6_001));
    assert!(matches!(
        policy
            .0
            .lock()
            .expect("hot-page policy mutex poisoned")
            .state,
        State::Evicted {
            reason: EvictionReason::Timeout
        }
    ));
}

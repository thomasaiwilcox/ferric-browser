fn pending_selection_fixture(owner: &mut BrowserUiRust, operation_id: &str, consumer: &str) {
    let target = current_target(owner.state.as_ref(), owner.tab).expect("fixture target");
    let token = format!("selection-{operation_id}");
    owner.pending_selection = Some(PendingSelection {
        token: token.clone(), target, primary: false, issued: true,
        created_at: Instant::now(), search_engine: None,
        operation_id: Some(operation_id.into()),
    });
    owner.operation_states.insert(operation_id.into(), "selection pending".into());
    match consumer {
        "spawn" => owner.pending_spawn = Some(PendingSpawn {
            token, target, argv: vec!["/bin/true".into()], operation_id: operation_id.into(),
        }),
        "action" => owner.pending_action_target = Some(PendingActionTarget {
            token, target, target_name: "fixture".into(),
            target_config: ActionTargetConfig {
                executable: "/bin/true".into(), argv: Vec::new(),
                subject_types: vec!["selection".into()], detach: false, allow_private: false,
            },
            operation_id: operation_id.into(), created_at: Instant::now(),
        }),
        "script" => owner.pending_userscript = Some(PendingUserscript {
            token, target, name: "fixture".into(), label: "Inspect selection".into(),
            operation_id: operation_id.into(),
        }),
        "search" => owner.pending_selection.as_mut().unwrap().search_engine = Some("default".into()),
        _ => panic!("unknown fixture consumer"),
    }
}

#[test]
fn selection_cancellation_revokes_every_consumer() {
    for consumer in ["spawn", "action", "script", "search"] {
        let mut owner = BrowserUiRust::default();
        pending_selection_fixture(&mut owner, "op-selection", consumer);
        cancel_pending_selection_operation(&mut owner, "op-selection");
        assert_eq!(owner.operation_states["op-selection"], "cancelled", "{consumer}");
        assert!(owner.pending_selection.is_none(), "{consumer}");
        assert!(owner.pending_spawn.is_none(), "{consumer}");
        assert!(owner.pending_action_target.is_none(), "{consumer}");
        assert!(owner.pending_userscript.is_none(), "{consumer}");
    }
}

#[test]
fn unrelated_cancellation_preserves_the_active_selection() {
    let mut owner = BrowserUiRust::default();
    pending_selection_fixture(&mut owner, "op-active", "action");
    cancel_pending_selection_operation(&mut owner, "op-other");
    assert_eq!(owner.operation_states["op-active"], "selection pending");
    assert_eq!(owner.pending_selection.as_ref().unwrap().token, "selection-op-active");
    assert!(owner.pending_action_target.is_some());
}

#[test]
fn retiring_selection_preserves_terminal_results_and_unrelated_work() {
    for terminal in ["cancelled", "completed", "detached", "failed (timeout)"] {
        let mut owner = BrowserUiRust::default();
        pending_selection_fixture(&mut owner, "op-selection", "spawn");
        owner.operation_states.insert("op-selection".into(), terminal.into());
        owner.operation_states.insert("op-running".into(), "running".into());
        finish_pending_selection_operation(&mut owner, "cancelled");
        assert_eq!(owner.operation_states["op-selection"], terminal);
        assert_eq!(owner.operation_states["op-running"], "running");
        assert!(owner.pending_spawn.is_none());
    }
}

#[test]
fn selection_failure_notices_explain_recovery_without_machine_codes() {
    assert_eq!(selection_failure_message("selection-source-changed"),
        "The selection changed. Select the text again and retry.");
    assert_eq!(selection_failure_message("selection-extraction-timeout"),
        "Reading the selected text took too long. Try again.");
    assert_eq!(selection_failure_message("unrecognized provider detail"),
        "Select text on the page and try again.");
    assert_eq!(selection_failure_message("password fields are not copied"),
        "Copying password fields as plain text is disabled.");
    assert_eq!(selection_failure_message("closed-shadow selection is unsupported"),
        "Plain-text selection is unsupported in this control. Use Copy instead.");
}

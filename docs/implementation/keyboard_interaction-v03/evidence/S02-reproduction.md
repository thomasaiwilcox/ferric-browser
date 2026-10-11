# S02 reproduction and result

S01's pre-production run recorded eleven failures in the new WebEngine hint fixtures and one native shadow-focus failure (`evidence/S01.json`, `evidence/logs/S01/current-qml.log`, and `evidence/logs/S01/current-native.log`). The S02 changes repair the shared DOM validation and final action path. On this worktree, the complete Qt Quick/WebEngine suite now reports **107 passed, 0 failed**; every focused T002–T009 case in that suite passes. The separate native offscreen CTest run also passes.

Focus and scroll actions now use the shared composed visibility and clipping walk with their own eligibility rules. The added pointerless-input case confirms that focus does not depend on pointer hit-testing, while the existing root/body overflow fixture confirms that a pointerless scroll region remains selectable and scrolls. Ordinary click freshness still requires an exact point hit.

The final focus-redirection fixture uses an ApplicationWorld `focus` override. The offscreen WebEngine direct-focus setup did not invoke the fixture's inline `onfocus` handler, so the override makes the redirected-active-element condition deterministic while still testing that the production action reports failure when the retained target did not become the deep active element.

`test_T020_map_teardown` passes and verifies the direct retained-map/listener cleanup path. The broader T020 callback-owner and destination-lease handoff is not exercised by this slice, so T020 remains unqualified and S03 must supply that proof. This status does not invalidate the direct cleanup result.

The final `cargo xtask check` exited 0. It includes all workspace Rust tests (including 396 `ferric-browser-engine-qt` tests) and the 107-test QML suite. `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo fmt --all -- --check`, `git diff --check`, the generated-script Node syntax checks, and the separate native offscreen CTest also exited 0. Qt header diagnostics were forwarded during the generated C++ build; they did not fail Clippy or the repository check.

Environment: Linux x86_64, Qt 6.11.2, Chromium 140.0.7339.225, offscreen software renderer, 640×480 CSS source viewport at zoom 1 for functional fixtures. Tests use local synthetic fixtures; no live site, physical desktop/IME, or Wayland editable-input qualification was performed.

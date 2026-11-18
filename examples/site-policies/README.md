# Local Site Policy examples

These files are opt-in, constrained `ferric-site-interaction` documents. They
are not configuration includes and are never installed automatically. The six
fixture examples below use `.example.test` hosts. The separate
[live examples](live/) were captured and checked on
15 November 2026; their exact tested scopes and limitations are recorded in
[live verification](LIVE_VERIFICATION.md).

For a practical starting setup, see the researched
[YouTube and Reddit presets](presets/README.md). They include numbered grid/feed
links, fixed navigation hints and a small YouTube passthrough player mapping,
with the tested scopes and current site limitations recorded separately.

| File | Example | Local fixture |
| --- | --- | --- |
| `navigation.toml` | Apple-style semantic Mac/Watch/iPhone anchors; absent labels stay reserved | `crates/ferric-browser-engine-qt/tests/fixtures/site-policy/interaction.html` |
| `navigation-two-character.toml` | The same semantic anchors using `mm`, `ww`, `ip` | `interaction.html` |
| `visible-list.toml` | Reddit-style visible logical items; primary links rather than menu buttons | `interaction.html`, `#fixture-suggestions` |
| `grid-sidebar.toml` | YouTube-style independent grid and sidebar, separate explicit code arrays | `crates/ferric-browser-engine-qt/tests/fixtures/site-policy/sidebar.html` |
| `shadow-grid.toml` | Per-item open-shadow primary links and row-major letter codes | `interaction.html`, `#fixture-grid` |
| `player-remap.toml` | `p` → `k` for noneditable passthrough receivers and selected Hint page keys | `interaction.html`, `#command-target` |

The production parser validates and round-trips the six fixture examples in
`site_policy_interchange::shipped_local_examples_are_valid_constrained_round_trips`.
Native builder tests exercise navigation, list, grid, sidebar, shadow-card and
trusted `p` → `k` delivery. The examples' domains deliberately do not match the
loopback test server. To try a local fixture, serve the fixture directory, import
the file into a temporary draft, and deliberately change its pattern to the
exact served origin/path in the builder before previewing.

## Import, review and share

Open `:site-policy`, choose **Import / export**, select the local path and import
into **temporary** first. Import creates a draft; it does not apply configuration.
Inspect every rule ID, scope, source, preserved ordinary settings, shadowed rule,
capability and replacement diff. Existing IDs require explicit replacement
acknowledgement. Choose a new ID when you do not intend a snapshot replacement.

Use **Live** preview on the selected tab. Static validation cannot prove a target
exists. Capture and preview do not activate links or send remapped keys. Apply
only the reviewed revision, and inspect its saved/active result. For key mappings,
use the separate **Test on page** action after applying and press the source key
through the native input path. Preview and synthetic DOM events do not prove
delivery. `Ctrl+Esc` returns to Normal.

The same service is available from the command line:

```text
site-policy-import /absolute/path/navigation.toml --temp
site-policy-preview DRAFT_ID --tab TAB_ID
site-policy-apply DRAFT_ID --revision REVISION --activate
site-policy-export /absolute/path/shared.toml --draft DRAFT_ID
```

`DRAFT_ID`, `REVISION` and `TAB_ID` come from the actual replies. A batch is applied
atomically. No rule from a rejected batch is partially published. Use
`--replace-existing` only to create a deliberate reviewed replacement draft.
Export requires explicit persistence acknowledgement in the workspace, including
private sessions. Existing files require explicit overwrite. Symlinks and
managed configuration files are refused. Exports contain Interaction operations,
not ordinary settings, captured page values, kept samples or credentials.

## Repair after a page changes

Open the exact source/ID in Site Policies and read the absent, ambiguous or stale
reason. Reopen the builder, capture a fresh representative target, check its root
and scope, and explicitly adjust predicates or item identity. Keep additional
samples only when you want to compare them. Review changed scope and the complete
rule, run a fresh preview, then apply its new revision. Rebase explicitly if local
configuration changed. A missing or stale collection position stays unavailable;
it must not inherit another item's reserved code.

The live examples include an [Apple Mac anchor](live/apple-uk-navigation.toml),
[Reddit post positions](live/reddit-home-posts.toml) and a
[YouTube player mapping](live/youtube-watch-player.toml). The latter was checked
in one-shot passthrough only. The [channel grid](live/youtube-channel-grid.toml) also passed one scoped replay;
the earlier crash is retained, and a confirmed Qt corruption path is now mitigated.
Its exact causal attribution remains unproven. Sidebar, Hint-context media delivery
and physical input qualification remain open. See
[live verification](LIVE_VERIFICATION.md) before using these policies.

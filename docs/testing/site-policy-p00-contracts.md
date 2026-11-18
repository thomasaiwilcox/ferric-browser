# Site policy P00 source contract inventory

Inspected baseline: `f2c5d6deebd97eb861084503f25613a6a819949d`.
This is source inspection and planned coverage, not executed qualification.
The binding implementation scope remains the owner’s implementation plan.
No phase gate is established by this inventory alone.
The Section 3 boundaries and interface owners are frozen in
[site-policy-binding-contracts.md](site-policy-binding-contracts.md); baseline
ownership evidence is indexed in [site-policy-no-policy-ownership.md](site-policy-no-policy-ownership.md).

## Invocation families and options

The production parser is `src/hint_policy.rs::parse_hint_options` in the
engine crate. Its eleven invocation families are:

| Family | Parser/action contract |
| --- | --- |
| `actions` | Default invocation; excludes focusable-only and region/surface kinds, except frame handling |
| `all` | Explicit broad discovery invocation, distinct from `actions` |
| `focus` | Current-target focus only; no rapid, script, pointer or alternate targets; first/index allowed |
| `links` | Link discovery and existing URL action semantics |
| `inputs` | Existing input action semantics |
| `buttons` | Existing button action semantics |
| `images` | Current action activates element; explicit URL actions may consume currentSrc |
| `media` | Same element-versus-explicit-URL distinction as images |
| `scrollables` | Existing scroll-target semantics |
| `regions` | Local action palette only, default/explicit `choose` |
| `surfaces` | Local action palette only, default/explicit `choose` |

`hint_payload.rs::validate_hint_candidates_for_family` enforces the closed
candidate-kind checks, including focus-only kinds and the 256 region/surface
caps. `hint_policy.rs::hint_uses_url_action` preserves the image/media
current-element distinction; a label must not turn every action into a click
or URL open.

There are only **seven** configurable discovery-selector families:
`all`, `links`, `inputs`, `buttons`, `images`, `media`, `scrollables`.
They are exhaustively enumerated by `HintSelectorFamily::ALL` in
`crates/ferric-browser-config/src/selector_config.rs`. Invocation families
must not be added to that enum simply to represent policy applicability.

The parser accepts targets `current`, `tab`, `tab-bg`, `window`, `yank`,
`clean-yank`, `download`, `userscript`, `ephemeral`, `choose`, `pointer`, and
`external:NAME`. Parser acceptance alone does not establish that every
candidate has a valid transport for every target.

| Combination | Baseline parser behavior |
| --- | --- |
| No arguments | `actions`, `current`, not rapid, not first, index 1 |
| `--first` | Immediate canonical selection; index defaults to 1 |
| `--first --index N` | N must be 1–5000 inclusive |
| `--index N` without first | Reject |
| `--rapid --first` | Reject |
| Rapid with current (including explicit current) | Normalize target to yank |
| Rapid after normalization | Only tab-bg, yank, clean-yank, download, userscript |
| Userscript | Requires `--script NAME`; script forbidden for other targets |
| Focus | Current only, no rapid or script; first/index remains valid |
| Pointer | Explicit family required; no rapid, first, index or script; focus and region/surface rules still apply |
| Regions/surfaces | Only choose; no rapid, first, index, script, pointer or URL target |
| Duplicate family/script/first/index/rapid | Reject (duplicate first/rapid fall through invalid-arguments branch) |

The baseline parser does **not** reject repeated `--target`: later target
replaces earlier target. Preserve evidence of this behavior separately from
any deliberate strictness correction; do not assume a duplicate-flag test
already exists for it.

Existing parser tests include
`parses_named_options_and_rejects_invalid_combinations`,
`action_default_and_explicit_focus_options_are_unambiguous`,
`parses_specialized_first_selection_and_rejects_conflicts`,
`region_surface_families_open_only_the_explicit_action_palette`, and
`pointer_origin_detection_does_not_treat_alias_names_as_local_commands`.
Pointer and palette permits are owner-bound, local and one-shot, as covered by
the adjacent permit tests. Policy addressing must preserve these authorities.

P00 adds `site_policy_baseline_family_target_invocation_matrix` in
`hint_policy.rs`: an explicit compatibility table covering all eleven families,
all twelve named targets plus omitted target, rapid on/off, interactive/first/
indexed/index-without-first selection, and script present/absent (2,288 cells).
Accepted cells assert exact normalized options as well as parser acceptance.
`site_policy_baseline_defaults_and_index_boundaries` records default options,
indices 1/5000, invalid/out-of-range/overflow indices, explicit-family Pointer
requirements and baseline last-target-wins behavior. The parser oracle passed
(`parser-matrix.log`); it establishes parser compatibility, with native
delivery/action qualification tracked separately.

## Hint controls and early input

`qml/components/FerricRuntimeController.qml::handleBrowserKey` owns Hint key
interpretation. `FerricHintWorkspace.qml::applyHintInteraction` converts `/`
to text-mode and literal Space to collision rotation in label mode. In text
mode they remain query characters. Therefore `/` and literal Space cannot
be special codes or fallback-prefix characters.

| Input | Existing Hint intent |
| --- | --- |
| Escape | Close Hint workspace |
| Ctrl+U | Clear |
| Backspace | Backspace |
| Tab, Right, Down | Next; Shift selects previous |
| Backtab, Left, Up | Previous |
| Return/Enter | Activate; Pointer instead begins separate confirmation, nonrepeat only |
| Ctrl+Space | Rotate collision |
| Single logical character | Character, subject to slash/Space label-mode conversion |

Pointer preflight/confirmation consumes other target input; Escape remains
cancellation. `HintInteraction` in core owns label/text query state and
FullMatch/Never/Always/UniqueMatch decisions. Baseline unmatched character
handling can restore the old prefix/selection. Enhanced sessions must add the
specified explicit no-selection behavior without silently rewriting ordinary
no-policy behavior.

The early queue admits only `character`, `text-mode`, `backspace`, `clear`,
`next`, `previous`, `activate`, `rotate` (workspace lines around 588–629).
P08 must prove provisional query ownership before candidates exist; observing
these strings alone does not prove early slash followed by p/k/f is safe.

## Modes and ownership

`crates/ferric-browser-core/src/model.rs::Mode` includes Normal, Insert,
InsertLocked, Command, Search, Hint, Grid, Caret, PassThrough and
PassThroughOnce. Mixed is a Normal shortcut policy, not a separate Mode.
Insert is temporary text entry; InsertLocked is persistent text entry.

Runtime routing gives observable native prompts/Qt menus their original event
before media, Mixed or browser bindings. Persistent and one-shot passthrough
return page ownership except existing protected rescue. Normal Mixed decisions
run before ordinary bindings for a fresh gesture. Active counts/keychains
remain browser-owned, and browser-owned repeat cannot become page input merely
because its first press changed modes.

`mixed_policy.rs::EffectiveMixedKeyPolicy::decide` orders explicit mode,
reserved controls, active count, active keychain, browser-first policy, exact
page key, then no mapping. Exact explicitly configured page keys can override
an ordinary binding while idle; a first mapped digit can beat count start,
but cannot interrupt an active count. Reserved sources include `:`, Escape,
Ctrl+Escape and Ctrl+Shift+Escape. Tests
`first_key_decision_preserves_policy_mode_reservation_and_sequence_order`,
`browser_prefix_mismatch_and_timeout_never_replay_to_a_mapped_page_key`, and
`mapped_first_digit_beats_count_start_but_not_an_active_count` cover the model.
New remaps must follow that admission decision, not become a second admission
mechanism.

Default Normal bindings in `core/src/input.rs` use `;v` for one-shot and `;V`
for persistent passthrough. `browser_key_router.h/.cpp` holds physical gesture
identity, entry keys, forwarded keys, accepted browser keys, page-owned keys
and native clipboard overrides. One-shot completion follows terminal real
release; autorepeat releases and entry releases do not terminate it. Overlapping
forwarded keys retain ownership after selected gesture completion. Composition,
native focus, ShortcutOverride and clipboard are native-router concerns.
These sets are baseline ownership machinery, not the required new normalized
output ledger or proof of transformed native delivery.

## Planned acceptance ownership and gaps

All entries below are **planned**, not passed. Each exact T-P/T-H/T-N/T-U row in
the implementation plan remains an independent required evidence cell; grouping
here does not replace that matrix.

| Requirement group | Owning phases | Pure evidence | Production evidence |
| --- | --- | --- | --- |
| T-P01–T-P15 policy/schema/composition/transactions | P02–P04, P11, P14–P15 | Config schema/golden/layer/limit and transaction failure tests | Coordinator publication and multiwindow editor receipt tests |
| T-P16 malformed renderer proof | P05, P14 | Bounded payload rejection/fuzz tests | Real collector-to-QML-to-Rust rejection |
| T-H01–T-H48 allocation/roots/witnesses/all-family behavior | P04–P08, P14–P15 | Codebook oracle, ordering, retirement, typed permit tests | qml-hints-v2 plus actual application/WebEngine local fixtures |
| T-N01–T-N27 native contexts/raw/recovery/platforms | P01–P02, P09–P10, P14, P16 | Token/route/ledger/cancellation state tests | Production native-insert/application traces plus real X11/Wayland/layout input |
| T-U01–T-U21 authoring/import/privacy/accessibility | P03, P11–P14, P16–P17 | Draft/revision/import/path-safety tests | Real Settings/builder keyboard journeys, accessibility, installed package |

P00-03 parser compatibility, P00-06 native matrix inventory and P00-08 fixture
subjects are reviewed complete; see the current baseline execution record.
Remaining work includes source-bound application ownership/prompt/recovery
evidence, the full acceptance-to-test mapping and prescribed performance
baselines. Actual native/platform qualification remains open; inventory
completion does not qualify its cells. No native transformation,
platform, live-site, performance or installed-product success is claimed here.

## Configuration sources, edits and publication (P00-05)

`application/src/configuration.rs::ConfigurationLayers` retains five inputs;
config `resolve_runtime_override_layers` applies them in this order:

| Layer | Actual source shape and public surfaces |
| --- | --- |
| Base | `Config`, including authored `site_rules`; `runtime_config.rs::load` merges trusted local includes, then including-file values; `merge` recursively merges tables and replaces arrays |
| Profile | `profiles.toml` `ProfileDefinition.overrides`, a setting map converted by `profile_override_layer` to `RuntimeOverrides.settings`; no profile site-rule array |
| Generated runtime | Durable profile state `runtime-overrides.toml`; `RuntimeOverrides` contains settings, bindings and site_rules; separate from authored configuration |
| Command line | `RuntimeOverrides` internally; startup `--set KEY=VALUE` calls `set_literal` in `ferric-browser/src/main.rs`; settings surface, not a demonstrated CLI site-rule document surface |
| Temporary | `RuntimeOverrides` retained by BrowserUi; temporary setting/binding operations use `browser_ui_runtime_settings.rs`/`browser_ui_runtime_config.rs`; typed Interaction editing is new work |

`validation.rs::validate_profile_definition` explicitly rejects
schema/include/bindings/site_rules/permission_rules and rule subpaths.
Internal use of RuntimeOverrides does not authorize inventing profile or CLI
source shapes. Config and RuntimeOverrides both currently have version 3,
but distinct constants and schema authorities.

`apply_runtime_overrides` validates each layer, overlays settings/bindings,
then replaces an earlier exact-ID rule in full and appends its replacement.
Replacement precedes URL matching: a replaced lower rule cannot return when
the higher pattern does not match. But flattened `Config.site_rules` loses
layer identity; `matching_site_rules` sorts all surviving matches by ascending
priority/index. P03 must retain layers so a lower high priority cannot outrank
a higher layer. Different IDs with equal patterns remain separate.

Legacy `RuntimeOverrides::set_site_literal`/`set_site_value` and
`unset_site_setting` currently search the first exact pattern, **not** canonical
ID. New rules get priority `i32::MAX` and `validation.rs::runtime_site_rule_id`:
FNV-1a over exact pattern bytes, 64-bit offset `0xcbf29ce484222325`, multiplier
`0x100000001b3`, rendered `runtime-site-{hash:016x}`. P03 must target that ID
and verify its pattern, preserving custom same-pattern rules. Unset currently
prunes when `set` becomes empty; Interaction requires contribution-aware pruning.
The setter clones and validates before replacement.

The deliberate touched precedence corrections are:

1. `selector_config.rs::resolve_hint_selector_policies` seeds from flattened
   `config.hints.selectors`, then overlays all matching sites. Provenance says
   `effective-config`/`site-rule:ID`, losing real layer context.
2. `routing.rs::hint_proxy_targets_for_url` seeds flattened proxy_targets,
   then applies all matching sites last.
3. Thus a lower authored site can override an explicitly higher generated,
   CLI or temporary global value. P03 must resolve layer by layer, preserve
   selector extend/replace, and release-note changed no-Interaction cases.
   Ordinary include-array replacement is outside this correction.

Engine consumers are `browser_ui_navigation.rs::effective_hint_selector_snapshot`
(session creation and `hint_selector_policy`) and
`browser_ui_hint_proxy_configuration.rs::update_hint_proxy_targets`.
`selector_validation.rs::selector_validation_items` enumerates layers;
`selector_validation_items_for_check` includes declared profiles.
`append_config`/`append_overrides` build browser-owned CSS checks with syntax
deduplication. New root/item/predicate/slot selectors must join raw-source
enumeration even when disabled/shadowed; effective-only validation is insufficient.

Application `prepare_configuration_update`/`prepare_configuration_reload` call
`prepare_configuration_at`, check the writable gate/profile and build a candidate
capturing application/profile revisions. `publish_configuration_update` checks
both revisions, persists before assigning prepared state, and updates shared
profile runtime. RuntimeUpdate versus Reload is a real apply boundary:
requested/saved values may remain pending until reload. `ConfigurationReservation`
is RAII with owner-checked release; `reserve_configuration_reload` gates
competing updates/reloads.

Persistent `request_configuration_write` captures durable profile path, profile
persistence lease, candidate and reservation. Its owned `SingleFlightWorker`
calls `save_runtime_overrides_atomic`, updates shared profile runtime after
successful replacement, and survives observer retirement.
`poll_configuration_write` releases the reservation and returns
`PersistedConfigurationUpdate` for publication. The writer distinguishes
pre-replacement errors from committed `RuntimeOverrideWriteOutcome::DurabilityUncertain`
after directory-sync failure; no retry/rollback claim is valid for that outcome.
Engine `browser_ui_configuration.rs` prepares active settings and publishes
through the application, updates retained layers/revisions, and coordinates
selector validation. `browser_ui_context_reload.rs` shares reload authority.
The new service must extend these paths, not add another queue/store.

The current-source checkpoint below records bootstrap/secondary/private input
ownership, constructor locations and sparse authored presence added after this
baseline inventory. The still-required P03 implementation regressions are exact
ID shadowing/edits, cross-layer priority, selector/proxy precedence,
close-during-write, saved-versus-active and durability-warning publication.
Inventory and those later implementation gates remain distinct.

## Current authored-source checkpoint (P00 inventory; P02 implementation)

The baseline observations above retain their original meaning. Current source
now adds `LoadedConfig.authored: Arc<ConfigurationSources>` with typed sparse
site rules and shared Hint fields for every include document, before array
replacement. This is validation/provenance input, never a second raw-rule
matcher. `None` and explicit empty arrays remain distinct; synthesized Hint
defaults are not authored leaves. Effective include arrays still replace.
All pre-existing SiteRule constructors have been updated with absent Interaction.
Unset now preserves rules whose Interaction contribution remains.

`ConfigurationLayers.base_authored` travels through prepared application
updates, reload publication and profile requests. Profile fallback retains its
captured base/catalogue pair; the deliberate built-in safe reset clears both.
`selector_validation.rs` enumerates the catalogue and retained runtime layers
before deduplicating exact CSS strings, including disabled and replaced rules.
Renderer validation requests use catalogue-local document IDs; file paths stay
in browser-owned source data. The authored input remains separate from saved
versus active settings and from the pending selector callback's revision gates.

Startup uses a native owned QVariant bundle (`startup_configuration.rs/h/cpp`)
containing the already-loaded base and source Arc. `GuiLaunchPlan` retains the
catalogue; `set_startup_configuration` checks the bundle/base pair and seals
construction before Qt setter signals. Profile requests use retained authored
input. Reload updates that input only after successful publication. Popup
handoff clones it before notifications; fresh secondary bootstrap recaptures the
ready primary's pair and handles initialization failure. No metadata JSON bag,
new include limit, global source registry or runtime file reread reconstructs
that catalogue. Startup native transport previously passed four cases with
matching production artifacts; see the baseline execution record. The router
lifetime fixes require a fresh production rebuild and source-bound application
rerun, currently pending.

The 2,288-cell parser compatibility oracle has now passed. Local baseline
fixture tests also verify recycled Element/href identity invalidates the old
ordinary target before fresh collection, and delayed insertion is discovered
only in a later result. These tests do not establish enhanced witnesses, code
reservations, early-query routing or native key transformations.

Outstanding implementation corrections remain exactly the P03 source-layer,
legacy canonical-ID edit and selector/proxy precedence corrections inventoried
above, plus their production regressions. P00 completion still requires the
remaining baseline/runtime/performance evidence; this inventory is not a phase
gate or proof that the enhanced product is implemented.

### Concrete constructor, serialization and consumer locations

The original 18 struct-literal constructors are one production constructor in
`config/src/runtime_config.rs::RuntimeOverrides::set_site_value`, 15 test
constructors in `config/src/tests.rs`, and one test constructor each in engine
`browser_ui_navigation.rs` and `mixed_policy_runtime.rs`. The type itself is
`config/src/lib.rs::SiteRule`. Other current schema/example fixtures deserialize
that same strict type rather than constructing a parallel policy record.

Authored serialization enters through `runtime_config.rs::load` /
`LoadState::read_file`: TOML value parsing, pre-merge typed authored capture,
include merge and final `Config` deserialization. Generated settings enter via
`load_runtime_overrides` (TOML strict typed decode), and leave through
`save_runtime_overrides_atomic` / `write_runtime_overrides` (validated
`toml::to_string_pretty`, private temporary file, replacement and directory
sync). `apply_runtime_overrides` round-trips the Config through
`toml::Value::try_from` / `try_into` for leaf application before binding/rule
replacement. Neither projection changes the independent schema constants.

Startup `ferric-browser/src/main.rs` serializes the base before profile/runtime/
CLI application and the effective config afterward; `GuiLaunchPlan` conveys
both JSON projections and the separate native captured catalogue. Reload
`browser_ui_runtime_config.rs` projects the newly loaded base through
`serde_json::to_value` / `to_string`; its `ConfigReloadPublication` retains the
matching layers/catalogue until `browser_ui_context_reload.rs` publishes.
`browser_ui_configuration.rs` prepares effective versus saved/apply-boundary
settings and emits active JSON after application publication. JSON here is a
presentation/bootstrap projection, not an opaque policy storage section.

Selector snapshot consumers include session creation and the public
`hint_selector_policy` bridge in `browser_ui_navigation.rs`, fresh selection
validation in `browser_ui_hint_selection.rs`, and scoped Hint/Search handoffs
in `browser_ui_scope_handoff.rs`. `FerricHintWorkspace.qml` obtains the bridge
snapshot through `hintSelectorPolicySnapshot` and passes it with proxy/scope
inputs to the production collector/frame provider. These must all share the
corrected P03 resolver and revision identity; changing only session creation
would leave stale action checks.

Proxy computation remains `browser_ui_hint_proxy_configuration.rs`. Its refresh
callers include configuration/navigation updates and committed publication in
`browser_ui_configuration.rs`, plus profile activation and profile refresh in
`browser_ui_profile_setup.rs`. They must use the same layer-correct result and
invalidate existing Hint state when the effective proxy policy changes.

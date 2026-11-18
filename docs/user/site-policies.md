# Site Policies

Open `:site-policy` to create site-specific hint codes and page-key mappings.
Policies are opt-in. Creating, importing or previewing a draft does not activate it.

## Create and test a rule

1. Open the intended page and choose **Guided builder**. Start with an exact
   HTTP(S) origin/path and a new rule ID. URL matching ignores query and fragment.
2. Choose an anchor for a named action, or a collection for ordered list/grid
   positions. **Capture target** selects a representative without activating it.
3. Review the captured document, shadow, frame or slot root and selector advice.
   Choose the intended action explicitly. For a collection, select the logical
   item and its primary action rather than a menu or a whole-card click area.
4. Choose invocation families and explicit codes. The numbered-grid helper
   generates a code array for review; numbers have no special matching behavior.
5. Validate, then **Preview on tab**. Read Live, Missing, ambiguous or incomplete
   results. Preview does not click links or send mapped keys.
6. **Review full diff**, check source/scope/replacements and choose whether to
   activate after save. Apply the exact reviewed revision. Saved and active
   configuration are separate outcomes; follow any reload or failure receipt.

An anchor preserves a named meaning. A collection preserves the selected
snapshot's ordered item meanings. Missing or stale positions stay reserved;
they cannot silently acquire another item's action. Restart hints for a new
snapshot after the page changes. A taller card can qualify through its visible
primary action; it need not be mostly visible.

Families matter: a policy configured for `all` is used with `hint all`; it does
not automatically configure the default `actions` family. Keep enough fallback
code capacity for ordinary targets. Validation reports code/prefix conflicts.

## Sources and editing

Inspect the rule's source and winning provenance before editing. Higher source
layers take precedence before numeric priority. A higher-source rule with the
same ID replaces that ID before URL matching. Authored sources are read-only in
the workspace; use the explicit copy flow to create an editable rule in a
writable destination. Ordinary settings remain visible in the reviewed diff.
Rebase a draft explicitly when its configuration revision becomes stale.

For manual authoring, start from the [checked example files](../../examples/site-policies/README.md)
and import the resulting Interaction document as a draft. The existing config
schema stays at version 3, but older builds that do not recognize `interaction`
cannot read these rules. This feature has no released minimum version yet; use
the supporting build and its capability manifest together.

The command service uses the same drafts and review requirements as the workspace:

| Command | Purpose |
| --- | --- |
| `site-policy` | Open the workspace |
| `site-policy-list --all` | Inspect rules and their source identities |
| `site-policy-explain --key Return` | Explain an effective key decision |
| `site-policy-edit --source generated --id navigation` | Edit one exact rule |
| `site-policy-import PATH --temp` | Import a local document into a draft |
| `site-policy-preview DRAFT_ID --tab TAB_ID` | Preview without activating |
| `site-policy-apply DRAFT_ID --revision REVISION --activate` | Apply the reviewed revision |
| `site-policy-export PATH --draft DRAFT_ID` | Export after persistence acknowledgement |
| `site-policy-discard DRAFT_ID` | Discard an unapplied draft |

Use IDs and revision numbers from the actual replies. The
[example workflow](../../examples/site-policies/README.md#import-review-and-share)
explains replacement acknowledgement, export and repair.

## Page keys and recovery

Mappings select their contexts explicitly:

| Context | When the source key may be mapped |
| --- | --- |
| `normal-page-key` | An existing permitted Mixed page-key route |
| `hint-page-key` | One explicitly admitted Hint page-key gesture, then Normal |
| `passthrough-once` | The next-key passthrough gesture |
| `passthrough` | Persistent passthrough |

A mapping is one hop. The destination is delivered natively to an eligible
receiver; it does not become another mapping or a browser macro. Editable
controls, composition, prompts and protected recovery retain their ownership.
Use **Test on page** after applying a mapping and press its actual source key.
A preview or a synthetic DOM event does not prove native key delivery.

In ordinary Hint mode, a successfully forwarded website shortcut returns to
Normal after its final key release. This applies to both remapped and unchanged
shortcuts: `f`, then `p` can pause YouTube and dismiss hints; `f`, then `f` can
enter fullscreen and dismiss hints. Held-key repeats finish with the same
gesture. A blocked shortcut preserves Hint mode and its label prefix.
Text-query input remains text. Use `;V` for sustained website-key interaction,
or `;v` to explicitly forward one key from Normal.

`Ctrl+Escape` returns to Normal. To temporarily bypass mappings, use
`mode-enter passthrough-once --raw` or `mode-enter passthrough --raw`.
See [Webpage shortcuts](webpage-shortcuts.md) for entry/exit behavior.

Tokens describe one logical gesture. Names and modifier prefixes are case-sensitive:
`Ctrl+`, `Alt+`, `Meta+`, `Shift+`; `Return` aliases `Enter`, `PageUp` aliases
`PgUp`, `PageDown` aliases `PgDown`, and `Shift+Tab` becomes `Backtab`.
`Shift+a` becomes `A`; with a command modifier, `Ctrl+A` becomes `Ctrl+Shift+a`.
Use produced punctuation such as `!` directly, and `Plus` for `+`.
Modifier-only tokens and multi-key sequences are rejected. Parsing a Unicode
source token does not promise native Unicode output: destination support is
checked separately. An admitted printable Hint source is reserved from every
label position; the mapped destination does not acquire that reservation.
Native destination flags describe the virtual event, not an OS remap or a
physical modifier-key timeline.

## Import, export and repair

Import a constrained local Interaction document into a temporary draft first.
Review every operation; matching IDs require explicit replacement acknowledgement.
Export requires persistence acknowledgement, including in private sessions, and
explicit permission to overwrite an existing file. Exports exclude captured
samples and ordinary settings.

The [example collection](../../examples/site-policies/README.md) includes parser-
checked fixtures and narrowly tested live examples. They are never installed
automatically. Capture and preview again before applying live selectors.
When a page changes, inspect the unavailable reason, capture fresh advice and
review a repaired rule; avoid broadening a selector just to make a warning vanish.

The current recorded native capability build targets owned Wayland input and
US/GB/DE layout subsets. X11 is not fully qualified. Consult the actual
[capability manifest](../../packaging/page-key-capabilities.json) and live-example
limitations rather than assuming every key, frame or website is supported.
The [qualification record](../testing/site-policy-qualification.md) distinguishes
the current tested build from unqualified platform and live-site cases.

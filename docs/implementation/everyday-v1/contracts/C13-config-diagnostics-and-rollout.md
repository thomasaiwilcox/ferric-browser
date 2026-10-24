# C13 — Typed configuration and release states

Add exactly the following user-facing choices through existing typed Config, setting registry, validation, precedence, atomic commits, settings UI and generated references:

```toml
[tabs]
presentation = "internal"  # internal | windows; global, startup/restart
[performance]
hot_tab = "off"            # off | auto | on; global, live eviction
[passkeys]
phone = "off"              # off | on; global/profile, document-bound effect
[hyprland]
page_grouping = "off"      # off | auto; global, live; existing namespace
```

Names are new proposed keys, not claims of current support. The coordinator checks collisions in S00/F00. A collision requires a reviewed schema amendment, not two aliases. Keep one-spare limit fixed; no arbitrary helper path, transport endpoint, world ID, pool count or insecure-origin UI toggle. Scope-sensitive settings are not writable from a page or new arbitrary IPC method.

Hot changes evict incompatible spares. Presentation changes are pending restart and do not partly alter existing windows. Phone off cancels bridge-managed requests and restores native behaviour for future supported documents; do not hot-swap APIs underneath an already running native ceremony. Define whether re-enable requires navigation and show it; v1 uses next-document installation to avoid altering live page APIs inconsistently. A setting change never bypasses an unqualified capability.

Diagnostics report requested and effective capability with reason and version/proof provenance. Distinguish disabled, unavailable, unqualified, unsupported-context and active. Do not equate dependency compiled with working Bluetooth/transport, mapped Qt window with Hyprland binding, or Ready spare with useful performance. Avoid raw URLs/titles/credential IDs in diagnostic exports and preserve existing private-query opt-ins.

Development flags stay off by default. Qualification is per backend/path: internal hot, window hot, phone transport/device, generic page windows, Hyprland grouping. Build-time inclusion and runtime on/off are distinct. Document rolling back by disabling the feature/restarting presentation; no reset-data or profile deletion should be needed.

Generate reference docs from the existing registries. Update packaging/dependency/license inventories explicitly for the separately built helper and PSL/BlueZ prerequisites. CI must validate both root and helper lockfiles and compile the real production paths, not only source-grep gates.

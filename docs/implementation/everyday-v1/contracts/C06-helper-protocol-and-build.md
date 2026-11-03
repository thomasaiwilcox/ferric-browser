# C06 — Transport helper, pinned build and bounded IPC

## Separate package

Create `helpers/ferric-phone-auth/` as an explicitly independent Cargo workspace, with its own Cargo.lock, rust-version and build instructions. Do not add libwebauthn to Ferric's root graph or silently raise Ferric's 1.85 declaration. The inspected candidate is libwebauthn 0.10.0 at git revision `9c0e5f78ac445d35dca05170efbb0afefc5a89ae`, declaring Rust 1.88. P01 audits it, its required D-Bus/hidraw/Bluetooth dependencies, transitive floor and license inventory. Pin exactly; no floating master. Do not invent Cargo transport feature names: its inspected defaults are empty but several transports are ordinary dependencies. Remove a transport only through a supported upstream configuration or a separately reviewed dependency decision.

The helper executable is a packaged path resolved by the application. No page/user-script supplied executable and no arbitrary PATH fallback. A development override is allowed only through an explicit test/build mechanism unavailable to page input. Process separation contains crashes; it is not by itself an OS sandbox. No root, added Bluetooth powers or system config changes.

## Private-pipe protocol v1

Host input is one newline-delimited JSON envelope: `{v:1,id:<native opaque ID>,op:create|get,origin:<verified serialization>,options:<WebAuthn JSON>,network:<closed policy>}`. Authority fields come from Rust, not the shim. The helper revalidates operation/RP/origin before a QR is produced. Progress messages have the same v/id, a closed `kind` (`qr`, `status`, `result`, `error`) and typed body. Only one result/error terminates. Unknown version/type, wrong ID, extra authority fields or output after terminal is a protocol failure. Redundant terminal output is ignored safely and recorded as a coarse fault, never delivered twice.

Selected v1 bounds: request and terminal response <=1 MiB each; stdout total <=4 MiB per ceremony; progress line <=32 KiB; JSON nesting <=32; native ID <=128 ASCII bytes; status enum only; <=10 accepted progress updates/s with coalescing. Validate operation field limits in one schema; bounds are resource policy, not universal WebAuthn maxima. Never allocate based on claimed length before checking limits. For QR matrices require a supported valid dimension <=177, exact square count, allowed modules only and a quiet zone added by UI. A raw payload, if used, stays secret and is encoded by a local library.

Do not log envelopes, stderr, raw options or results. Keep sanitized error classes. Supervise asynchronously: bounded 180 s total deadline, cancel/terminate on lease cancellation, then a proposed 1 s grace before kill and asynchronous reap. No GUI-thread waitForFinished. Kill only the owned child/process group. Native request data never appears in argv/environment.

Use library protocol/crypto; implement no home-grown signing, tunnel or RP suffix rules. Pin the PSL packaging/source and fail closed when validation data is missing. Registration side effects on the phone cannot be rolled back by killing the helper; never auto-retry an uncertain create.

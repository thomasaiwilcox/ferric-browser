# M0-52: Ephemeral marker scan and recovery boundary

## Scope

Transient profiles never participate in startup recovery. This slice adds a
bounded read-only marker scan for release verification and diagnostics.

## Evidence

- `ferric_browser_storage::transient_marker_scan` checks browser-owned config, data,
  state, cache, runtime, and managed temporary roots by entry name only.
- The scan reports clean, marker matches, or incomplete status without
  exporting paths, file contents, URLs, or credentials.
- Diagnostics explicitly disclose user-owned downloads and clipboard copies,
  engine/OS swap, core dumps, clipboard managers, sites, and network observers
  as outside the browser guarantee.
- A lifecycle test covers a marker during an unclean restart and the clear
  state after clean marker finalization.

## Verification

`cargo xtask check` and the storage lifecycle test are the automated gates.
The installed-candidate normal-close/crash scan remains a release acceptance
test because it requires process termination and the host’s real storage
locations.

# M0-171: profile deletion preview worker

The interactive profile manager no longer opens the durable profile registry
from the deletion-preview callback. `ProfilePreviewWorker` performs the
registry lookup on the named `ferric-browser-profile-preview-reader` thread and
returns the exact bounded metadata preview through a capacity-one queue. Qt
publishes pending/text properties and QML waits for the result before exposing
the confirmation action.

The command-driven `profile-delete` preview remains a separate typed command
completion migration; this slice covers the interactive manager boundary and
does not change deletion confirmation or the mutation worker.

Evidence:

- `cargo test -p ferric-browser-engine-qt profile_preview_worker_reads_delete_metadata_off_thread --locked --offline`
- `cargo xtask check --locked --offline`
- `cargo fmt --all -- --check`

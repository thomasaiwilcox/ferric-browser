# M0-132 hint document identity

Hint collection assigns each candidate a bounded, session-local element ID and
retains the exact DOM `Element` in Qt WebEngine's isolated application world.
Fresh selection resolves that retained identity, requires it to remain
connected to the captured frame, and returns current visibility, metadata, and
geometry to Rust. The tab generation and browser document ID still invalidate
the whole session on navigation.

This supersedes the original whole-document mutation counter. That counter
rejected every hint on pages with unrelated live DOM updates, including search
homepages, even when the selected element was unchanged. Detached, replaced,
retargeted, hidden, or materially moved elements still fail closed. The
existing 5,000-candidate cap and 200 ms collection/selection deadlines remain.

Verification:

```text
cargo fmt --all -- --check
cargo test -p ferric-browser-engine-qt --locked --offline
cargo xtask check --locked
```

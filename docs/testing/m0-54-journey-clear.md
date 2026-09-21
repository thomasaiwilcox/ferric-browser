# M0-54 — Journey clearing follows history clearing

`ProfileStore::clear_history` now removes matching journey nodes in the same
SQLite transaction as profile history pages. Foreign-key cascades remove
incident edges and current-node pointers. Time and origin filters apply to the
journey committed timestamp and safe URL; bookmarks do not exempt nodes.

Evidence:

```text
cargo test -p browser-storage --locked
```

The storage regression test verifies that clearing one origin removes its
journey node while retaining another origin's node.

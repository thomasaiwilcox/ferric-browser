# Frozen qualification recipes

These are byte-for-byte copies of the one-off recipes used for the qualified
`1257101` product inputs. They let reviewers inspect the application/everyday
complete-entry report assembly, targeted idle reassessment and final receipt
construction. `manifest.json` records the original local paths and checksums.

They are historical evidence-collection recipes, not general-purpose entry
points or required build dependencies. Their paths and expected inventories are
bound to this qualification host. Run from the repository root after recreating
the original builds and raw receipts; original command receipts still refer to
the copies retained under `target/kde-free-evidence`. The idle recipe's historical
KWindowSystem prefix belongs only to the frozen baseline comparison.

Current tooling lives in `tools/qualification` and `tools/performance`. These
copies do not change that tooling or the qualified product/source manifest.
Full raw logs, generated build inputs and binaries remain on the qualification
host; the committed receipt and performance data retain their hashes.

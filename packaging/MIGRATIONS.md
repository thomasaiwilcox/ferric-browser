# Clean-break pre-alpha data policy

This Ferric Browser pre-alpha release does not import legacy browser data or
data from earlier Ferric builds. Existing legacy roots are never read, modified,
or deleted.

The v3 configuration/root marker policy continues to refuse pre-v3 configuration,
runtime overrides, session snapshots, and profile databases. Normal startup does
not import or rewrite that older data.

The start-page feature upgrades a recognised schema v3 profile database to v4.
It creates a private-permission SQLite backup beside the database as
`<database-name>.sqlite.pre-start-page-v3`, then adds the view preference, eight
bookmark-backed favourite slots, and dismissed-site tables in one transaction.
Existing bookmarks, history, permissions, and other metadata are preserved.
Failed transactions leave the database at v3 for a later retry. The backup is
kept, and startup rejects newer schemas and unrecognised migration checksums.
The root marker, configuration format, and session snapshot format do not change.
A database already at v4 does not repeat the upgrade or replace its backup.

If an earlier Ferric root is detected, normal startup refuses to interpret it.
To start fresh, explicitly erase only Ferric-owned contents:

```sh
ferric-browser reset-data --confirm
```

The command retains resolved root directories and their parents, rejects unsafe
root symlinks and special filesystem entries, and writes the current schema
marker only after the reset completes.

# Clean-break pre-alpha data policy

This Ferric Browser pre-alpha release does not import legacy browser data or
data from earlier Ferric builds. Existing legacy roots are never read, modified,
or deleted.

Schema v3 also refuses pre-v3 configuration, runtime overrides, session
snapshots, and profile databases. Normal startup never attempts a migration,
backup copy, or in-place rewrite of that data.

If an earlier Ferric root is detected, normal startup refuses to interpret it.
To start fresh, explicitly erase only Ferric-owned contents:

```sh
ferric-browser reset-data --confirm
```

The command retains resolved root directories and their parents, rejects unsafe
root symlinks and special filesystem entries, and writes the current schema
marker only after the reset completes.

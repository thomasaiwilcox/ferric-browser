# Migration notes

Legacy pre-alpha XDG data is migrated only by the explicit command:

```sh
ferric-browser migrate --from-rustbrowser
```

The command copies into staging roots, verifies every copied file byte for
byte, performs read-only SQLite integrity/schema checks, and commits only into
empty Ferric roots. It never moves, edits, or deletes the source. First launch
does not migrate silently.

Ferric Browser migrations are versioned at the Rust storage boundary. Before an
older SQLite schema is migrated, the application creates one bounded,
versioned pre-migration backup beside the database and validates the known
schema checksum. Session generations are replaced atomically.

QtWebEngine profile data is not treated as downgrade-compatible. Do not open
an upgraded profile with an older engine build as a repair procedure; restore
the application-owned backup or use a separately copied profile.

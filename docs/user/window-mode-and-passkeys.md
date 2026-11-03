# Window mode and phone passkeys

Both features are optional. Internal tabs remain the default, and phone
passkeys are disabled until explicitly enabled.

## Window mode

Set the following in your configuration and restart Ferric:

```toml
schema_version = 3

[tabs]
presentation = "windows"
```

Each page has its own native window. Existing tab commands operate on pages
in the same logical group: opening a foreground or background tab creates a
page window, and selecting another tab activates its window. The tab strip is
hidden. Separate windows, private profiles and browsing contexts retain their
own identities.

`tab-detach` and `tab-give` move a live page within the same profile when the
engine can preserve its view. `reopen-in-window` opens a safe URL descriptor
and does not preserve the live document. See the [command reference](commands.md).

On a compatible Hyprland session, grouping is separately opt-in:

```toml
[hyprland]
page_grouping = "auto"
```

Logical page groups do not depend on compositor grouping. If native grouping
is unavailable, pages remain independent windows. Normal-profile session
snapshots preserve logical membership and page order; private sessions are
not written to durable session storage. Returning to internal tabs also
requires a browser restart.

## Phone passkeys

Install Ferric with its packaged phone-auth helper, then enable:

```toml
[passkeys]
phone = "on"
```

The helper needs Bluetooth through BlueZ, a working adapter, system TLS roots
and Public Suffix List data. Start a fresh browser process with an explicitly
admitted direct network route:

```sh
ferric-browser --direct-network open https://example.org
```

An eligible top-level HTTPS credential request offers the Phone provider.
Choose it and follow the QR-code prompt on your phone. Cancel the dialog to
abandon that ceremony; a later request can start a new one. Unsupported
requests continue through Qt's native authentication path.

Proxy, offline and unsupported network policies prevent phone admission.
Site blocking rules remain effective; the provider also declines routes whose
policy it cannot prove. Ferric does not remember phones or store their
passkeys. Credentials belong to the provider on your phone and may sync with
that provider.

Disposable tests verified registration, allow-listed sign-in, discoverable
sign-in and cancellation recovery with an operator-reported iPhone 17 Pro Max,
iOS 27.2 and Apple Passwords. This covers one account on a local HTTPS test
site. Android, hardware security keys, multiple-account selection and public
sites have no physical qualification from these tests. See the
[qualification record](../implementation/WINDOW_PASSKEYS_PLAN.md) and
[configuration reference](configuration.md).

# M0-173: profile-open worker lookup

The command-driven `profile-open NAME [URL]` path now validates the navigation
input on the UI thread, then queues the profile-registry read on the bounded
profile-list worker. The profile window action is created only after the
worker response is polled, so opening a durable profile does not synchronously
open or parse the registry from the Qt thread.

The worker result is also covered by the profile-list worker test, including
exact name matching and the missing-profile case. Registry errors remain
visible as a command status instead of creating a partial window action.

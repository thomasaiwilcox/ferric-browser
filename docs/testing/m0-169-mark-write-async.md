# M0-169: asynchronous bookmark and quickmark writes

Bookmark and quickmark commands submit one bounded `MarkWrite` to the
profile-storage worker and return a queued result without waiting on SQLite in
the Qt callback. The poll loop reports the committed or failed result and
invalidates the cached library snapshot so subsequent completion and switcher
queries reload durable data.

The storage-worker contract remains single-flight: a second mutation while one
is pending receives a bounded busy response. Worker startup failure is reported
as an unavailable durable profile operation rather than silently writing from
the GUI thread.

# M0-168: profile registry mutation worker

Confirmed durable profile creation, label renaming, and deletion now run
through a bounded `rustbrowser-profile-mutator` worker. The worker reopens the
registry for each request; deletion additionally revalidates the active/held
profile lock, removes the exact profile data root, and removes the registry
record before reporting completion. Qt invokables only queue operations;
completion and failure are surfaced by the normal Qt polling loop.

The regression test creates a temporary profile and marker, queues deletion
through the worker, then verifies both the data root and registry record are
gone. The same worker API is used for create and rename so the registry's
durable writes do not run in the Qt callback.

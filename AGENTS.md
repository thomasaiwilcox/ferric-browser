# Repository dependency policy

Keep Ferric's required build, installation and exercised runtime independent of
KDE components. Introducing a KDE package, fetched source, static/shared library
or required plugin requires explicit user approval. Standard portal interfaces
remain interoperable with whichever backend the desktop selects.

Use the qualified Qt 6.11.2 SDK and matching private headers. The owned Wayland
bridge's Qt Gui native-window interface must be included in future SDK
qualification. Keep generated protocol files in build directories and preserve
the pinned upstream XML notices/provenance.

Current qualification uses `tools/qualification/kde_free.py` (schema v2).
Historical implementation-pack schemas and accepted KF6 evidence retain their
original assertions; do not rewrite or bypass them to qualify current code.

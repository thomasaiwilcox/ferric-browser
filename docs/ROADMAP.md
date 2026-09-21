# Public pre-alpha roadmap

Ferric remains pre-alpha until these gates are complete:

- route every command and adapter callback through `BrowserRuntime`;
- finish registry-generated CLI/IPC decoding; public CLI and IPC error codes
  are now carried by typed error values rather than inferred from prose;
- finish typed Qt presentation models and remove non-allowlisted JSON bridges;
- split the Qt engine and QML composition root into bounded subsystems;
- move all browser-owned JavaScript to versioned, adversarially tested assets;
- qualify native Wayland interaction, clean shutdown, and migration recovery;
- complete accessibility review and cross-origin hint behavior;
- record release performance baselines and enforce the regression budget;
- pass dependency, license, packaging, install, and uninstall validation;
- reserve the `ferricbrowser` namespace and complete a final name review.

Current known limitations are intentionally not hidden: cross-origin frame
hints are incomplete, accessibility qualification is incomplete, Linux native
Wayland is the only target, and QtWebEngine security updates depend on the
distribution package cadence.

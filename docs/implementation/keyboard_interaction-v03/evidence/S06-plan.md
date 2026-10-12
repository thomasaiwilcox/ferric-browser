# S06 extraction plan

`BrowserScripts.js` remains the public QML facade. The shared scroll target runtime, scroll script builders and Hint script builders move as one cohesive unit to `qml/scripts/PageInteractionScripts.js`; keeping the scroll primitives with Hint preserves one target identity and the existing action-specific checks. The facade forwards the current API and checks the asset version before delegating.

The new module is permitted by S06's allowance for a small module beside the existing anchor. It is an ordinary QML JavaScript resource, so the existing build manifest hashes it and the generated QRC packages it. No runtime installation, cache, or page-world ownership change is introduced.

Native offscreen tests load both production resources into ApplicationWorld and verify the facade's missing-builder, mismatched-version and exception paths. Existing WebEngine interaction fixtures continue to exercise the facade with the packaged QML import. A Rust source-contract test checks the generated manifest and QRC entries.

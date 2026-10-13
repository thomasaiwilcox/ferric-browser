# S24 bounded integration contract

S24 enables no production accessibility provider and makes no change to browser-owned QML semantics, Grid, hints, search, page scripts, or accessibility defaults. Existing visible controls and the manual visual Grid fallback remain the supported interaction paths.

Any future provider integration must satisfy all of these limits before it is enabled:

- Use only public platform accessibility interfaces and metadata explicitly exposed by the target. Treat names, descriptions, roles, values, and relationships from page content as untrusted presentation data; do not elevate them into browser authority.
- Bind a candidate to its owning window, tab, WebEngine view, document generation, and accessibility object identity. Revalidate that owner and generation immediately before action. On destruction, navigation, or view replacement, drop the retained target; the stale-object probe is a regression gate.
- Require a real component rectangle from the public accessibility interface before using a semantic object in any spatial selection UI. Do not infer bounds from pixels, DOM scripts, matching text, or a canvas drawing.
- Keep semantic target collection read-only and bounded. Do not instrument closed shadow roots, inspect page data to synthesize missing accessible nodes, or use private Qt/QPA/Chromium APIs. A closed-root control is supported only if the platform provider itself exposes it with usable metadata and geometry.
- Make every action an explicit user choice with a one-action/one-effect check. Never invoke a destructive action implicitly, and never let a keyboard-navigation feature replay an AT action after the target goes stale.
- Keep page-derived accessible text ephemeral. Do not add it to history, logs, site profiles, or persistent browser state. Test fixtures remain loopback-only and use synthetic content.
- Qualify actual screen-reader coexistence on each supported platform, including focus movement and announcements, before making a user-facing support claim. An AT-SPI client smoke test alone is insufficient.

The current result fails the geometry and WebEngine-tree requirements and lacks a real screen-reader run. No keyboard semantic target capability may advance from this evidence.

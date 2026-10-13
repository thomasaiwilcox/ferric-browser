# S27 live task matrix

No live-site task was run. The available checkout has no installed-release disposable profile or authorised test documents/accounts, and the nested Wayland compositor lacks the virtual-keyboard protocol required to send physical input. Fixture and offscreen results below stay in their own evidence classes.

| Reference task | Fixture/offscreen evidence | Native Wayland | Live site | Qualification result |
| --- | --- | --- | --- | --- |
| Docs/Sheets/Slides: navigation, editing, autocomplete, and submission | Existing workspace and QML suites only | Not run: no virtual-keyboard protocol | Not run: no authorised disposable profile/account | Unqualified |
| Mail/chat: compose, shortcuts, and confirmation boundaries | Existing mode, Insert, and key-routing fixtures only | Not run | Not run: no authorised disposable profile/account | Unqualified |
| Advanced editor: focus versus rich writeback | S22 capability investigation recorded no-go; no generic rich writeback enabled | Not run | Not run: no authorised editor instance | Research no-go; no live compatibility claim |
| GitHub/Reddit: roving widgets and virtualised targets | Existing target-selection and WebEngine fixtures | Not run | Not run: no authorised disposable profile/task | Unqualified |
| YouTube: media controls and page shortcuts | Existing local interaction tests only | Not run | Not run: no authorised disposable profile/task | Unqualified |
| Kanban, maps, diagrams, canvas, and embedded viewers | Scoped Grid, frame and drag probes are fixture/offscreen evidence; S23 provider is no-go, S20 is blocked, S26 found no documented wheel-only task | Startup only; no physical input | Not run: no authorised site task | Unqualified; no universal widget claim |
| Physical key and pointer journeys in installed Ferric | 59 Qt offscreen tests and bounded adapter smoke passed | Startup passed; input not run | Not run | Blocked by environment |

No URL, account identifier, typed content, or private profile data was captured. A future run needs an installed build, an input-capable Wayland seat, and user-authorised disposable profiles/tasks; until then, these rows are not passes.

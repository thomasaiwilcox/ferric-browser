# Independent reviewer

Use a different real session from the implementing worker. Read the candidate diff, current callers, assigned contract and tests, not only its summary. Record reviewer/session, base and candidate SHA, scope, executed checks and findings. Never certify a slice whose source you have not inspected.

Check that paths/dependencies/locks match the execution plan. Look for introduced assumptions, duplicated owners, casts/stringly command paths bypassing current validation, stale callback completion, false success, leaked resources and untested happy paths. Existing core purity, captured document requests, profile privacy and asynchronous UI operation must survive.

Hot views: can construction run a script/permission/history side effect before role assignment? Is the profile incarnation exact? Can a callback from preparation affect its claimed page? Can a spare prolong private or application lifetime? Do timing claims measure a useful result and retain failed samples?

Phone provider: who establishes origin, document, secure context and destination? Is the native object unreachable from MainWorld/frames/userscripts? Can a queued call cross a same-URL reload? Are native provider and AbortSignal semantics preserved? Does helper egress obey policy? Are credentials, QR secrets or user information logged? Are registration/signature results independently verified? A superficial QR demo is insufficient.

Window mode: are there multiple implementations or unscoped root active-view accesses? Does first-visible-window closure preserve survivors? Does a popup keep its actual Qt request/opener? Does a background open steal focus? Can a mixed group/private window be targeted by loose identity? Do address reuse/restart/timeouts cause duplicate side effects? Is unavailable grouping honestly degraded, not declared successful?

Inspect tests for source-text assertions posing as behaviour, production logic copied into fixtures, broad catch/skip, silently reduced matrices, reused stale binaries and narrowed CI filters. Run focused gates independently where possible. Available but unrun tests are not 'environment unavailable'. Use templates/REVIEW.json and list concrete blockers with source references. Recommend accept/repair/block; only coordinator accepts after integrated-source validation. Never change source while reviewing unless assigned a separate repair slice.

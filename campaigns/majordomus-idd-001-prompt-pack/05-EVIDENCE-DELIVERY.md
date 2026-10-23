# MISSION 05 — VERIFIABLE DELIVERY CLOSURE

Wire the already-established forge, PR/CI, release/deploy and evidence capabilities into the canonical work graph. Link every relevant outcome to intent criteria and execution artifacts. Model distinct statuses for work produced, tests passed, PR merged, package released, deploy requested, deploy observed and behavioral verification passed. No false equivalence between command exit 0 and deployed service health.

Use existing PR queue/integration train; establish critical-path readiness and gate order. Auto-merge only when authorized and all required review, CI, security, and protected-branch gates succeed. Avoid admin bypass. Respect existing semver/versioning doctrine; bump only in approved release workflow and keep changelog/docs generated from authentic changes. Verify GitHub Pages and other deployments with actual reachable endpoint, expected version/artifact hash and health route where available.

Provide action views in `/ui`: request PR, inspect CI, integrate ready PR, release, deploy, verify, close. Clicking an action shows exact capability, side effect, approval and observed output. When unavailable due to external credentials show BLOCKED/UNAVAILABLE rather than a green icon.

Tests: evidence provenance, stale CI, changed commit after green checks, force-merge denial, partial deploy, health regression, rollback/cancel, missing docs drift, failure to reach endpoint, duplicated release request. One dogfooding demo goes intent→issues→execution→tests→PR→merge→version→deploy→observed verify, excluding steps unavailable without authorization and explicitly reporting their block.

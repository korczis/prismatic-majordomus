# PHASE 04 — VERSION, RELEASE, DEPLOY, VERIFY AND INTENT SATISFACTION

Audit `docs/RELEASE.md`, `docs/DELIVERY.md`, CI workflows, existing distribution and deployments, version doctrine/Elm-like change classification, manifests, changelog and release capabilities. Integrate with present machinery; no independent bespoke deploy pipeline. Identify *actual* deployment targets and credentials capability via safe detection; never expose secrets or create production credentials. Dry-run first.

Release policy: compare accepted public API/schema/capability changes against prior release; infer semver/Elm-like bump from canonical change analysis; preserve reproducibility and generate release notes/changelog from verified merged work and traceability. Version update is atomic and validated across workspace/package manifests and generated docs. Ensure parallel PRs cannot create conflicting releases; reconcile existing tags/releases/artifacts with idempotent release identity. A tag is not a deploy.

Deployment: use existing target adapters, release artifacts and CI authority; typed launch/approve/status/rollback (if supported). Enforce target-specific allowlist and approvals, especially production. Record artifact digest, source SHA, tag/version, environment, strategy, startup/health/smoke probes, deployment receipt, timestamps, signer/principal and outcome. Deploy completion is only valid after observed smoke checks, not HTTP 200 from an upload endpoint.

Evidence: each criterion points to typed, attributable, durable evidence with provenance including source commit, CI run, execution ID, release version, deployment environment, build artifact digest and temporal freshness. Define freshness invalidation on code edits, intent criterion changes, failed redeploys, withdrawn releases. Criterion satisfied only by applicable evidence. Derive intent verdict rather than manually closing it. Expose explanation graph and contradictory/unknown evidence. Distinguish `merged`, `released`, `deployed`, `healthy` and `satisfied` and ensure UI labels do too.

Safe failure behavior: unsuccessful deployment leaves intent unsatisfied; rollback uses authorized existing mechanism and records new evidence; missing smoke endpoint is BLOCKED not green; interrupted release resumes/reconciles, never retags different commit under same version. Avoid sending public docs false readiness claims.

Tests: version classifier against API break/add/patch; simultaneous release claim; existing release rerun; artifact mismatch; deploy denied without policy; smoke fail; rollback; invalid/stale evidence; intent satisfied only after success; rerun after coordinator crash. Produce a signed or otherwise attributable sample release/verify receipt per existing infra.

Acceptance: verified merged work becomes a versioned artifact and authorized verified deployment, and intent stage derives to satisfied only once its original criteria are actually met.

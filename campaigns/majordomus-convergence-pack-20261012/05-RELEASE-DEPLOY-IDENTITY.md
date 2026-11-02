# PROMPT 05 — One canonical verified release truth

Goal: every public claim about version/build/commit/deployment must be exact, explainable and freshness-aware.

1. Inspect existing Cargo version, tags, release metadata, generated site metadata, footer/header, API/OpenAPI/UI snapshot, deployments observed and Pages branch. Map which sources are authoritative and distinguish component version from product/release version.
2. Recheck previously observed 0.21.0 vs 0.22.0 public labels. Verify if still present; if yes determine expected semantics before changing.
3. Reuse existing ReleaseIdentity/Deployment observation representations; conceptually track semantic component, version, head SHA, build ID/digest, deployment ID/URL, observation SHA and timestamps. Do not create a competing top-level manifest.
4. Validate build artifact SHA, site snapshot list, route budget, generated page counts and live publication. Incorporate existing `scripts/pages verify-ui`, `scripts/site-deploy`, and canonical pages-live gate only after confirming they exist and current contracts.
5. Implement failure-proof verification: a stale public version is STALE not PASS, an unobserved deploy UNKNOWN, a built but unpublished artifact NOT DEPLOYED. Include bounded retries for eventual consistency.
6. Regression test Markdown alignment normalization, 1894-route scale only if still actual, CSS policy preservation, idempotence and failures for unknown inline styles. Do not drop CSP/content policy to pass.
7. Update docs and derived surfaces from typed canonical truth. Run public HTTP and UI snapshot checks after authorized deployment.

## Acceptance
One verifiable release identity chain from master SHA to deployed website (or exact externally blocked stage), no unexplained contradictory labels, tests preventing false deployment success.

## Non-negotiable Majordomus constraints
Read AGENTS.md, CLAUDE.md, applicable .ai context chain and existing campaign/skill/doctrine definitions before writing code. Existing behavior, tests and actual checked-out HEAD trump this prompt. Reuse repository tools and names discovered, never invent CLI switches. Source-of-truth first; typed, derived, inferable, deterministic, explainable, provenance-carrying where appropriate. No duplicate registries, mock-green tests, weakened checks, force-pushing protected branches, secret leakage, broad destructive cleanup, or unsupported deploy claims. Preserve legitimate branch protections and approvals. Use --admin only where authorized, not as a bypass. No human interaction for routine decisions; if an irreversible action lacks approval, mark BLOCKED and continue independent work. Work in narrow vertical slices: implementation + tests + governance + docs + integration + deployment verification. At the end record exact SHA, commands, status and URLs. Never report PENDING/UNKNOWN as PASS.

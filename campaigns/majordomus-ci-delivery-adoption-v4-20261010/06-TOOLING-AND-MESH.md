# M06: Tooling and mesh

## Goal
Update executable entry points and connected workers to use new CI path by default.

## Issues

## Issue V4-06-01: Canonical developer commands

**Implementation sequence**
1. Inspect current `just`, `majordomus`, `bin/majordomus`, direnv, shell tools and command registry.
2. Expose existing or new command variants for `ci plan`, `ci check --affected`, `ci status`, `ci explain`, `ci benchmark` only where they fit current canonical command conventions.
3. Reuse identical planner/verdict and evidence types locally and in Actions; no hard-coded copies.
4. Regenerate shell completions, OpenAPI/MCP command exposure and docs from existing generator.

**Acceptance / evidence**
- [ ] Local parity with CI for same SHA/diff
- [ ] New commands automatically discoverable in surfaces

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-06-02: Adopt fast path in PR drain

**Implementation sequence**
1. Inspect current `prs drain`, CI polling, PR queue scoring and branch ownership.
2. Use fast verdict as eligible evidence only when complete for target merge SHA and selected-impact plan; never treat early green while required checks pending as success.
3. Tune worker polling/backoff and incremental rebase/replan after each merge; avoid stampedes and duplicate derive.

**Acceptance / evidence**
- [ ] PR drain integration tests for stale SHA and red verdict
- [ ] Queue proceeds around blocked independent PRs

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-06-03: Mesh worker continuation

**Implementation sequence**
1. Read deployed mesh capabilities and durable state; distinguish claimed CLI/API capability from actually running supervisor.
2. Integrate CI evidence and work IDs into existing mesh handover/checkpoints and launch rules.
3. Give peers least necessary scoped permissions for clone/PR/test; release/deploy authority delegated separately under existing profile.
4. Simulate lost worker, expired claim, network partition and restart with no duplicate merge/deploy.

**Acceptance / evidence**
- [ ] No manual `proceed` needed when supervisor is actually running
- [ ] Recovery proof and permissions audit

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-06-04: Projection parity

**Implementation sequence**
1. Expose CI critical path, selected gates, cached evidence, blockers and PR verdict age through existing capability/projection graph.
2. Add high-level mobile-friendly UI view where current UI supports it; do not invent second orchestration UI.
3. Verify CLI / HTTP / MCP / UI identifiers and statuses match golden contract.

**Acceptance / evidence**
- [ ] Same canonical object IDs across surfaces
- [ ] No frontend-only status computations

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Milestone exit gate
- [ ] Installed consumer repository can invoke and use new fast flow
- [ ] Mesh uses current code rather than documentation promises

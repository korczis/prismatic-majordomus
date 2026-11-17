# M08: Enforcement, docs and generated Pages

**Objective:** Deliver this capability as a proven vertical slice of the current Majordomus architecture. The issues below are implementation contracts, not assumptions about missing code.

## Binding architectural rules (apply to EVERY issue)
1. Inspect current `AGENTS.md`, `.ai/**`, ADRs, schemas, tests, `docs/COCKPIT.md`, `apps/majordomus-cli/src/cockpit/**`, `apps/majordomus-cli/src/web/**`, `share/design/tokens.yaml`, `site/**`, and current GitHub issues/PRs before editing. Names are hints, discover actual paths and runtime behavior.
2. Cockpit remains a server-rendered enhancement of canonical Rust `capability!` + typed registries, index, graph and executor. No parallel SPA, no parallel command registry, no hand-maintained navigation/source-of-truth, no JS business decisions, no extra server or independently tracked status.
3. Reuse `share/design/tokens.yaml` and generated theme/CSS/status vocabulary. Respect existing Flowbite/Tailwind semantics and actual component contracts. Never modify generated `apps/majordomus-cli/src/design/tokens.yaml` directly. Prefer reusable view primitives and clean, minimal front-end enhancement.
4. Use existing API/OpenAPI/MCP execution and event channels. The same action must have the same server-side authorization, validation, execution ID, and evidence regardless of the UI entry point. A browser is not an authority.
5. Every displayed count, percentage, trend, version, branch, user, peer, CI latency, PR ID, roadmap date, activity event and health indicator comes from a canonical typed projection and identifies its provenance/freshness. NO mock values, fake avatars, placeholder chart series, invented release names, illustrative activity events or synthetic network topology in production.
6. Loading, missing, unsupported, permission-denied, offline, error, and empty states are separate, legible, accessible states. If a metric is not measured, show `Not measured`, not zero or a fabricated trend. Never display `Autonomy enabled` unless the run is actually authorized and active.
7. Keep HTML useful and navigable with JS disabled; graph/chart enhancement progressive. Keyboard, 320/375/768/1024/1440 widths, dark/light themes, contrast, reduced motion and focus handling must work. Avoid indiscriminate chart library loading.
8. Tests and enforcement MUST be executable in canonical CI paths; changes to `.ai/` rules and docs in the same increment. No decorative Markdown rules; migration/removal of superseded rules required. Inspect and avoid widening expensive unconditional gates.
9. Work in incremental vertical slices, reconcile canonical intent/milestones/issues/PRs, run targeted validation, derive artifacts in repository order, commit, merge if authorized and green, analyze version bump, release/deploy if eligible and authorized, then probe served Cockpit and public Pages. Never claim completion with missing evidence.
10. Adopt the provided screenshot for visual *composition*, not data or product claims. All mock screenshot labels, names, versions and metrics are non-authoritative fiction.


## UI-M08-A — Migrate `.ai` rules/doctrines for derived UI, zero mocks, provenance, measured KPIs, a11y, cross-surface parity and speed budgets.

**Task:** Migrate `.ai` rules/doctrines for derived UI, zero mocks, provenance, measured KPIs, a11y, cross-surface parity and speed budgets.

**Procedure**
1. Find the current owning model/capability, canonical source, callers, relevant PRs, tests and governance proofs; record file paths and live observations.
2. Propose minimal change that reuses existing typed domain representation and shared views; write adversarial failure cases before implementation.
3. Implement data/model and presentation together; eliminate superseded duplicates. Preserve no-JS path and return honest unavailable states.
4. Add focused unit/contract/behavior tests, accessibility and visual snapshots where relevant; update `.ai` policy and docs based on actual changed guarantees.
5. Measure relevant latency/bundle/derive impact and compare with baseline. Run canonical impacted gates plus any newly required coverage.
6. Create or update canonical issue record, attach evidence, integrate green branch, regenerate derived artifacts and verify deployed consumer if eligible.

**Acceptance:** Real data only; deterministic state; same facts across transport surfaces; no false success; keyboard and responsive operation where UI present; tests actually run and CI enforces invariant; traceable changed files and execution evidence.

**Release checkpoint:** Check scope compatibility, version policy, current PR conflict plan, documentation projections, generated artifact freshness and authorized release/deploy status. If unreleasable, document precise blocker, continue unrelated work.

## UI-M08-B — Wire minimal, fast targeted gates into canonical `.ai/repo/ci/gates.yaml` plus nightly/full depth tests

**Task:** Wire minimal, fast targeted gates into canonical `.ai/repo/ci/gates.yaml` plus nightly/full depth tests; prove required checks cannot silently be skipped.

**Procedure**
1. Find the current owning model/capability, canonical source, callers, relevant PRs, tests and governance proofs; record file paths and live observations.
2. Propose minimal change that reuses existing typed domain representation and shared views; write adversarial failure cases before implementation.
3. Implement data/model and presentation together; eliminate superseded duplicates. Preserve no-JS path and return honest unavailable states.
4. Add focused unit/contract/behavior tests, accessibility and visual snapshots where relevant; update `.ai` policy and docs based on actual changed guarantees.
5. Measure relevant latency/bundle/derive impact and compare with baseline. Run canonical impacted gates plus any newly required coverage.
6. Create or update canonical issue record, attach evidence, integrate green branch, regenerate derived artifacts and verify deployed consumer if eligible.

**Acceptance:** Real data only; deterministic state; same facts across transport surfaces; no false success; keyboard and responsive operation where UI present; tests actually run and CI enforces invariant; traceable changed files and execution evidence.

**Release checkpoint:** Check scope compatibility, version policy, current PR conflict plan, documentation projections, generated artifact freshness and authorized release/deploy status. If unreleasable, document precise blocker, continue unrelated work.

## UI-M08-C — Update docs/COCKPIT.md, installation/UI/quickstart/reference, ADR when warranted, capability examples and help/completion generation.

**Task:** Update docs/COCKPIT.md, installation/UI/quickstart/reference, ADR when warranted, capability examples and help/completion generation.

**Procedure**
1. Find the current owning model/capability, canonical source, callers, relevant PRs, tests and governance proofs; record file paths and live observations.
2. Propose minimal change that reuses existing typed domain representation and shared views; write adversarial failure cases before implementation.
3. Implement data/model and presentation together; eliminate superseded duplicates. Preserve no-JS path and return honest unavailable states.
4. Add focused unit/contract/behavior tests, accessibility and visual snapshots where relevant; update `.ai` policy and docs based on actual changed guarantees.
5. Measure relevant latency/bundle/derive impact and compare with baseline. Run canonical impacted gates plus any newly required coverage.
6. Create or update canonical issue record, attach evidence, integrate green branch, regenerate derived artifacts and verify deployed consumer if eligible.

**Acceptance:** Real data only; deterministic state; same facts across transport surfaces; no false success; keyboard and responsive operation where UI present; tests actually run and CI enforces invariant; traceable changed files and execution evidence.

**Release checkpoint:** Check scope compatibility, version policy, current PR conflict plan, documentation projections, generated artifact freshness and authorized release/deploy status. If unreleasable, document precise blocker, continue unrelated work.

## UI-M08-D — Update GitHub Pages as a public *explanation* and interactive derived demonstration linked to live Cockpit when available

**Task:** Update GitHub Pages as a public *explanation* and interactive derived demonstration linked to live Cockpit when available; no claim of runtime streaming on static hosting.

**Procedure**
1. Find the current owning model/capability, canonical source, callers, relevant PRs, tests and governance proofs; record file paths and live observations.
2. Propose minimal change that reuses existing typed domain representation and shared views; write adversarial failure cases before implementation.
3. Implement data/model and presentation together; eliminate superseded duplicates. Preserve no-JS path and return honest unavailable states.
4. Add focused unit/contract/behavior tests, accessibility and visual snapshots where relevant; update `.ai` policy and docs based on actual changed guarantees.
5. Measure relevant latency/bundle/derive impact and compare with baseline. Run canonical impacted gates plus any newly required coverage.
6. Create or update canonical issue record, attach evidence, integrate green branch, regenerate derived artifacts and verify deployed consumer if eligible.

**Acceptance:** Real data only; deterministic state; same facts across transport surfaces; no false success; keyboard and responsive operation where UI present; tests actually run and CI enforces invariant; traceable changed files and execution evidence.

**Release checkpoint:** Check scope compatibility, version policy, current PR conflict plan, documentation projections, generated artifact freshness and authorized release/deploy status. If unreleasable, document precise blocker, continue unrelated work.

## UI-M08-E — Add content provenance and deploy freshness smoke checks, derive/check, page audits, link audits, snapshots and no-stale-claim enforcement.

**Task:** Add content provenance and deploy freshness smoke checks, derive/check, page audits, link audits, snapshots and no-stale-claim enforcement.

**Procedure**
1. Find the current owning model/capability, canonical source, callers, relevant PRs, tests and governance proofs; record file paths and live observations.
2. Propose minimal change that reuses existing typed domain representation and shared views; write adversarial failure cases before implementation.
3. Implement data/model and presentation together; eliminate superseded duplicates. Preserve no-JS path and return honest unavailable states.
4. Add focused unit/contract/behavior tests, accessibility and visual snapshots where relevant; update `.ai` policy and docs based on actual changed guarantees.
5. Measure relevant latency/bundle/derive impact and compare with baseline. Run canonical impacted gates plus any newly required coverage.
6. Create or update canonical issue record, attach evidence, integrate green branch, regenerate derived artifacts and verify deployed consumer if eligible.

**Acceptance:** Real data only; deterministic state; same facts across transport surfaces; no false success; keyboard and responsive operation where UI present; tests actually run and CI enforces invariant; traceable changed files and execution evidence.

**Release checkpoint:** Check scope compatibility, version policy, current PR conflict plan, documentation projections, generated artifact freshness and authorized release/deploy status. If unreleasable, document precise blocker, continue unrelated work.

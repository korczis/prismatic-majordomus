# Mission: ship the next progressive Majordomus release today

You are inside `prismatic-majordomus`. Take the current intent-focused release candidate to a real, externally verifiable release TODAY using Majordomus' own lifecycle, integration, validation, release and publication machinery.

Do not stop at analysis, local tests, merge, or tag. The mission ends only after publication and external verification.

## Expected target

Expected semantic target: **0.15.0**. This is not permission to hardcode a version. The repository's measured release engine decides the minimum legal version.

Known historical candidate: GitHub PR #794, `Intent batch: verdicts by evidence, preflight, doctor rule ids (release 0.15.0)`, historical head `f7b9d6d4c833fd1d5dc8d5d912b4cfc605662b42`. Historical CI was green, but master moved afterward. Re-read current reality.

## Autonomous operating mode

Do not ask for permission for routine repository inspection, canonical worktree creation, integration refresh, non-ambiguous conflict resolution, regeneration, testing, fixing regressions caused by this batch, correcting overclaims, recording evidence, pushing release branches/commits where repository workflow expects it, or invoking established integration/release automation.

Stop only at a genuine tooling/security boundary that requires explicit human confirmation.

## Phase 0 — load Majordomus itself

Before substantive mutation:

1. Read `AGENTS.md`.
2. Read `.ai/README.md`.
3. Resolve context for paths likely to change.
4. Load effective rules and dependencies.
5. Read task lifecycle.
6. Inspect ADRs for intent, evidence, completion, integration, release, version measurement, CI and publication.
7. Inspect current intent/issue/milestone records serving this release.
8. Announce/claim work using existing cooperation mechanisms if required.
9. Ensure canonical worktree topology before committing.

Do not infer obsolete architecture from this prompt if the repository has evolved.

## Phase 1 — freeze release scope

Establish the exact release contract from code and tests.

Intended 0.15.0 story:

- evidence-backed intent verdict;
- verdict is not generic task/session completion;
- criterion satisfaction requires evidence current for the judged revision;
- intent-specific CI/enforcement proves the rule;
- `intent preflight` understands what an issue serves;
- maintenance/non-intent work is explicit and typed, not a silent bypass;
- GitHub sync hardening carried by the batch remains correct;
- intent docs/site claims match reality;
- generated rule/bootstrap integrity checks remain correct.

Classify every discovered adjacent change as MUST SHIP, BLOCKER FIX, DEFER, or OUT OF SCOPE. Record that scope in the canonical task/session mechanism.

## Phase 2 — inspect current git/GitHub state

Determine current master HEAD, current #794 head, merge-base, ahead/behind/conflict state, CI state, overlapping PRs/branches, whether candidate work landed elsewhere, whether newer master changes supersede candidate pieces, and whether generated artifacts must be rederived.

Run current collision/overlap checks before composition.

## Phase 3 — refresh/compose candidate

Bring candidate onto current master using the repository's current integration model. Read integration ADRs before choosing merge/rebase/composition strategy.

During conflicts:

- identify canonical source vs derived projection;
- preserve later master fixes;
- preserve intended intent semantics;
- regenerate generated output rather than hand-merging it;
- remove duplicate/superseded implementations;
- never keep both sides merely to avoid a decision.

If conflict reveals a semantic bug, add failing-before/passing-after regression coverage before continuing.

After composition:

- derive once under repository lock if required;
- derive-check;
- inspect diff for accidental reversion of newer master work;
- run focused tests.

## Phase 4 — prove the intent contract

### Verdict ownership

Prove intent verdict is derived from canonical intent criteria/evidence and generic completion does not independently declare intent success. Unknown must stay explicit rather than silently becoming satisfied.

### Evidence freshness

Prove evidence valid for revision R does not automatically satisfy a materially changed judged revision R+1 when the canonical freshness rule requires current evidence. Diagnostics must explain staleness. Re-recording valid evidence must restore satisfaction deterministically.

### Preflight

Prove representative branches:

- issue serves one or more criteria -> allowed and mapping explained;
- legitimate maintenance/non-intent work -> allowed only through explicit typed semantics;
- work that should be intent-bound but is unbound -> refused;
- broken intent/criterion references -> validation failure;
- coverage gaps behave according to current intent policy.

### Cross-surface projection

From the same canonical intent state, verify all applicable surfaces agree semantically: CLI, JSON, REST, OpenAPI/Swagger, MCP, Cockpit, generated docs/site data. Renderings may differ; verdict/stage/criteria/evidence semantics may not.

### Mutation/drift proof

Deliberately create fixtures for stale evidence, missing evidence, broken criterion reference, and projection drift where practical. Prove gates go red.

Do not weaken policy to make the suite green.

## Phase 5 — audit claims

Search authored prose for intent claims, including README, homepage, features, intent docs, limitations, roadmap and release-facing prose. Classify each substantive statement as shipped, advisory, planned, rejected, or historical.

Do not claim session auto-loading, executed opposition, GitHub intent reconciliation, full Cockpit intent UX, automatic plan derivation or closed-loop autonomy unless the current tree proves them.

Run claim-proof/site/reference mechanisms.

## Phase 6 — full release readiness

Use the repository's current canonical aggregate where possible, plus focused checks needed for diagnosis. Cover applicable:

- doctor/context/rules/plan/intent validation;
- fmt/build/test/clippy/doctests;
- portable shell checks and relevant cases;
- all intent-specific cases;
- API/OpenAPI/MCP/Cockpit tests;
- generated-artifact drift;
- site/docs/claims;
- use-case impact/coverage for public capability changes;
- differential coverage;
- release analyze/version-surface gates.

Classify every failure as RELEASE REGRESSION, PRE-EXISTING ON CURRENT MASTER, ENVIRONMENTAL WITH PROOF, REPRODUCIBLE FLAKE, or POLICY BLOCKER. "Unrelated" without master proof is not a classification.

Release blocker count must reach zero.

## Phase 7 — integrate to master

Immediately before integration, re-read master. If it moved, refresh candidate and rerun impacted/binding checks.

Use the project's PR-integration control plane. No admin bypass, no forbidden force-push, no stale-head merge, no red binding CI.

Confirm intended commits reach master and observe binding master CI. Repair merge-caused failures immediately through canonical workflow.

## Phase 8 — measure and bump

On actual release tree run current equivalents of:

```bash
majordomus release analyze
majordomus release analyze --explain
majordomus release analyze --format json
scripts/ci/version-matches-surface
```

Expected likely result: minor -> 0.15.0 from 0.14.x.

If PATCH, investigate why intended public change was not measured. If MAJOR/breaking, inspect exact contract movement and do not hide it.

Run canonical writer:

```bash
majordomus release bump
```

Regenerate/derive and rerun release gates. Do not manually force version numbers.

## Phase 9 — publish

Use current release pipeline. Expected shape from repository history/docs: integrate version bump according to policy, create/push release tag through established tooling, release workflow builds/publishes artifacts, release record is produced from real artifacts, changelog projections are derived rather than hand-written.

Monitor workflow to terminal success. If it fails, inspect logs, fix root cause, and rerun correctly.

## Phase 10 — verify externally

From a clean temp environment where practical verify:

- tag/release exists;
- expected artifacts and checksums exist;
- installer resolves released version;
- fresh install succeeds;
- installed binary reports released version;
- basic smoke command works;
- public docs/site are current;
- release/changelog projections include the release;
- intent validate/preflight/verdict smoke behaves as documented.

Do not use a locally built binary as proof of public installation success.

## Phase 11 — close the release session

Finish using Majordomus lifecycle. Leave no unexplained blocker, stale active issue for completed work, abandoned integration claim/worktree, generated drift, or unpushed release commit.

Record only the genuine next-wave gaps that remain, likely among: intent-bound session start, executed opposition, satisfaction/completion integration, stronger GitHub reconciliation, first-class Cockpit intent UX, desired outcome -> plan derivation, reassessment loop.

Do not implement them before this release ships.

## Hard anti-goals

Do not before release:

- implement the entire next intent architecture;
- broadly redesign Cockpit;
- rewrite session state;
- invent a parallel intent schema;
- add another source of truth;
- hand-author changelog data;
- bypass capability generation;
- bypass version measurement;
- weaken CI/policy;
- treat stale evidence as current;
- merge with red binding CI;
- market full intent-driven development without proof.

## Final report

Return exactly:

### Release outcome
Version, tag, publication state.

### What shipped
Concrete user-visible/architectural changes.

### Intent contract proved
Verdict, freshness, preflight, projection evidence.

### Integration
How candidate was refreshed and conflicts resolved.

### Validation evidence
Exact local commands and CI runs.

### Version decision
Measured change and rationale.

### Published artifacts
Names/targets/checksums or canonical references.

### External verification
Fresh install/version/site/API/MCP smoke evidence.

### Deferred
Only true next-release work.

### Repository state
Master SHA, cleanliness, blockers, remaining release-related PRs.

Do not claim completion without binding evidence.

# Majordomus 0.15.0 — Same-Day Release Prompt Pack

Purpose: ship a visibly progressive Majordomus release today without scope-creeping into the next architectural wave.

Primary consumer: Claude Code Opus 5.5 or another strong repository-capable coding agent working directly inside `korczis/prismatic-majordomus`.

## Release story

Expected semantic target: **0.15.0**, subject to the repository's own measured release engine.

Known integration candidate when this pack was created: PR **#794**, `Intent batch: verdicts by evidence, preflight, doctor rule ids (release 0.15.0)`. Historical head: `f7b9d6d4c833fd1d5dc8d5d912b4cfc605662b42`. That head had a successful `validate` workflow, but `master` moved afterwards, including later merged work such as #797 and #798. Therefore historical green CI is not release proof. Refresh/composition against current `master` and fresh binding validation are mandatory.

The intended release narrative is narrow and evidence-backed:

> Majordomus 0.15.0 moves intent from a recorded planning object toward executable governance: intent verdicts are evidence-backed, stale evidence does not satisfy changed criteria, preflight understands what work serves, and the same intent state is projected consistently across the control plane.

Do NOT claim full autonomous intent-driven development, automatic desired-outcome-to-plan derivation, or a fully closed session/opposition/completion loop unless current repository evidence proves those features by execution time.

## Repository wins

Before acting, read and obey the current repository, especially `AGENTS.md`, `.ai/README.md`, resolved rules/doctrines, task lifecycle, release docs, current ADRs, capability architecture, plan/intent records, and generated-artifact rules.

If this pack conflicts with the repository, the repository wins.

## Versioning rule

Majordomus measures the required bump. Do not hardcode `0.15.0`.

Use the current equivalents of:

```bash
majordomus release analyze
majordomus release analyze --explain
majordomus release analyze --format json
scripts/ci/version-matches-surface
majordomus release bump
```

The expected result is minor -> 0.15.0, but the canonical comparator is the judge.

## How to run

### One-shot autonomous mode

Give Claude Code:

`00_MASTER_RELEASE_ORCHESTRATOR.md`

and instruct it to execute through publication and external verification without stopping for routine implementation choices.

### Milestone mode

Run in order:

1. `10_RELEASE_PREFLIGHT_AND_SCOPE_FREEZE.md`
2. `20_REFRESH_AND_COMPOSE_INTENT_BATCH.md`
3. `30_INTENT_CONTRACT_PROOF.md`
4. `40_CROSS_SURFACE_AND_DOC_CLAIM_AUDIT.md`
5. `50_FULL_VALIDATION_AND_RELEASE_READINESS.md`
6. `60_INTEGRATE_TO_MASTER.md`
7. `70_MEASURE_BUMP_AND_PUBLISH.md`
8. `80_POST_RELEASE_EXTERNAL_VERIFICATION.md`
9. `90_RELEASE_CLOSEOUT_AND_NEXT_WAVE.md`

## Hard scope freeze

Do not implement these merely because they are attractive:

- full outcome -> plan derivation,
- a new planning engine,
- a new session runtime,
- a new intent storage format,
- broad Cockpit redesign,
- new mesh features,
- rewritten completion architecture,
- new provider abstraction,
- unrelated DX polish,
- speculative 0.16.0 refactors.

Only expand scope when required to make the intended release truthful, integrated, green, publishable, or externally verifiable.

## Definition of done

The release is complete only when:

- candidate contains current master;
- conflicts are resolved semantically;
- intent verdict semantics are proven;
- stale evidence cannot satisfy a changed judged revision;
- preflight correctly handles serves / explicit maintenance-exemption / refusal;
- cross-surface projections agree semantically;
- authored prose does not overclaim;
- generated artifacts are current;
- local binding checks are green;
- CI on the actual integration commit is green;
- release engine measures the bump;
- canonical writer performs the bump;
- release is integrated to master;
- publication workflow succeeds;
- released artifacts and version are externally verified;
- public docs/site reflect current truth;
- release/session records are closed cleanly;
- remaining intent-loop work is deferred explicitly rather than smuggled into today's release.

At every milestone report: inspected state, changes, deliberate non-changes, exact commands/tests, failures, blocker count, and next step.

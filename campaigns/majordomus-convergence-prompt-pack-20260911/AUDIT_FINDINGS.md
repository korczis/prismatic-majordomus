# Audit Findings — Starting Hypotheses

These findings came from the supplied 2026-09-11 snapshot. Re-run them against the actual checkout before modifying code.

## A. Split-brain development semantics

Observed baseline:

```text
backing:adr
backing:checkpoint
backing:decision
backing:evidence
backing:finish
backing:handover
backing:init
backing:migrate
backing:plan
backing:question
backing:rules
backing:session
backing:start
backing:update
backing:usecase
surface:apps/majordomus-cli/src/cockpit/pages.rs:1787
```

Interpretation: repository doctrine already wants development semantics to be canonical/capability-backed, but accepted migration debt allows multiple public mutating commands to remain outside that canonical runtime.

Target: `backing:* = 0`, surface violation `= 0`, and delete the baseline if no remaining ratchet is needed.

## B. Capability mutation surface narrower than shell lifecycle

Observed obvious mutating Rust capability IDs included:

```text
executions.start
executions.cancel
peers.announce
```

This must be re-inventoried from canonical capability metadata. Do not hardcode the expected count. The goal is to migrate lifecycle/domain mutation ownership into canonical capabilities and make shell/CLI thin adapters.

## C. Coverage floor

Observed:

```text
scripts/rust-coverage-threshold = 90
```

Target: retain a sensible historical repository floor while enforcing 100% coverage for new/changed production code and 100% behavioral coverage of all new/changed rules/capabilities. Add branch coverage where supported.

## D. Provider lifecycle asymmetry

Observed in `lib/capture.sh`:

```text
MJ_CAPTURE_ADAPTERS='claude-code ...'
MJ_CAPTURE_LIFECYCLE='claude-code ...'
```

Provider metadata/configuration elsewhere supported additional providers such as Codex/Gemini/generic. Target: explicit provider capability matrix plus equivalent lifecycle semantics wherever the provider offers the necessary hooks/events, and explicit unsupported/degraded status otherwise.

## E. Canonical ordering debt grew

Observed baseline:

```text
crate_sorts=106
shell_sorts=0
```

Audit scan approximately observed:

```text
crate_sorts=122
shell_sorts=13
```

Target: do not update the baseline upward. Classify algorithmic/internal ordering versus presentation ordering. Migrate presentation/default ordering to the canonical ordering subsystem. Pin deterministic collation where shell sorting is legitimately required. Drive relevant debt to zero or to a rigorously justified non-presentation allowlist owned by the canonical checker.

## F. Lease reader gate self-detection

`scripts/ci/lease-reader-check` appeared able to match its own references to lease/schema identifiers and flag itself. Reproduce before editing. Target: a robust structural check that detects independent parsers/readers without self false positives and has regression tests.

## G. Cockpit navigation/model drift

`cockpit/nav.rs` contained `Executions` and `Quality` concepts while runtime area inventory required consistency verification. Target: a single canonical area/navigation inventory or derivation where practical, with tests ensuring enum/docs/routes/nav cannot diverge.

## H. Rules versus enforcement

Vendored doctrines had machine-readable `x-majordomus` enforcement wiring. Project rules appeared materially less uniform. Target: every blocking project rule must expose machine-readable enforcement status, validator/test linkage, and current evidence. Human-only blocking rules should be migrated to executable enforcement or reclassified honestly.

## I. Claims are stronger than most repos, but evidence can be made current

The repository already distinguishes claim states such as guaranteed/advisory/planned and connects many claims to implementation/tests/docs. Target: guaranteed claims should point to executable obligations and current verification evidence, not merely paths that could theoretically contain tests.

## J. Roadmap breadth exceeds unfinished foundation

There are planned/active areas such as richer execution telemetry, runtime adapters, routing, auth/deployment and distributed cooperation. Do not expand those horizontally until core convergence gates pass. Late stages of this pack can then resume them from a stable substrate.

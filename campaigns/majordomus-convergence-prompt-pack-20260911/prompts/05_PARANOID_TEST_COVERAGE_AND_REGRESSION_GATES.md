# Stage 05 — Paranoid Test Coverage and Regression Gates

Implement the project's requested quality doctrine without turning coverage into a vanity number.

## Goals

1. 100% coverage for all newly added or modified production code in a change.
2. 100% behavioral obligation coverage for every changed public command/capability/rule.
3. Root-cause regression test for every bug fix.
4. Existing repository-wide coverage floor never decreases.
5. Coverage/test evidence is linked to relevant rules/claims/docs.

## Do not do this

Do not merely replace `90` with `100` globally if historical untouched code is not currently at 100%. That would either block convergence indefinitely or invite exclusions/gaming.

Implement a changed-code policy plus a monotonic whole-repository policy.

## Rust

Use existing `cargo llvm-cov` infrastructure if present. Derive changed production lines/branches from git diff against the correct merge base. Account carefully for:

- generated code,
- macro expansions,
- impossible defensive branches,
- platform-specific code,
- test-only code,
- deleted lines.

Any exclusion must be explicit, minimal and enforceable. Prefer redesigning untestable code over excluding it.

Require changed-line coverage = 100%. Require changed-branch coverage = 100% when llvm-cov can map it meaningfully.

## Shell / scripts

The repository has meaningful shell behavior. Establish equivalent behavior-level coverage obligations. If line coverage tooling is practical and portable, integrate it. If not, define a stronger command/function/test obligation registry so every changed public shell path is exercised by behavior tests. Shell should shrink as stage 03 migrates domain semantics to Rust.

## Rule/capability coverage

Generate a coverage matrix from canonical metadata rather than hand-maintaining lists. Every public capability/command/rule must have test obligations. For mutating capabilities include E2E state-change tests.

## Bug doctrine

Every fixed bug must record:

```text
symptom
root cause
why existing gates allowed it
regression test
preventive invariant/gate if systemic
```

Make this part of issue/claim/evidence metadata if repository conventions support it.

## CI

Use the same canonical commands locally and in CI. Avoid a CI-only script nobody runs. Add failure tests for the coverage gate itself if practical.

## Acceptance

- New/changed production code in this convergence series reports 100% changed-line coverage.
- Changed branch coverage is 100% where measurable and exceptions are explicit/justified.
- Aggregate repository floor is not lowered.
- Public capability/rule coverage matrix is machine-generated and green.
- Intentionally adding an uncovered changed branch makes the gate fail.
- Docs explain the policy and local command.

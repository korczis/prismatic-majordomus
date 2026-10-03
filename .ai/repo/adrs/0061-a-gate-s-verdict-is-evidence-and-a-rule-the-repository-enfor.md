---
schema: adr/v1
id: adr-0061
kind: adr
title: A gate's verdict is evidence, and a rule the repository enforces itself is proven by it
status: proposed
date: 2026-09-14
tags:
  - governance
  - evidence
provenance:
  origin: extracted
  derived_from:
    - decision:adr-0048
    - decision:adr-0041
---

# 61. A gate's verdict is evidence, and a rule the repository enforces itself is proven by it

## Context

On 2026-09-14 a supervised repository, `catharsis-as-a-service`, added a blocking rule of its own:
`project.no-scroll-containers` — nothing on the published site carries a scrollbar of its own. It is
enforced, and by machinery of exactly the kind this tool asks for. `scripts/validate-layout.py` reads the
stylesheets and the templates and exits 1 on a scrolling declaration, a scrolling utility class or a
minimum width inside a figure or table selector. It is a step of `scripts/validate.sh`, which is that
repository's gate and runs in its CI. Five unit tests prove the validator fails on each defect it exists
to catch. A browser test walks every element of seven pages at three widths in two languages.

`majordomus doctor`, in that tree, said:

> blocking without a validator — 7 of 48 blocking rule(s) carry no `x-majordomus` block, so no validator
> runs for them and the doctrine verdict below says nothing about them

The verdict is correct and the vocabulary is the defect. ADR 0048 gave a rule two declarable shapes and a
third state for the rest: *dispatched* names a `validator` that `lib/doctrine.sh` calls, *gated* names
`tests`, and everything else is *reviewed*. The join behind them is already good: `rules/` does not
re-implement evidence, it binds the tests a rule names to the executions recorded against them and reads
ADR 0041's `ProofState`; gate coverage is derived from `.ai/repo/ci/gates.yaml` "so that a rule's gate
coverage is derived from the planner's own input rather than restated here"; and a case reaches a gate
"without either naming the other".

It reaches them through constants. `CASES_DIR = "test/cases/"`, `SUITE_RUNNER = "test/run.sh"`,
`CRATE_TESTS_DIR`, `CRATE_RUNNER` — the derivation knows this repository's layout, because until now the
only rules that mattered were this repository's own. A supervised repository's tests are at
`tests/python/test_validators.py`, its enforcement is `python3 scripts/validate-layout.py`, its gate is
`scripts/validate.sh`, and none of those can be named: `validator` means a function of this tool,
`enforced_by` means a command of this tool, `tests` resolves only under paths this tool ships. The rule
that stops a scrollbar reaching production is machine-enforced, and the only thing Majordomus can say
about it is that somebody reads it.

ADR 0048 saw the shape of this and deferred one half of it deliberately: *"Record a verdict for gates, so
`gated` could reach `proven`. Tempting, and rejected for now … teaching it to record an arbitrary gate's
exit code is a larger change than this decision needs."* Since then the larger change has arrived in
pieces — ADR 0041's execution record, the CI model the planner reads, and `gates/`, which already judges
what each gate said and whether that verdict still describes the tree. Nothing joins them to a rule.

## Decision

**A rule may name the enforcement its own repository wrote.** The enforcement block gains `command`: the
executable that refuses the rule, as the repository runs it (`python3 scripts/validate-layout.py`), with
the exit code that means refusal. It is neither of the existing modes renamed — a `validator` is
dispatched by this tool, a `command` is dispatched by the repository's gate — and the derivation order of
ADR 0048 is unchanged: a validator still wins, then tests, then a command, then a reason.

**What makes a test a test is data, not a constant.** The directories and runners that let a case reach a
gate move out of the Rust and into the repository's layer, where `CASES_DIR` and `SUITE_RUNNER` become
this repository's own values of a declaration every repository writes. Nothing changes here; everywhere
else becomes expressible.

**The gate's verdict is an execution, and the ledger is the one that exists.** A gate run records what
ADR 0041 requires of any run — outcome, duration, commit, tree state, source digest, origin, and the
command that reproduces it — under an identity derived from the gate id, as a test's is derived from its
path. `gates/` already reads verdicts; this is what it means to keep one.

**A rule is proven by a verdict at the commit that carries the rule.** ADR 0041's states apply unchanged:
a verdict from an earlier commit is stale, a verdict against a dirty tree proves no commit, a gate that
never ran is `not_run`. `proven` keeps one meaning across claims, tests and rules, which is the reason for
reusing the model instead of adding a fourth one.

**A declaration that does not resolve is a defect of the rule.** A `command` whose executable is not in
the tree, or a test path outside every declared test root, fails the loader the way a category without a
validator already does. Absence of any declaration is still `reviewed`, still counted apart, still never
counted as passing.

## Alternatives rejected

**Write a Majordomus validator for each supervised repository's rule.** It does not scale and it is the
wrong boundary: one repository's rule is about stylesheets, the next one's is about SQL migrations. The
repository had already written the validator; what was missing was the join.

**Let the author write `reviewed_because`.** That is what the tree does today, and it files five unit
tests, a CI step and a browser test under "a person's opinion". A mode that absorbs everything reports
nothing.

**Let the gate declare which rules it proves.** Enforcement would then live in two files that must agree,
and the rule — the document a reader opens — would stop saying how it is held up. ADR 0048's principle
holds: the block on the rule is the declaration.

**Record every gate's verdict.** Only a gate some rule's proof depends on needs one, and only at the
commit. Recording all of them turns the ledger into the build log ADR 0041 refused.

## Consequences

The rule model stops being about this repository. A supervised repository that declares its test roots and
names its own enforcement gets the same proof graph, the same states and the same ratchet as this one —
which is the difference between a supervisory layer and a self-supervising program.

CI needs a way in: a gate must be able to record an execution with `origin: ci` from inside a workflow.
That is a capability and a credential question this decision opens and does not answer.

`rule-proof-check` ratchets: rules leaving `review_only` change the baseline, and a repository that names
a gate it never runs is told so instead of being counted apart in silence.

Two constants become schema, which is a migration for this repository and an enablement for every other.

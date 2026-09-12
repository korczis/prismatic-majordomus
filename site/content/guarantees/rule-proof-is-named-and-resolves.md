+++
title = "Every blocking rule names what proves it, and every path it names is in the tree"
description = "A rule of this repository declares a class. blocking means a gate refuses work that"
weight = 172
[extra]
claim_id = "rule-proof-is-named-and-resolves"
status = "guaranteed"
source = "docs/claims/rule-proof-is-named-and-resolves.md"
+++
{% raw %}

## What it means

A rule of this repository declares a class. `blocking` means a gate refuses work that
violates it — that is a claim about the world, and it is either true or it is not. Before
this guarantee, nothing decided which. A rule could say, in a `# Verification` section
written for a human reader, that `test/cases/28_no_hardcoded_values.sh` proved it, and
nothing noticed when that case was renamed. The rule went on reading as enforced on every
surface that asked, because every surface asked the same question: does the rule carry an
`x-majordomus` block? A green badge for the presence of a YAML key.

The guarantee is the relation, held in both directions. What the rule names is in the front
matter, where a program reads it, and what it names is in the tree.

## How it works

`scripts/ci/rule-proof-check` reads every rule object under the rules section — the
denominator comes from the tree, so a rule added tomorrow is measured tomorrow — and reports
two findings, which fail differently on purpose:

**`dangling`** — the block names a path that is not in the tree. Never ratcheted, at any
class. A proof that does not exist is worse than no proof, because the rule reads as proven,
and recording it in the baseline does not silence it: the baseline is about rules with no
proof, not about rules whose proof is a lie.

**`unproven`** — a blocking rule names neither a validator, nor a test, nor a reason why
nothing executable can express it. This is the half nothing else in the repository can
express. `scripts/ci/reference-check` already refuses a path named in any authored document
that is not in the tree, and a rule's front matter is prose to it, so it catches the
dangling half today — measured, not assumed. What no other check knows is that a blocking
rule is *supposed* to name anything at all.

## How to see it

```
scripts/ci/rule-proof-check            report, and fail on new debt
scripts/ci/rule-proof-check --strict   fail on any finding, baseline ignored
```

The `rule-proof` gate in `.ai/repo/ci/gates.yaml` runs the first on every change that
touches a rule. The baseline is empty: every blocking rule in this repository names
executable proof, or declares why a reader is the proof.

## What it does not cover

That the proof is any *good*. The gate decides that a blocking rule names something and that
what it names is in the tree. Whether the case it names actually exercises the behaviour the
rule describes is not decidable here, and a case that asserted `true` would satisfy it.
That judgement is review's, and it is the reason naming a proof is a floor rather than a
ceiling.

Nor does it run anything. A named case that has never been executed still satisfies this
claim; whether a run happened, passed, and is younger than what it is about is a different
question, answered by the proof graph and by [`EVIDENCE.md`](../EVIDENCE.md).

## Why it exists

Because a rule that reads as enforced and is not is worse than a rule everyone knows is
unenforced. The first is trusted. On 2026-09-11 the repository held ten blocking rules whose
only verification was the word "Review.", and six more whose proof existed but was named in
prose no program reads — so a case could be renamed and the rule would go on displaying a
green badge on every surface that asked.

## What proves it

`test/cases/125_rule_proof.sh`, by mutation. The tree is green; one fact changes — a case is
deleted while the rule still names it, a blocking rule is added naming nothing, a
declaration is written with no reason — the gate goes red naming that fact; the change is
undone; the tree is green again. A mutation nothing survives is a guarantee nothing gives.
{% endraw %}

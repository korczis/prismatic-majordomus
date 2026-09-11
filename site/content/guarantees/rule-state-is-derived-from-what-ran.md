+++
title = "A rule's proof state is derived from the tree and the ledger together, and a mechanism that refuses violations is never reported as a run that passed"
description = "There are two questions about a rule and they are not the same question."
weight = 168
[extra]
claim_id = "rule-state-is-derived-from-what-ran"
status = "guaranteed"
source = "docs/claims/rule-state-is-derived-from-what-ran.md"
+++
{% raw %}

## What it means

There are two questions about a rule and they are not the same question.

*Does the repository satisfy this rule right now?* is about the tree, and
`majordomus doctrine` answers it by running the validator.

*Is this rule in a state where it could be satisfied at all?* is about the rule — whether
what it names is in the tree, whether a runner drives it, whether anything ever ran it,
whether what ran is older than what it is about. Until the proof graph, nothing asked it,
and a rule whose case had been deleted answered the first question with silence and the
second not at all.

## How it works

`crate::rules` is the one typed reading of the rule corpus, and it joins the declaration to
the tree and to the ledger of recorded executions that [`EVIDENCE.md`](../EVIDENCE.md)
describes. It does not re-implement evidence: a rule's test-level proof *is* that
vocabulary, and a second one would be a second answer to the same question.

Ten states, ranked strongest to weakest, and the three that must never collapse into one
another are:

- **`proven`** — a passing run, and nothing in the repository has changed since it.
- **`gated`** — an executable check refuses violations and a CI gate runs it. That is the
  mechanism. This repository records no verdict for a gate, so what can be shown is that
  violations are refused, not that the last run passed.
- **`reviewed`** — the rule declares, with its reason, that nothing executable can express
  it.

A rule's state is the **weakest of its parts**. One dangling case makes the rule dangling
however many of its other cases pass, because the half that does not resolve is the half a
reader would have trusted.

A **finding** is a rule whose declared class the state does not support. `dangling` is a
finding at every class; for a blocking rule, so are `unproven`, `failing` and `unrunnable`.
`gated`, `reviewed` and `not run` are not findings — they are weaker states, said out loud
and counted, which is a different thing from a defect.

## How to see it

```
majordomus-cli rules report              every rule, with the tallies counted from the tree
majordomus-cli rules show <id>           one rule, its proof, and what would fix it
majordomus-cli rules proves <test>       what a test proves, and what would lose its only proof
```

The same answer is the HTTP operation `GET /api/v1/rules`, the MCP tool `majordomus_rules`
and the resource `majordomus://rules`, derived from one declaration rather than implemented
four times.

## What it does not cover

A verdict for a gate. This repository's ledger records behavioural cases and the crate's
integration tests; a check wired as its own CI gate has no execution recorded against it, so
its rule reports `gated` — violations are refused, and what the last run decided is not
stated here. That is a limit the state names rather than hides, and it is why `gated` is not
`proven`.

It also says nothing about whether the repository currently *satisfies* a rule. That is the
dispatcher's question and `majordomus doctrine` answers it. Collapsing the two would produce
one exit code for "the rule is in order" and "the repository is in order", which are
different sentences.

## Why it exists

Because before it, the question could not be asked at all. `x-majordomus` was read by exactly
one test — does the block exist — and every surface printed a badge from the answer. A rule
naming a case deleted three months earlier was indistinguishable from a rule whose case runs
on every push.

## What proves it

`test/cases/133_rule_graph.sh`, which drives the graph through the executable in a fixture
repository of its own — because the states that matter are states about commits, about files
that stop existing, and about runs that happened and then went out of date. It records a
run and asserts `proven`, edits the case and asserts `stale`, deletes it and asserts
`dangling`, and asserts that a check wired as its own gate is `gated` while the same shape
with no gate behind it is `unrunnable`.
{% endraw %}

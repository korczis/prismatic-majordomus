+++
title = "Every claim, rule, feature, command, capability, MCP tool and use case the repository declares is an evidence subject whose members and tests are derived from declarations that already exist, and a committed subject index that differs from that derivation is refused"
description = "A verdict is always a verdict about something. A reader asks \"is the feature evidence"
weight = 187
[extra]
claim_id = "evidence-subjects-are-derived"
status = "guaranteed"
source = "docs/claims/evidence-subjects-are-derived.md"
+++
{% raw %}

## What it means

A verdict is always a verdict about something. A reader asks "is the feature `evidence`
proven?", "is `majordomus commit plan` tested?", "what proves the rule
`project.scope-is-declared`?" — and each of those is a question about a *subject*: one
declaration the repository already makes, named by a key of the form `<kind>:<id>`, such as
`feature:evidence`, `command:commit plan` or `rule:project.scope-is-declared`.

The subject index answers the half of that question that needs no run: which tests the
subject reaches, directly through its own routes and indirectly through the subjects it is
made of. Nothing lists a subject, a member or a route by hand. Adding a claim, a rule, a
feature, a use case, a command, a documented example, a capability or a case header adds
what it declares to the index; removing it removes it.

The committed copy, `site/data/registry/evidence-subjects.json`, is what the site and the
shell tools join against. `majordomus generate --check` refuses a copy that differs from
the derivation, so a hand edit — a subject someone wished were tested, a route someone
added to make a page look covered — is a failure rather than a fact.

## How it works

`majordomus generate site` derives the index in the executable, from declarations whose
owners already read them:

- a claim comes from the evidence join over `docs/CLAIMS.yaml`, with an empty ledger, so its
  test identity follows the one grammar the ledger uses; its one route is the test it names,
  with the claim's source, implementation and test source as the inputs a run is judged by;
- a rule comes from the rules report's definitions; every path its enforcement block names
  that a runner drives is a route, and every other path is listed as a mechanism;
- a feature comes from the product model, and is made of the claims, rules, commands and
  use cases it names;
- a use case comes from the index, and is made of the commands it runs, the rules it
  exercises, the claims it evidences and the MCP tools it calls — the edges the graph already
  draws from the same fields; its own route is its scenario;
- a command is a public command of the shell tool, a documented command-line example path of
  the executable or the command-line path of a capability: the words of the command graph,
  whose runnable commands of the executable each carry a documented example, so a group that
  only holds commands, such as `evidence`, is not a word of the executable. Its behaviour
  and negative routes are the cases whose **first** `# majordomus-covers:` and first
  `# majordomus-negative:` lines name it; a later line of the same kind names nothing, and
  `none` names nothing. A documented example path also reaches the binary that runs every
  example, but only where that binary exists;
- a capability is a builtin capability of the registry. It is made of its command-line path
  and of every claim implemented in the file its module is composed in, which is the rule the
  capability pages already apply;
- an MCP tool is an alias of the capability that declares it, with no page of its own.

The members form a directed graph with no cycles: a feature is made of claims, rules,
commands and use cases; a use case of commands, rules, claims and MCP tools; an MCP tool of
its capability; a capability of its command and its claims. A use case never includes the
features that name it, and nothing includes a feature.

A command word that both the shell tool and the executable answer to is one subject that
names both programs, with an advisory finding that says so: its header routes prove the
shell tool, its example route proves the executable, and one page shows both.

A planned or rejected claim names no test, so it reaches none. A use case's scenario is
listed as a route with no test, because the ledger cannot record a scenario run. A feature
whose members reach no test at all carries the advisory finding
`feature_without_evidence`, an error for a stable feature and a warning otherwise.

The file carries structure only: no ledger row, no commit and no time. Recording a run
never makes it stale, and both generate passes of the derivation write the same bytes.

## How to see it

```bash
jq '.pages[:3]' site/data/registry/evidence-subjects.json
jq '.subjects["feature:evidence"]' site/data/registry/evidence-subjects.json
majordomus generate site --check
bash test/run.sh 507_evidence_subjects_are_derived
```

The case derives the index of a fixture repository of its own, removes a feature, a test
binary and a header and derives it again, and hand-edits the committed copy to show that
`generate site --check` refuses it. Over this repository it holds the index equal to the
projections that already existed: the claims matrix, the product model, the use-case
catalogue, the registry, the command pages, the capability pages, the doctrine pages and the
command graph.

## What it does not cover

Whether a reached test passed. The index says which tests a subject reaches, never what
they said; that is the verdict, which the `evidence subject` command will answer from the
ledger at a presented revision.

The findings are advisory. Nothing refuses a feature without evidence or a command word two
programs answer to until the gate slice holds them.

## Why it exists

No badge or page may carry a hand-written list of what proves it. A list someone keeps is a
list that drifts: the test is renamed and the page still names it, a feature gains a use
case and no page learns of it. Deriving the subjects from the declarations the repository
already makes means the only way to change what a page says proves a subject is to change
what the repository declares, and `generate --check` refuses every other way.
{% endraw %}

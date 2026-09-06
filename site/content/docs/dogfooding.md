+++
title = "Dogfooding"
description = "the one rule: Majordomus cannot recommend a development discipline it does not use itself — and what following it costs"
weight = 19
[extra]
source = "docs/DOGFOODING.md"
+++

{% raw %}

**Majordomus cannot recommend a development discipline that it does not use itself.**

That is the whole rule. Everything below is what following it costs.

## What it means here

This repository is supervised by the tool it ships. `majordomus doctor` runs on every commit
through `.githooks/pre-commit`; `majordomus finish --check` runs on every push. The
provider instruction files at the root are generated from `.ai/repo/policy.yaml` and
regenerated when it changes. Now the plan is under the same rule: non-trivial work here
belongs to a milestone and to one issue, with a position in the dependency graph, acceptance
criteria, a validation command and evidence.

Concretely, non-trivial work does not begin with a decision about what to do. It begins with:

```bash
majordomus plan next
```

## What counts as trivial

The exception exists and is deliberately small:

- a typo, a broken link, a formatting fix
- a change forced by something outside the repository — an action version, a moved URL
- an emergency fix to something that is broken right now

Everything else — a new behaviour, a new rule, a change to what a command does, a document
that makes a claim, a refactor — needs an issue. If you are unsure, it needs an issue: the
cost of writing one is a few minutes, and the cost of a change nobody can trace is the whole
reason this layer exists.

Even the exceptions are auditable: they are commits, with messages, in a repository whose
`plan` state can be read at any commit.

## Why this is not ceremony

Three things break without it, and all three broke here before it existed.

**Work gets chosen from memory.** A session with the whole context in front of it picks what
looks next. A session that inherits a repository has no such view, so it picks what the last
handover mentioned, which is not the same as what is unblocked.

**"Done" gets asserted.** A model reporting on its own work has no incentive to say the
validation did not run. The evidence gate turns that into a command that either exists or
does not.

**Plans multiply.** The moment a roadmap exists in a README paragraph and a GitHub milestone
and a conversation, one of them is wrong and nobody knows which. Deriving them all from one
file removes the question.

## What dogfooding costs

It is worth being honest about the friction, because a discipline nobody follows is worse
than none.

Writing an issue before starting work is slower than starting work. The evidence gate refuses
completion until a test exists, which means the test comes first whether or not that suited
the session. The bootstrap of this very model had to be executed partly outside itself,
because the tooling did not exist yet — `M000` records that, and its issues were imported and
closed through the system as soon as `plan validate` ran.

And the model can be wrong. A dependency edge nobody needed serialises work that could have
run in parallel; the tool will not notice, because it cannot tell a necessary edge from a
cautious one.

## The rule for a worker

```bash
majordomus context                 # what the last session left
majordomus plan status             # where the outcome stands
majordomus plan next               # the one issue that is ready
majordomus plan show <id>          # the whole contract
majordomus plan start <id>         # refused unless it is READY
majordomus start "<id>: <title>" --scope <the issue's declared scope>
# ... execute, only inside that scope ...
majordomus plan evidence <id> --covers <token> --type test --command "<cmd>" --result "<what it showed>"
majordomus plan done <id>          # refused while evidence is missing
majordomus plan next               # derived, not decided
```

A worker may take something other than the issue the graph offers when it is a blocker, when
it invalidates an assumption the milestone rests on, or when a person reprioritised. Those
are the three reasons. "It seemed more useful" is not one of them.

## See also

- [`docs/PLANNING.md`](@/docs/planning.md) — what the model means
- [`docs/CONTINUITY.md`](@/docs/continuity.md) — what survives a session ending
- [`docs/DOCTRINE.md`](@/docs/doctrine.md) — how a rule becomes enforced rather than documented
{% endraw %}

+++
title = "Every record names the commit, the branch and the episode that made it"
description = "The ledger records every durable event with its task and its head; identity fields on any record are computed from git and refused when authored; a closed session is an immutable record of what one episode produced; the person's prompts are captured by the provider's hooks; and retention is capped by policy rather than by forgetting."
weight = 110
[extra]
id = "provenance"
status = "stable"
source = ".ai/repo/features/provenance.md"
+++
{% raw %}

## What it does

Every command that changes durable state appends one event to the ledger, with the task and
the head it happened at, from a closed vocabulary. `majordomus history` reads it back and
validates it; `watch` reports drift between policy, projections, state and git with the
command that reproduces each finding. A closed session record names the branch, the commit
it started from, the one it ended at, the commits between, the paths changed and every
record the episode produced, all derived; the working copy is named by a hash, never by a
path on somebody's machine.

That is the foundation for tracing a change from the provider and model class that made it,
through the session and the task, to the commit and the decision it rests on — as data a
program can read, on every surface.

## What it does not do

It measures no tokens and no cost; the ledger records the events that a cost model would
need, and the economics document says what honest measurement would take. It makes no
compliance claim: what it provides is a trail with computed identities, and what an
organisation does with it is the organisation's.
{% endraw %}

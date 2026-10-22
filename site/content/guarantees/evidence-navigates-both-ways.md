+++
title = "Every claim names the test that proves it and every test names the claims it proves, from one derivation"
description = "A relation that can only be walked one way is half a relation. majordomus evidence claim"
weight = 176
[extra]
claim_id = "evidence-navigates-both-ways"
status = "guaranteed"
source = "docs/claims/evidence-navigates-both-ways.md"
+++
{% raw %}

## What it means

A relation that can only be walked one way is half a relation. `majordomus evidence claim
<id>` answers *what proves this claim*: the proof state and the sentence explaining how it
was derived, the execution behind it with its outcome, duration, commit, tree state,
digest, time and origin, the paths that changed since, the command that reproduces it, and
the other claims the same test proves. `majordomus evidence proves <test>` answers the
converse: the test's latest execution, whether its own source still hashes to what that
execution recorded, the command that runs it again, and every claim that names it with the
state each is in.

Both read the same join, computed on the same read path, so the two directions cannot
disagree. Neither is a stored index that the other could drift from.

## How it works

One declaration in `apps/majordomus-cli/src/capability/builtin/evidence.rs` carries four
capabilities, and every surface is derived from it. `evidence.claim` and `evidence.test`
both call the same `report` over the index and the ledger; the first selects one claim and
then lists the other claims whose test identity matches it, the second selects one test and
then lists every claim naming it. A test shared by twelve claims is a test whose failure is
twelve findings, and a reader of one of them can see the other eleven without searching.

A test is named either way round. `suite:84_distribution_model`, `crate:why`, and the path
a claim writes — `test/cases/84_distribution_model.sh` — all resolve to the same identity,
because a client that has one spelling should not have to know the other. An argument that
is neither is refused with the three shapes it could have been, and a claim id the matrix
does not declare is a not-found rather than an empty answer: a typo that read as "this
claim has no evidence" is the one answer these capabilities must never give.

The command line spells the second direction `majordomus evidence proves`; the capability
behind it is `evidence.test`, reached over HTTP at `GET /api/v1/evidence/test` and over MCP
as the tool `majordomus_evidence_test`. None of the read capabilities caches, because the
ledger is a file that changes outside the process and a cached answer would be exactly the
stale evidence the subsystem exists to name. `evidence.record`, the only one that writes,
declares a command line and nothing else — this executable's MCP and HTTP surfaces are
read-only, so the recorder is offered to whoever runs the executable and to nobody over a
socket.

## How to see it

```bash
majordomus evidence claim distribution-canonical-model
majordomus evidence proves suite:84_distribution_model
majordomus evidence proves test/cases/84_distribution_model.sh   # the same test
majordomus evidence proves crate:why --format json | jq '[.proves[].id]'
```

`test/cases/124_evidence.sh` asserts the round trip in both directions in a fixture: the
test lists exactly the claims that name it, every claim it lists names that test back, and
a claim reports the other claims its own test proves.

## What it does not cover

The relation is over what the matrix declares, and nothing else. A test that no claim names
proves nothing here — it is answered with an empty list rather than inferred to cover
something — and a claim that names no test has no direction to walk in.

It is a relation between a claim and a whole test, not between a claim and the assertions
inside one. Two claims sharing a case share its verdict entirely: when the case fails, both
are `failing`, even if the failing assertion belongs to only one of them. Splitting that
would mean a test format that declares which assertion proves which claim, which is a
larger thing than this and is not what the matrix says.

Whether the test proves the claim at all is not answered here. The join says which test a
claim named and what that test did; that the test exercises the behaviour the sentence
describes is a question for review.

## Why it exists

The matrix could always be read from the claim outwards, by opening the file and following
a path. What nobody could ask was the question a person actually has when a case goes red:
*what does this failure mean for what we publish?* Deriving both directions from one join
makes that a command instead of a search, and makes it impossible for the answer to depend
on which end the reader started from.
{% endraw %}

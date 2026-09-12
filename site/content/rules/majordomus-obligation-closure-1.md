+++
title = "Obligation closure"
description = "Obligation closure"
weight = 30
[extra]
kind = "rule"
slug = "majordomus-obligation-closure-1"
identity = "majordomus.obligation-closure@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/obligation-closure.v1.md"
+++
{% raw %}

## Rationale

`finish` already refuses, but everything it refuses over is inside the working tree. A
worker can satisfy every line of the contract with the work uncommitted, unpushed, absent
from the trunk, unpublished and unverified, and the record will say completed. The words
implemented, committed, pushed, integrated and deployed name different facts, and a report
that treats them as one is not a report.

The second half is staleness. Evidence that cannot expire is a claim about the past
presented as a claim about the present: a test result recorded before a change says nothing
about the tree that exists after it. This repository already solved that for its website,
by hashing the inputs a derived artifact was built from and recomputing the hash to decide
whether the artifact is current. An obligation's evidence is the same shape of fact and is
judged the same way.

The third part follows from the second. Where the fact is one the tool can hold — the tree
is clean, the remote has the commit, the trunk reaches it, the published site serves it — a
record of it is a copy of an answer that can be read directly, and a copy is the thing that
goes stale. Those obligations are settled live instead, which is the same cure as
recomputation applied to a fact that has no inputs to hash. It also removes the failure a
ledger cannot catch: a worker recording a push that never happened.

## Required behaviour

A task may declare `requires`, a list of tokens `share/obligations.yaml` defines. Each token
declares `established_by`: what settles its fact without asking a worker, or `none`, in which
case it also declares `unestablished` — one line saying why nothing here can settle it, so
that a gap is written down rather than left to be inferred.

A token whose `established_by` is not `none` is settled live, at HEAD, on every evaluation.
It discharges by being true and refuses by being false, and a recorded `task.evidence` line
for it neither discharges it nor rescues it: establishment beats recording in both
directions. A fact established this way is by construction taken at HEAD, which is
`mj_git_label`'s `exact`. Where the checkout cannot settle it — no remote configured, no
default branch recorded, a published site that never answered — the obligation falls back to
the recorded line and is judged exactly as an unestablishable one is, and the finding names
what could not be settled and why. Unreachable and unpublished are different findings.

Evidence for every other token is recorded as a `task.evidence` ledger line naming the
obligation, how it was taken, the command or artifact that produced it — narrative is not
evidence — and the hash of the tracked files the obligation's `inputs` select.

When the outcome is `completed`, every declared obligation must be established or have
evidence. Evidence for an obligation with inputs discharges it only while the recomputed
hash of those inputs equals the recorded one. Evidence for an obligation whose fact is
remote — a push, an integration, a publication, a deployment — is bound to the commit it was
taken at, and discharges the obligation only while `mj_git_label` reports `exact` against
that commit.

An outcome other than `completed` is not refused: the obligations are named as outstanding
and the task is allowed to be honest about being unfinished.

## Failure behaviour

A violation is a `FAIL` finding under the category `obligation`, naming the token and the
reason — the fact refuted with what was found instead, owed with no evidence, inputs that
have changed with both hashes, or a commit the branch has left — and the command that would
establish or record it again. `finish` exits 10 and writes nothing.

## Verification

`mj_validate_obligations` decides it, dispatched from `check, finish`. The behavioural case
`test/cases/103_obligations.sh` proves it, and CI runs that case.
{% endraw %}

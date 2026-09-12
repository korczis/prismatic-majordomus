+++
title = "An obligation whose fact the tool can hold is established live rather than recorded — a clean tree, a remote-tracking ref that reaches the head, a trunk that reaches it, a published site that serves it, a deployed surface that states it — and a hand-recorded line neither discharges it nor rescues it"
description = "Six of the eleven obligation tokens name a fact that lives outside the working tree:"
weight = 149
[extra]
claim_id = "obligation-establishment"
status = "guaranteed"
source = "docs/claims/obligation-establishment.md"
+++
{% raw %}

## What it means

Six of the eleven obligation tokens name a fact that lives outside the working tree:
`commit`, `push`, `target`, `pages`, `deploy`, `verify`. Until this landed, all six were
discharged the same way — a person ran something, or said they had, and typed
`majordomus evidence --covers push --command 'git push'`. The ledger held the sentence. Git
held the answer, and nobody asked it.

All six are asked rather than recorded:

<div class="overflow-x-auto" tabindex="0">

| token | the fact | how it is settled |
|---|---|---|
| `commit` | the task's changes are in the branch's history, not the working tree | no path inside the task's scope is dirty, and HEAD has moved past the commit the task started from |
| `push` | the branch head exists on the remote it tracks | a remote-tracking ref reaches HEAD |
| `target` | the repository's default branch reaches the commit | `refs/remotes/<remote>/HEAD` reaches HEAD |
| `pages` | the published site serves this commit | `scripts/pages verify --timeout 0` — one probe of the identity document the site publishes |
| `deploy` | every active deployment object serves the trunk's revision | `deploy.verify`, asking each object's `/api/v1/distribution/build`, once the trunk reaches HEAD |
| `verify` | every surface the change reaches states the trunk's revision | `deploy.verify` over the site, the release metadata and the active deployments (claim `deployment-verified-live`) |

</div>


The last two were hand-recorded until ADR 0057, and the vocabulary said why in one line;
now it names what asks.

An established obligation discharges by being true and refuses by being false. Recording
evidence for one has no effect in either direction: a `task.evidence` line saying `push`
against a branch the remote has never seen does not survive contact with `git`.

## How it works

`share/obligations.yaml` gives every token an `established_by` — `git`, a command, or
`none`. A token whose value is `none` must also carry `unestablished`, one line saying why
nothing here can settle it; the behavioural case fails if a token declares one without the
other, in either direction, because a token declared establishable with nothing to establish
it would otherwise pass silently.

`mj_validate_obligations` in `lib/evidence.sh` asks `mj_obligation_establish` first. That
dispatches to `mj_obl_est_<token>`, which exits 0 established, 1 refuted, or 2 undecidable —
nothing in this checkout could settle it. Only the third case reads the ledger, and it then
behaves exactly as it did before this existed.

Nothing fetches. The git tokens read the remote-tracking refs as they stand, because a
validator that went to the network on every `check` would make a diagnostic depend on
connectivity. The reading is safe in the direction that matters: a tracking ref cannot
contain a commit the remote never received, so a stale ref can only say "not yet" when the
answer is "yes" — never the reverse.

`pages` calls the probe that already existed. `scripts/pages verify` has polled the
published site's identity document for the commit it names since the Pages fast path was
written, and until now the Pages workflow was its only caller. Two things made it callable
from a validator: `--timeout 0` is a single probe, because the loop reads the site before it
looks at the clock; and each probe is now bounded by `--max-time`, so an unreachable host
costs a known number of seconds. It also tells its two failures apart now. A site that
answers and names another commit is a refusal, exit 10 — this work is not published. A site
that never answered was not measured, exit 12 — the check could not be made, and a laptop
with no network must not be able to fail a task it cannot see.

## Why it is live rather than recorded

Both are defensible and the alternative is worth stating. A ledger line records *when* the
fact was true, which is what somebody auditing the history a month later wants, and it keeps
one mechanism instead of two.

Live wins on the argument this whole subsystem rests on. The obligation ledger exists
because a record outlives the thing it described, and the cure it applies everywhere else is
recomputation, never a timestamp: `mj_obligation_inputs_hash` recomputes, the site's
`source_hash` recomputes. A git fact recomputes in milliseconds and is always a statement
about now. Recording it would manufacture exactly the staleness this subsystem was written
to remove, and then need the staleness machinery to take it away again — a round trip whose
only product is a window during which the record and the repository disagree.

And the ledger already holds what a worker *did*: every `start`, `checkpoint`, `evidence`
and `finish` is in it, with the head, the branch and the session in the envelope. It is not
where the tool writes down what it can look up. A hand-recorded remote fact is also the one
kind of evidence a worker can be wrong about in the direction that flatters them.

The staleness discipline is kept rather than dropped. A fact established here is taken at
HEAD by construction, which is `mj_git_label`'s `exact`, and `exact` is the word the finding
says. `advanced`, `diverged` and `different_context` keep their meaning on the fallback path.
No fifth vocabulary was invented.

## How to see it

```bash
majordomus check
# OK   obligation  commit — exact: the tree is clean and 1bcd0b3790c5 carries the task's changes
# OK   obligation  push   — exact: origin/master contains 1bcd0b3790c5
# OK   obligation  target — exact: origin/master reaches 1bcd0b3790c5

git commit -qm 'one more'
majordomus finish --outcome completed --note done
# FAIL obligation  push   — no remote-tracking ref reaches 9c8a74105c8c; the commit has not
#                           reached the remote
# FAIL obligation  target — origin/master does not reach 9c8a74105c8c; the work is not on
#                           the trunk

majordomus evidence --covers push --command 'git push'   # recorded, and it changes nothing
majordomus finish --outcome completed --note done
# FAIL obligation  push   — no remote-tracking ref reaches 9c8a74105c8c; ...
```

## What it does not cover

`deploy` and `verify` are still discharged by a person, deliberately. `majordomus deployment`
reads the declared deployment object and contacts nothing — no capability in
`apps/majordomus-cli/src/capability/builtin/deploy.rs` reaches the provider, which is a rule
of that module and not an oversight — and the object it reads is declared rather than
running, so there is no host to ask. A check that always passed would be worse than a gap
that is written down.

Nor are the local tokens established. `tests` could only be settled by running the suite,
which would make `check` cost what `test/run.sh` costs. `generated` could be settled by
`majordomus generate --check` in about twelve seconds, and `docs` by
`scripts/ci/reference-check`; neither is wired, because what a diagnostic may spend on every
invocation has not been decided. Both say so in `unestablished`, which is where this
repository puts a gap it has not closed.

An established `commit` also does not judge whether the committed change is the change the
task promised. That is `implementation`, and it is a judgement, not a fact.
{% endraw %}

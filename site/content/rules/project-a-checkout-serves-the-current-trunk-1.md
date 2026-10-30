+++
title = "A checkout serves the current trunk, and says how far it is from it"
description = "A checkout serves the current trunk, and says how far it is from it"
weight = 64
[extra]
kind = "rule"
slug = "project-a-checkout-serves-the-current-trunk-1"
identity = "project.a-checkout-serves-the-current-trunk@1"
status = "active"
source = ".ai/repo/rules/project/a-checkout-serves-the-current-trunk.v1.md"
+++
{% raw %}

## Rationale

On 2026-10-16 the primary checkout of this repository was 779 commits behind
`origin/master` and held uncommitted changes nobody could attribute. Its shared server — the
one every session attached to the primary reaches over MCP — answered plan questions from
that tree, so a sweep of fourteen "open" issues was planned against a plan in which thirteen
of them were already done. The local `master`, which `worktree create` then used as the base
of every new branch, was exactly as far behind. Every surface was ready, every check was
green, and nothing anywhere said "this tree is not the trunk".

A server that serves a stale tree is not a stale server: its version is current, its lease is
valid, its peers are attached. The staleness is a fact about the checkout, and it was in no
reading at all. That is the defect this rule closes: the distance is measured, stated
wherever an answer is served, and judged against a declared number.

The cooperation model the owner set on 2026-10-16 — always current, rescue before reset,
release often — is recorded once, in ADR 0167 and the rule that carries it, and ADR 0156
(proposed) names *current* as one of the facts a session joins the cluster with.
This rule is not a second statement of either. It is the measurement both rely on, and the
gate that makes a stale checkout impossible to miss.

## Required behaviour

**The distance is derived, never kept.** `worktree::trunk::measure` counts, with
`rev-list --count`, the remote trunk's commits the checkout's `HEAD` lacks (behind) and the
`HEAD` commits the remote trunk lacks (ahead), and the same for the local trunk branch. The
remote trunk is the trunk's configured upstream, else the remote whose `HEAD` names the
trunk, else the first remote carrying a branch of that name. Nothing is fetched and nothing
is written: the answer is true as of the last fetch, and the time of that fetch
(`FETCH_HEAD`) is part of it.

**It is judged against the policy, and only the policy.** `checkout.trunk_distance` declares
`warn_behind` and `fail_behind`. At or past the first a checkout is `lagging`; at or past the
second it is `stale`. A distance with no declared threshold is `unjudged`, a trunk with no
remote-tracking branch is `unmeasured`; neither is ever reported as `current`, and no reader
supplies a number the policy did not write.

**It is reported wherever a tree is served.** `majordomus worktree trunk` (`worktree.trunk`,
`majordomus_worktree_trunk`) answers it alone and exits 10 when stale; `majordomus worktree
status` prints and returns it (`trunk_distance`); `server.status` (`serve status`, `majordomus_server`,
`/api/v1/server`) carries it as `trunk`; the MCP `initialize` instructions name a lagging,
stale or unjudged distance in the sentence every client reads on every connect; doctor
renders it under `clone/trunk`.

**Stale fails.** Doctor FAILs a stale checkout — and the commit hook, which runs doctor, with
it — and WARNs a lagging one with the commands that bring it current. A merge in progress
counts the commits it brings in, so the commit that makes a stale branch current is never
refused for the tree it is replacing. The local trunk of a linked worktree's repository is
another checkout's to move, so in a linked worktree it is reported as a WARN
(`clone/local-trunk`) and never fails that worktree's commits.

**Bring it current without losing anything.** A dirty checkout is rescued before it is
moved: `git stash create` gives a commit of the uncommitted state without touching the tree,
`git update-ref refs/rescue/<checkout>-<timestamp> <sha>` keeps it, and the untracked files
are copied beside it; only then is the trunk checkout fast-forwarded (or reset to the remote
trunk), and a feature branch merges the remote trunk rather than rewriting what was pushed.
A typed `majordomus worktree update` that does this and refuses while a process stands in
the checkout is the next slice; until it lands, the remedy each reading names is the
sequence a person runs.

## Proof

- `test/cases/3176_a_checkout_serves_the_current_trunk.sh`: a fixture remote moved ahead of
  its clone; `worktree status` reports the exact distances for the checkout and the local
  trunk, judged `stale` and then `current` once fast-forwarded; doctor FAILs `clone/trunk`
  while stale and passes once current; a merge in progress is counted as present; a clone
  with no remote is `unmeasured`.
- The unit tests of `apps/majordomus-cli/src/worktree/trunk.rs` and the doctests of `judge`,
  `TrunkThresholds` and `CheckoutPolicy` hold every branch of the judgement.
{% endraw %}

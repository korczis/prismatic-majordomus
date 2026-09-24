+++
title = "The preflight"
description = "the preflight: what is proven about a checkout rather than what it is — a verdict per claim (git, episode, briefing, task, handover, policy, rules, ADRs, server and surfaces, peers, test runs, enforcement, projections, generated docs, deployment) with the evidence behind it; the nine verdicts, what entry reads and never reads, the measured cost, and what to run for each verdict"
weight = 48
[extra]
source = "docs/PREFLIGHT.md"
+++

{% raw %}

Entering a Majordomus repository answers one question within the time a `cd` takes: *is
Majordomus actually in force here, what context am I working in, what integrations are alive,
and which claims are currently proven?* The answer is one typed value,
`environment::preflight::Preflight`, built under
[`apps/majordomus-cli/src/environment/preflight.rs`](https://github.com/korczis/prismatic-majordomus/blob/master/apps/majordomus-cli/src/environment/preflight.rs).
Every surface renders that value and none computes a status of its own. Behaviour as implemented
and tested; where this document and the executable disagree, the document is wrong.

The rule is `project.entry-reports-only-evidence`
([`.ai/repo/rules/project/entry-reports-only-evidence.v1.md`](https://github.com/korczis/prismatic-majordomus/blob/master/.ai/repo/rules/project/entry-reports-only-evidence.v1.md));
the decision is ADR 0066.

## Architecture

```text
repository state (git, .ai/local/state, lease, evidence ledger, gh-pages ref, index)
        │  observe()  — the only reading; bounded, loopback only, never a build
        ▼
Observations           — plain data
        │  derive()   — pure; every verdict is a test over a value
        ▼
Preflight              — sections of checks, each a verdict and its evidence
        │
        ├── env enter      the compact line drawn on entering (direnv, via .envrc)
        ├── env preflight  full, --compact, --format json, --full
        ├── GET  /api/v1/environment/preflight
        ├── MCP  tool majordomus_preflight, resource majordomus://environment/preflight
        └── Cockpit overview, the Preflight card (each verdict as data-check / data-verdict)
```

`RepositoryEnvironment` ([`ENVIRONMENT.md`](@/docs/environment.md)) says what a checkout *is*. The
preflight says what is *proven* about it. The two are separate values on purpose. Folding
proof into the snapshot would change its digest, so every banner would read as news.

## Repository bootstrap

`.envrc` is unchanged: it makes one call, `bin/majordomus-env enter --shell direnv`, and
evaluates what that call exports (`project.envrc-is-an-adapter`). That call resolves the
snapshot, ensures the runtime, and then draws the banner followed by the compact preflight, all
from the same readings. A separate `majordomus env preflight --compact` line in `.envrc` would
be a second `git status` on every `cd`, which the adapter rule forbids. The client bootstrap
files (`.mcp.json`, `.gemini/settings.json`, `.codex/config.toml`) start the launcher and decide
nothing.

## Status semantics

<div class="overflow-x-auto" tabindex="0">

| Verdict | Means | Drawn |
|---|---|---|
| `verified` | proven against this tree by a recorded result or a direct answer | `✓` |
| `active` | in force now, observed directly, nothing recorded to verify | `✓` |
| `fresh` | recent enough to act on, against a declared threshold or the current commit | `✓` |
| `stale` | evidence exists, about another commit, tree or moment | `◐` |
| `degraded` | in force, in a form this executable would not serve | `◐` |
| `unavailable` | nothing is there to observe | `○` |
| `failed` | observed, and it did not hold | `✗` |
| `unknown` | nobody looked, or the looking did not finish | `?` |
| `not_applicable` | nothing here to judge | `-` |

</div>


The first three cannot be produced without evidence. `Check::new` turns a bare claim of force into
`unknown`, so no renderer can draw a success that nothing backs. `attention` lists every check
that needs a look, the most urgent first: failed, degraded, stale, unavailable, unknown.

## The checks

<div class="overflow-x-auto" tabindex="0">

| Check | Evidence read | Verdicts |
|---|---|---|
| `repository.git` | `git status --porcelain=v2 --branch` (the snapshot's own call) | active, unavailable |
| `session.episode` | `.ai/local/state/session-current.yaml` | active, degraded (another checkout's), unavailable |
| `session.task` | `.ai/local/state/current.yaml` | active (`outcome: active`), not_applicable |
| `session.context` | the episode's `start_head` and `start_working_tree` against HEAD, the tree and the task's start | fresh, stale, unknown (no episode, or written over a dirty tree) |
| `session.handover` | continuity's record resolution and `session.freshness` | fresh, stale, unknown, not_applicable |
| `governance.policy` | `LoadedPolicy` | active, failed |
| `governance.rules` | `rules.report`, counted at a commit | active (at HEAD), stale, unknown |
| `governance.adrs` | the index, kind `adr` | active (indexed; relevance not claimed), unknown |
| `integration.server` | the lease, `standing_of` over one `GET /` — or, in the process that holds the lease, over its own lease document, so a rebuilt executable is `degraded` there too | verified, degraded (outdated), failed (stale lease), unavailable, unknown |
| `integration.mcp` / `api` / `cockpit` | the surfaces `GET /` lists with `ready` | verified, degraded, failed, unavailable, unknown |
| `integration.peers` | this checkout's peer board (`checkouts=this`), the same board a served request reads | active, degraded, unavailable, unknown (not asked on entry) |
| `verification.tests` | `.ai/repo/evidence/ledger.json`, each run judged by the evidence module's own tree comparison and the test's digest | verified, stale, failed, unavailable |
| `verification.coverage` | nothing is recorded | unavailable |
| `verification.enforcement` | the rule proofs of `rules.report` | verified, degraded, stale (another commit, or a tree that is not clean — checked before a failure), failed, not_applicable (nothing owes a proof), unknown |
| `verification.projections` | provider projections against their templates | verified, stale, unknown, not_applicable |
| `verification.docs` | no recorded generation check | unknown |
| `verification.deployment` | `refs/remotes/origin/gh-pages`, whose commits name `source: <sha>` | verified (source is HEAD, or a prefix of it), stale, unknown, unavailable |

</div>


## Governance status

A rule file on disk is **discovered**. `governance.rules` is `active` when `rules.report` counted
the corpus at HEAD, which means it is **loaded**. Its summary gives how many rules are blocking,
how many are the project's own and how many are vendored, and how many a CI gate runs.
**Enforced** and **verified** belong to `verification.enforcement`. That check is `verified` only
when every rule that owes an executable proof is `proven` against the tree. A gate that exists is
not a verdict that passed, so a corpus that is gated and never run is `degraded`. ADRs are counted
as indexed; nothing joins an ADR to a task, so none is claimed relevant.

## Session context freshness

An episode's briefing is exact at the commit and tree it was written at. It is `stale` once HEAD
moves, the tree changes under it, or a task starts after it. The summary names which of these
happened, and `majordomus context` writes a current briefing. A briefing written over a dirty
tree is `unknown` rather than `fresh`: the episode records the word `dirty`, not which changes, so
an edit or a revert since cannot be seen. The preflight never regenerates
context itself: entry is not the place for a write the provider's own lifecycle owns. No compiled
context revision exists in a checkout yet, so none is shown.

## MCP and Cockpit detection

A lease on disk is a claim, not a server. The server is `verified` only when the address the lease
names answers `GET /` as this checkout's leaseholder, at this executable's version. That is
`lease::probe_reply`, the same decision the launcher's election makes, with a 250 ms budget. A
server that answers from other code is `degraded`. A lease whose address does not answer is
`failed`. No lease is `unavailable`. The MCP, API and Cockpit checks read the `ready` flag the
server's own index lists for each surface. The banner no longer marks a service answering unless
the server is verified. `MAJORDOMUS_URL` is still exported, because a client can still reach the
address. A served request holding the lease is its own evidence, and it reads its own peer board.

## Verification evidence

A run in the evidence ledger verifies the tree only by the evidence module's definition of proven:
it passed, its test still hashes as recorded, and the tree in front of you is the tree the run
measured, with the ledger's own row excluded. A run at any other tree is `stale`. That comparison
costs git two processes per recorded commit, so its answers are cached in
`.ai/local/state/environment/snapshot.json`. The cache is keyed on HEAD, every path git reports
changed, and the ledger file, so any change recomputes it. A served request never writes the
cache. Coverage is reported `unavailable` until a coverage measurement leaves a record in the
repository.

## Deployment freshness

`scripts/pages` publishes to `gh-pages`, and every deployment commit names the source commit it was
built from. The preflight reads the local ref `refs/remotes/origin/gh-pages`. The deployment is
`verified` when that source is HEAD, `stale` when it is another commit, and `unavailable` without
the ref. The ref is only as recent as the last fetch, and the evidence says so. Whether GitHub
serves that commit is the business of `scripts/pages verify`. Local runtime state is never
published to the static site.

## Performance guarantees

Entry never builds the index. The rule tally comes from the cache that `env preflight --full`
writes, stamped with the commit it was counted at. Entry never asks the peer board, which costs a
round trip that gathers every checkout's board. It never runs a test, a build, the generator or a
remote request. Its only socket is to the loopback address the lease names.

The measurement below was taken on 2026-09-15, debug build, in a linked worktree of this
repository, while several other sessions were building on the same machine. It is warm entry
through `bin/majordomus-env enter --shell direnv`, the call `.envrc` makes:

<div class="overflow-x-auto" tabindex="0">

| | wall time |
|---|---|
| banner off (snapshot, runtime ensured, bridge) | 219–229 ms |
| compact banner and compact preflight | 248–301 ms |
| first entry after a change of HEAD or tree, ledger comparison recomputed | about 2.2 s once |
| `env preflight` (fast, asks the board) | about 230 ms |
| `env preflight --full` (builds the index, counts the rule proofs) | about 1.1 s |

</div>


So the preflight adds about 30–70 ms to a warm entry. The floor under it is the entry that was
already there. Every git process on this machine cost 40–90 ms under that load, and the ledger
comparison is two of them per recorded commit, which is why its answers are cached. The mandate's
targets of 50 ms warm, 150 ms cold and 250 ms hard are **not** met on this machine. The budgets the
repository enforces are the policy's `benchmark.budget.enter_ms` (750 ms) and `enter_cold_ms`
(1000 ms), measured by `test/cases/190` on every run.

## Troubleshooting

<div class="overflow-x-auto" tabindex="0">

| You see | It means | Do |
|---|---|---|
| `server ◐ degraded` | a server answers from another version or a replaced executable | `majordomus serve stop && majordomus serve ensure` |
| `server ✗ failed` | the lease names an address that does not answer as this checkout | `majordomus serve ensure` |
| `rules ?` / `enforced ?` | the rule corpus was never counted in this checkout | `majordomus env preflight --full` |
| `rules ◐` | counted at another commit | `majordomus env preflight --full` |
| `tests ◐ stale` | the recorded runs measured another tree | run the tests and record them (`majordomus evidence record`) |
| `context ◐ stale` | HEAD, the tree or the task moved since the briefing | `majordomus context` |
| `deploy ◐ stale` | the site was built from another commit, or the ref is not fetched | `git fetch origin gh-pages`; `scripts/pages verify` |
| nothing drawn | a pipe, `CI`, or `MAJORDOMUS_BANNER=off` | `majordomus env preflight` |

</div>

{% endraw %}

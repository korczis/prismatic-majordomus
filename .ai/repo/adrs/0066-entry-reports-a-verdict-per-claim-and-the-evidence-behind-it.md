---
schema: adr/v1
id: adr-0066
kind: adr
title: Entry reports a verdict per claim and the evidence behind it, from one preflight value
status: proposed
date: 2026-09-15
tags:
  - entry
  - environment
  - evidence
  - verification
provenance:
  origin: authored
related:
  - rule:project.entry-reports-only-evidence
  - rule:project.envrc-is-an-adapter
  - rule:project.entry-converges
  - test:test/cases/358_entry_reports_only_evidence.sh
  - test:apps/majordomus-cli/tests/preflight.rs
---

# 66. Entry reports a verdict per claim and the evidence behind it

## Context

The mandate of 2026-09-15 asked that entering the repository prove whether Majordomus is
actually in force: governance, session context, MCP, the Cockpit, peers, tests, coverage,
documentation and deployment. It proposed a new canonical snapshot, a thin `.envrc`, and
surfaces on the command line, the API, MCP and the Cockpit.

Measured before anything was built:

- `.envrc` was already one call to `majordomus env enter` (ADR 0043,
  `project.envrc-is-an-adapter`), and entry took about 190 ms in a debug build.
- `environment::RepositoryEnvironment` already was the canonical snapshot of what a checkout
  *is*, with provenance per field, served on every surface.
- The banner drew `✓` beside the Cockpit, docs, Swagger, API, events and MCP while the shared
  server was `outdated` (0.6.1 serving an executable at 0.7.0). The mark came from
  `ServiceAvailability::Available`, a TCP connection.
- The evidence to do better already existed and was nowhere on the entry path:
  `server.status`'s `standing_of`, the server's own `GET /` listing each surface with `ready`,
  `rules.report` (137 rules, 0 with a recorded passing run), the evidence ledger (6 executions,
  none at HEAD), `continuity.state`'s freshness judgement, and the `gh-pages` commits, each of
  which names its `source:` commit.

## Decision

1. **A second value, not a wider snapshot.** `environment::preflight::Preflight` answers what is
   *proven*; `RepositoryEnvironment` keeps answering what the checkout *is*. Widening the
   snapshot would have changed its digest and schema and made every banner look like news.
2. **Nine verdicts, not booleans:** `verified`, `active`, `fresh`, `stale`, `degraded`,
   `unavailable`, `failed`, `unknown`, `not_applicable`. The three that assert force cannot be
   constructed without evidence (`Check::new`).
3. **Observe, then derive.** `observe` does the reading; `derive` is pure. Every verdict is a
   test over a value.
4. **Reuse, never re-derive.** Server standing is `standing_of` over one `GET /`
   (`lease::probe_reply`, the probe's own decision). Handover freshness is continuity's
   resolver. Test currency is the evidence module's `changed_since`. The rule tally is
   `rules::report`. No detector was written twice.
5. **Entry stays inside its budget by what it does not read.** The rule tally needs the index,
   which entry never builds. So `env preflight --full` counts it and caches it with the commit
   it was counted at, and entry reports it `stale` at any other commit. The peer board costs
   about 90 ms because it gathers every checkout, so entry does not ask it and says so.
   `env preflight` does ask.
6. **The banner no longer marks a service answering unless the server is verified.** The
   assignments still export `MAJORDOMUS_URL` as resolved, because a client may still reach it.
7. **One capability**, `environment.preflight`, projected to HTTP, MCP tool and resource. The
   Cockpit overview renders it with each verdict as data. A served request that holds the lease
   is its own evidence for the server and reads its own board.
8. **What nothing records is said, not hidden.** Coverage is `unavailable` because
   `scripts/rust-coverage` leaves no record. Generated documentation is `unknown` because no
   generation check is recorded. ADR relevance is not claimed because nothing joins an ADR to a
   task.

## Consequences

- The entry line is honest and, in this repository today, mostly not green. That is the point:
  0 of 107 rules owing a proof are proven against the tree, and the recorded test runs are about
  other commits.
- The `.envrc` shape the mandate proposed (a separate `env preflight --compact` call) was not
  adopted. It would be a second `git status` on every `cd`, which `project.envrc-is-an-adapter`
  forbids. `env enter` draws the compact preflight from the same snapshot.
- Joining coverage evidence and a recorded generation check to the preflight is future work, and
  each is one observation plus one check when the record exists.
- GitHub Pages is not given runtime state. The deployment verdict is about the local
  `gh-pages` ref as last fetched, and `scripts/pages verify` remains the authority on what is
  served.

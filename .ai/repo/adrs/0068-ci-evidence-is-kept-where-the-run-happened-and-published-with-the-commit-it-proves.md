---
schema: adr/v1
id: adr-0068
kind: adr
title: CI evidence is kept where the run happened, and published with the commit it proves
status: proposed
date: 2026-09-15
tags:
  - evidence
  - verification
  - ci
  - pages
provenance:
  origin: authored
  derived_from:
    - decision:adr-0041
related:
  - file:docs/EVIDENCE.md
  - file:scripts/ci/evidence-collect
  - file:scripts/pages
  - file:.github/workflows/validate.yml
  - file:.github/workflows/pages.yml
---

# 68. CI evidence is kept where the run happened, and published with the commit it proves

## Context

ADR 41 made a claim's proof a recorded execution, with a commit, a digest and an origin, and
kept the latest execution of every test in `.ai/repo/evidence/ledger.json`. It named `ci` as an
origin. Nothing ever recorded one.

Measured on 2026-09-15 at `c3f20da`: the ledger held six executions, every one `local`, the
newest from 2026-09-12. Of the 173 claims of the matrix, 149 were `not_run` and 14 `stale`. The
behavioural suite ran on every push to master and wrote `suite.tsv`, which CI kept for fourteen
days as a timing artifact and nobody recorded; `cargo test` printed its results into a log; the
coverage job uploaded an llvm-cov export that only the gate read. The evidence existed on every
run, and the repository could not show a reader any of it.

Three constraints decide where CI evidence can live:

- **It cannot be committed by the run.** A run that writes the ledger and pushes it to master
  needs a writer past branch protection, adds a commit per merge to a trunk that already moves
  every few minutes, and the commit it adds is not the commit it proved.
- **The site publishes before the suite ends.** `pages.yml` is the fast path: a master push is
  public in minutes, while the suite takes over an hour. A page built for commit X cannot hold
  evidence of X when it is published.
- **Publication has one scheduler.** `test/cases/97_pages_fast_path.sh` refuses a `workflow_run`
  trigger on `pages.yml`, because a chain through another workflow puts a second scheduler in
  front of publication.

## Decision

**CI evidence is an artifact of the run that produced it.** A job of `validate.yml`, `evidence`,
reads the reports the suite, the crate and the coverage jobs left behind and gives them to
`scripts/ci/evidence-collect`. It records the executions through `majordomus evidence record
--origin ci` and derives the report through `majordomus evidence show`. It summarises the
coverage through `scripts/rust-coverage --summary-json` and writes a manifest naming the commit,
the run, the outcomes and what was absent. The directory is kept as the artifact `evidence` for
ninety days. The tracked ledger stays what ADR 41 made it, the evidence of whoever records into
the tree. CI's evidence is not committed into it.

**An execution names the run it happened in.** `Execution` gains an optional `run`: the
provider, the run's identifier, its attempt, the workflow, the job and the address a reader
follows. The address is the one the provider gave when the run happened, not one a template
builds later. An adapter constructs it (`RunRef::from_env` reads a GitHub Actions environment)
and only for a `ci` origin: a local recording made inside a CI shell is still not that job's
evidence. Rows recorded before this carry no `run`, which is why the ledger's version is
unchanged.

**The site publishes the nearest evidence it has, and says how near it is.** `scripts/pages
evidence` runs before the build, finds the `evidence` artifact of master's validation recorded
against the commit being published or its nearest ancestor, and writes `site/data/evidence.json`.
Like `site/data/build.json`, that file is an input of one deployment and is never committed.
Evidence of the published commit is `current`. Evidence of an ancestor is published as stale,
naming the commits and the changed files between. No evidence at all is published as
unavailable, with the reason. None of the three blocks publishing. The one thing ruled out is
evidence of an older commit shown as if it proved this one.

**The refresh is a dispatch, not a chain.** After it keeps the artifact, the `evidence` job of a
master push dispatches `pages.yml` if its commit is still master's tip. That publication finds
evidence of its own commit and says `current`. If master has moved on, nothing is dispatched:
the newer commit's publication already shows this evidence as stale, and the newer commit's own
run will refresh it. `pages.yml` stays triggered by the push alone, and a dispatched run already
waits for a running publication rather than cancelling it.

**The run link is decided offline.** `scripts/ci/link-check` refuses a link it cannot resolve
without the network. It now accepts exactly one Actions link: the run named by the publication's
own evidence, which exists because it produced that evidence.

## Consequences

- A reader of `/evidence/` sees what CI ran, the outcomes, the crate's coverage, the commit and
  the run, and whether any of it describes the commit the site was built from.
- `pages.yml` gains `actions: read`, argued in the case that pins its permissions.
- In a busy hour master moves faster than the suite, so most publications show stale evidence.
  That is the true state of a trunk that outruns its verification, and the page says so rather
  than hiding it.
- The `evidence` job is not needed by the `ci` verdict. A broken collector makes the site say
  "unavailable", which is visible, but it does not stop a merge. Making it a gate is a separate
  decision.

## What this does not yet decide

The subject a verification belongs to beyond a claim (a feature, a command, an MCP tool, a
rule), coverage attributed to such a subject, a verification panel on their pages, and the
Cockpit's view of the same evidence. Each reads the artifact this decision creates, and none of
them is built by it.

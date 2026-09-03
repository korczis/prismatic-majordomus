# Contributing

Thank you. This project is small on purpose. The best contribution is often a deletion.

## Before you start

Read [`AGENTS.md`](AGENTS.md); it is the operating contract for humans too. Read
[`docs/DESIGN.md`](docs/DESIGN.md), then the pattern ledger in
[`docs/EXTRACTION_REPORT.md`](docs/EXTRACTION_REPORT.md) for any decision that seems
arbitrary. Most of them were paid for.

## What is welcome

- a failing test that shows a documented behaviour is not real
- a smaller implementation of an existing command
- a portability fix for bash 3.2 or BSD userland, with the platform named
- a projection adapter for a provider, if it translates and adds no rules
- a correction to a claim in any document, with the reproduce command

## What is not

- new agents, personas, roles, tiers, or registries
- a daemon, server, database, queue, MCP surface, or background process
- a check that reports but is described as enforcing
- a number written into prose that a command could compute
- a feature the design's "Intentionally Absent" list already rejects, without a
  pull-request description that answers that list point by point

## Process

1. Open an issue describing the failure you observed, with the command that shows it.
2. Branch from `master`. Keep the branch to one concern.
3. Add or change the test in `test/cases/` first. Tests run in a disposable temporary
   repository, never against this checkout.
4. Run `test/run.sh` and `shellcheck` if available.
5. Commit in conventional format, `type(scope): description`, small commits that each
   leave the tree consistent.
6. Before marking the pull request ready, compare every capability sentence you touched
   in `README.md` or `docs/` against the tests. Wording that outruns the tests is
   downgraded or removed in the same pull request.

## Language

English throughout: code, comments, commits, documents.

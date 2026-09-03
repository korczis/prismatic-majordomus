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
- an edit to a generated file instead of to the source it is generated from
- a feature the design's "Intentionally Absent" list already rejects, without a
  pull-request description that answers that list point by point

## Process

0. Once per clone: `git config core.hooksPath .githooks`. The pre-commit hook runs
   `bin/majordomus doctor`; this repository supervises itself.
1. Open an issue describing the failure you observed, with the command that shows it.
2. Branch from `master`. Keep the branch to one concern.
3. Add or change the test in `test/cases/` first. Tests run in a disposable temporary
   repository, never against this checkout.
4. Run `test/run.sh` and `shellcheck` if available. The site cases skip themselves when
   `zola`, `jq` or `node_modules/` are absent, so a CLI-only change needs none of them.
5. If you changed a file the website reads — `README.md`, `docs/*.md`,
   `docs/CLAIMS.yaml`, `share/skeleton/**`, `bin/majordomus`, `lib/**` — run
   `scripts/generate-site-data` and commit the regenerated `site/data/generated/` and
   `docs/SITE_CLAIMS.md`. CI runs `scripts/generate-site-data --check` and refuses to
   deploy stale derived files. Never hand-edit a generated file;
   [`docs/GITHUB_PAGES_ARCHITECTURE.md`](docs/GITHUB_PAGES_ARCHITECTURE.md) lists which
   they are. Renaming a level-2 heading in `README.md` fails the generator on purpose:
   the homepage renders those sections by heading, so update the generator in the same
   commit. `scripts/site-build` then `scripts/site-check` reproduces what CI does, and
   needs `zola`, `jq` and `npm ci`.
6. Commit in conventional format, `type(scope): description`, small commits that each
   leave the tree consistent.
7. Before marking the pull request ready, compare every capability sentence you touched
   in `README.md` or `docs/` against the tests. Wording that outruns the tests is
   downgraded or removed in the same pull request. A new capability sentence also needs
   a row in `docs/CLAIMS.yaml` with its status, implementation and test.

## Language

English throughout: code, comments, commits, documents.

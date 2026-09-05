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
- a daemon, database, queue, or background process; the Rust executable's `mcp` (stdio)
  and `serve` (loopback HTTP) are read-only and end with the process that started them
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
   `zola`, `jq` or `node_modules/` are absent, so a CLI-only change needs none of them;
   `72_rust_mcp.sh` skips itself when `cargo` is absent.
   For a change under `apps/majordomus-cli/`, run `scripts/rust-check`: format, clippy
   with warnings as errors (a public item without documentation is one), `cargo test`
   including doctests, `cargo doc`, `majordomus generate --check`, `capabilities validate`
   and the coverage threshold; CI runs the same. A capability is defined once and every
   interface is derived from it (`project.interfaces-are-projections`): change the
   descriptor, the declarative file or the schema, never a projection, and run
   `majordomus generate` so the committed snapshots under `docs/generated/` and the
   allow-lists under `share/allow/` follow.
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
6. **A new rule the tool enforces is a doctrine, not an inline check.** Add an entry to
   the rule package ([`share/standard/majordomus/`](share/standard/majordomus/), which
   `majordomus rules vendor update` copies into `.ai/repo/rules/vendor/majordomus/`)
   as a rule object with an `x-majordomus` block, run `scripts/rules-package write` so
   the package manifest carries the file's hash, and write its `mj_validate_<name>`
   function in `lib/`; no command may select checks by name. `majordomus doctor` refuses
   a doctrine whose validator does not exist, a doctrine declared for a command that does
   not dispatch, a blocking doctrine whose command cannot exit non-zero, a doctrine with
   no test file, a claim id that is not in `docs/CLAIMS.yaml`, and — in the other
   direction — an `mj_validate_*` function that no doctrine declares. Report violations
   with `mj_doctrine_fail`, never `mj_fail`: the doctrine's class decides the level, and
   your validator must always `return 0`, because a non-zero return means the validator
   itself broke. [`docs/DOCTRINE.md`](docs/DOCTRINE.md) is the full contract.
7. **A public command is a declared surface, not a shell function.**
   [`share/commands.yaml`](share/commands.yaml) says what each command means;
   `bin/majordomus` decides what runs; neither derives from the other and `doctor`
   reconciles them. Adding one means, in any order: the dispatch arm and the usage line in
   `bin/majordomus` with `lib/<name>.sh`; the registry entry and a `` ## `majordomus <name>` ``
   section in [`docs/CLI.md`](docs/CLI.md); a case that declares
   `# majordomus-covers: <name>` and one that declares `# majordomus-negative: <name>`; and
   `test/fixtures/commands/<name>.json`, whose scenarios `test/cases/34_command_fixtures.sh`
   executes against the real binary and the site renders as that command's demonstration.
   Miss any of them and you are told which: the registry reconciliation, the coverage
   doctrine and the site generator each refuse separately and name what is absent.
8. Commit in conventional format, `type(scope): description`, small commits that each
   leave the tree consistent.
8. Before marking the pull request ready, compare every capability sentence you touched
   in `README.md` or `docs/` against the tests. Wording that outruns the tests is
   downgraded or removed in the same pull request. A new capability sentence also needs
   a row in `docs/CLAIMS.yaml` with its status, implementation and test.
9. A change made for speed carries its measurement: `MJ_TIMING=1 bin/majordomus <command>`
   before and after, both numbers in the commit message, and `majordomus bench <command>`
   when the change is accepted. The baseline under `.ai/repo/benchmarks/` changes only
   through `majordomus bench --write-baseline`, in its own commit; `majordomus bench --check`
   is the regression gate. The mechanism and the rules are in
   [`docs/PERFORMANCE.md`](docs/PERFORMANCE.md).

## Language

English throughout: code, comments, commits, documents.

## The site

The public site is a projection of the files above, not a second copy of them. It needs
[Zola](https://www.getzola.org/) and Node; the CLI needs neither.

```bash
npm ci
scripts/site-build      # generate canonical data, build with Zola, compile Tailwind
scripts/site-check      # the checks CI runs
scripts/site-serve      # watch mode
```

`site/data/generated/`, `site/content/` and `public/` are rewritten on every build and are
gitignored. Change the canonical file instead — a policy field, a profile, a claim in
`docs/CLAIMS.yaml`, a row of the README table — and the site follows.
`docs/GITHUB_PAGES_ARCHITECTURE.md` explains the pipeline and what must never be edited
by hand.

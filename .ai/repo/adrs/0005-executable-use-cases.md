# 5. Use cases are executable canonical objects of the layer

Status: accepted, 2026-09-05. Extends ADR 2 (one canonical registry) to the catalogue.

## Context

The site's use-case catalogue was six entries in `share/use-cases.yaml`: prose about
what a person does with the tool, each naming the commands, rules and claims it relies
on, checked for dangling references and rendered to a page. The references were checked;
the behaviour was not. A step could be in the wrong order, an expectation could be false,
and the page would say so with confidence, because nothing executed it. The file lived in
the tool's distribution, so a repository could not write its own use cases without
editing the tool. The operator asked for the catalogue to become an integration backbone:
every externally meaningful capability demonstrated by a use case that runs, every
example on a page generated from an execution, categories, coverage, impact analysis,
scaffolding, and the whole thing impossible to forget during ordinary development.

The invariant this repository already holds for claims (`docs/CLAIMS.yaml`: a guarantee
names the test that proves it), for commands (`share/commands.yaml`: a public command
needs a fixture with scenarios, a page, a behavioural and a negative case) and for the
Rust registry (a capability without a benchmark case does not compile) is the one to
extend, not a new one.

## Decision

- **A use case is a file of the layer**: `.ai/repo/use-cases/<id>.md`, Markdown with
  front matter under `share/schemas/use-case.schema.json`, discovered through the
  manifest's `use-cases` section and `sources.yaml` like every other object, served by
  the Rust executable as `majordomus://use-case/<id>`. Applications the same way under
  `.ai/repo/applications/`; the categories in `taxonomy.yaml` beside the use cases,
  presentation metadata only. `share/use-cases.yaml` and `share/applications.yaml` are
  gone; a managed repository writes its own.
- **The scenario is the proof.** A use case carries a prepared state (a setup script
  from the fixtures the command demonstrations already use), the commands it runs with
  their argv, expected exit codes and output, and what that proves, in words. `majordomus
  usecase run` executes it against `bin/majordomus` in a disposable repository and
  writes normalised evidence under `.ai/local/evidence/`. The site generator runs every
  scenario itself and embeds the evidence, so a page shows what the tool did and
  `generate-site-data --check` proves it again; a pasted transcript has nowhere to go.
- **Maturity is observed.** `status` (active, draft, deprecated) and `target`
  (guaranteed, advisory) are authored; what a use case is (draft, described, executable,
  verified, guaranteed) is computed from the evidence and the objects it names, when the
  site data is generated. Nobody writes `guaranteed`.
- **Coverage is a doctrine.** `majordomus.use-case-coverage` counts every public
  command, guaranteed claim and MCP tool against the active use cases that name and run
  it; the policy's `use_cases.coverage` says, per class, whether a gap fails `doctor`,
  `check` and `finish` (`required`), is reported (`advisory`) or is not looked at. The
  skeleton makes everything advisory; this repository requires every public command.
- **Impact and scaffolding are commands, not memory.** `majordomus usecase impact` maps
  changed files to the commands, rules, use cases, scenarios and cases they reach;
  `majordomus usecase scaffold` writes a draft for a gap from what the registry, the
  fixture and the claims already know, and a draft never counts.
- **The site is a projection.** Pages, category pages, the coverage page, related use
  cases (a deterministic score over shared claims, rules, commands, category and
  applications) and every count come from `catalogue.json`; `site-check` refuses a
  page whose evidence did not pass or whose links do not resolve.
- **One command, in the shell tool.** `usecase` is a public command of the registry with
  its fixture, its `docs/CLI.md` section and its case, because the scenarios execute the
  shell tool and the site generator is shell; the Rust executable reads the objects and
  serves them, and gains no second runner.

## Alternatives refused

- **Keep the catalogue in `share/` with a `group` field.** Cheap, and it would have
  kept the tool's own use cases in the distribution where no other repository can add
  one, and kept the pages prose. It was shipped as an interim for one day and replaced.
- **A separate runner and evidence format in Rust.** The scenarios run the shell tool;
  a Rust runner would spawn it anyway, and the site generator that consumes the evidence
  is shell. One runner, in the tool the scenarios exercise.
- **Commit the evidence as a tracked artifact.** It would be derived state under
  version control, stale on every commit that changed a message; embedding it in the
  generated site data, which is already checked for freshness, gives the same visibility
  without a second thing to keep in step.
- **Require a use case per guaranteed claim from day one.** Ninety claims, nine use
  cases; the gate would have been red and ignored. The policy makes claims and MCP tools
  advisory until the catalogue reaches them, and required for commands now.

## Consequences

- Adding a command creates its coverage gap the moment the registry names it, and
  `finish` refuses completion until an active use case runs it.
- Adding a use case is one file; the page, the category, the counts, the related links of
  every other page and the coverage table follow from regeneration.
- The site generator executes the scenarios; generation takes a minute longer and is
  honest. Runs are parallel (`MJ_UC_JOBS`); a cache keyed by the inputs is the next step
  if that grows.
- The vendored rule package gained `majordomus.use-case-coverage`; a repository that
  upgrades it gets the doctrine with the skeleton's advisory policy.
- Evidence is normalised (paths, timestamps, ids, hashes, durations, the budget line);
  a behaviour that differs by machine is not hidden and fails the check, which is the
  point.

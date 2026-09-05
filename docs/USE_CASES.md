# Executable use cases

A use case is a task somebody performs with Majordomus, written once as data and proved
against the real tool. It is not a tutorial, not a feature list and not a page: it is one
file under [`.ai/repo/use-cases/`](../.ai/repo/use-cases/) whose front matter names the
commands, rules, claims, responsibilities and applications it touches and carries a
scenario, and whose body says what the situation is and what you are left holding. Every
projection of it is derived: the page, the category it sits in, the links to and from it,
the tally that says which capabilities have a use case, and the evidence a page shows.
The rules behind it are `majordomus.catalogue-integrity` and
`majordomus.use-case-coverage` in the vendored baseline, and the claims are
`use-case-coverage`, `use-case-evidence` and `use-case-impact` in
[`CLAIMS.yaml`](CLAIMS.yaml).

The principle, stated once: anything derivable is derived; anything not derivable has one
canonical home; anything claimed as guaranteed is backed by executable evidence.

## Where things live

| what | where | who owns it |
|---|---|---|
| a use case | `.ai/repo/use-cases/<id>.md` | authored; the manifest's `use-cases` section |
| the categories | `.ai/repo/use-cases/taxonomy.yaml` | authored; presentation only |
| an application | `.ai/repo/applications/<id>.md` | authored; the manifest's `applications` section |
| the contracts | `share/schemas/{use-case,application,taxonomy}.schema.json` | the distribution; the shell allow-lists are generated from them |
| the prepared states | `test/fixtures/commands/setup/<name>.sh`, `stdin/<name>` | the distribution's fixtures, shared with the command demonstrations |
| the evidence | `.ai/local/evidence/use-cases/<id>.json` | written by `majordomus usecase run`; never tracked |
| the site's copy | `site/data/generated/catalogue.json` | written by `scripts/generate-site-data`, which runs the scenarios itself |
| the pages | `site/content/use-cases/**`, `site/content/applications/**` | derived; `rm -rf`'d and rewritten by the generator |

`majordomus init` seeds the two sections with their context documents and the taxonomy,
so the first use case of a repository is one file.

## The use-case object

```yaml
---
id: prove-a-rule-is-enforced          # the file name, stable, the URL slug
kind: use-case
title: 'Prove a rule is actually enforced, not merely written down'
summary: 'One sentence.'
category: policy                       # an id of taxonomy.yaml
status: active                         # active | draft | deprecated (authored)
target: guaranteed                     # guaranteed | advisory: what the author aims at
actors: [maintainer, reviewer]
difficulty: intermediate
commands: [doctrine, doctor]           # bin/majordomus must dispatch each
mcp_tools: [majordomus_repository]     # optional; the executable's registry must project each
doctrines: [majordomus.enforcement-wiring]   # rules with an x-majordomus block
claims: [dispatcher-wiring]            # ids in docs/CLAIMS.yaml
responsibilities: [doctor]             # ids in docs/RESPONSIBILITIES.yaml
applications: [ci-gated-project]       # each names this use case back
scenario:
  setup: installed                     # test/fixtures/commands/setup/installed.sh
  given:
    - 'Majordomus installed; no git hook invokes the tool yet'
  steps:
    - id: find-the-gap
      run: ['doctor']
      note: 'the enforcement the policy declares reaches no hook'
      expect:
        exit: 10
        stdout_contains: ['^FAIL wiring', 'doctor-on-commit']
  then:
    - 'a declared enforcement that nothing invokes is a failure, not a green line'
---

# Situation
...
# Outcome
...
```

What the object does not carry, because it is derived: a command's description or
syntax, a rule's text, a claim's wording, an application's summary, captured output, the
related use cases, the category's title or count, and the observed maturity. The schema
refuses a key it does not declare, and `majordomus usecase validate` refuses a reference
that does not resolve, a category the taxonomy lacks, a setup script or stdin body that
does not exist, a scenario step that runs a command the use case does not list, a body
without `# Situation` and `# Outcome`, an id that is not the file name, a duplicate id, and
an active use case that targets `guaranteed` without a scenario.

## Given, when, then

The scenario is the executable form of the narrative. `setup` names a prepared state (a
shell script under the fixtures, shared with the command demonstrations, so what a use
case starts from is what a command page shows); `given` says it in words. Each step is
one real invocation of `bin/majordomus`, in a disposable repository, in order, in the same
repository: `run` is argv, never a shell string; `stdin` names a body file; `expect`
carries the exit code and, optionally, `stdout_contains`, `stdout_not_contains`,
`files_exist` and `files_contain` (extended regular expressions over the combined
output or a file). `then` says in words what the assertions proved.

## Running and evidence

```bash
majordomus usecase run                     # every scenario; exit 10 when a step fails
majordomus usecase run prove-a-rule-is-enforced --keep   # keep the repository for a look
majordomus usecase show prove-a-rule-is-enforced         # the file, and its last evidence
```

Each run writes `.ai/local/evidence/use-cases/<id>.json`: the use case, the setup, the
result, and for every step the command, argv, stdin, exit code, expected exit code, the
normalised output, every assertion with its result, and the timing. Normalised means the
scenario repository's path, the tool's own path, the home directory, timestamps, task and
session ids, record hashes and durations are replaced by placeholders; nothing behavioural
is. Two runs of one scenario on one tool are byte-identical, which is what lets the site
embed the evidence and `--check` prove it again.

Evidence is local state. The site generator does not read it: it runs every scenario
itself, through the same command, and embeds the result in `site/data/generated/catalogue.json`.
A page that shows what a command printed shows that; a pasted transcript has nowhere to
go.

## Maturity is observed

`status` is authored (`active`, `draft`, `deprecated`) and `target` is authored
(`guaranteed`, `advisory`). What the use case *is* is computed when the site data is
generated, from the evidence and from what it names:

| observed | when |
|---|---|
| `draft` | `status: draft` |
| `described` | active, no scenario |
| `executable` | active, a scenario, no passing evidence in this generation |
| `verified` | active, the scenario passed |
| `guaranteed` | verified, and every claim it names is `guaranteed`, and every doctrine it names is enforced |

Nobody writes `guaranteed` into a use case. A draft can be run (`usecase run` accepts
it) and can be validated, and counts for nothing.

## Coverage

```bash
majordomus usecase coverage [--json] [--check]
```

Every public command of the command registry, every guaranteed claim with a
responsibility, and every MCP tool the executable projects is a target. For each, the
active use cases that name it, the ones whose scenario runs it, and the ones with passing
evidence are counted; the status is `covered`, `partial` (named, never run) or `gap`. The
policy says what a gap means, per class:

```yaml
use_cases:
  coverage:
    commands: required     # a gap fails doctor, check, finish and --check
    claims: advisory       # a gap is reported
    mcp_tools: advisory
```

The skeleton policy makes every class advisory: a fresh repository has no use cases and
is told so, not failed. This repository requires every public command. The doctrine
`majordomus.use-case-coverage` runs the same tally under `doctor`, `check` and, through
the finish key `use_cases_covered` in `verification.finish_requires`, under `finish`: a
required gap refuses completion, naming the capability and the scaffold command.

## Impact

```bash
majordomus usecase impact [--base <ref>] [--json]
```

From the files changed since `--base` (the upstream by default) plus the work tree, the
command names what is affected: the commands (a `lib/<command>.sh`, a responsibility's
files, or everything when `bin/majordomus`, `lib/common.sh`, the registry, the manifest or
the policy changed), the rules (by front-matter id), the use cases (their own file, their
setup script, their stdin body, a command or doctrine they name, a claim when
`docs/CLAIMS.yaml` changed, an MCP tool when the crate changed), the scenarios among
them, and the behavioural cases that declare coverage of an affected command or that a
rule names as its tests. The last line is the command to run next.

## Scaffolding

```bash
majordomus usecase scaffold --missing              # one draft per required gap
majordomus usecase scaffold --for command:pack     # one draft
majordomus usecase scaffold --missing --dry-run
```

A draft is written to `.ai/repo/use-cases/<command>-draft.md` with what is already known:
the command, a category from the command's lifecycle stage, the responsibility whose
command it is, the guaranteed claims of that responsibility, and a scenario taken from the
command's own fixture (its first scenario's setup, argv and expected exit). Everything
else is `TODO`, `status` is `draft` and `target` is `advisory`. The draft validates and
runs; it covers nothing until a person or an agent completes the narrative, tightens the
assertions and sets `status: active`. Nothing here needs a model, and nothing a model
writes is evidence.

## What the site shows

The generator embeds, for every use case: the front matter, the body, the observed
maturity, the evidence of its scenario (every step's command and normalised output), the
resolved commands, rules, claims, responsibilities and applications, and the related use
cases computed from what they share (same workflow of commands, shared claims, shared
rules, shared commands, same category, shared applications, in that weight order; a
use case may `boost` or `suppress` an id under `related`). The categories come from the
taxonomy and carry their counts. `/use-cases/` lists by category; `/use-cases/<id>/` is
the page; `scripts/site-check` refuses a page whose evidence, links or category do not
resolve. Nothing on a page was typed into a template.

## The gates, in one place

| gate | where | what it refuses |
|---|---|---|
| schema | `share/schemas/use-case.schema.json`, `application`, `taxonomy`; the Rust executable at index time | a key nothing reads, a missing required field, a bad id |
| references | `majordomus usecase validate`; `doctor` under `majordomus.catalogue-integrity` | a command, rule, claim, responsibility, application, category, setup or stdin that does not resolve; a one-way application link |
| execution | `majordomus usecase run`; `test/cases/94_use_cases.sh`; the site generator | a step whose exit code or output is not what the scenario says |
| coverage | `majordomus usecase coverage --check`; `doctor`, `check`, `finish` under `majordomus.use-case-coverage` | a required capability no active use case runs |
| generated freshness | `scripts/generate-site-data --check` in CI | site data (evidence included) that differs from a clean regeneration |
| site integrity | `scripts/site-check` | a use-case page missing, a link that does not resolve, a category page without its members |
| impact | `majordomus usecase impact` | nothing; it names what to run |

## Extending it

Add a file. Name what it touches. Give it a scenario. Run `majordomus usecase validate`,
`majordomus usecase run <id>`, `scripts/generate-site-data`, and commit the regenerated
data with it. There is no registry to edit, no index to update, no template to touch, no
count to bump, and the related links of every other page recompute themselves.

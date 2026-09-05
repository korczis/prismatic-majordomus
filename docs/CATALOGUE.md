# The catalogue

Three lists say what the tool is for, from three angles, and every one of them is data
the tool checks:

| list | the question it answers | where |
|---|---|---|
| use cases | what task does a person perform, and does the tool do it | `.ai/repo/use-cases/<id>.md` |
| applications | in what context does the tool fit, and where does it not | `.ai/repo/applications/<id>.md` |
| commands | what does one command do, flag by flag | `share/commands.yaml`, `docs/CLI.md` |

A `why` page is a moment you recognise; a use case is a task you perform; an
application is a context it suits; a command page is one command's surface.

## Why they are data

Written as prose, none of this is checkable: a use case could name a command that was
renamed, a rule that was never enforced, a promise nobody tests, and the page would say
so with confidence. As objects of the layer, every reference resolves or the build
refuses: a command against the dispatch table, a rule against the doctrine registry, a
claim against `docs/CLAIMS.yaml`, a responsibility against `docs/RESPONSIBILITIES.yaml`,
an application against the use cases it names and back, a category against the taxonomy.
`majordomus doctor` applies the same check under `majordomus.catalogue-integrity`, so a
dangling reference cannot reach the published site.

And a use case is more than a description: it carries a scenario the tool executes
against itself, and the page shows that execution. `docs/USE_CASES.md` is the contract;
this document is the map.

## The objects

A use case (`share/schemas/use-case.schema.json`):

```yaml
---
id: prove-a-rule-is-enforced          # the file name, the URL slug
kind: use-case
title: '...'
summary: '...'
category: policy                       # an id of taxonomy.yaml
status: active                         # active | draft | deprecated
target: guaranteed                     # what the author aims at; the maturity is observed
actors: [maintainer, reviewer]
difficulty: intermediate
commands: [doctrine, doctor]           # bin/majordomus dispatches each
doctrines: [majordomus.enforcement-wiring]
claims: [dispatcher-wiring]
responsibilities: [doctor]
applications: [ci-gated-project]       # each names this use case back
scenario:                              # setup, given, steps (run, expect), then
  ...
---
# Situation
# Outcome
```

An application (`share/schemas/application.schema.json`): `id`, `kind: application`,
`title`, `summary`, `fits_when`, `does_not_fit_when` (both required: a catalogue that
only lists fits is marketing), `use_cases` (mutual), `doctrines`, `responsibilities`,
and a body with `# Context`.

The taxonomy (`.ai/repo/use-cases/taxonomy.yaml`): the categories, with a title, a
summary and an order. Membership is each use case's own `category`; a category with no
use case renders an empty page and counts for nothing.

## Two rules

Cross-references are mutual: a use case names its applications and each application
names it back, and a link that resolves one way only is refused.

`does_not_fit_when` is required. An application that only says where it fits is not a
description.

## Extending it

Add a file. `majordomus usecase validate` resolves it, `majordomus usecase run <id>`
executes it, `scripts/generate-site-data` regenerates the data, the route, the category,
the cross-links, the related links of every other page and the coverage table. There is
no template to edit and no list to update; commit the regenerated data with the file.
`majordomus usecase scaffold` writes a draft for a capability no use case covers.

## What is checked, and what is not

Resolution is checked; execution is checked; accuracy of the narrative is a person's
judgement. A use case whose scenario asserts little proves little, and a reviewer reads
the prose against the evidence beside it.

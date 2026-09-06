+++
title = "Say what your work is for, once, and have every surface say it"
description = "Record an operational failure mode as one file and have the command line, the API, MCP and the website answer it without registering it anywhere."
weight = 11
[extra]
id = "say-what-the-tool-is-for"
source = ".ai/repo/use-cases/say-what-the-tool-is-for.md"
category = "extension"
maturity = "described"
+++

## Situation

A repository states what its work is for in a README paragraph, a landing page and a slide,
and the three disagree within a quarter. Worse, the statement is invisible to everything
that is not a person reading: a worker cannot ask what problems this repository exists to
answer, and neither can a script.

## What you run

- `context resolve .ai/repo/why`: the section's contract, composed from the layer's root down
- `knowledge sources`: the three source classes that discover the moments, audiences and areas
- `majordomus why list`, `why show <id>`, `why audiences`, `why areas`, `why diagnose`, `why validate`: the catalogue, from the executable
- over MCP: `majordomus_why` and the tools beside it, and `majordomus://moment/<id>` as a resource

## Scenario

```yaml
setup: why-catalogue
given:
  - 'the layer installed, with the why section and its three source classes seeded by init'
  - 'one moment, one audience and one area written as three files and committed'
steps:
  - id: the-section-explains-itself
    run: ['context', 'resolve', '.ai/repo/why/moments']
    note: 'the section has a contract from the first install, so the first moment is one file'
    expect:
      exit: 0
      stdout_contains: ['ai.repo.why']
  - id: discovered-not-registered
    run: ['knowledge', 'sources']
    note: 'the three files are knowledge because a source class discovered them, not because anything listed them'
    expect:
      exit: 0
      stdout_contains: ['^moment .*the-same-explanation-twice', '^audience .*small-team', '^area .*continuity']
  - id: nothing-was-registered
    run: ['init', '--extend']
    note: 'the section is seeded, not generated per entry: there is nothing left to add'
    expect:
      exit: 0
      stdout_contains: ['nothing to add']
  - id: still-healthy
    run: ['doctor']
    note: 'the layer is real, and the section is part of it'
    expect:
      exit: 0
      stdout_contains: ['doctor: 0 failure']
then:
  - 'the catalogue is a section of the layer from the first install, with its contract'
  - 'adding a moment is adding one file; the executable half of that is proved by test/cases/98_why_catalogue.sh'
```

## Outcome

One Markdown file with schema-valid front matter is the whole act of adding an operational
failure mode. From that moment the command line lists it, the HTTP API returns it, the
OpenAPI document describes its type, MCP serves it, the website gives it a route and puts it
in every filter, its audience and area pages gain it, and the questionnaire gains its
signals — with no registry, navigation file, template list or code change anywhere.

What the catalogue refuses is as important: an unresolved reference is an error with the
nearest candidate offered, a file whose name disagrees with its id is refused, and a derived
value written into a source — a route, a count, a backlink — is a key the schema does not
have.

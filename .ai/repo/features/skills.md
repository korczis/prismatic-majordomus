---
schema: feature/v1
id: skills
kind: feature
title: Skills are procedures as data, discovered and checked, never registered
short_title: Skills
headline: A skill is one directory under the layer; adding it is the whole act, and the command line, the website and MCP all read it from the same file.
summary: A skill is a provider-neutral procedure for one bounded kind of work, front matter under a schema over a body with a purpose, a procedure and an output contract; the source class discovers it, skills check holds every one to its contract, and nothing loads a skill unless the task is about it.
status: stable
weight: 160
featured: false
areas: [documentation]
commands: [skills]
kinds: [skill]
rules: [majordomus.skill-integrity]
docs: [docs/CONCEPTS.md]
adrs: [adr-0007]
claims: [skill-catalogue, skill-check, skill-site-projection]
use_cases: [follow-a-skill-the-repository-defines]
related: [context, declare-once]
tags: [skills]
---

## What it does

`majordomus skills list` shows every skill the source class discovered, `skills show` one
of them, `skills check` every one against its contract; the website's skills section and
the MCP resources are projections of the same files.

## What it does not do

A skill that merely describes is documentation and belongs in the docs. Nothing ranks
skills and nothing loads one by default.

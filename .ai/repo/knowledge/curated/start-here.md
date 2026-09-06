---
schema: knowledge/v1
id: start-here
kind: knowledge
class: convention
title: The documents that explain this repository are read in a fixed order
description: What to read before changing this repository, and which document answers which question.
status: verified
epistemics: decided
date: 2026-09-05
tags:
  - onboarding
  - documentation
provenance:
  origin: authored
  derived_from:
    - file:docs/DESIGN.md
    - file:docs/CLI.md
    - file:docs/SCHEMAS.md
    - file:docs/EXTRACTION_REPORT.md
    - file:docs/DOCTRINE.md
relations:
  - type: documents
    target: file:docs/CLAIMS.yaml
---

# Start here

What to read before changing this repository, in order:

1. `docs/DESIGN.md` — what the tool is and why, the models, the boundaries, and what is
   intentionally absent.
2. `docs/CLI.md` — every command: behaviour, reads, writes, exit-code contract.
3. `docs/SCHEMAS.md` — every file the tool reads or writes, with an example each.
4. `docs/EXTRACTION_REPORT.md` — the evidence behind each decision, and what was rejected.
5. `docs/DOCTRINE.md` — what is enforced, by what, and how the wiring is verified.

`docs/CLAIMS.yaml` is the shortest honest answer to what the tool does today. The rules
this repository holds itself to are under `../../rules/project/`; the ones every
Majordomus-supervised repository holds to are under `../../rules/vendor/majordomus/`.

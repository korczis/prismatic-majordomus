---
schema: knowledge/v1
id: a-render-is-an-experience-target-not-data
kind: knowledge
class: convention
title: "A mockup or render is an experience target, never a specification of data"
description: "When implementing a screen from screenshots, renders or a model's sketch."
status: verified
epistemics: decided
date: 2026-10-18
tags:
  - ui-surfaces
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0089-an-interface-shows-only-what-is-derived-and-unknown-is-never-healthy.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0089-an-interface-shows-only-what-is-derived-and-unknown-is-never-healthy.md
---

# A mockup or render is an experience target, never a specification of data

Concept renders of the Cockpit (2026-09-11) and the 21 reference screens of 2026-10-10 showed numbers such as agents online and healthy badges. A render may reveal a missing capability, which is then built through core, API, MCP, UI and tests; it never supplies a value, a count or a status. A concept with no canonical source is omitted or shown as not measured.

## Sources

- `chatgpt-majordomus:c117#2`

---
schema: knowledge/v1
id: read-the-ui-rules-before-proposing-a-ui
kind: knowledge
class: lesson
title: "A UI proposed before reading the repository's UI rules, tokens and gates violates blocking gates"
description: "Before proposing any change to the site, /ui or /cockpit, especially from an outside model."
status: verified
epistemics: observed
date: 2026-10-18
tags:
  - ui-surfaces
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:docs/UI.md
    - file:docs/MAJORDOMUS_UI.md
---

# A UI proposed before reading the repository's UI rules, tokens and gates violates blocking gates

On 2026-10-10 the owner rejected a landing-page proposal as inadequate and told the assistant to study the repository's UI, Cockpit and UI rules first. The corrected proposal read share/design/tokens.yaml, the shared primitives, cockpit/mod.rs, homepage.toml and scripts/ci/homepage-check, and kept the declared sections, the token-only colours and the asset budget. Read docs/UI.md, docs/MAJORDOMUS_UI.md, the ui-* and homepage rules and the design declaration before proposing libraries, sections or components.

## Sources

- `chatgpt-majordomus:c215#7`

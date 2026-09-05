---
schema: adr/v1
id: adr-0007
kind: adr
title: Skills are data under the layer's skills section, and every surface that shows one is a projection
status: accepted
date: 2026-09-05
tags:
  - skills
  - projections
provenance:
  origin: authored
---

# 7. Skills are data under the layer's skills section, and every surface that shows one is a projection

Extends ADR 2 and ADR 5.
## Context

The manifest has named a `skills` section since the layer existed, and it held a README
and nothing else. The operator asked for the first skill, `repo-review`, and for the
condition that the next one must cost nothing but its own files: no registry, no site
list, no MCP entry, no provider file, no Rust match arm to touch. The source material of
this tool is the warning: a catalogue of five hundred and forty-six agents of which one
was registered with anything that ran, because existing and being registered were two
acts and only one of them was ever done.

## Decision

- **A skill is a directory.** `.ai/repo/skills/<id>/SKILL.md` — YAML front matter under
  `share/schemas/skill.schema.json` (`schema: skill/v1`, `id` equal to the directory,
  integer `version`, `title`, `description`, `status` from a closed set, optional `tags`,
  `related`, `inputs`, `outputs`) over a Markdown body with non-empty `# Purpose`,
  `# Procedure` and `# Output` sections — with optional `examples/*.md`.
- **Discovery is the existing one, declared once.** `share/kinds.yaml` declares the kind
  `skill`; the source classes `skill` and `skill_example` in
  `.ai/repo/knowledge/sources.yaml` (and the skeleton a fresh `init` writes) say which
  tracked files carry it. The Rust executable indexes them as it indexes every kind and
  serves `majordomus://skill/<id>` with no code that names the kind; the shell tool's
  `lib/skills.sh` derives one catalogue from the same class through
  `mj_knowledge_discover`, validating against the allow-list `majordomus generate allow`
  derives from the same schema.
- **Every surface reads the catalogue.** `majordomus skills list|show|check`, the doctrine
  `majordomus.skill-integrity` (dispatched by `doctor` and `watch`), and
  `scripts/generate-site-data`, which writes `site/data/generated/skills.json` and one page
  per skill under `site/content/skills/` and replaces that directory whole. The site's
  templates render from those two files and the section's own pages; the navigation names
  the section, never a skill.
- **Drift fails.** `generate --check` refuses a stale allow-list or registry dataset;
  `generate-site-data --check` refuses stale site data; `skills check` and `doctor` refuse
  a skill that breaks its contract, name the file and every reason, and report the counts
  of what they examined; a repository with no skills is a `WARN`, not a pass.
- **Nothing is registered by name.** No file in the repository lists skills by hand. The
  test `test/cases/95_skills.sh` adds, breaks, renames and removes a skill and asserts that
  the command, doctor, the Rust index over MCP, the site data and the built pages follow,
  and that no page outlives its file.

## Consequences

Adding a skill is writing its directory and tracking it. The cost of the decision is in
the contract: a skill that lacks a section or names a related skill that does not exist
fails `doctor` for everyone, which is intended. Two readers (shell and Rust) parse the
same front matter with the same schema; the shell reads the schema's projection, the Rust
executable reads the schema, and the case proves they refuse the same file for the same
reason. What is not decided here: which skill a task should load (the generated
instructions say "the ones the task is about"), and whether a skill is any good.

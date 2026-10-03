---
schema: context/v1
id: ai.repo.skills
kind: context
title: Skills
description: Reusable provider-neutral operational procedures, loaded only when the task is about them.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
children:
  require_contract: false
---

# Skills

Reusable, provider-neutral operational procedures: how to do one bounded kind of work in
this repository well, written so that any capable worker can follow it. A skill is loaded
when the task is about what the skill covers, never by default. Keep each one short and
executable; a skill that merely describes is documentation and belongs in `docs/`.

A skill's directory is an instance of the kind, not a section of the layer, so the
directories below this one owe no contract of their own (`children.require_contract:
false`): the format they follow is the one stated here, once, and a copy of it beside
every `SKILL.md` is the duplication this contract exists to prevent.

## The contract

A skill is one directory `<id>/` in this section holding `SKILL.md`, and nothing else
registers it: the source class `skill` in `../knowledge/sources.yaml` discovers every
`*/SKILL.md` here, and that one declaration is what `majordomus skills`, `doctor`, the
website and the MCP server all read. Adding the directory and tracking it in git is the
whole act of adding a skill; removing it removes the skill from every surface.

`SKILL.md` is YAML front matter over a Markdown body. The front matter satisfies
`share/schemas/majordomus/skill/skill.v1.schema.json` of the tool distribution — an unknown key is an error:

```yaml
---
schema: skill/v1          # the contract this file follows
id: repo-review           # lower-case letters, digits, hyphens; must equal the directory name
version: 1                # an integer; raise it when the procedure changes materially
title: Repository review
description: One sentence; shown by every listing.
status: active            # draft | active | deprecated
tags: [review, evidence]  # optional
related: [implement]      # optional; every id must name a skill in this section
inputs:                   # optional; what the worker must have before starting
  - a checkout with git history available
outputs:                  # optional; what the procedure leaves behind
  - findings with severity, evidence and the smallest fix
provenance:               # optional; only when the concept was studied elsewhere first
  origin: prior-art
  ledger: import-2026-09-09#2   # the entry in the local, gitignored .ai/local/imports/<date>.yaml
  decision: adapted             # adapted | reimplemented | merged
---
```

`provenance` is an opaque marker and nothing more: the source it points at is recorded only in
the local import ledger, never in a committed file, a commit message or a fixture.

The body is the procedure and carries three level-one sections, each non-empty:
`# Purpose` (what the skill establishes and what it is not for), `# Procedure` (the
ordered steps, with what to read and what to look for), and `# Output` (the contract of
what the worker produces). Any other section is allowed; `# When to use` is conventional.
The body names commands and files, never a provider or a provider's tool.

`examples/*.md` beside `SKILL.md` are optional: each opens with a level-one heading (its
title) and shows an invocation intent in provider-neutral terms. They are tracked as
documents, rendered on the skill's page, and checked for their heading.

## Adding one

```bash
mkdir -p .ai/repo/skills/my-skill/examples
$EDITOR .ai/repo/skills/my-skill/SKILL.md      # the front matter and the three sections above
git add .ai/repo/skills/my-skill
majordomus skills check                        # every reason, or the counts of what passed
majordomus skills show my-skill
scripts/generate-site-data                     # the site's skills.json and page (this repository)
```

`majordomus skills list` shows every skill in discovery order; `skills check` (and
`doctor`, through the doctrine `majordomus.skill-integrity`) refuses an unknown key, a
wrong schema version, a non-integer version, a status outside the closed set, an id that
is not the directory name, a missing or empty required section, two skills with one id,
two skills whose descriptions do not tell them apart, a `related` id that names nothing,
and an example without a heading. A repository with no skills is reported as such, never
passed silently.

A description is what a worker selects on, so two skills that describe themselves the same
way cannot both be chosen and the one that was wanted is unreachable. Descriptions are
compared folded to lower case, with runs of whitespace collapsed and trailing sentence
punctuation dropped: the difference has to be in what the description says, not in how it
is typed.

## A skill is a capability only when something proves it

A valid file makes a skill exist; it does not make it a capability. Four facts are derived
about every skill on every read, and none of them is written anywhere
(`project.skills-are-proven-capabilities`, ADR 0076):

| fact | derived from |
|---|---|
| tested | a case or crate test naming it with `majordomus-skill: <id>` on a comment line, and the evidence ledger's latest run of that test |
| documented | the tracked page `site/content/skills/<id>.md` |
| enforced | the doctrine whose validator is `skills`, and a CI gate running `skills verify` that a change to the skill selects |
| used | a workflow, prompt, profile, provider template, recipe, CI workflow or root bootstrap referencing `majordomus://skill/<id>` or the path of its `SKILL.md` |

```bash
majordomus skills status             # every skill with its standing: proven, partial, orphan, invalid, not_required
majordomus skills explain my-skill   # each naming test and its evidence, the page, the gates, every invocation
majordomus skills verify             # exit 10 on an orphan, a failing run, a broken contract, a binding to nothing
```

These are commands of the Rust executable (`bin/majordomus-cli`), also served as
`GET /api/v1/skills`, `/api/v1/skills/explain?id=` and `/api/v1/skills/verify`, and as the MCP
tools `majordomus_skills`, `majordomus_skill_explain` and `majordomus_skills_verify`. An active
skill no test names or nothing invokes is an orphan and fails `skills verify`, `doctor` and the
`skills-proof` gate; so does a test or an invocation naming a skill that does not exist.
Evidence that is not current, a missing page and a missing gate are warnings. A draft or
deprecated skill owes none of it. Adding an active skill therefore means adding, in the same
change, the test that names it and the surface that invokes it.

## What is derived, and must not be edited

`share/allow/skill.txt` (from the schema, by `majordomus generate allow`), the MCP
resources `majordomus://skill/<id>` and the entries `majordomus_list` returns for the
kind, `site/data/generated/skills.json` and `site/content/skills/` (by
`scripts/generate-site-data`), and the pages under `/skills/` on the website. Each is
regenerated from the files here, and the drift checks CI runs (`generate --check`,
`generate-site-data --check`) fail while one is behind. No list of skills exists anywhere
by hand; the inventory is this directory.

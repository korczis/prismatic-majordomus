---
schema: adr/v1
id: adr-0076
kind: adr
title: A skill is a capability only when something proves it — tested, documented, enforced and used are derived, never authored
status: proposed
date: 2026-09-17
tags:
  - skills
  - evidence
  - derivation
provenance:
  origin: authored
related:
  - rule:project.skills-are-proven-capabilities
  - rule:majordomus.skill-integrity
  - file:apps/majordomus-cli/src/skill/mod.rs
  - file:apps/majordomus-cli/src/capability/builtin/skills.rs
  - test:test/cases/391_skills_are_proven_capabilities.sh
  - test:test/cases/95_skills.sh
---

# 76. A skill is a capability only when something proves it

Extends ADR 7.

## Context

ADR 7 made skills data: one `SKILL.md` per directory, discovered by a source class, validated
against a generated schema, projected to the site and served as `majordomus://skill/<id>`. That
settled what a skill *is*. It left open what the repository can *show* about one. Three skills
existed, all valid, and measured against the tree none of them was named by any test, and two
of them were referenced by nothing a worker follows: a prompt, a workflow, a recipe. The
catalogue was complete and every entry in it was unproven — the failure a hand-maintained
registry has, reached one level up, through data that is correct and says nothing about use.

The obvious remedy, a `tested: true` or `status: verified` field in the front matter, is the
failure restated: an authored claim no gate can check, which goes stale the day the test that
justified it is deleted.

## Decision

**Four facts about a skill are derived on every read, by one derivation in the executable
(`crate::skill::Skills`), and no file states any of them.**

- **tested** — a test a runner owns names the skill with `majordomus-skill: <id>` on a comment
  line, and the evidence ledger's latest run of that test is judged by the same function that
  judges a rule's named test (`rules::test_state`): proven, inputs unchanged, stale, failing,
  not run. A marker inside a string is data and binds nothing, so a test that writes fixtures
  does not bind itself to them.
- **documented** — the site's page for the skill is a tracked file.
- **enforced** — a dispatched rule validates skills (its validator is `skills`) and a CI gate
  whose command runs `skills verify` is selected by a change to the skill's path. The gate is
  recognised by what it runs, not by its id.
- **used** — an invocation surface references the skill by `majordomus://skill/<id>` or by the
  path of its `SKILL.md`: the layer's workflows, prompts and profiles (from the manifest's
  sections), provider templates, `justfile` and `.just/`, CI workflows and the root bootstraps.
  Documentation is not an invocation; a page that describes a skill does not make anyone
  follow it.

The derivation is the `skills` capability module — `skills.status`, `skills.explain`,
`skills.verify` — declared once and projected to the command line, HTTP, MCP and OpenAPI. The
module is `skills` because `skill` is the declarative kind.

**An active skill no test names, or nothing invokes, is an orphan and a failure.** So is a test
or an invocation naming a skill that does not exist, a failing recorded run, and a skill file
the index refuses (reported as `contract`, and still listed as `invalid` so a listing never
shrinks). Evidence that is not current, a missing page and a missing gate are warnings: each is
a debt with the command that settles it, and evidence in particular is a local artifact that
every edit of a test makes stale by construction, so refusing on it would refuse every commit
that improves a test. A draft or deprecated skill owes nothing.

**One judge per fact.** The skill contract (schema, sections, duplicate descriptions, related
ids, examples) stays with `lib/skills.sh` and the index, as ADR 7 left it. Proof is judged only
by `skills.verify`; the doctrine validator `mj_validate_skills` reads that verdict and reports
each finding through `doctor` and `watch`, and says it is unknown — never a pass — when the
executable is not built. The CI gate `skills-proof` runs the same command.

**Provenance is an opaque marker.** A skill whose concept was studied outside this repository
carries `provenance: {origin: prior-art, ledger: import-<date>#<n>, decision: adapted |
reimplemented | merged}`, closed and pattern-checked by the schema. The mapping to the source —
repository, path, revision, notes — lives in the gitignored `.ai/local/imports/<date>.yaml`. A
public repository names no private material, and a later reader can still tell imported from
native.

**Derived status is not written into committed projections.** `site/data/generated/skills.json`
stays the catalogue it was. The standing depends on the ledger and on the diff against the
commit a run was recorded at, so a projection of it would change on every commit and every
recorded run, and derivation would never converge.

## Alternatives rejected

- **An authored `tested`/`verified` field**, checked against the tree. It is two sources for
  one fact, and the checker is the derivation anyway.
- **Binding tests to skills from the skill's front matter** (`tests: [...]`). A rule does it
  that way, and a skill could; but the thing that exercises a procedure is the test, and a
  skill author listing tests they did not write is the direction that goes stale. The marker
  lives where the proof is; the derivation reads it from there.
- **Counting any mention as use.** Documentation, the skill's own examples and other skills'
  `related` lists all mention skills; none of them makes a worker follow one. Use is restricted
  to the surfaces that instruct a worker.
- **Refusing on evidence that is not current.** See above; it would make improving a test
  impossible to commit, and it is what `evidence-check` and a recorded run already measure.
- **Showing the standing on the public site.** Not a stable projection (see the decision); the
  standing is read live from the executable's surfaces instead.

## Consequences

- Every active skill of this repository is named by `test/cases/391_skills_are_proven_capabilities.sh`
  and `apps/majordomus-cli/tests/skills.rs`, each of which reads the skill from the repository
  and fails when it is not a valid, invoked capability that counts the test among its own, and
  fails when an active skill is left out of its marker line.
- The existing skills gained their first invocations: the task lifecycle workflow names
  `implement`, `assess-before-deleting` and `report-verification-state`; the review prompt names
  `repo-review` and `pack-for-review`; the handover prompt names `report-verification-state`;
  `deploy-site` was already invoked by its recipe.
- The first wave of adopted concepts — `assess-before-deleting`, `report-verification-state`,
  `pack-for-review` — carries provenance markers; a fourth concept, planning before changing
  working code, was merged into `implement`, which already required it, and adds no skill.
- A fixture skill in a behavioural case now needs a naming test and an invocation for `doctor`
  to pass when the executable is built; `95_skills` carries both.
- `rules::test_state` and `rules::ledger_diffs` are the shared judgement; a change to how a
  recorded run is judged changes rules and skills together.

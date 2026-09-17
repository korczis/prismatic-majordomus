---
id: project.skills-are-proven-capabilities
version: 1
kind: rule
title: A skill is a capability only when something proves it
description: An active skill is named by a test with evidence behind it and invoked by a surface that follows it; whether it is tested, documented, enforced and used is derived from the repository on every read and never written down, and an active skill that no test names or nothing invokes is refused as an orphan.
statement: Every active skill is named by a test (`majordomus-skill: <id>` on a comment line of a case or crate test) and referenced by an invocation surface (`majordomus://skill/<id>` or the path of its SKILL.md in a workflow, prompt, profile, provider template, recipe or CI workflow); its standing is derived by `skills.status` and refused by `skills verify`, and no file states it.
status: active
class: blocking
depends_on: [project.derived-once@1]
tags: [skills, evidence, derivation]

x-majordomus:
  tests: [test/cases/391_skills_are_proven_capabilities.sh, apps/majordomus-cli/tests/skills.rs]
---

# Rationale

A skill file makes a procedure *exist*. Existence is the weakest thing a catalogue can say
about an entry, and a catalogue that says only that is a registry whose entries rot: nothing
exercises the procedure, nothing points a worker at it, and nobody can tell from the
repository whether it still describes what the tools do. Writing `tested: true` into the
file would restate the problem as data — an authored claim that no gate can check.

So the four facts a reader needs are derived where they are true: the tests that name the
skill and the runs the evidence ledger recorded for them, the page the site projects, the
doctrine and CI gate that hold the file, and the surfaces that reference it (ADR 0076).

# Required behaviour

- An **active** skill is named by at least one test that a runner owns, with the marker
  `majordomus-skill: <id>` on a comment line. A marker inside a string is data and binds
  nothing.
- An active skill is referenced by at least one invocation surface: a workflow, prompt or
  profile of the layer, a provider template, `justfile` or `.just/`, a CI workflow, or a root
  bootstrap, by `majordomus://skill/<id>` or by the path of its `SKILL.md`.
- A test or an invocation that names a skill the repository does not hold is a defect: a
  binding to nothing reads as proof.
- Status is derived. `tested`, `documented`, `enforced`, `used` and the standing are answered
  by `skills.status` and `skills.explain` over every surface from one derivation; no skill
  file, catalogue or site page states them.
- A skill whose concept was studied outside this repository carries only the opaque
  `provenance` marker (`origin`, `ledger: import-<date>#<n>`, `decision`); the mapping to its
  source stays in the local import ledger and is never committed.
- A draft or deprecated skill owes none of this.

# Failure behaviour

`skills verify` exits 10 and names each failure: `untested` or `unused` (an orphan),
`failing` (the latest recorded run of a naming test did not pass), `contract` (the index
refused the file), `unknown_skill_in_test`, `unknown_skill_invoked`. The same findings reach
`majordomus doctor` through the doctrine `majordomus.skill-integrity`, and the CI gate
`skills-proof` runs the command on every change to a skill, an invocation surface or the
derivation. Evidence that is not current, a missing page and a missing gate are warnings,
each with the command that settles it.

# Verification

```sh
majordomus skills verify
bash test/run.sh 391_skills_are_proven_capabilities
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --test skills
```

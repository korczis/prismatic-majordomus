+++
title = "Every skill is validated against the allow-list generated from its schema, its directory, its sections and its references, a violation names the file and every reason, and a repository with no skills is reported rather than passed"
description = "majordomus skills check, and doctor and watch through the doctrine majordomus.skill-integrity, examine every discovered skill: no front-matter key outside the schema, schema: skill/v1, an integer version of at least 1, a status from draft, active, deprecated, an id equal to the directory name, and non-empty # Purpose, # Procedure and # Output sections. Across the catalogue: no two skills claim one id, every related id names a skill, and every tracked example opens with a level-one heading. Every violation is a FAIL naming the file and every reason at once, and the command exits 10. The result ends with what was examined — skills, examples, references — and a repository with no skills gets a WARN, never an OK over nothing."
weight = 131
[extra]
claim_id = "skill-check"
status = "guaranteed"
source = "docs/claims/skill-check.md"
+++
{% raw %}

## What it means

`majordomus skills check`, and `doctor` and `watch` through the doctrine `majordomus.skill-integrity`, examine every discovered skill: no front-matter key outside the schema, `schema: skill/v1`, an integer `version` of at least 1, a `status` from `draft`, `active`, `deprecated`, an `id` equal to the directory name, and non-empty `# Purpose`, `# Procedure` and `# Output` sections. Across the catalogue: no two skills claim one id, every `related` id names a skill, and every tracked example opens with a level-one heading. Every violation is a `FAIL` naming the file and every reason at once, and the command exits 10. The result ends with what was examined — skills, examples, references — and a repository with no skills gets a `WARN`, never an `OK` over nothing.

## How it works

`mj_skills_examine` in `lib/skills.sh` walks the catalogue once and calls a reporter for each violation; `skills check` passes the plain `FAIL` reporter, the doctrine passes `mj_doctrine_fail`, which takes the rule's class and, under `watch`, becomes `DRIFT` with exit 11. The unknown-key test reads `share/allow/skill.txt`, which `majordomus generate allow` derives from the schema, so the shell tool refuses exactly the keys the Rust executable's JSON Schema validation refuses. `scripts/generate-site-data` runs the same examination and exits 10 before writing a byte when any skill fails.

## How to see it

```bash
majordomus skills check
# OK   skill       1 skill(s) — every one parses, matches its directory and carries its sections
# OK   skill       5 reference(s) — every related id and every example resolves
# skills: 1 discovered, 1 valid; examples: 5; references: 5 checked; failures: 0
printf 'owner: me\n' >> .ai/repo/skills/repo-review/SKILL.md      # after the front matter's last key
majordomus skills check          # FAIL skill .ai/repo/skills/repo-review/SKILL.md — unknown front-matter key(s): owner ...; exit 10
majordomus doctor | grep skill   # the same finding, from the doctrine
git checkout .ai/repo/skills/repo-review/SKILL.md
```

## What it does not cover

It validates structure and references, not whether a procedure is a good one; a well-formed skill that gives poor advice passes. It does not run examples.

## Why it exists

A skill is loaded only when a task is about what it covers, so a broken one is discovered by the worker that needed it, mid-task. Checking every skill on every `doctor` run moves that discovery to the tree, where the file is named and the fix is one edit.
{% endraw %}

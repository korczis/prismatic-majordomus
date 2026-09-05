---
id: migrate-from-the-old-layout
kind: use-case
title: 'Move a repository from the pre-.ai layout to the layer'
summary: 'See what a migration would move, move it with a backup, and prove the result is a layer the tool reads.'
category: adoption
status: active
target: guaranteed
actors: [maintainer]
difficulty: basic
commands: [migrate, doctor]
doctrines: [majordomus.ai-layout-integrity, majordomus.layout-integrity]
claims: [legacy-migration, ai-layer-manifest]
responsibilities: [layer, doctor]
applications: [repository-with-authored-governance]
scenario:
  setup: legacy-layout
  given:
    - 'project data under .majordomus/, the pre-.ai layout, and no manifest'
  steps:
    - id: plan-it
      run: ['migrate', '--dry-run']
      note: 'every move named, nothing written'
      expect:
        exit: 0
        stdout_contains: ['^migrate: \.majordomus/ \(pre-\.ai layout\) -> \.ai/', 'move  \.majordomus/policy\.yaml -> \.ai/repo/policy\.yaml']
    - id: do-it
      run: ['migrate']
      note: 'the files move, the old directory is backed up, the manifest is written'
      expect:
        exit: 0
        stdout_contains: ['^migrated: \.ai/ is the layout']
    - id: nothing-left
      run: ['migrate']
      note: 'a second migration says the layer is already there and moves nothing'
      expect:
        exit: 0
        stdout_contains: ['^already migrated: \.ai/manifest\.yaml is present']
  then:
    - 'the migration is explicit, dry-runnable and backed up'
    - 'afterwards every command reads the layer and refuses the old path by name'
---

# Situation

A repository installed Majordomus before the `.ai/` layer existed and keeps its policy under `.majordomus/`. Every command refuses to read it, and the maintainer wants to know what will move before anything does.

# Outcome

`migrate --dry-run` lists every move; `migrate` performs it with a backup and writes the manifest; a second run says there is nothing to migrate, and `doctor` proves the layer is real.

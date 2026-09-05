---
id: follow-a-skill-the-repository-defines
kind: use-case
title: 'Follow a procedure the repository defines, and add another one'
summary: 'Read the repository''s skills, take one as it is written, and add a new one by writing its file — no registry, no code, and a check that says what it examined.'
category: extension
status: active
target: guaranteed
actors: [maintainer, agent]
difficulty: basic
commands: [skills]
doctrines: [majordomus.skill-integrity]
claims: [skill-catalogue, skill-check]
responsibilities: [layer, doctor]
applications: [repository-with-authored-governance, repository-opened-in-ai-clients]
scenario:
  setup: installed-skill
  given:
    - 'a repository whose skills section holds one skill directory with SKILL.md and one example, tracked in git'
  steps:
    - id: list
      run: ['skills', 'list']
      note: 'the inventory is the directory: what is tracked is what is listed'
      expect:
        exit: 0
        stdout_contains: ['^release-notes +active +v1 ']
    - id: read-one
      run: ['skills', 'show', 'release-notes']
      note: 'the canonical path, then the file as written — the procedure, not a paraphrase'
      expect:
        exit: 0
        stdout_contains: ['^\.ai/repo/skills/release-notes/SKILL\.md$', '^# Procedure$']
    - id: machine-readable
      run: ['skills', 'show', 'release-notes', '--json']
      note: 'the same skill for a program: metadata, provenance, examples and the body'
      expect:
        exit: 0
        stdout_contains: ['"uri":"majordomus://skill/release-notes"', '"path":".ai/repo/skills/release-notes/SKILL.md"', '"valid":true']
    - id: check
      run: ['skills', 'check']
      note: 'a clean result states what it examined; a repository with no skills is reported, never passed'
      expect:
        exit: 0
        stdout_contains: ['^OK   skill', 'skills: 1 discovered']
    - id: refuse-unknown
      run: ['skills', 'show', 'nosuch']
      note: 'a skill that does not exist is named, with the command that lists what does'
      expect:
        exit: 12
        stdout_contains: ["no skill 'nosuch'", 'majordomus skills list']
  then:
    - 'the listing, the catalogue the site renders and the resources the MCP server serves all come from the same discovery'
    - 'adding a skill is adding its directory; removing it removes the skill from every surface'
---

# Situation

A repository has procedures that its workers, human and machine, are supposed to follow: how a review is done here, how a change is carried out, how the site is deployed. They live in somebody's head, in a wiki nobody updates, or pasted into a provider's own configuration file where the next tool cannot read them.

# Outcome

Each procedure is one directory under the layer's skills section holding `SKILL.md`, discovered because the file exists. `skills list` is the inventory, `skills show` is the procedure as written with the path it came from, and `skills check` reports what it examined — skills, examples and references — so a pass is evidence rather than a word. The same catalogue is what the website's skills pages and the MCP resources are projected from, so no surface keeps its own list.

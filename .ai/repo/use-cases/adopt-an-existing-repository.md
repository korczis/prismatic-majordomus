---
id: adopt-an-existing-repository
kind: use-case
title: 'Adopt a repository that already has its own rules'
summary: 'Install the supervisory layer into a repository with a hand-written CLAUDE.md, without the tool taking ownership of a file somebody else wrote.'
category: adoption
status: active
target: guaranteed
weight: 1
actors: [maintainer]
difficulty: basic
commands: [init, update, doctor]
doctrines: [majordomus.projection-integrity, majordomus.enforcement-wiring, majordomus.policy-integrity]
claims: [region-projection, no-silent-overwrite, projection-fingerprint, dispatcher-wiring]
responsibilities: [projection, policy, doctor]
applications: [repository-with-authored-governance]
scenario:
  setup: authored-governance
  given:
    - 'a hand-written CLAUDE.md people follow, committed'
    - 'Majordomus installed, the policy projecting CLAUDE.md in region mode'
    - 'the two enforcements wired as git hooks'
  steps:
    - id: refuse-overwrite
      run: ['init']
      note: 'a second init never overwrites a layer that is there'
      expect:
        exit: 15
        stdout_contains: ['already exists']
    - id: render-the-region
      run: ['update']
      note: 'the generated region is appended inside the authored file and stamped; the authored text is untouched'
      expect:
        exit: 0
        stdout_contains: ['^create CLAUDE.md$']
        files_contain:
          - path: CLAUDE.md
            pattern: 'Hand-written governance'
          - path: CLAUDE.md
            pattern: '^<!-- majordomus:begin [0-9a-f]{12} [0-9a-f]{16} -->$'
    - id: prove-it-holds
      run: ['doctor']
      note: 'the projection matches its stamp and the declared enforcement is invoked by the hooks'
      expect:
        exit: 0
        stdout_contains: ['^OK   projection', 'doctor: 0 failure']
  then:
    - 'the authored text is byte for byte what it was'
    - 'the region carries the policy hash and its own content hash'
    - 'doctor is green because the hooks exist and call the tool'
---

# Situation

The repository already has a governance root — a CLAUDE.md, an AGENTS.md, a contributing guide people actually follow. A tool that regenerates that file wholesale would overwrite months of authored judgement, so the usual answer is to not adopt the tool at all.

# What you run

- `init`: writes .ai/ and refuses if an installation is already there
- `update`: renders the projection into the region between the markers, leaving the rest of the file alone
- `doctor`: proves the projection matches its own stamp and the declared enforcement is actually invoked

# Outcome

The authored text stays authoritative and the generated section sits inside it, stamped like any other projection. An edit outside the markers is never drift; an edit inside them is caught.

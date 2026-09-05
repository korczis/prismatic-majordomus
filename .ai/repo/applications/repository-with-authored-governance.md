---
id: repository-with-authored-governance
kind: application
title: 'A repository whose rules were written by people'
summary: 'There is already a CLAUDE.md, an AGENTS.md or an equivalent that somebody wrote and others follow.'
weight: 1
status: active
fits_when:
  - 'The authored text should stay authoritative and a generated section can sit inside it'
  - 'Enforcement already exists as git hooks, including dispatchers over a hook.d directory'
  - 'Rules are written down and nobody can currently prove which of them run'
does_not_fit_when:
  - 'The repository wants a single generated instruction file and is happy to hand ownership over'
  - 'There is no governance to supervise yet, in which case start with the shipped skeleton instead'
use_cases: [adopt-an-existing-repository, prove-a-rule-is-enforced, find-out-what-drifted, extend-what-the-executable-serves, add-a-use-case-and-prove-it, migrate-from-the-old-layout, read-the-rules-the-tool-applies, classify-what-belongs-in-the-context, keep-the-bootstrap-thin-and-within-budget, trace-a-change-to-the-context-it-affects, trust-the-policy-before-reading-it, follow-a-skill-the-repository-defines]
doctrines: [majordomus.projection-integrity, majordomus.enforcement-wiring]
responsibilities: [projection, policy, doctor]
---

# Context

The governance root is authored, not generated. It carries judgement that no tool produced and none should overwrite. Adoption usually stalls here, because the obvious integration is for the tool to own the file.

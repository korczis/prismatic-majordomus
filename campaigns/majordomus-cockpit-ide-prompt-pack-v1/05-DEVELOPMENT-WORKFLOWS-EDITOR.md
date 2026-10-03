# Prompt 05 — Development Workflows, Source Inspection, Editing, Diff, Tests

## Mission

Make Cockpit useful for actual development while preserving Majordomus governance.

Do not reinvent VS Code. The goal is a repository-aware controlled development surface, not a browser text editor arms race.

## Required workflows

Expose canonical actions for:

- inspect file/source;
- resolve context for path;
- show related rules/doctrines/ADRs/tests/issues;
- show git diff for path;
- run focused tests;
- run broader quality gates;
- format/lint/check;
- generate derived artifacts;
- detect docs drift;
- inspect impacted use cases;
- inspect capability impact;
- start task/mandate;
- open/create canonical worktree;
- prepare commit;
- verify "done".

## File explorer

If added:
- derive from repository state;
- respect ignore rules and path policy;
- show git status;
- show context boundary/README chain;
- show relevant object/kind metadata;
- support search;
- never become a second source index if an index already exists.

## Editing

Before implementing in-browser editing, inspect existing write semantics and governance.

Any edit path must:
- require an active task/scope if governance requires it;
- operate only inside the intended worktree/repo;
- expose before/after diff;
- use atomic writes;
- refuse generated files whose canonical source should be edited instead;
- explain the canonical owner;
- trigger/offer appropriate focused tests and generation;
- preserve file permissions/encoding;
- record actor/session/execution provenance;
- never bypass collision/worktree/scope rules.

If safe editing is too broad for this phase, implement source inspection + "open/reproduce locally" and canonical mutation capabilities first. Do not fake editing with a textarea that can corrupt the repo.

## Test UX

A test result should render:
- command/capability;
- scope;
- pass/fail/skip;
- duration;
- failing cases;
- logs;
- relevant source links;
- rerun action;
- regression linkage where known.

Test inventory must be derived from canonical tooling where feasible.

## Diff UX

Render:
- working tree diff;
- staged diff;
- branch vs base;
- commit diff;
- PR diff if integration exists.

Do not duplicate git semantics in JavaScript.

## Acceptance

Add integration tests proving:
- editing/mutation respects worktree/task scope;
- generated file edits are rejected with canonical-source guidance;
- focused test execution produces a canonical execution;
- diff output is stable and escaped;
- file-related context resolution matches CLI/MCP/API answers.

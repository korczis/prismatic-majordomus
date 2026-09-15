# Prompt 08 — Issues, Milestones, Git, Worktrees, Planning, Release Linkage

## Mission

Make Cockpit a real planning/development surface by connecting work to canonical Git/GitHub/project state.

First audit what is already implemented. Do not assume GitHub is canonical for everything.

## Unified work graph

Where repository philosophy supports it, derive a graph linking:

```text
milestone
  ↕
issue
  ↕
task / mandate / session
  ↕
branch / worktree
  ↕
commit
  ↕
PR
  ↕
capability / file / test / docs
  ↕
version / changelog / release / deploy
```

Links must come from canonical metadata/conventions/APIs, not fuzzy UI heuristics.

## Cockpit planning views

Provide derived views for:
- milestones and completion;
- issues and status;
- current work attached to issue/milestone;
- branches and worktrees;
- ahead/behind;
- dirty changes;
- unpushed commits;
- PR state;
- CI state;
- linked release/version;
- orphan work;
- conflicting worktrees;
- stale work.

## Bidirectional sync

If the repository already intends issue↔milestone or local plan↔GitHub sync, make it robust:
- explicit ownership of each field;
- deterministic conflict resolution;
- idempotent sync;
- dry-run;
- provenance;
- timestamp/version/ETag where available;
- retry/error handling;
- no duplicate issue creation;
- tests with fixtures/mocks;
- rate-limit awareness.

Do not advertise bidirectional sync unless it is actually implemented and verified.

## Worktrees

Reuse canonical worktree topology:
- trunk location;
- branch canonical worktree;
- misplaced worktree diagnostics;
- migration plan;
- create/migrate actions;
- collision checking;
- dirty-state protections.

Expose actions via canonical capabilities and therefore CLI/API/MCP/Cockpit where safe.

## Git mutation safety

Any browser mutation must:
- be explicit;
- show affected branch/worktree;
- obey task scope;
- refuse destructive ambiguous actions;
- return structured result;
- record execution;
- never hide failure.

## Acceptance

Prove:
- issue/milestone state shown in Cockpit equals API/CLI data;
- worktree topology matches git;
- orphan/unpushed work diagnostics work;
- no planning inventory is duplicated in frontend/docs;
- representative linkage propagates into generated docs/graphs if project policy requires.

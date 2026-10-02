# Prompt 14 — Integration, Cleanup, Versioning, Changelog, Release, Deployment

## Mission

Finish the work as a repository change, not a local demo.

## Pre-integration audit

Inspect:
- current branch/worktree;
- all worktrees;
- peer claims;
- open related PRs;
- unpushed commits;
- generated drift;
- current CI;
- milestone/issues;
- version/changelog state.

Do not overwrite concurrent work.

## Consolidate

- merge/rebase according to repository policy;
- remove obsolete duplicate implementations;
- delete stale frontend catalogues;
- delete handwritten transport duplication;
- remove temporary migration code when no longer required;
- run formatting;
- run complete canonical quality gates.

## Versioning

Use repository semantic version policy.

Derive whether the change is patch/minor/major from canonical versioning doctrine/tooling, not intuition.

Update version through canonical source only.
Ensure all displayed versions are derived.

## Changelog

Generate/update changelog using repository tooling.
Link:
- issues;
- milestones;
- PRs;
- capabilities;
- docs where policy supports it.

## Release

Run canonical release flow:
- build artifacts;
- package;
- checksums/signatures if existing;
- release notes;
- tag;
- publish.

Do not invent release mechanics if repository does not currently have them. If absent, record the gap instead of claiming deployment.

## GH Pages deployment

Trigger/verify site deployment according to actual workflow.

## Runtime deployment

If Majordomus has deployable service/package targets, deploy only through existing canonical workflow and verify.

## Post-deploy smoke

Verify:
- CLI version;
- server version;
- Cockpit route;
- OpenAPI;
- Swagger;
- representative API call;
- MCP startup/call;
- generated docs;
- GH Pages;
- peer connection;
- execution runner.

## Repository hygiene

Before finish:
- intended tree clean;
- no untracked generated junk;
- no stale worktree created for this task;
- no unpushed commits;
- no forgotten PR if workflow requires integration;
- issue/milestone updated;
- handover/session context updated;
- finish invariant passes.

Do not call it done until this evidence exists.

# Instructions for Codex

## Mandatory preflight for every prompt

Before implementation work, use maximum repository context and perform this preflight:

1. Synchronize repository/worktree according to Majordomus freshness rules.
2. Read repository instructions and governance.
3. Resolve or create the current Majordomus session automatically.
4. Resolve relevant handover/predecessor state automatically.
5. Resolve current issue/milestone/task state.
6. Inspect current provider/model/runtime state.
7. Inspect active peers/worktrees if tooling exists.
8. Compile relevant context before proposing or changing architecture.
9. Record the work against the canonical session/issue/milestone machinery.

Do not ask the user for information that the repository, Git, sessions, handovers, issues, runtime or canonical registries can provide.

## Work discipline

- Inspect -> design -> implement -> migrate -> test -> document -> enforce -> validate -> land -> deploy -> verify.
- Prefer extending canonical registries/schemas/services over adding parallel systems.
- Delete obsolete duplicate implementations after migration.
- Treat documentation claims as testable claims where project tooling supports this.
- Do not leave work in stale branches/worktrees/PRs.
- Do not claim completion without exact validation evidence.
- If unrelated pre-existing failures exist, prove they predate the work.

## Testing discipline

Every new/changed path must include meaningful tests. At minimum, cover:

- happy path
- negative path
- migration/legacy path
- concurrency/isolation where relevant
- restart/resume where relevant
- secret redaction
- deterministic routing
- direct-bypass prevention
- cross-surface consistency
- E2E continuity

Use the repository's strongest coverage standard. If 100% new-code coverage is policy, satisfy it without gaming assertions.

## Documentation discipline

Update canonical docs, generated docs, architecture pages, landing/features where appropriate, and GH Pages. Documentation must be derived/generated from canonical state where possible and must not contain manually duplicated provider/model inventories.

## Delivery discipline

Do not stop at local green tests. Follow repository conventions to commit, push, land/merge, deploy and verify the deployed/public result.

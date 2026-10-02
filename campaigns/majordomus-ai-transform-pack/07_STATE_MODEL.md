# Local state and worktree model

## Approved ownership change

All mutable repository-specific Majordomus operational state moves from:

```text
.majordomus/state/
```

to:

```text
.ai/local/state/
```

The entire `.ai/local/` subtree is Git ignored.

## Consequence

This intentionally changes durability scope:

Before:
some operational records were tracked and therefore could travel through Git.

After:
operational task/session state is local to the machine/checkout unless explicitly
promoted to tracked repository knowledge.

Update documentation and claims so they do not continue implying Git-distributed
session history.

## State categories

Keep conceptual distinctions:

```text
current task
ledger/events
checkpoints
handover records
session envelopes
local decisions
open questions
archive/completion records
```

Do not collapse these into one opaque transcript or JSON blob.

## Raw prompts

New path:

```text
.ai/local/prompts/
```

Use timestamped append-only records if/when an integration can observe user
prompts.

Do not fabricate provider capture.

Do not store model transcript automatically.

## Session contexts

New path:

```text
.ai/local/session-contexts/
```

These are bounded assembled/working contexts, not authority.

Git and tracked repository sources outrank local session context.

## Cross-worktree behavior

Do NOT introduce `.ai/worktrees/` as a committed or global namespace in this
transformation.

Preferred v1 model:

```text
each checkout/worktree has its own .ai/local/
cross-worktree repository/worktree identity is discovered from Git
other worktree local state is read from those worktree paths when needed
```

Use:

```bash
git worktree list --porcelain
git rev-parse --git-common-dir
```

or existing portable equivalents.

Avoid a second machine-global state database unless a concrete invariant cannot
be met without it.

## Scope overlap

Preserve existing overlap reporting across worktrees. Moving state to ignored
paths MUST NOT regress overlap detection.

Add/modify behavioral tests using multiple real temporary worktrees.

## Gitignore behavior

Target:

```gitignore
.ai/local/
```

`majordomus init` must ensure the ignore boundary exists idempotently, but
should not overwrite unrelated `.gitignore` content.

## State path API

Do not sprinkle `.ai/local/state` literals through commands.

Introduce central path resolution in `lib/common.sh` or equivalent, such as:

```text
MJ_AI_DIR
MJ_AI_REPO_DIR
MJ_AI_LOCAL_DIR
MJ_STATE_DIR
MJ_PROJECT_DIR
MJ_POLICY_FILE
MJ_PROFILES_DIR
```

Names may differ, but repository path semantics must have one implementation
point.

The Majordomus installation root must be a separate concept, e.g.:

```text
MJ_HOME / MJ_DIST_DIR / MJ_LIB_DIR / MJ_SHARE_DIR
```

Never reuse one variable for both tool installation and repository data.

## Migration of existing state

Migration must:

- preserve files byte-for-byte where format is unchanged,
- create `.ai/local/`,
- move old state,
- ensure new local subtree is ignored,
- remove old tracked state from the index as appropriate,
- not delete unknown files,
- refuse ambiguous mixed layouts,
- be idempotent or explicitly detect already-migrated state.

Because ignored state will no longer be present in future clones, migration
tests must distinguish same-checkout preservation from fresh-clone behavior.

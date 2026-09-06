<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `worktree` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.2.0 -->
# Module `worktree` — Worktree topology

Where every linked git worktree of this repository belongs and where each one is. The container is the primary checkout's sibling named with `-wt`, the path under it is the branch name with its hierarchy kept, and both are derived from git's own identity — the common directory, the registered worktrees, the branches — never from a registry, a configuration or the current directory. A worktree somewhere else is a typed diagnostic with a remedy; the migration that repairs it is a command-line operation of the same service.

Stability: behaviorally_verified. Capabilities: 4.

## `worktree.inspect` — One branch: where its worktree belongs and what is there

The canonical path of a branch, derived from its name alone, whether the branch exists, whether something occupies that path, the worktree holding the branch when one does, and what stands in the way of creating or migrating it. The answer for a branch that does not exist yet is the path `worktree create` would use.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_worktree_inspect` |
| HTTP | `GET /api/v1/worktrees/inspect` |
| CLI | `majordomus worktree inspect` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::worktree |
| tags | worktree, git, topology |

| input | type | required | description |
|---|---|---|---|
| `branch` | string | yes | The branch, full name (`feature/improve-cli`). It need not exist: the canonical path
derives from the name alone. |

Output: `InspectReport`.

## `worktree.migration_plan` — What it would take to bring every worktree home

One step per misplaced worktree with a branch: where it is, where it belongs, how it would move, the uncommitted work that moves with it, and what blocks it; plus the exceptions the migration cannot address by design — detached worktrees, stale registrations, the primary checkout off the trunk — each with what a person does about it. Planning changes nothing; `majordomus worktree migrate` applies it with a fingerprint taken before and after every move.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_worktree_migration_plan` |
| HTTP | `GET /api/v1/worktrees/migration` |
| CLI | `majordomus worktree migrate` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::worktree |
| tags | worktree, git, topology, migration |

Input: none.

Output: `MigrationPlan`.

## `worktree.status` — Where this is, and whether that is where it belongs

One worktree — the repository's own, or the one holding the directory the caller names — with its standing, its branch, its canonical path, its uncommitted work counted, whether it is where it belongs, and how many errors the whole topology carries. What an agent reads before it starts, and what the guard decides on.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_worktree_status` |
| HTTP | `GET /api/v1/worktrees/status` |
| CLI | `majordomus worktree status` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::worktree |
| tags | worktree, git, topology |

| input | type | required | description |
|---|---|---|---|
| `path` | string or null | no | A directory inside one of this repository's worktrees; the answer is about that
worktree. Relative to the primary checkout when relative (`.` is the primary
checkout itself). Default: the repository the server was started for. A path in
another repository is refused: the topology answers only about its own. |

Output: `StatusReport`.

## `worktree.topology` — The whole topology

The repository, the container, the trunk and how it was decided, every registered worktree with its standing (primary, canonical, misplaced, detached, missing), its uncommitted work, its upstream distance and the issue its branch provably names, every local branch with or without a worktree and whether it is eligible for cleanup, every diagnostic with its code and remedy, and the tallies. Read from git on every call: the topology changes outside this process.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_worktrees` |
| MCP resource | `majordomus://worktrees` |
| HTTP | `GET /api/v1/worktrees` |
| CLI | `majordomus worktree topology` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::worktree |
| tags | worktree, git, topology, introspection |

Input: none.

Output: `RepositoryTopology`.


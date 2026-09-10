<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `worktree` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.2.0 -->
# Module `worktree` — Worktree topology

Where this repository's linked git worktrees belong, which ones exist, and whether each is where the canonical policy says it should be. The container is derived from the repository's identity and the policy's suffix, so the answer is the same from the primary checkout and from inside any linked worktree.

Stability: behaviorally_verified. Capabilities: 3.

## `worktree.list` — Every registered worktree, judged against the policy

Every work tree git has registered for this repository, primary first: its path, branch, HEAD, whether it is locked, prunable or the one this call came from, and whether the layout rule holds for it. The primary checkout is exempt by definition. Violations are listed separately with the destination each one would move to.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_worktrees` |
| MCP resource | `majordomus://worktrees` |
| HTTP | `GET /api/v1/worktrees` |
| CLI | `majordomus worktree list` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::worktree |
| tags | worktree, git, policy, introspection |

| input | type | required | description |
|---|---|---|---|
| `status` | boolean or null | no | Also report whether each work tree has uncommitted or untracked content. It costs one
`git status` per existing work tree, and the layout rule never needs it, so it is off
unless asked for. |

Output: `WorktreeReport`.

## `worktree.root` — The canonical worktree container

The directory every linked worktree of this repository belongs under, derived from the primary checkout's name and the policy's suffix — never from the current directory. Also names the primary checkout, the work tree the call came from, and the common git directory that identifies the repository.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_worktree_root` |
| HTTP | `GET /api/v1/worktree/root` |
| CLI | `majordomus worktree root` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::worktree |
| tags | worktree, git, policy |

Input: none.

Output: `RootReport`.

## `worktree.status` — Where this call is, and whether that is where it belongs

The current work tree in one answer: the repository it belongs to, whether it is the primary checkout or a linked worktree, its branch and HEAD, the canonical container, whether the layout rule holds here, whether there is uncommitted work, and how many worktrees of the repository are out of place.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_worktree_status` |
| HTTP | `GET /api/v1/worktree/status` |
| CLI | `majordomus worktree status` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::worktree |
| tags | worktree, git, policy |

Input: none.

Output: `StatusReport`.


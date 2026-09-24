<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `convergence` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.8.0 -->
# Module `convergence` — Convergence

Whether every unit of work in this repository is somewhere another worker could find it. A holding is anything that can hold work — a work tree with uncommitted files, a branch with commits, a stash — and each carries a disposition read from git: integrated (the trunk reaches it), published (a remote-tracking ref reaches it), local_only (committed on this disk and nowhere else) or uncommitted. The last two are at risk: they are invisible to every other worker, and a worker that stops is not a rollback. Measured offline, from this checkout alone; whether a pull request exists for a branch is a question for the forge and is not asked here.

Stability: behaviorally_verified. Capabilities: 1.

## `convergence.report` — Is any work held where it can be lost?

Every holding of this repository with its disposition, the evidence read for it and the command that would move it out of danger, the tallies per disposition, how many holdings exist on one disk only, and the one verdict over them. Read from git on every call: a commit, a push or an editor's save changes the answer between two calls.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_convergence` |
| MCP resource | `majordomus://convergence` |
| HTTP | `GET /api/v1/convergence` |
| CLI | `majordomus convergence` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::convergence |
| tags | convergence, git, worktree, integration |

Input: none.

Output: `ConvergenceReport`.


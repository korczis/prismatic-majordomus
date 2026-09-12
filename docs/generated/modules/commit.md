<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `commit` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.6.0 -->
# Module `commit` — Commit

The commit as a value: the scope vocabulary this repository's own history yields, what the working tree would commit and how it divides, and the verdict on one message against the repository's commit policy.

Stability: behaviorally_verified. Capabilities: 4.

## `commit.history` — Judge a range of history

Every commit in a git range against the commit policy, in one pass: how many were read, how many git composed and are exempt, how many carry an error, and every commit that has a finding with what it is. What the gate reads to refuse new debt, and what answers `is the history of this branch clean?` without a process per commit.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_commit_history` |
| HTTP | `GET /api/v1/commit/history` |
| CLI | `majordomus commit history` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::commit |
| tags | commit, git, governance |

| input | type | required | description |
|---|---|---|---|
| `range` | string | no | Anything `git log` accepts: `origin/master..HEAD`, `v0.6.0..`, a bare `HEAD`.
Default `HEAD`, which is the whole history reachable from here. |

Output: `HistoryReport`.

## `commit.plan` — What the working tree would commit

The working tree as git reports it — branch, upstream, divergence, every staged, unstaged and untracked path, and any merge or rebase in progress — divided into the commits the history's own scoping supports, each with the evidence for it, under a fingerprint of the repository, the worktree, HEAD and the exact change set, so that a plan acted on later can be refused rather than applied to a tree it was not made for.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_commit_plan` |
| MCP resource | `majordomus://commit/plan` |
| HTTP | `GET /api/v1/commit/plan` |
| CLI | `majordomus commit plan` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::commit |
| tags | commit, git, plan |

Input: none.

Output: `CommitPlan`.

## `commit.scopes` — The scope vocabulary

Every scope this repository's commit history uses, how often, and the directories each one is written about — learned from the history rather than declared in a table, so that a subsystem committed today is in the vocabulary today and one nobody has touched falls to the bottom on its own.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_commit_scopes` |
| MCP resource | `majordomus://commit/scopes` |
| HTTP | `GET /api/v1/commit/scopes` |
| CLI | `majordomus commit scopes` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::commit |
| tags | commit, git, introspection |

Input: none.

Output: `ScopesReport`.

## `commit.validate` — Judge one commit message

One message against this repository's commit policy: the grammar, the subject width, whether the scope is one the history uses, whether every record the message names exists, whether a fix carries a test, and whether a breaking change explains itself — as typed findings with severities, or an exemption for a subject git composed. The same verdict the commit-msg hook refuses with and the gate reads the history through.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_commit_validate` |
| HTTP | `GET /api/v1/commit/validate` |
| CLI | `majordomus commit validate` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::commit |
| tags | commit, git, governance |

| input | type | required | description |
|---|---|---|---|
| `message` | string | yes | The commit message, as it would be stored. Comment lines are ignored, as git ignores
them, so what is judged is what would be committed. |
| `paths` | array or null | no | The paths the commit would contain. Omit them and the judgements that need them —
whether a fix carries a test — are not made rather than guessed. |

Output: `CommitVerdict`.


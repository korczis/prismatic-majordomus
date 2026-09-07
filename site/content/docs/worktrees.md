+++
title = "Worktrees"
description = "the branch-to-worktree topology: `<repo>-wt/<branch>` derived from git identity and never registered, the standings and diagnostic codes, the commands, the lifecycle, the layered enforcement, the fingerprint-verified migration, failure modes and recovery"
weight = 34
[extra]
source = "docs/WORKTREES.md"
+++

{% raw %}

Where every linked git worktree of this repository belongs, how that is decided, and what
the tool does about one that is somewhere else. Behaviour as implemented and tested; where
this document and the executable disagree, the document is wrong and changes in the same
commit. The decision is [ADR 21](../.ai/repo/adrs/0021-the-branch-to-worktree-topology-is-derived-from-git-identity-and-enfor.md);
the rule is `project.worktree-topology`.

```text
~/dev/prismatic-majordomus                    the primary checkout: hosts the trunk, never moves
~/dev/prismatic-majordomus-wt/                the container: the checkout's sibling, name + "-wt"
~/dev/prismatic-majordomus-wt/feature/x       the one worktree of branch feature/x
~/dev/prismatic-majordomus-wt/fix/a/b         the one worktree of branch fix/a/b — hierarchy kept
```

The branch name *is* the relative path. Nothing is flattened (`feature-x`), hashed
(`feature-x-a93f`) or numbered (`worktree-1`), and nothing is registered anywhere: given the
repository and a branch, there is exactly one path, and it is derived the same way by every
surface.

## Why

Several sessions — people and agents — work on one repository at once and each needs its
own checkout. Git's linked worktree is the mechanism, and git takes the destination as an
argument, so every caller decides where one goes. Fifty-three of them had decided
differently here: flat siblings named after nothing, session scratch directories, one
inside the primary checkout, one occupying the container's own path. The cost was not
untidiness. It was that "where is branch X" and "where does a new one go" had no answer
that did not involve looking, guessing or inventing, and a session that has to invent a
path leaves its work where the next session does not look.

The topology gives the answer once: feature isolation with no decision, parallel agents
that cannot collide on a path, a predictable session → branch → worktree mapping that a
handover can name and a resumed session can navigate back to, cleanup that is a derived
list rather than an archaeology, and a topology that every surface can show because every
surface computes it the same way.

## How the path is derived

```text
git rev-parse --git-common-dir             which repository this is, from any directory of any worktree
git worktree list --porcelain              its worktrees; the first record is the primary checkout
parent(primary) / name(primary) + "-wt"    the container
container / branch name                    the worktree, one directory per component
```

The second line is the one that is easy to get wrong. `git rev-parse --show-toplevel`
answers *the current worktree*, so deriving a container from it inside a linked worktree
would produce `…-wt/feature/x-wt`, a container per worktree, nesting for ever. The primary
checkout is read from git's shared metadata instead, so the answer is identical from the
primary checkout, from a subdirectory of it, from a linked worktree, and from four
directories deep inside one.

A branch name is filesystem-derived input and is treated as such: the name is validated by
git's own reference rules (no `..`, no component beginning with `.`, none ending in
`.lock`, no control characters, none of `~ ^ : ? * [ \`, no `@{`, no leading `-`), every
component of a valid name is an ordinary directory name, and the derived path is proved on
every call to be strictly below the container. Unicode components are ordinary directory
names. Two names that differ only by case are reported (`worktree.case_collision`), because
one directory holds both on a case-insensitive filesystem.

The trunk is discovered, never hardcoded: the remote's HEAD (`refs/remotes/origin/HEAD`),
then `init.defaultBranch` when that branch exists, then whichever of `main` and `master`
exists alone, then the primary checkout's own branch. Which one decided is reported.

`-wt` is written once, as `CONTAINER_SUFFIX` in `apps/majordomus-cli/src/worktree/path.rs`.
There is no configuration: a container that could be configured is a container two
checkouts of one repository could disagree about.

## The architecture

```text
   git (common dir · worktree list · for-each-ref · status)
                        ↓
   RepositoryIdentity           primary, current, container, trunk — from anywhere inside
                        ↓
   path::expected_path()        container / branch, proved to stay inside the container
                        ↓
   WorktreeService              topology · status · guard · inspect · create · migrate · repair · remove
        ↙        ↓        ↘
   CLI      capability       git hook
             registry ──── MCP tools + majordomus://worktrees · /api/v1/worktrees* · OpenAPI · Swagger UI
                       ──── Cockpit /cockpit/worktrees · docs/generated
```

Business logic exists once, in `apps/majordomus-cli/src/worktree/`. Every surface is an
adapter that renders or projects what the service answered; none derives a path, decides a
standing or judges whether a move is safe. The four read-only questions are capabilities
of the registry (`worktree.topology`, `worktree.status`, `worktree.inspect`,
`worktree.migration_plan`), which is what puts them on MCP, HTTP, OpenAPI, the Swagger UI,
the Cockpit and the generated reference without any of those carrying a route or a schema
of their own. Creating, migrating, repairing and removing are command-line operations of
the same service: a capability of the registry never writes to the repository, which is the
contract the shared MCP server rests on, and the Cockpit names the exact command for each.

## Standings and diagnostics

Every registered worktree has one standing:

<div class="overflow-x-auto" tabindex="0">

| standing | meaning |
|---|---|
| `primary` | the main worktree; exempt from the path rule, held to the trunk rule |
| `canonical` | a linked worktree at exactly its branch's path |
| `misplaced` | a linked worktree somewhere else; migration brings it home |
| `detached` | no branch, so no canonical path; never moved |
| `ephemeral` | a session's scratch checkout, under the temporary directory or `<primary>/.claude/worktrees/`; reported, refused for commits, moved only on request |
| `missing` | a registration whose directory is gone; `repair` drops it |

</div>


and every condition has a stable code, the same on every surface, each with a remedy:

<div class="overflow-x-auto" tabindex="0">

| code | severity | what it means |
|---|---|---|
| `worktree.path_mismatch` | error | a linked worktree is not at its branch's canonical path |
| `worktree.container_occupied` | error | a worktree occupies the container path itself |
| `worktree.nested` | warning | it sits inside the primary checkout or another worktree |
| `worktree.destination_conflict` | error | the canonical path is occupied by something else — a registered worktree of another branch, a foreign checkout, a directory, a file, a symbolic link |
| `worktree.missing` / `worktree.stale_registration` | warning | the directory is gone |
| `worktree.branch_already_checked_out` | error | the branch is checked out somewhere other than its canonical path |
| `worktree.detached` | info | no branch |
| `worktree.ephemeral` | warning | a session's scratch checkout holding a branch |
| `worktree.primary_on_non_trunk` | error | the primary checkout holds a branch that is not the trunk |
| `worktree.trunk_in_linked_worktree` | warning | the trunk is checked out in a linked worktree |
| `worktree.path_escape` / `worktree.invalid_branch_name` | error | the name cannot derive a path |
| `worktree.locked` | warning | git will not move it until it is unlocked |
| `worktree.migration_verification_failed` | error | the fingerprint after a move differs from the one before |
| `worktree.trunk_unknown` | warning | nothing said which branch the trunk is |
| `worktree.case_collision` | warning | two branch names derive one directory on a case-insensitive filesystem |
| `worktree.cross_device` | info | a move was made by copy across filesystems |

</div>


The topology is *valid* when no error-level diagnostic stands. Detached and ephemeral
worktrees do not make it invalid; the guard still refuses a commit from an ephemeral one.

## Commands

```bash
majordomus worktree                            # where am I, and is that where I belong (exit 10 if not)
majordomus worktree create feature/improve-cli # start: the path is derived, never given
cd "$(majordomus worktree path feature/improve-cli)"
majordomus worktree list                       # every worktree with its standing
majordomus worktree topology --format json     # the whole document the API and the Cockpit render
majordomus worktree doctor                     # every diagnostic with its code and remedy
majordomus worktree migrate --plan             # what would move; changes nothing
majordomus worktree migrate                    # move, verify, report
majordomus worktree cleanup                    # what is merged and clean; deletes nothing
```

<div class="overflow-x-auto" tabindex="0">

| command | what it does | exit 10 when |
|---|---|---|
| `worktree` / `worktree status` | this worktree: branch, standing, canonical path, uncommitted work, upstream, issue | this worktree is out of place |
| `worktree list` | every worktree, one line each | the topology has an error |
| `worktree topology` | repository, container, trunk, worktrees, branches without a worktree, diagnostics, tallies | the topology has an error |
| `worktree root` | the container, one path | |
| `worktree path <branch>` | the canonical path of a branch, one path; the branch need not exist | the name is invalid |
| `worktree inspect <branch>` | canonical path, whether the branch exists, what occupies the path, what stands in the way | |
| `worktree create <branch> [--base REF] [--issue ID]` | the canonical worktree, the branch created from the trunk when new | occupied, checked out elsewhere, invalid |
| `worktree ensure <branch>` | the same, answering an existing canonical worktree instead of refusing | checked out elsewhere |
| `worktree migrate [--plan] [--only B]… [--allow-copy] [--include-ephemeral]` | bring misplaced worktrees home, fingerprint-verified | a step was blocked or failed |
| `worktree validate` / `worktree doctor` | the errors, or every diagnostic | an error stands |
| `worktree guard [--quiet]` | may a commit proceed from here | no |
| `worktree repair [--dry-run]` | drop stale registrations, repair git's links; deletes no directory | |
| `worktree remove <branch\|path> [--force]` | remove one linked worktree; never the primary, never a branch, never dirty work unforced | refused |
| `worktree cleanup` | branches merged into the trunk whose worktree is clean or absent, with the commands that would remove them | |
| `worktree branches [--without-worktree]` | every local branch, one per line | |

</div>


`wt` is an alias for `worktree`. `--format json` is available everywhere and is the same
typed answer the human form renders. A **selector** for `remove` is exact — a branch name or
a path — and nothing is matched by prefix or similarity. There is no `worktree cd`: a child
process cannot change its parent shell's directory, so `path` prints one and the shell does
the rest.

`worktree create --issue I0042` names the branch `feature/I0042-<slug>` from the issue's
own record, and the topology reads the issue back from any branch that carries an issue id
as a path component (`feature/I0042-live-page` → `I0042`). Nothing is inferred from
similarity: `feature/I00420-x` names no issue.

Shell completion of the branch arguments reads the live set rather than a list:

```bash
# zsh
_mj_wt_branches() { compadd -- ${(f)"$(majordomus worktree branches 2>/dev/null)"} }
compdef '_arguments "1:sub:(status list topology root path inspect create ensure migrate validate doctor guard repair remove cleanup branches)" "2:branch:_mj_wt_branches"' majordomus-worktree
```

## Lifecycle

```text
issue / task                majordomus plan next
      ↓
branch                      feature/<ID>-<slug>, or any name git accepts
      ↓
canonical worktree          majordomus worktree create <branch>   → <repo>-wt/<branch>
      ↓
session / agent             majordomus context, start, checkpoint, handover — in that worktree
      ↓
commit / PR                 the pre-commit hook asks the guard; push and open the PR from there
      ↓
cleanup                     majordomus worktree cleanup → worktree remove, git branch -d, by a person
```

A handover records the branch and the worktree; a session resumed elsewhere derives the
worktree from the branch rather than trusting the recorded path, because the path is
ephemeral and the branch is not.

## Enforcement

Layered, and honest about what each layer can do:

<div class="overflow-x-auto" tabindex="0">

| layer | what it does |
|---|---|
| the doctrine | ADR 21 and `project.worktree-topology` say what the rule is |
| the agent bootstraps | `AGENTS.md`, `CLAUDE.md` and the other provider files, generated from the templates, tell every worker to establish the topology before implementing and to correct a mismatch with the tool |
| the workflows | `.ai/repo/workflows/task-lifecycle.md` starts with `worktree status` |
| the command line | `worktree create` is the way a branch's worktree comes into being; a path is never an argument |
| the git hook | `.githooks/pre-commit` asks `majordomus worktree guard`; a feature branch is committed only from its canonical worktree, the primary checkout only on the trunk |
| the wiring check | the policy's `enforcement` list declares the guard, so `majordomus doctor` proves the hook asks it |
| the repository entry | `.envrc` prints `worktree status` on entry, without building anything |
| the surfaces | the topology is on the command line, MCP, HTTP, the Swagger UI and the Cockpit, so a wrong one is visible everywhere |
| the tests | the crate's suite proves the derivation, the safety and the migration against real git; the shell case proves the wiring |
| the CI gate | `scripts/ci/worktree-check` holds the constant, the hook, the policy, the documents and the case together |

</div>


A git hook is bypassed with `--no-verify`, and a CI runner cannot see a contributor's local
filesystem; the gate therefore measures the rule's machinery, not this machine's
directories. Within Majordomus-controlled workflows a wrong worktree is refused; outside
them it is visible on every surface.

## Migration

`worktree migrate --plan` lists every misplaced worktree with a branch: where it is, where
it belongs, how it would move, the uncommitted work that moves with it, and what blocks it.
The container's occupant, if any, is first — every other destination is inside the
container, and while a worktree *is* the container those destinations would be created
inside that worktree. Exceptions — detached worktrees, stale registrations, ephemeral
scratch checkouts, the primary checkout off the trunk — are listed with what a person does
about each.

`worktree migrate` recomputes the plan under the repository lock and, for each movable step:

1. takes a fingerprint of the worktree: branch, HEAD, the index (every tracked path with
   mode, blob and stage), the staged diff, the unstaged diff, every untracked file with its
   size and content hash, every ignored entry by path and size;
2. asks git to move it — `git worktree move`, one rename, dirty state included; the
   container's occupant goes out to a staging path and then in; a move across filesystems
   is refused unless `--allow-copy`, and then made by copy, `git worktree repair`,
   verification against a manifest of every entry of the tree, and only then removal of
   the original;
3. takes the fingerprint again at the destination, verifies that git registers it there,
   and reports the step as `moved` only when the two fingerprints are equal — otherwise
   `failed`, with what differs, and nothing further is touched;
4. names relative symbolic links that no longer resolve from the new path
   (`node_modules -> ../sibling/node_modules`), which the move did not change and does not
   rewrite.

Nothing is reset, stashed, cleaned, checked out or deleted. A locked worktree is blocked
until it is unlocked. A destination that exists — a registered worktree of another branch,
a foreign git checkout, a directory empty or not, a file, a link — is a conflict named by
kind and never overwritten. If the command was run from a worktree that moved, the report
says where to `cd`.

## Failure modes and recovery

<div class="overflow-x-auto" tabindex="0">

| situation | what you see | what to do |
|---|---|---|
| a dirty worktree in the wrong place | `worktree.path_mismatch`, the dirty counts on the plan step | `majordomus worktree migrate`; the work moves with it |
| the canonical path is taken | `worktree.destination_conflict` naming what is there | move it aside by hand; nothing overwrites it |
| a detached worktree | `worktree.detached`, standing `detached` | `git switch -c <branch>` there, then migrate; or leave it |
| a missing directory | `worktree.missing` / `worktree.stale_registration` | `majordomus worktree repair` |
| a locked worktree | `worktree.locked`, the step blocked | `git worktree unlock <path>`, then migrate |
| the wrong branch in a worktree | `worktree.branch_already_checked_out` on create | migrate the worktree that holds it, or work there |
| the primary checkout on a feature branch | `worktree.primary_on_non_trunk`; the guard refuses | when clean, `git switch <trunk>`; then `worktree create <branch>` |
| a scratch checkout of a session on a branch | `worktree.ephemeral`; the guard refuses | `git switch --detach` there and continue in the canonical worktree, or `migrate --include-ephemeral --only <branch>` |
| a move that crossed devices | `worktree.cross_device` on the step, or a refusal | `migrate --allow-copy` |
| a fingerprint mismatch | `worktree.migration_verification_failed`, the differences listed | inspect the worktree where git registers it before touching anything; a rename cannot lose content, so something wrote to it during the move |

</div>


## Performance

`worktree status` and `worktree guard` read the registrations, the branches and one `git
status` of the current worktree: a handful of subprocesses, no index build, no network, no
scan. `topology` and `list` add one `git status` per existing worktree and nothing else.
The migration hashes the content of every modified and untracked file of the worktrees it
moves, which is the one place that cost is right. Nothing is cached across calls: the
topology changes outside the process.

## Where things are

<div class="overflow-x-auto" tabindex="0">

| | |
|---|---|
| the constant and the derivation | `apps/majordomus-cli/src/worktree/path.rs` |
| identity and trunk | `apps/majordomus-cli/src/worktree/identity.rs` |
| the typed topology | `apps/majordomus-cli/src/worktree/model.rs` |
| the service | `apps/majordomus-cli/src/worktree/service.rs` |
| the migration and fingerprints | `apps/majordomus-cli/src/worktree/migrate.rs`, `fingerprint.rs` |
| the capabilities | `apps/majordomus-cli/src/capability/builtin/worktree.rs` |
| the command line | `apps/majordomus-cli/src/commands/worktree.rs`, declared in `src/cli.rs` |
| the Cockpit page | `/cockpit/worktrees`, `apps/majordomus-cli/src/cockpit/pages.rs` |
| the launcher the hook and `.envrc` use | `bin/majordomus-cli` |
| the hook | `.githooks/pre-commit`; the entry in `.ai/repo/policy.yaml` `enforcement` |
| the gate | `scripts/ci/worktree-check`, gate `worktree-topology` in `.ai/repo/ci/gates.yaml` |
| the tests | `apps/majordomus-cli/tests/worktree.rs`, `test/cases/96_worktree_topology.sh` |
| the decision and the rule | ADR 21, `.ai/repo/rules/project/worktree-topology.v1.md` |

</div>

{% endraw %}

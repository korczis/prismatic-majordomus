# Worktrees

Where this repository's linked git worktrees live, how that is decided, and what the tool
does about a worktree that is somewhere else.

```text
~/dev/prismatic-majordomus          the primary checkout — never moves
~/dev/prismatic-majordomus-wt/      the container
~/dev/prismatic-majordomus-wt/*     every linked worktree
```

Five commands are the whole mental model:

```bash
majordomus worktree create issue-184-websocket   # somewhere is not your decision
majordomus worktree list                          # what exists, and what is out of place
majordomus worktree status                        # where am I, and is that all right
cd "$(majordomus worktree path issue-184-websocket)"
majordomus doctor                                 # does the topology hold
```

## Why a sibling container

Git takes a destination for `git worktree add` and has no opinion about it, so every caller
has one. The decision reached for one is: put it where nobody has to decide.

Inside the repository (`.worktrees/`) means every build tool, watcher, linter and search
walks into checkouts of other branches, and `rm -rf` of the repository takes their
uncommitted work with it. A global registry (`~/.worktrees/<repo>/`) outlives the
repositories it indexes and collides on basenames. Arbitrary paths are what happens when
nothing decides — this repository accumulated thirty-seven worktrees in four different
shapes that way. Temporary directories lose the work at reboot and hide it from every other
session in the meantime.

The sibling is outside the source tree, beside the thing you already have, named after the
repository, one per repository, and derivable from a single suffix. ADR 20 has the full
argument.

## How the container is derived

```text
worktree.root.suffix in .ai/repo/policy.yaml      "-wt"
                    +
the primary checkout, from git's shared metadata   ~/dev/prismatic-majordomus
                    ↓
parent(primary) / basename(primary) + suffix       ~/dev/prismatic-majordomus-wt
```

The second input is the one that is easy to get wrong. `git rev-parse --show-toplevel`
answers *the current work tree*, so deriving a container from it inside a linked worktree
would produce `~/dev/prismatic-majordomus-wt/issue-184-wt` — a container per worktree,
nesting for ever. The primary checkout is read instead from the repository's shared
metadata: the common git directory that every work tree of one repository points at, and the
main work tree that `git worktree list --porcelain` reports first.

So the answer is identical from the primary checkout, from a subdirectory of it, from a
linked worktree, and from four directories deep inside a linked worktree. That is what makes
the commands usable by an agent that does not know where it was started.

## The architecture

```text
   WorktreePolicy            .ai/repo/policy.yaml — strategy and suffix, the one truth
         +
   RepositoryIdentity        git common dir, primary checkout, current checkout,
         ↓                   registered worktrees — three git subprocesses, once
   canonical_root()
         ↓
   WorktreeService           list, status, create, remove, migrate, prune
      ↙    ↓    ↘
    CLI  doctor  capability registry → MCP, HTTP, OpenAPI, Swagger, cockpit, docs
```

Business logic exists once. `apps/majordomus-cli/src/worktree/` holds all of it; every
surface is an adapter that renders or projects what the service answered. The command line
executes the same capability the MCP tool does for every read-only question, `doctor` reads
the same report, and the generated reference is a projection of the same descriptors.

Creating, removing, migrating and pruning are command-line operations and are deliberately
*not* capabilities: a capability of this registry never writes, which is what lets the shared
MCP server be safe to attach to. They call the same service, so there is still exactly one
implementation of each.

## Commands

| Command | What it does |
|---|---|
| `worktree` | `status`, the default |
| `worktree root` | The container, one path, for `cd "$(...)"` |
| `worktree list [--status] [--tsv]` | Every worktree with its policy verdict |
| `worktree status` | Where this call is, and whether that is where it belongs |
| `worktree path <selector>` | One worktree's path |
| `worktree create <name> [--branch B] [--base REF] [--issue ID] [--detach]` | Create under the container |
| `worktree remove <selector> [--force]` | Remove one linked worktree |
| `worktree migrate [--plan\|--apply]` | Bring worktrees under the container |
| `worktree prune [--dry-run]` | Drop git's records for worktrees that are gone |

`wt` is an alias for `worktree`. `--format json` is available everywhere and is derived from
the same typed answer as the human form, so nothing has to parse the human form.

A **selector** is exact: a path, a directory name, or a branch name. Nothing is matched by
prefix, substring or similarity, and two matches is an error naming both — these selectors
are given to commands that delete things.

There is no `worktree cd`. A child process cannot change its parent shell's directory and
nothing here pretends otherwise; `worktree path` prints one path and the shell does the rest.

## Naming

A branch name is a path with slashes and almost no other rules; a directory under the
container is one component with the filesystem's rules. One function maps between them:
everything outside `A-Za-z0-9._-` becomes `-`, runs collapse, and leading and trailing
punctuation is trimmed, so nothing produces a hidden directory, `.` or `..`.

```text
feature/foo          → feature-foo
bugfix/bar/baz       → bugfix-bar-baz
user@example/test    → user-example-test
issue/123/foo        → issue-123-foo
..                   → refused
```

The mapping is lossy on purpose: `feature/foo` and `feature-foo` both name `feature-foo`.
Hashing to avoid that would make every directory unreadable to prevent a collision that
almost never happens. Instead the collision is *detected*: creating the second one finds the
first registered at that path and refuses, naming the branch that is already there.

With `--issue <id>`, the name comes from the repository's own issue record — its id and its
authored slug — giving `issue-<id>-<slug>`. No number is invented, nothing is fetched, and an
id the repository does not have is an error rather than a new directory.

## Safety

Before anything is deleted or moved, four things are proved: the path is a work tree git
registered **for this repository**, it is not the primary checkout, it is not locked, and it
holds no uncommitted or untracked work. Paths are compared canonicalised, so a symlink inside
the container pointing somewhere else does not make that somewhere else look compliant.

Git does the moving and removing — `git worktree remove`, `move`, `prune` — rather than
recursive deletion of a directory whose name looked right. Git commands are built with
process arguments, never a shell string, so a path with a space, a quote, a newline or a
leading hyphen is passed exactly as it is.

`--force` waives the dirty and locked refusals and waives nothing else: a forced removal
still has to be a registered worktree of this repository and still cannot be the primary
checkout.

Removing a worktree never removes a branch. The two lifecycles are separate, and deleting a
branch is a command a person types deliberately.

Every mutating operation holds one lock at
`<git-common-dir>/majordomus/locks/worktrees.lock` — under the *common* directory, which is
the one place every linked worktree of a repository shares, so there is one lock visible from
all of them and none shared between repositories. It is released when the process ends, and
one abandoned for five minutes is reclaimed.

Nothing reaches the network. There is no `git fetch` anywhere in the subsystem; a base ref
that does not resolve locally does not resolve.

## Doctor

`majordomus doctor` reports one finding per linked worktree outside the container, naming the
worktree, its branch, where it belongs and the destination it would move to:

```text
FAIL worktree  /tmp/example-feature-c — it is outside the canonical root
               /home/me/dev/example-wt (branch feature/c); would move to
               /home/me/dev/example-wt/example-feature-c
               [reproduce: majordomus worktree migrate --plan]
```

The primary checkout is exempt by definition — it is the thing the container is named after.
Doctor diagnoses and repairs nothing; `init` and `update` do not touch the topology either.

`worktree.enforcement.outside_root` in the policy decides what a violation costs: `error`
(the default) fails doctor, `warn` reports it, `off` does not evaluate it.

Where the Rust executable is not built, the check reports `INFO` and is recorded as skipped,
never as passed.

## Migration

```bash
majordomus worktree migrate --plan     # changes nothing, ever
majordomus worktree migrate --apply    # makes the safe moves, reports the rest
```

A step is blocked, with the reason on it, when the worktree is dirty, is locked, its
directory is gone, it is the primary checkout, or the name it would take inside the container
is already used. `--apply` recomputes the plan under the lock rather than replaying one that
was printed earlier, checks again immediately before each move that the destination is free
and inside the container, and exits 10 if anything was left blocked.

Nothing migrates automatically. A worktree outside the container keeps working; it is
reported, not broken.

## Agent behaviour

The doctrine reaches agents through the provider instruction files, which are generated:
`share/providers/*.tmpl` and `.ai/repo/providers/*.tmpl` carry it once, with the container
written as `{{WORKTREE_SUFFIX}}`, and `AGENTS.md`, `CLAUDE.md` and the rest are rendered from
them. Changing the policy's suffix changes every one of those files, and
`majordomus generate --check` fails until they are regenerated — which is the mechanical
proof that the instruction is not a fifth hand-maintained copy.

The rule is repository-wide. A nested context document under `.ai/` composes *additively*
with the ones above it; there is no override mechanism, so a subtree cannot switch this off.

## Common failures

| Message | What happened |
|---|---|
| `a worktree is already registered at …` | The name is taken. Use it, or remove it first. |
| `… already exists and is not a registered worktree` | Something else occupies the destination. |
| `branch … is already checked out at …` | Git allows one checkout of a branch at a time. |
| `worktree … is dirty (…)` | Commit or stash there, or pass `--force`. |
| `worktree … is locked` | `git worktree unlock <path>`. |
| `… is this repository's primary checkout` | Worktree commands never move or delete it. |
| `… is not a registered worktree of this repository` | The path belongs to another repository, or to none. |
| `'…' matches N worktrees` | Name one exactly; destructive commands do not guess. |
| `the canonical worktree root … is itself a registered worktree` | The container path is occupied by a checkout. Move it by hand first. |
| `worktree management requires a non-bare primary checkout` | Bare repositories are out of scope. |
| `the worktree root suffix … is carrying a quote character` | A quoted scalar with a trailing comment on the same line; put the comment above the key. |

## Bare repositories

Not supported, and refused by name rather than mishandled. Worktree management is defined
against a primary checkout, and a bare repository has none.

## Where things are

| | |
|---|---|
| Canonical policy | `worktree:` in `.ai/repo/policy.yaml` |
| Policy schema | `share/schemas/majordomus/policy/policy.v1.schema.json` |
| Doctrine | `.ai/repo/rules/project/worktree-layout.v1.md` |
| Decision | `.ai/repo/adrs/0020-linked-worktrees-live-in-a-sibling-container-derived-from-t.md` |
| Implementation | `apps/majordomus-cli/src/worktree/` |
| Capabilities | `apps/majordomus-cli/src/capability/builtin/worktree.rs` |
| Command line | `apps/majordomus-cli/src/commands/worktree.rs` |
| Doctor check | `lib/worktree.sh` |
| Tests | `apps/majordomus-cli/tests/worktree.rs`, `test/cases/96_worktree_layout.sh` |

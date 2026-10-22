---
id: project.worktree-topology
version: 1
kind: rule
title: A branch's worktree is at <repo>-wt/<branch>, derived from git, never registered
description: Every non-trunk branch that is worked on has exactly one linked worktree, at the primary checkout's sibling container `<repo>-wt` under the branch's own name with its hierarchy kept; the primary checkout hosts the trunk; the path is derived from git identity by the executable's worktree service and recorded nowhere; a worktree somewhere else is a typed diagnostic that only an explicit, fingerprint-verified migration repairs, and no repair ever destroys uncommitted work.
statement: Start work on a branch with `majordomus worktree create <branch>` and work in the path it derives; never choose a worktree path, never register one, never commit a feature branch from the primary checkout or from a worktree that is not the branch's canonical one, and bring a misplaced worktree home with `majordomus worktree migrate` rather than continuing there.
status: active
class: blocking
depends_on: [project.interfaces-are-projections@1, project.rust-canonical-declaration@1, project.no-machine-paths@1]
tags: [git, worktree, filesystem, safety, agents, coordination]

x-majordomus:
  tests: [test/cases/96_worktree_topology.sh, scripts/ci/worktree-check]
---

# Rationale

Several sessions work on one repository at once, and each needs its own checkout of its
own branch. Git provides the linked worktree and takes its destination as an argument, so
every caller decides where one goes — and fifty-three of them decided differently before
this rule (ADR 0021). The cost was not the disorder; it was that no person and no agent
could answer "where is branch X" or "where does a new one go" without looking or inventing,
and an answer that has to be invented every time is invented differently every time.

The topology removes the decision. Given the repository's identity — the primary checkout,
read from git's common directory and never from the current directory — and a branch name,
there is exactly one path, and it is the same answer from the primary checkout, from a
linked worktree, and from a session that does not know where it was started.

# Required behaviour

1. **The derivation is one function of two inputs.** The container is the primary
   checkout's sibling, its directory name with `-wt` appended; a branch's worktree is the
   branch name under the container, one directory per `/`-separated component, hierarchy
   kept. `feature/providers/openai-streaming` lives at
   `<repo>-wt/feature/providers/openai-streaming`. Nothing flattens, hashes, numbers or
   renames.
2. **Git is the registry.** No policy section, configuration file, table or list records a
   container or a branch's path. The executable reads the common git directory, `git
   worktree list --porcelain` and `for-each-ref`, and derives everything else. Adding a
   branch requires editing nothing anywhere.
3. **The primary checkout hosts the trunk.** The trunk is discovered — the remote's HEAD,
   `init.defaultBranch`, the one conventional name, the primary checkout's own branch —
   never hardcoded. Every non-trunk branch that is checked out has exactly one worktree,
   at its canonical path.
4. **Work starts with the tool.** `majordomus worktree create <branch>` (or `ensure`)
   derives the destination, validates the branch name by git's own rules, creates the branch
   from the trunk when it is new, holds the repository's worktree lock, refuses a
   destination that is occupied or a branch that is checked out elsewhere, and verifies
   what git registered. A bare `git worktree add <path>` is how a worktree ends up where
   nobody will find it.
5. **An agent does not choose a path.** Before implementing, it establishes the current
   branch, the current worktree, the canonical worktree and the issue if the branch names
   one (`majordomus worktree status`); a mismatch is corrected with the tool, never
   worked around by continuing in the primary checkout or wherever the session happened to
   start.
6. **A commit is guarded.** The pre-commit hook asks `majordomus worktree guard`, which
   refuses a feature branch committed from anywhere but its canonical worktree and the
   primary checkout committed on a branch that is not the trunk. Detached worktrees are
   exempt. The hook carries no logic of its own.
7. **A worktree somewhere else is a diagnostic, and only migration repairs it.** Every
   condition has a stable code (`worktree.path_mismatch`, `worktree.container_occupied`,
   `worktree.destination_conflict`, `worktree.ephemeral`, ...) carried by the command line,
   the API, MCP, the Cockpit and the tests, each with a remedy. `worktree migrate --plan`
   shows the moves and changes nothing; `worktree migrate` makes them.
8. **Migration is lossless or it is not a migration.** A dirty worktree is moved as it is —
   modified, staged, unstaged and untracked files included — never reset, stashed, cleaned
   or checked out. A fingerprint is taken before and after each move and compared; a step is
   reported as moved only when they are equal, and a difference is reported as a failure
   with what differs. A destination that exists is never overwritten. A locked worktree
   waits to be unlocked. A detached worktree is never moved; a session's scratch checkout
   (under the temporary directory or `.claude/worktrees/`) is reported and moved only on
   request.
9. **Removal is explicit and keeps the branch.** `worktree remove` refuses uncommitted work
   without `--force`, never touches the primary checkout, and never deletes a branch.
   Cleanup eligibility — merged into the trunk, clean or not checked out — is derived state
   and acted on by a person.
10. **The projections follow.** The four `worktree.*` capabilities are declared once and
    project to MCP, HTTP, OpenAPI, the Cockpit and the generated reference; the command line
    renders the same typed answers; `-wt` is written once in the crate and the CI check holds
    every document and template that names it to that constant.

The rule is repository-wide. A context document under `.ai/` may add to it and may not
weaken it.

# Failure behaviour

`majordomus worktree guard` exits 10 with the diagnostic and its remedy; the pre-commit
hook relays the exit code, so the commit does not happen. `worktree status`, `list`,
`topology`, `validate` and `doctor` exit 10 while an error-level diagnostic stands.
`worktree create` refuses, exit 10, naming what is in the way, and creates nothing.
`worktree migrate` leaves a step blocked or failed with its reason and exits 10 when any
step did not complete; nothing half-moved is reported as moved.

`scripts/ci/worktree-check` fails the build when the hook no longer asks the guard, when
`-wt` appears in the crate anywhere but its one constant, when a document or template that
names the container disagrees with the constant, or when the behavioural case fails.

# Verification

`cargo test --manifest-path apps/majordomus-cli/Cargo.toml --test worktree` (the
derivation, the identity from every directory, the standings, the migration of dirty
worktrees with fingerprints, the container occupant, conflicts, locks, the ephemeral
standing, concurrency, parity between the command line and the registry);
`test/cases/96_worktree_topology.sh` (the built executable through the shell tool's own
wiring: the hook refuses, doctor sees the enforcement entry, the migration moves and
verifies); `scripts/ci/worktree-check` (the gate).

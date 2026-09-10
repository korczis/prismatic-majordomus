---
id: project.worktree-layout
version: 1
kind: rule
title: Every linked worktree lives in the repository's sibling container
description: A linked git worktree of this repository belongs at <parent>/<repo>-wt/<name>, derived from the primary checkout and the canonical policy; the primary checkout stays where it is, worktrees are created with `majordomus worktree create`, and one outside the container is a violation doctor reports and only an explicit migration repairs.
statement: Create a linked worktree only under the container the policy derives, with `majordomus worktree create`; never at a path you chose yourself, and never with a bare `git worktree add`.
status: active
class: blocking
depends_on: [project.interfaces-are-projections@1, project.rust-canonical-declaration@1]
tags: [git, worktree, filesystem, safety, agents]
x-majordomus:
  validator: worktree_layout
  category: worktree
  enforced_by: [doctor]
  exit_code: 10
  claims: []
  tests: [test/cases/96_worktree_layout.sh]
---

# Rationale

A repository worked on by several people and several agents at once grows worktrees fast,
and each one has to go *somewhere*. Left to whoever creates it, that somewhere is a
different answer every time: inside the repository (where git then has to be told to ignore
it, and a build tool will find it anyway), in `/tmp` (where it survives exactly until the
machine reboots, taking uncommitted work with it), in a home-directory registry (which has
to be searched to find out what belongs to what), or beside the checkout under a name that
made sense to one person on one afternoon.

The cost is not the disorder. It is that nobody — no person and no agent — can answer
"where are this repository's worktrees" without looking, and "where should this new one go"
without deciding. A decision that has to be made every time is a decision that will be made
inconsistently, and the artefacts of inconsistency are directories full of work that nobody
remembers creating.

The sibling container removes the decision. `<parent>/<repo>-wt/` is derived, not chosen:
it follows from where the primary checkout is and one suffix in the policy. It is outside
the source tree, so nothing is committed and no ignore rule is needed. It is beside the
checkout, so it is found by looking next to the thing you already have. It holds exactly one
repository's worktrees, so deleting it is bounded. And it is the same answer from the
primary checkout and from four directories deep inside a linked worktree, which is what
makes it usable by an agent that does not know where it was started.

# Required behaviour

1. **The primary checkout stays where it is.** It is the thing the container is named after
   and is never moved into it. It is exempt from this rule by definition, and a check that
   counted it as a violation would fire in every repository.
2. **Every linked worktree sits directly under the container.** The container is
   `parent(primary) / basename(primary) + suffix`, with `strategy` and `suffix` read from
   `worktree.root` of `.ai/repo/policy.yaml`. That section is the single source of truth:
   no command, document, template, test or provider file writes the suffix itself.
3. **The container is derived, never remembered.** It is computed from the repository's
   identity — the common git directory and the main work tree git lists first — and not from
   the current directory, an environment variable, or the marker file inside it. Run from a
   linked worktree, the answer is the repository's container and never a container inside
   that worktree.
4. **Worktrees are created with `majordomus worktree create`.** It derives the destination,
   validates the name, branch and base, holds the repository's worktree lock, creates the
   container if it does not exist, and verifies afterwards that what git registered is where
   the policy says it belongs. A bare `git worktree add <path>` is how a worktree ends up
   somewhere nobody will find it.
5. **An agent does not choose a path.** It chooses what to work on; where that goes is not
   its decision, and a prompt asking it to pick a directory is a defect in the prompt.
6. **`majordomus doctor` reports every linked worktree outside the container**, one finding
   each, naming the worktree, its branch, the container, and the command that plans the move.
7. **Nothing is deleted or moved to satisfy this rule on its own.** `doctor`, `init` and
   `update` diagnose and never repair. A worktree outside the container keeps working.
8. **Migration is explicit.** `majordomus worktree migrate --plan` shows every move and
   changes nothing; `--apply` carries out the moves that are safe and reports the rest with
   the reason each was refused.
9. **A dirty or locked worktree is never moved or removed silently.** `worktree.cleanup` of
   the policy says so, `--force` is the only override, and even `--force` does not waive the
   proof that the path is a registered worktree of this repository and is not the primary
   checkout.
10. **Removing a worktree never removes a branch.** The two lifecycles are separate, and
    deleting a branch is a command a person types on purpose.

The rule is repository-wide. A context document under `.ai/` may add to it and may not
weaken it: the composition of directory contracts is additive, and there is no override
mechanism that could remove a filesystem-safety rule for a subtree.

# Failure behaviour

`majordomus doctor` dispatches `mj_validate_worktree_layout`, which reads the topology from
the Rust executable's `worktree.list` capability and reports one `FAIL` per violation under
the `worktree` category, exit code 10. The severity is the policy's:
`worktree.enforcement.outside_root` is `error` by default, and `warn` or `off` change what a
violation costs without changing what is reported. Where the executable is not built, the
check reports `INFO` and is recorded as skipped, never as passed.

`majordomus worktree status` and `majordomus worktree list` exit 10 when the layout rule
fails, so a script and a hook can gate on them without reading any output.

This is a machine-local check about this machine's directories. It is deliberately not a CI
gate: a CI runner's checkout is a normal checkout and is never expected to sit under a
container. What CI validates is the repository-level half — that the policy parses against
its schema, that the provider projections are current, and that the behavioural cases pass.

# Verification

`bash test/run.sh 96_worktree_layout` creates a disposable repository, adds a worktree
outside the container, and proves that doctor fails on it, that the migration plan proposes
the move without making it, and that doctor made no change.
`cargo test --manifest-path apps/majordomus-cli/Cargo.toml` proves the derivation, the
identity resolution from inside a linked worktree, the naming, the safety refusals and the
locking. `majordomus generate --check` proves the provider projections still carry the rule.

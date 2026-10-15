+++
title = "A personal layer changes nothing shared"
description = "A personal layer changes nothing shared"
weight = 68
[extra]
kind = "rule"
slug = "project-a-personal-layer-changes-nothing-shared-1"
identity = "project.a-personal-layer-changes-nothing-shared@1"
status = "active"
source = ".ai/repo/rules/project/a-personal-layer-changes-nothing-shared.v1.md"
+++
{% raw %}

## Rationale

A team adopts a tool on evidence, and evidence comes from use. Until this rule a developer
could not use the tool on real work without changing files the whole team shares, so a
personal trial became a proposal before there was anything to propose. The hand-written
exclude file that stood in for an answer was verified by nothing, written past by `update`
and `capture install`, and unreadable to the executable, which enumerates tracked files.
"I changed nothing you share" is a claim about other people's work, and a claim like that
is checked by the command that makes it. ADR 0145.

## Required behaviour

1. **One cover, git's own.** The layer and every path written for it are listed in a marked
   block of `$GIT_COMMON_DIR/info/exclude`, anchored, one a line. Lines outside the block
   are never changed. The tracked `.gitignore` is not written.
2. **No stored mode.** `mj_personal_active` reads the block. No manifest key, policy key,
   environment variable or state file says "personal".
3. **Cover before write.** A writer outside the layer calls `mj_personal_guard <path>…`
   before it writes. Under a personal layer the guard covers the path and the staging name
   beside it; elsewhere it does nothing, so the call is unconditional.
4. **A tracked path is the project's.** It is refused with exit 15 while the command is
   still planning, before any path is written and before any advice to force it; a dry run
   refuses the same way. `doctor` does not hold such a file to a stamp or a bootstrap line.
5. **Verified where it is claimed.** `init --personal` exits 10 when the status after
   differs from the status before. The installer compares the status itself. `doctor`
   reports `OK personal` or names what is tracked, uncovered or shown.
6. **The project's README is not asked to name the bootstrap** under a personal layer.
7. **Discovery is derived.** With the manifest on disk and tracked nowhere, the executable
   walks the work tree; no option selects it.
8. **Sharing is explicit.** `init --extend --shared` removes the block, writes the two
   `.gitignore` lines of a plain init, prints the paths that are now the repository's to
   commit, and commits nothing. It refuses a layer that is not personal. A path that is
   one person's by its own name (`*.local`, `*.local.*`) stays covered.

## Failure behaviour

`test/cases/2979_a_personal_layer_changes_nothing_shared.sh` fails when, in a repository
that already holds uncommitted work, `init --personal`, `update`, `capture install` or an
`--extend` changes `git status` or the tracked `.gitignore`; when a projection target the
policy names is not in the block; when `update` writes any target after meeting a tracked
one; when `doctor` does not name a tracked file of a personal layer, or fails the README;
when a tracked layer is accepted as personal; when a linked worktree shows the layer; when
the executable lists no rule of an untracked layer; when `--shared` leaves the block,
commits, or accepts a layer that is not personal; or when the installer reports success
without verifying. `test/cases/86_installer_platform.sh` fails when the installer's parser
accepts an option its `--help` does not document.

## Verification

```
MJ_TEST_JOBS=2 bash test/run.sh 2979_a_personal_layer_changes_nothing_shared 86_installer_platform
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --doc layer_is_untracked
```
{% endraw %}

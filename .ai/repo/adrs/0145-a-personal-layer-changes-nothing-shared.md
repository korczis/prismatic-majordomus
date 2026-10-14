---
schema: adr/v1
id: adr-0145
kind: adr
title: "A personal layer changes nothing shared: it is covered by git's own exclude file, checked where it is claimed, and handed to the team by one command"
status: proposed
date: 2026-10-12
tags:
  - architecture
  - distribution
  - adoption
  - discovery
  - enforcement
provenance:
  origin: authored
related:
  - file:.ai/repo/adrs/0005-one-projection-plan-canonical-owners-and-the-site-as-registry-view.md
  - file:.ai/repo/adrs/0019-distribution-is-one-model-and-every-surface-that-ships-the-t.md
  - file:.ai/repo/adrs/0027-a-command-is-declared-once-and-every-surface-is-a-projection.md
  - file:lib/common.sh
  - file:lib/init.sh
  - file:lib/update.sh
  - file:lib/capture.sh
  - file:lib/doctor.sh
  - file:share/install/install.sh.in
  - file:apps/majordomus-cli/src/discovery/mod.rs
  - file:apps/majordomus-cli/src/app.rs
  - file:test/cases/2979_a_personal_layer_changes_nothing_shared.sh
---

# 145. A personal layer changes nothing shared: it is covered by git's own exclude file, checked where it is claimed, and handed to the team by one command

## Context

`majordomus init` makes a layer the repository owns. Its tracked half, `.ai/repo/`, is
meant to be committed; the line that keeps the local half out is written into the tracked
`.gitignore`; `update` writes `AGENTS.md`, `CLAUDE.md` and `GEMINI.md` at the root; and
`doctor` fails until `README.md` names `AGENTS.md`. Each of those is a change to what the
whole team shares.

That is the right way to adopt the tool as a team and the wrong only way to meet it. A
team decision deserves evidence, and evidence comes from use. A developer who wants to try
the tool on real work — in the team's repository, a client's, or an open-source project's
— had to change shared files to do it, which turns a personal trial into a proposal before
there is anything to propose. The workaround was a hand-written exclude file that nothing
verified, that `update` and `capture install` wrote past, and that the Rust executable
could not read through: discovery asks git for tracked files, so an untracked layer is a
layer with no files in it (`--discovery filesystem` existed, and nothing passed it).

## Decision

1. **Personal is a way to install, not a second layout.** `majordomus init --personal`
   creates the same files a plain init creates, in the same places. What differs is whose
   they are. Nothing is relocated, so every command, path and document means what it meant
   before, and what a person learns in personal mode is what the team would get.

2. **The cover is git's own exclude file.** The layer is covered by an anchored pattern in
   `$GIT_COMMON_DIR/info/exclude`, inside a marked block, with every other line of the
   file left alone. git never commits, pushes, clones or stashes that file, and every
   worktree of the repository obeys the common one, which is why the common directory is
   asked for rather than `.git` — in a linked worktree `.git` is a file. The tracked
   `.gitignore` is not written.

3. **There is no setting.** A layer is personal when the exclude block covers it. The
   block is the mechanism and the record at once, and it is read each time it is asked
   (`mj_personal_active`), so an `--extend` of a personal layer stays personal without the
   flag. Nothing stores a mode that could disagree with the repository.

4. **Every writer outside the layer covers before it writes.** `update` and `capture
   install` call one guard (`mj_personal_guard`) for each path they are about to write: it
   adds the path, and the staging name beside it, to the block. A path git already tracks
   is the project's and is refused, before anything is written, because half an update is
   worse than none. Outside a personal layer the guard does nothing, so a writer calls it
   unconditionally and there is no second code path to keep in step.

5. **The claim is checked where it is made, twice, by programs that share nothing.**
   `init --personal` records `git status --porcelain --untracked-files=all` before it
   writes and exits 10 unless it reads the same afterwards. The installer's `--personal`
   runs `init --personal` and compares the status itself, before and after, having written
   none of it. `doctor` holds the layer to the claim on every run and names a file of it
   that was tracked anyway; it does not ask the project's `README.md` to name the
   bootstrap, because that line would be a change to a shared file.

6. **Discovery is derived.** When the manifest is on disk and the index holds no entry for
   it, the executable walks the work tree instead of asking the index
   (`discovery::layer_is_untracked`). It is asked of git at each start. No flag has to be
   remembered by a launcher, a hook or an MCP client configuration, and a layer that is
   committed later is read through the index from that moment.

7. **Handing it to the team is one command, and still the person's decision.**
   `majordomus init --extend --shared` removes the block, adds the two `.gitignore` lines
   a plain init adds, and prints the paths that are now the repository's to commit. It
   commits nothing. From then on the layer is an ordinary one.

## Consequences

A personal layer is one person's, on one machine. It does not travel with a clone, a push
or a stash, so handovers to another machine go through the mesh or not at all, and a
colleague who wants the same installs their own. The guide says so.

Personal mode keeps a layer apart from what the repository shares; it is not access
control and not a way around an organisation's rules about tools. The files are on the
disk in plain sight, and whether a tool may be used on a codebase is the organisation's
decision, made the way it makes such decisions.

The commit and push hooks are not installed by a personal init any more than by a plain
one. `.git/hooks/` is already local, so the two lines `doctor` names can be added there; a
repository whose `core.hooksPath` points at a tracked directory keeps its own hooks, and
`doctor` says which.

`scripts/derive` and `generate` keep their contract — a derive sees what the commit will —
which means a personal layer is not a source of committed projections. It has none to
commit until it is shared.

## Alternatives refused

- **A mode recorded in the manifest or the policy.** Two sources for one fact: the file
  would say personal while the index said tracked, and something would have to decide who
  wins. The exclude block cannot disagree with itself.
- **Relocating the layer outside the work tree** (an environment variable naming another
  directory). Every path in every document, hook and client configuration would mean two
  things, and the worker reading `.ai/README.md` would not find it.
- **Writing `.gitignore` and marking it `assume-unchanged`.** The change is still in the
  work tree, `git stash -u` and a checkout of another branch both meet it, and the flag is
  lost on the next index rewrite.
- **A global `core.excludesFile` entry.** It covers `.ai/` in every repository on the
  machine, including the ones that track their layer.

## Status

Proposed. Implemented with rule `project.a-personal-layer-changes-nothing-shared`, claim
`personal-layer-changes-nothing-shared`, case 2979 and issue I2491.

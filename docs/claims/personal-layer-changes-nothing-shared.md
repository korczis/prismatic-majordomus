# A layer installed with --personal is one person's own in a shared repository: it changes no tracked file, no .gitignore line and nothing git status shows, and the command that makes the claim verifies it

## What it means

`majordomus init --personal`, and the installer's `--personal`, create the same layer a
plain `init` creates. What differs is whose it is. After the command, in a repository that
already held uncommitted work:

- `git status --porcelain --untracked-files=all` prints exactly what it printed before;
- no tracked file was written, `.gitignore` and `README.md` included;
- nothing under `.ai/` is tracked or staged, and `git add -A` would stage none of it;
- the later writers keep it so: `update` and `capture install` cover each file before
  writing it, and leave a file the project already tracks as it is.

When the trial has made its case, `majordomus init --extend --shared` hands the layer to
the team: it removes the cover and prints what is now the repository's to commit.

## How it works

The cover is a marked block in `$GIT_COMMON_DIR/info/exclude`, the exclude file git reads
for a repository and never commits, pushes, clones or stashes. The common directory is
asked for, not `.git`, so a linked worktree is covered by the same file. The helpers are in
`lib/common.sh`: `mj_personal_cover` writes the block and leaves every other line alone,
`mj_personal_active` reads it, `mj_personal_guard` is what a writer calls before it
writes, `mj_personal_verify` is the check `init` and `doctor` share, and
`mj_personal_uncover` is what `--shared` runs.

No mode is stored. A layer is personal when the block covers it, so an `--extend` stays
personal without the flag and nothing can say "personal" while the repository says
otherwise.

The executable reads an untracked layer without being told to: when the manifest is on
disk and the index holds no entry for it, discovery walks the work tree
(`discovery::layer_is_untracked` in `apps/majordomus-cli/src/discovery/mod.rs`, selected in
`apps/majordomus-cli/src/app.rs`).

## How to see it

```bash
git status --porcelain --untracked-files=all > /tmp/before
majordomus init --personal
git status --porcelain --untracked-files=all | diff /tmp/before -
majordomus doctor | grep personal
bash test/run.sh 2979_a_personal_layer_changes_nothing_shared
```

The case works in a repository with a modified file and an untracked one, because a clean
tree staying clean proves little. It runs `init --personal`, `update`, `capture install`
and an `--extend`, and compares the status after each. It commits a `CLAUDE.md` and
requires `update` to refuse it and write nothing. It tracks one file of the layer and
requires `doctor` to name it. It shares a layer and requires the cover gone and nothing
committed. It installs through the real installer against a fixture release and requires
`Verified (personal)`.

## What it does not cover

Personal mode keeps a layer apart from what the repository shares. It is not access
control: the files are on the disk in plain sight. It is not a way around an
organisation's rules about tools either; whether a tool may be used on a codebase is the
organisation's decision.

It does not install the commit and push hooks; `.git/hooks/` is local, so the two lines
`doctor` names can be added there, and a repository whose `core.hooksPath` points at a
tracked directory keeps its own. A personal layer does not travel — not with a clone, a
push or a stash — and it is not a source of committed projections, because a derive sees
what the commit will. A tool that writes into the repository without calling the guard is
not covered; the three writers this distribution ships are.

## Why it exists

A team adopts a tool on evidence, and evidence comes from use. Before this, using the tool
on real work meant changing files everyone shares, so a personal trial became a proposal
before there was anything to propose. "I changed nothing you share" is a claim about other
people's work, and the hand-written exclude file that used to stand in for it was checked
by nothing. ADR 0145 decides it; rule `project.a-personal-layer-changes-nothing-shared`
holds it.

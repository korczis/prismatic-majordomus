---
id: project.a-derive-sees-what-the-commit-will
version: 1
kind: rule
title: A derive sees what the commit will
description: scripts/derive refuses, before it waits for the lock or builds anything, when the tree holds an untracked file that is not ignored, because a derive projects only what git tracks and the commit that adds the file would be refused.
statement: Stage every new file before you derive, and ignore or remove what will never be committed. scripts/derive refuses an untracked, non-ignored file before it takes the machine's lock and names each one.
status: active
class: blocking
depends_on: [project.derived-files-regenerated@1, project.one-derive-at-a-time@1]
tags: [derived, machine]

x-majordomus:
  tests: [test/cases/1020_a_derive_sees_what_the_commit_will.sh]
---

# Rationale

A derive projects what git tracks. `majordomus generate` enumerates through the index
(`--discovery vcs`, the layer's contract) and the site generator reads `git ls-files`. That
is right, because the committed artifacts must project the committed tree. But it means a
file the next commit adds is invisible to the derive that precedes the commit. The
projections leave the file out, and the `derived-current` gate refuses the commit after the
derive has cost its minutes.

On 2026-10-09 that cost the 0.19.0 release two extra derives on a machine at load 40. An
episode record and two generated pages were untracked when the derive ran. A
`majordomus generate` repaired the registry and moved an input of the site data, so a full
derive had to run again behind the queue. A memory note that already read "stage new files
before derive" did not prevent it. A note can be skipped; a refusal at the right moment
cannot.

# Required behaviour

- Before it waits for the lock or builds anything, `scripts/derive` lists
  `git ls-files --others --exclude-standard`. If anything is listed, it names every file,
  says to `git add` it or to ignore or remove it, and exits 10.
- What git ignores never refuses: `.gitignore`, `.git/info/exclude` and the global excludes.
  The release job keeps its downloaded artifacts in `dist/`, which `.gitignore` names for this
  reason.
- `MAJORDOMUS_DERIVE_ALLOW_UNTRACKED=1` lets the derive run with a warning that names how many
  files it cannot see. It is for a worker who knows the files will never be committed.
- `scripts/derive --preflight` runs this check alone. `scripts/derive --locked -- <command>`
  projects nothing and owes no preflight.
- The arguments are judged first, so a mistyped flag is a usage error (exit 2) and never a
  refusal of the tree.

# Failure behaviour

The refusal lands before the lock is taken, so a refused derive never holds the queue. It
costs the time `git ls-files` takes.

# Verification

`test/cases/1020_a_derive_sees_what_the_commit_will.sh` runs a copy of `scripts/derive` in a
disposable repository and proves each behaviour:

- an untracked page is named and refused with exit 10, by `--preflight` and by a full derive
  alike;
- a refused derive never builds and never takes the lock;
- the same file, once added, passes;
- files ignored by `.gitignore` and by `.git/info/exclude` never refuse;
- the override goes on with its warning;
- `--locked` owes no preflight.

Two mutations each fail the case: removing the preflight from the derive's path, and dropping
`--exclude-standard`.

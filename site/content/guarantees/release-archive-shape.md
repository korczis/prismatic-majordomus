+++
title = "A release archive carries every path once and carries nothing but files and directories"
description = "The tar that a release publishes holds regular files and directories and nothing else: no"
weight = 20
[extra]
claim_id = "release-archive-shape"
status = "guaranteed"
source = "docs/claims/release-archive-shape.md"
+++
{% raw %}

## What it means

The tar that a release publishes holds regular files and directories and nothing else: no
hard link, no symbolic link, no device node, no duplicated path. An installer that unpacks
it is unpacking a tree, not following a graph.

## How it works

Two independent refusals, and the first is at the source:

- `scripts/release-package` reads back the archive it has just written, and deletes it
  rather than return one that carries a repeated path or an entry that is not a file or a
  directory. There is deliberately no fallback packer: if `tar` cannot pack the list it was
  given, the release stops, because an archive packed some other way is not the archive
  anything verified.
- `scripts/release-verify` checks the same two properties of the archive it is handed, in
  the release workflow's build job, on the runner that built it.

The installer relies on the second property for its own safety: it refuses an archive
carrying a link before it unpacks anything, which is what stops an archive from writing
outside the directory it claims.

## How to see it

```bash
bash test/run.sh 87b_release_archive_shape
```

That case packs a real archive from the tree it runs in and asserts both properties of it,
then reproduces the packer that violated them — a `tar` on `PATH` with `--no-recursion`
stripped out — and asserts that the packer refuses its own output and leaves no archive
behind.

## What it does not cover

Bit-identical rebuilds. Two builds of one tag produce functionally identical archives with
the same entries in the same order; they are not guaranteed to be byte-identical, because
that needs a reproducible compiler invocation the toolchain here does not pin.

## Why it exists

Release `v0.2.0` was tagged and never published. The packer passed a complete file list to
a `tar` that also recursed into it, so every path was archived twice; GNU tar writes the
second copy of a path as a hard link; the verifier refused all 931 of them; the build job
exited 10; `fail-fast` cancelled the other five targets; and publication never ran. For a
day and a half the advertised one-line installer answered "no stable release is published
yet" — a correct message about a state that a silent fallback in one shell script had
created. The property is now checked where the archive is written, so that the same class
of failure costs a second rather than a release.
{% endraw %}

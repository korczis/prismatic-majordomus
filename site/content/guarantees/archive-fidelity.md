+++
title = "An archive that lost the file modes fails rather than travelling"
description = "majordomus archive writes a snapshot of the tracked tree and then opens it again before returning. The entry count in the container must equal what was staged, and if executable files went in and none came back out, the command fails with exit 13 and a reproduce command instead of handing you a copy in which nothing can run."
weight = 38
[extra]
claim_id = "archive-fidelity"
status = "guaranteed"
source = "docs/claims/archive-fidelity.md"
+++
{% raw %}

## What it means

`majordomus archive` writes a snapshot of the tracked tree and then opens it again before returning. The entry count in the container must equal what was staged, and if executable files went in and none came back out, the command fails with exit `13` and a reproduce command instead of handing you a copy in which nothing can run.

## How it works

The mode of every entry is read from the **git index** — the authority on what a file is, rather than the working tree, whose modes an earlier careless copy may already have flattened. It is written into `_ARCHIVE/MANIFEST.txt` as `<mode> <size> <path>`, and `_ARCHIVE/restore.sh` inside the archive re-applies every one of them at the far end and creates a local git index, because this repository enumerates itself with `git ls-files` and a check run without one examines nothing and reports that nothing is wrong.

The container is written without `zip -X`. That flag strips the extra fields that carry the Unix mode, which is how the failure this claim exists for was introduced: an archive with the right entry count, every byte of content present, and not one executable in it.

## How to see it

```bash
majordomus archive audit --out /tmp/a.zip
cd /tmp && unzip -q a.zip && cd <repository>
ls -l scripts/derive          # still 755
sh _ARCHIVE/restore.sh        # modes re-applied, one commit, a real index
scripts/ci/command-furnished  # a gate of the repository, running in the copy
```

## What it does not cover

The restored copy has one commit, no remotes and no reflog. Any check that reads more than the working tree will say so; none of them is made to pass by this claim. Nor is the archive signed, reversible into the original repository, or a backup.

## Why it exists

A defect at the far end cannot be distinguished there from a defect in the repository. An archive is believed by someone who cannot compare it with the original, so the comparison has to happen before it is sent.
{% endraw %}

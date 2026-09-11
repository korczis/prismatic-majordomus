+++
title = "Hand the repository to someone who cannot clone it"
description = "Take a snapshot that a model can read or a gate can run in, without sending anything untracked and without losing the file modes on the way."
weight = 26
[extra]
id = "hand-the-repository-to-a-reader"
source = ".ai/repo/use-cases/hand-the-repository-to-a-reader.md"
category = "knowledge"
maturity = "described"
+++

## Situation

The repository has to go somewhere it cannot be cloned: into a language model's context, to a reviewer on a locked-down machine, onto an air-gapped network, into an audit that must run the checks on a copy. Done by hand it is a `git ls-files`, a `tar` and a list of exclusions retyped from memory, and it goes wrong in two ways that only appear at the far end — where nobody can compare the copy with the original, and where both failures read as the repository being broken rather than the copy being wrong. Something untracked travels that should not have, or the file modes are gone and nothing in the tree will run.

## What you run

- `archive --list`: the profiles, each one a different reader and therefore a different snapshot
- `archive --dry-run`: what would travel, with every file left out attributed to the rule that left it out
- `archive`: the snapshot, verified through its own container before the command returns

## Scenario

```yaml
setup: installed
given:
  - 'a repository with the layer installed and its files committed'
steps:
  - id: profiles
    run: ['archive', '--list']
    note: 'the snapshots are declarations in share/archive.yaml, not flags'
    expect:
      exit: 0
      stdout_contains: ['context', 'audit', 'governance']
  - id: what-would-travel
    run: ['archive', '--dry-run']
    note: 'the count archived is stated beside the count tracked, so a selection that matched nothing cannot look like one that matched everything'
    expect:
      exit: 0
      stdout_contains: ['profile context', 'tracked file', 'nothing written']
  - id: written-and-read-back
    run: ['archive', '--format', 'tar.gz']
    note: 'the archive is opened again before the command returns'
    expect:
      exit: 0
      stdout_contains: ['OK   archive', 'read back']
  - id: no-such-profile
    run: ['archive', 'nosuch']
    note: 'exit 12 is MISSING_ARTIFACT, and the message names the command that lists what exists'
    expect:
      exit: 12
      stdout_contains: ['no profile', 'archive --list']
then:
  - 'nothing untracked is in the archive, because the file set is the git index'
  - 'every entry carries the mode the index records, and the archive says so in its own manifest'
  - 'the archive was read back before the command returned, and an unreadable one is a failure rather than a delivery'
```

## Outcome

One gesture instead of a remembered incantation. What is left out is declared rather than retyped, and the declaration it reads for the generated files is `.gitattributes`, which the repository already maintains for another reason — so there is no second list to go stale. The copy arrives with its modes and can be made runnable with one script, which means a check that fails in it failed for a reason that belongs to the repository.

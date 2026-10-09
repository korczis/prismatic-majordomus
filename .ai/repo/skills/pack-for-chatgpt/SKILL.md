---
schema: skill/v1
id: pack-for-chatgpt
version: 1
title: Pack the sources for a ChatGPT project
description: Turn a committed tree into token-bounded Markdown shards that a ChatGPT project's file search indexes, verified to carry no binary, worktree, link, build output, projection or leaked value, and hand them over with the commit they were cut from.
status: active
tags: [review, archive, privacy, chatgpt]
related: [pack-for-review, report-verification-state]
inputs:
  - a committed tree at the revision the conversation is about
  - the question the ChatGPT conversation is meant to answer
outputs:
  - a verified pack directory under tmp/packs/ holding 00-INDEX.md, the shards and pack.json
  - the upload list, the commit, the token total and what the profile left out
---

# Purpose

A ChatGPT project searches its files one text file at a time, and each file has a token
ceiling. A zip is not searched, a copied directory carries whatever sits in the working
tree, and a hand-picked set of files carries the picker's assumptions. This skill hands
ChatGPT the tracked sources of one commit as a few Markdown shards, each under the
profile's token budget, and proves before anything is uploaded that no binary, gitlink or
worktree, symbolic link, build output, generated projection, machine path or credential is
in them.

It is not for a reviewer who can read the checkout (use `repo-review`), and not for a
sandbox that unpacks archives and runs the gates (use `pack-for-review` with the `audit`
profile of `majordomus archive`).

# When to use

Before uploading the repository, or a part of it, to a ChatGPT project, and again whenever
the conversation must see a newer commit. Also when an advisor consultation
(`scripts/advisor-consult`) needs the sources in a ChatGPT project rather than in a prompt.

# Procedure

## 1. Commit what the conversation is about

The pack is read from the git index and labelled with `HEAD`. Commit first; a plan whose
`index_matches_head` is false names staged changes that the commit id does not contain, and
uncommitted working-tree edits are never in a pack at all.

## 2. Plan, and read the verdict

```sh
majordomus pack plan chatgpt
```

Read every line before building. `left out:` counts what the profile drops by reason —
`worktree`, `link`, `artifact`, `binary`, `derived`, `excluded` — and lists every dropped
file that is not a projection. Exit 10 means a finding refuses the build:

- `pack.leak`: a selected file holds a machine path or a credential shape. Remove the value
  in a commit, or exclude the file in the profile. Never send it.
- `pack.file_too_large`, `pack.too_many_shards`: the selection does not fit what ChatGPT
  indexes. Narrow the profile in `share/archive.yaml`; never raise a limit past the reader's.
- `pack.empty`, `pack.no_limits`: the profile is wrong, not the tree.

Exit 12 means the tree could not be read, which is not a pass.

## 3. Build, which verifies

```sh
majordomus pack build chatgpt
```

The command writes `00-INDEX.md`, the shards and `pack.json` under `tmp/packs/`, then reads
them back with `pack verify`: every digest, no stray file, no forbidden path, no NUL byte, no
file over the token budget, no leak. Anything but `pack verify: clean` and exit 0 means the
pack is not sent. To re-check a pack later, before an upload:

```sh
majordomus pack verify tmp/packs/<repo>-chatgpt-<commit>
```

## 4. Upload and say what was uploaded

Upload `00-INDEX.md` and every shard to the project. `pack.json` is the manifest for
verification and may stay behind. Tell the conversation the commit, and ask it to cite
files by path at that commit.

# Output

The pack directory, the upload list (index plus shards), the commit and whether the index
matched it, the token total, the counts left out by reason, and the `pack verify` verdict.
An answer from the conversation is read against that commit, never against whatever the
branch has moved to since.

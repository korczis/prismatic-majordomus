---
schema: skill/v1
id: pack-for-review
version: 1
title: Pack the repository for an outside reviewer
description: Prepare a bounded, reproducible snapshot of the tracked tree for a reviewer who cannot read the repository directly, with the local half and anything secret left out and the questions the review must answer written down.
status: active
tags: [review, archive, privacy]
related: [repo-review, report-verification-state]
inputs:
  - the question the outside review must answer, and the paths it concerns
  - a committed tree at the revision to be reviewed
outputs:
  - an archive of the tracked tree at a named commit, with its file list and size
  - a brief for the reviewer naming the commit, the scope, the questions and what was left out
provenance:
  origin: prior-art
  ledger: import-2026-09-09#4
  decision: reimplemented
---

# Purpose

An outside reviewer, a person or a model without access to the checkout, sees only what is
handed over. A copied directory carries ignored files, local records and whatever secret sits
in the working tree; a hand-picked set of files carries the picker's assumptions. This skill
hands over the tracked tree at a named commit, bounded to what the question needs, with a
brief that makes the review answerable.

# When to use

When the review happens outside the repository: a second model in another tool, an external
auditor, a person without clone access. A reviewer who can read the checkout uses
`repo-review` directly and needs no pack.

# Procedure

## 1. Write the question first

One paragraph: what the reviewer must decide, the paths that matter, and what a useful
answer looks like. A pack without a question gets a summary back, not a review.

## 2. Fix the revision

Commit what is to be reviewed; the pack is of a commit, never of a working tree. Record the
full commit id. Uncommitted work is either committed first or named in the brief as absent.

## 3. Choose the file set

Start from the index, never from the directory: `majordomus archive --list` shows the
profiles, and `majordomus archive <profile> --dry-run` shows the file set and its size. The
archive reads the git index, so ignored files and the layer's local half are excluded by
construction. For a narrower question, name the paths in the brief rather than editing
the archive by hand.

## 4. Check what leaves

Read the dry run's file list for anything that must not leave: credentials, personal data,
absolute paths of a machine, material that is not yours to share.
`majordomus scope <p>` names why a path is in or out. Remove such a file from the
tree in a commit, or do not send the pack.

## 5. Build and describe

Run `majordomus archive <profile>`, record the output path, its size and the file count, and
write the brief: commit, profile, scope, the question, what is deliberately absent, and how
the reviewer should report (the `report-verification-state` states work for findings too).

# Output

The archive path, its size and file count, the commit it was built from, and the brief. The
reviewer's answer is read against that commit, never against whatever the branch has moved
to since.

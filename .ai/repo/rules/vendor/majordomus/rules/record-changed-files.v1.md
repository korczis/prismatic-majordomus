---
id: majordomus.record-changed-files
version: 1
kind: rule
title: A record's changed_files names the work, not the tool's exhaust
description: The changed_files block of a checkpoint, a handover or a closed session names what the episode produced. Paths a declaration already calls generated, projected or local are classified out, and the classification reads those declarations rather than a list of its own.
statement: A record's changed_files is the working tree classified against the declarations that say what is derived — the scope file's generated and never-read sets, the policy's projection targets, the merge driver's derived trees, and the record stores themselves — never git status verbatim; and the record states how many paths that classification excluded and by which declaration, so that the list carries its own denominator.
status: active
class: blocking
depends_on: [majordomus.session-records@1, majordomus.handover-integrity@1]
tags: [records, session, doctrine]

x-majordomus:
  validator: record_changed_files
  category: records
  enforced_by: [doctor]
  exit_code: 10
  claims: []
  tests: [test/cases/136_changed_files_classifier.sh, test/cases/281_record_excluded_accounting.sh]
---

# Rationale

Running Majordomus dirties the tree. `majordomus update` rewrites CLAUDE.md and AGENTS.md,
`majordomus generate` rewrites `share/allow/` and `share/schemas/`, `scripts/derive` rewrites
`docs/generated/` and `site/data/generated/`, and every command appends a line to the ledger.
Every record this tool writes took its `changed_files` straight from `git status
--porcelain`, so each one claimed all of that as the episode's work.

Measured, on this repository. The record at
`.ai/repo/sessions/20260910T103933Z--s-20260909152316-024f--master--38f056b--…` carries 122
changed files. Among them are the whole of `site/data/generated/`, eleven pages under
`site/content/sessions/`, twelve files under `docs/generated/` — and the three sibling
records of that very episode, plus `site/content/sessions/s-20260909152316-024f.md`, which
is that episode's own site projection. The record names itself as its own work product.

This is not cosmetic. A record is the account a later reader has of what a stretch of work
contained, and the site publishes it. A list in which the signal is three real files and the
noise is a hundred and nineteen is a list nobody reads, which makes the record an archive
rather than a memory — exactly what `majordomus.session-records` exists to prevent.

# Required behaviour

`lib/changed.sh` classifies the working tree and every record writer uses it. It carries no
list of derived paths of its own. Five things already say what is derived, each maintained
because something else needs it, and the classifier reads all five:

- `.ai/repo/scope.yaml`, `out.generated.paths` and `out.generated.names` — the tool's own
  declaration, shipped in the skeleton, so this works in any repository Majordomus
  supervises and not only in its own;
- `.ai/repo/scope.yaml`, `out.paths` — the never-read set, which is where `.ai/local/` is
  declared. A work product nobody may read is not a work product;
- `.ai/repo/policy.yaml`, `projections[].target` — CLAUDE.md, AGENTS.md, `.bb/AGENTS.md`;
- `.gitattributes`, every path marked `merge=derived` — the repository's own list of what
  its generator writes, maintained because the merge driver needs it (in this repository,
  covered by `test/cases/57_derived_merge_driver.sh`);
- the record stores themselves, by the resolvers that name them. These are not derived — a
  closed session record is a tracked object the executable indexes — but the tool writes
  them and the worker does not.

A repository that declares none of them loses nothing: the classifier then excludes nothing
and the list is what it was before. Adding a path to any of those declarations is what
changes the outcome, which is the point — a sixth list inside the classifier would be the
sixth place to forget.

## The list carries its own denominator

A filter that silently shortens a list is indistinguishable from a tree that was cleaner
than it was. So the record states the classification as well as its result:

```
changed_files:
  - deploy/docs/
  - scripts/force-merge-all
changed_files_excluded: 10
changed_files_excluded_by:
  - record-store=10
```

`changed_files_excluded` is written on every record, `0` included — a key that appears only
when something was excluded would make "nothing was excluded" and "this record predates the
classifier" the same observation. `changed_files_excluded_by` names the declaration that did
it, one `<declaration>=<count>` per entry, because "something excluded it" is a sentence
nobody can act on while `gitattributes-derived` names the file to edit when the answer is
wrong.

Measured on this repository. The record above is
`.ai/repo/sessions/20260911T072724Z--s-20260910214152-71cb--…`, which named twelve paths: ten
were sibling session records of the same episode, one of them a `.tmp.` file the store was
mid-write on. Classified, the list is two. Two with no denominator is a weaker and
differently wrong claim than "two, and ten were the store's own".

# Failure behaviour

Blocking, over two populations the records themselves tell apart.

Every record written before `lib/changed.sh` carries the unclassified list, and a record is
immutable by contract: rewriting eighteen committed records so that a check goes green would
be fabricating history, which is the thing the work this rule arrived with exists to refuse.
Those are reported by `doctor` — how many, which carries the most, which is the newest — and
never block. They age out of the stores on their own.

A record written *by the classifier* that still names a declared-derived path is not a
legacy; it is a regression, and it blocks. The two are separated by the record itself:
`changed_files_excluded` is written by every writer on this path and by nothing that came
before it, so the grandfather boundary is a property of the object rather than a date
constant or a list of eighteen filenames somebody must maintain. The boundary closes itself.

The residual is a record produced by neither writer. A hand-authored one would carry no key
and be classified as a legacy, which is why `test/cases/281` asserts that the writers always
emit the key: a missing key is then only producible by something that is not the tool, and
`majordomus.session-records` already forbids that.

# Verification

`mj_validate_record_changed_files` in `lib/changed.sh`, dispatched from `doctor`, judges the
records that exist, with one verdict per population.

`test/cases/136_changed_files_classifier.sh` judges what the writers exclude: it sets each
declaration in turn, writes one record of each kind, and asserts both halves — that the work
is named and that the exhaust is not. Its last section removes a declaration and proves the
path comes back, which is what shows the exclusion came from the declaration and not from a
constant in the classifier.

`test/cases/281_record_excluded_accounting.sh` judges what they say about it: that all three
record kinds carry the count, that the count equals the number actually excluded, that the
breakdown names two different declarations and sums to the total, that a record with nothing
to exclude states `0` rather than omitting the key, and that removing a declaration moves
the kept list and the count in opposite directions together.

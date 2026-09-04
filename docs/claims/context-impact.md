# The impact of a change set on the scoped context is reported from git — the documents, the scopes below them, moves with their ancestry, tracked sources to review, and stale projections — and an unrelated change reports nothing

## What it means

A change to the tree is a first-class event. `majordomus context affected` reads a change
set from git — the working tree against `HEAD`, the index (`--staged`), or a base ref
(`--base <ref>`), with renames detected — and names what it touches: a changed document
and every document resolved below its scope, a move with the ancestry it changed and the
identity that travelled with the file, a deletion, the manifest (every document), the
policy (the projections), and every document whose `tracks` pathspecs cover a changed
path. `majordomus context check-sync` runs validation, then every projection the policy
names against its own stamp, then the same impact report.

## How it works

`mj_ctxd_affected` in `lib/context_docs.sh` reads `git diff --name-status -M` for the
chosen change set plus untracked files, matches each path against the loaded tree, and
reports through the ordinary finding format. A tracked source change is a `WARN` naming
the document, the pathspec and the changed files, and never an exit code: the tool cannot
decide whether prose still describes code, so it names what to review rather than
demanding an edit. `check-sync` exits 10 when the tree does not validate, 11 when a
projection no longer matches its stamp (`stale-projection`), and 0 otherwise; a projection
that is absent is `doctor`'s finding, not drift.

## How to see it

```bash
majordomus context affected                              # the working tree against HEAD
majordomus context affected --base origin/master         # everything since the base
majordomus context check-sync --json                     # one finding per line
```

## What it does not cover

No generated index or lock file exists: the answer is computed from git and the tree on
every call, so there is nothing to go stale and nothing to rebuild. The tool does not
decide semantic drift in prose; it reports where to look.

## Why it exists

A hierarchy nobody re-checks after a move or a rename is a hierarchy that lies about what
applies where. `test/cases/70_context_impact.sh` and `test/cases/71_context_sync.sh`
prove each kind of change is reported, that an unrelated change is silence, and that a
semantic no-op is not drift.

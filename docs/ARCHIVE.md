# Archiving the repository

A repository leaves the machine more often than it looks. It goes to a language model
that will read all of it at once, to a reviewer who cannot clone it, to an auditor who
has to run the checks somewhere other than the checkout they came from, to a support
case, to an air-gapped network. Every one of those was being done by hand here, with
`git ls-files`, a `tar` and a list of exclusions retyped from memory, and the hand-made
version got two things wrong that only surface at the far end — where nobody can compare
the copy with the original, and where both failures read as *the repository is broken*
rather than *the copy is wrong*.

`majordomus archive` is that gesture, written down once. The reference for the flags is
[`CLI.md`](CLI.md); this document is why it is shaped the way it is, and how to change
what it produces.

## What travels

The file set is the git index, and only the git index.

Nothing untracked is ever archived: not `target/`, not `node_modules/`, not a cache, and
— the one that matters — not `.ai/local/`, which is this checkout's own state, is ignored
by git on purpose, and is never shared. That is one rule rather than a list of things to
remember to exclude, and it is what makes the command safe to point at a repository
nobody has read. A list of exclusions fails open: whatever it forgot, travels. This fails
closed.

The consequence worth knowing is that uncommitted work in a tracked file *is* archived —
the content comes from the working tree — while a new file that was never `git add`ed is
not. `_ARCHIVE/GIT.txt` records the commit, the branch and whether the tree was dirty
when the archive was taken, so the far end can tell which it is holding.

## What is left out, and who decides

Beyond "untracked", the subtractions are declared in
[`share/archive.yaml`](../share/archive.yaml), one block per profile:

| Term | Meaning |
|---|---|
| `derived: drop` | Drop every path `.gitattributes` marks `merge=derived`. |
| `binary: drop` | Drop by file extension, from the one list at the top of the registry. |
| `exclude:` | Shell globs, for what neither rule expresses. `*` matches `/` as well. |
| `include:` | Shell globs, applied last, that put a dropped path back. |

The first of those is the interesting one. Every generated projection in this repository
is already marked `merge=derived` in [`.gitattributes`](../.gitattributes), for an
unrelated reason — a merge of two branches is never the answer for a file whose content
is a function of the merged tree. An archive for a reader wants to drop exactly that set,
because each of those files is a projection of a canonical file the archive already
carries, and carrying both spends the reader's attention on the same statement twice. So
the archive reads that declaration rather than keeping a second list of generated paths.
A second list would be the duplication
[`project.commands-are-projections`](../.ai/repo/rules/project/commands-are-projections.v1.md)
exists to refuse, and it would go stale the first time someone added a generator and
updated only one of the two.

On this repository that one rule accounts for about 15 MB of the 29 MB tracked.

## Fidelity: modes, and the index

`zip -X` strips the extra fields that carry the Unix mode. The archive looks fine, the
entry count is right, every byte of content is there — and in the unpacked tree nothing
is executable. Every script fails to run, and the person at the far end reports that the
repository is broken.

A missing git index is the same class of failure and quieter still. This repository
enumerates itself with `git ls-files`; a check run in a tree with no `.git` does not fail,
it examines nothing and reports that nothing is wrong. That is the shape
[`project.empty-is-not-failure`](../.ai/repo/rules/project/empty-is-not-failure.v1.md)
was written about, arriving through the archive rather than through the code.

Both are answered inside the archive, not in a note beside it:

- The mode of every entry is read from the **git index** — the authority, not the working
  tree, whose modes a previous careless copy may already have flattened — and written
  into `_ARCHIVE/MANIFEST.txt` as `<mode> <size> <path>`.
- `_ARCHIVE/restore.sh` re-applies every mode from that manifest and creates a git
  repository whose single commit is the tree. It is POSIX `sh`, it runs where the archive
  is opened, and it is idempotent about an existing `.git`.
- Before the command returns, the archive it just wrote is opened again. The entry count
  must equal what was staged, and if executables went in and none came back out, that is
  a failure with a reproduce command — not a surprise for the recipient.

What the restored copy still lacks is history: one commit, no remotes, no reflog. Any
check that reads more than the working tree will say so rather than being quietly wrong.

## The profiles

| Profile | For | Shape |
|---|---|---|
| `context` | A model that will read the repository | Every tracked source, without the projections generated from it. The default. |
| `audit` | Running the repository's own checks on a copy | Every tracked file, nothing dropped. The derived artifacts are kept precisely because the freshness gates compare them against their sources. |
| `governance` | Handing over the operating contract | `.ai/`, `docs/`, `AGENTS.md`, `CLAUDE.md`. No code, deliberately: it answers what the rules are and cannot answer how they are implemented. |

```
majordomus archive --list            what there is, and which is the default
majordomus archive --dry-run         what would travel, and why the rest would not
majordomus archive                   the default profile, into tmp/archives/
majordomus archive audit --out /tmp/a.zip
```

Running the gates on a copy is three commands:

```
majordomus archive audit --out /tmp/audit.zip
cd /tmp && unzip -q audit.zip && cd <repository>
sh _ARCHIVE/restore.sh && scripts/ci/command-furnished
```

## Adding or changing a profile

A repository adds its own in an `archive.yaml` under `.ai/repo/`, in the same shape. A profile
declared there under an id the tool ships **replaces** the shipped one whole, rather than
merging field by field: a half-overridden profile is a third thing neither file
describes, and neither file would then tell you what you were about to send.

Adding one is a block in that file and nothing else — no code, no flag, no case
statement. What a new profile owes is the `orientation` list: the paragraphs that become
`_ARCHIVE/README.md` at the far end. Write them for someone who has the tree and none of
the context, and say what to read first; every other line of that file — the counts, the
commit, the table of what was dropped and why — is computed when the archive is taken and
must not be written down here.

## What this is not

It is not a backup, not a release artifact, and not a distribution format. It carries no
history, it is not signed, and nothing reads it back into a repository: `restore.sh`
makes the copy usable where it landed, it does not reconstitute the original. For a
release see [`DISTRIBUTION.md`](DISTRIBUTION.md); for publishing the repository's own
content see [`GITHUB_PAGES_ARCHITECTURE.md`](GITHUB_PAGES_ARCHITECTURE.md).

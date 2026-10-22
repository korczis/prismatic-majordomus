---
id: project.release-is-a-projection
version: 1
kind: rule
title: The changelog is composed and the version has one writer
description: What this project has shipped is derived from the layer's own release records, the decisions dated inside each release's window and the commits in its range, so no changelog is kept by hand; the version is stated in the two sites that must state it and raised by one command that writes both, so the two never drift apart by an edit somebody forgot to make twice.
statement: Compose the changelog from the records, the decisions and the commits the repository already holds, and raise the version with the one writer that owns both sites; a changelog anybody maintains, and a version site anybody edits alone, is a second declaration of a fact this repository already states.
status: active
class: blocking
depends_on: [project.interfaces-are-projections@1, project.generated-artifacts-are-typed@1]
tags: [release, changelog, version, projections]

x-majordomus:
  tests: [scripts/ci/release-check, test/cases/103_release_projection.sh]
---

# Rationale

A changelog is the classic file that is correct on the day it is written. Everything in one
is already in the repository that produced it — which tag was published and from which
commit, which decisions were taken and when, what each commit said it did — and the file
exists only because nothing joined those three sources. So it is written by hand, and then
it is not: the entry for the release nobody remembered is missing, and the reader cannot
tell a quiet release from a forgotten one, because a maintained changelog says nothing
about what it left out.

This repository had the second half of the same failure and not the first. There was no
changelog at all: what changed in a version lived in GitHub's release notes, outside the
tree that produced them. And the version — the one fact both shipped programs must state —
was two hand-written strings with a check comparing them. The check was real, and it ran
only when a tag was cut, so a half-applied bump could sit on the default branch until
somebody tried to publish and the release failed at its first step.

Both are the shape `DYNAMICITY.md` names: a fact written twice, one copy of which changes.
The answer here is the one the rest of the repository already uses. The changelog is not a
file anybody owns — it is composed, on every read, from the records the release pipeline
writes, the ADRs the layer holds and the commits git already has. And the version stays in
two places, because it must — an installed tree has no `Cargo.toml` and the crate is
compiled before the shell tool exists, so neither program can read the other at run time —
but it gains what it never had: a single *writer*, so that the check which proves the two
agree is proving the work of one command rather than the memory of one person.

# Required behaviour

**The changelog is composed, never kept.** Its sources are the ones the repository already
states, and it reads them and nothing else:

```text
  .ai/repo/releases/*.yaml   what shipped, when, from which commit, with which artifacts
  .ai/repo/adrs/*.md         the decisions, placed by the date each carries
  git log <from>..<to>       everything that has no object of its own
```

- a release record put into the layer becomes a section, with no document, template, index
  or list edited anywhere;
- a commit whose subject follows no convention is carried under `Other`, never dropped — a
  changelog that silently omits what it cannot classify lies by omission, and the omission
  is invisible to the reader by construction;
- what could not be read is stated rather than hidden: a shallow clone, a record naming a
  commit this clone does not have, a range that yields nothing;
- no file named `CHANGELOG` is authored, at the repository root or under `docs/`. The
  written form is a generated artifact under `docs/generated/`, and a page that renders that
  document is a projection of it rather than a second copy.

**The version is declared exactly twice, recorded once more, and written by one command.**

- `apps/majordomus-cli/Cargo.toml` — the crate's version, the authority;
- `bin/majordomus` — `MJ_VERSION`, which the shell tool prints;
- `apps/majordomus-cli/Cargo.lock` — not a declaration, since cargo derives it and nobody
  chooses the value, but a tracked *site*: the build runs `--locked`, so a bump that leaves
  it behind fails the next build with `cannot update the lock file`, which reads as a
  toolchain fault rather than as a half-applied bump. The one writer owns it too.

Both sites are named in the module that writes them
(`apps/majordomus-cli/src/release/version.rs`) and again in `scripts/release-version`, which
reads them from a tree that may have no toolchain. That second naming is the only duplication
this rule permits, and it is gated rather than trusted. `majordomus release bump` is the one
writer: it rewrites the one line each file owns and nothing else, it is idempotent, it reads
all three sites back after writing and refuses when they disagree, and it declines to invent
a version it was not given or cannot derive. What version it writes is not its own judgement:
`project.the-version-is-measured` decides that from the public contract, and this rule is
about where the number is written rather than which number it is.

**Raising the version is a repository mutation.** It writes tracked files, so no capability
answers it and the exposure policy withholds it from every machine surface; the read half —
the changelog and the version report — is a capability, and is therefore on all of them. As
everywhere else, which surface carries which follows from what running the command changes
and is configured by nobody.

# Failure behaviour

The gate `release-check` (`scripts/ci/release-check`) fails, exit 10, when:

- the two version sites the crate declares are not the two `scripts/release-version` reads —
  the drift that would have one program raising files the other never looks at;
- the two sites do not state one version, which until now was only asked at the moment of
  publication;
- the lock file records a different version for this crate than the manifest declares;
- a changelog is authored at the repository root or under `docs/` outside `docs/generated/`;
- the version the tree declares is behind the newest release the layer records, which means
  the bump that shipped it was never made.

`majordomus generate changelog --check`, through the existing `generate --check`, is what
proves the written form still matches what the composer would write from this tree.

`test/cases/103_release_projection.sh` is the behavioural half. In a disposable repository
with a real history it proves that the changelog carries a section per record and every
commit in the history, that a non-conventional subject survives, that a decision lands in the
window that contains its day, that the command line, the JSON and the generated document are
one value, that the version report names both sites and agrees on a clean tree, that
`--dry-run` writes nothing and `--exact` writes exactly one line in each of the two files,
that a record added to the layer becomes a section with nothing else edited, and that the
gate rejects both a tree whose writers disagree and a tree that keeps a changelog by hand.

# Verification

```sh
scripts/ci/release-check                    # the gate
bash test/run.sh 103_release_projection     # the behavioural case
majordomus release                          # the changelog, composed
majordomus release version                  # both sites, whether they agree, the implied bump
majordomus release bump --dry-run           # what the one writer would write
majordomus generate changelog --check       # the written form against the tree
```

`apps/majordomus-cli/src/release/` carries the unit half: that the bump is a total function
of the changes, that raising zeroes what it supersedes, that a decision belongs to the window
containing its date, and that writing the version touches only the two lines that state it.

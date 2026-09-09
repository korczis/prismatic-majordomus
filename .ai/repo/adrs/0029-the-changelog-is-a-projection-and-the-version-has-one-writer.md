---
schema: adr/v1
id: adr-0029
kind: adr
title: The changelog is a projection of what the repository already records, and the version has one writer rather than one source
status: proposed
date: 2026-09-09
tags:
  - release
  - changelog
  - versioning
  - projections
related:
  - rule:project.interfaces-are-projections
  - rule:project.derived-files-regenerated
  - rule:project.conventional-commits
  - file:apps/majordomus-cli/src/capability/builtin/release.rs
  - file:apps/majordomus-cli/src/command_graph/semantics.rs
  - file:scripts/release-version
  - file:docs/RELEASE.md
  - file:docs/DISTRIBUTION.md
provenance:
  origin: extracted
  derived_from:
    - file:apps/majordomus-cli/src/release/mod.rs
    - file:apps/majordomus-cli/src/release/changelog.rs
    - file:apps/majordomus-cli/src/release/version.rs
    - file:apps/majordomus-cli/src/release/commits.rs
---

# 29. The changelog is a projection of what the repository already records, and the version has one writer rather than one source

## Context

ADR 0002 made the capability registry the one declaration behind every interface, ADR 0027
did the same for commands, and `project.interfaces-are-projections` requires it generally.
By the time both had landed, the release was the last public fact of this repository with no
canonical declaration — and it was missing in two distinct ways.

**There was no changelog.** Not a stale one: none. What changed in a version lived in
GitHub's release notes, written by whoever cut the release, stored outside the repository
that produced it, and never read again by anything here. A person asking "what changed in
0.3.1?" had to leave the tree to find out, and no gate could notice when the answer was
wrong, because nothing in the repository claimed to hold it.

Everything such a document would say was already in the tree, and already canonical for what
it stated:

```text
  .ai/repo/releases/*.yaml   what shipped, when, from which commit   (kind release-record)
  .ai/repo/adrs/*.md         the decisions, dated                    (kind adr)
  git log <commit>..<commit> everything that has no object of its own
```

Three sources, three vocabularies, no join. A record knows its commit and its artifacts and
nothing about why; an ADR knows a decision and a date and nothing about which release
carried it; git knows every change and nothing about what any of them was for.

**Nothing raised the version.** It is stated in `apps/majordomus-cli/Cargo.toml` and in
`bin/majordomus` (`MJ_VERSION`), and `scripts/release-version --check` exits 10 when the two
disagree. The duplication is not a defect and the script says why: an installed tree has no
`Cargo.toml`, and the crate is compiled before the shell tool exists, so neither program can
read the other's copy at run time. The defect was on the other side. A check with no writer
behind it verifies a person's memory — it can only speak after someone has already edited
one file and forgotten the other, and the first step of the release procedure in
`docs/DISTRIBUTION.md` was literally an editor opened on both.

## Decision

**Join what is already declared, and add one writer.**

- **The changelog is composed, never authored.** A section per release record, newest first,
  with the unreleased work leading. Its decisions are the ADRs dated inside that release's
  window, its changes the conventional commits in its range, its artifacts the record's own
  evidence. There is no `CHANGELOG.md` in this repository and adding one would be the
  defect, not the fix.
- **The window is one interval said twice.** Decisions fall in `(previous release's date,
  this release's date]`, commits in `previous..this` — because an ADR carries a date and no
  commit, and a commit carries no date the layer indexes. Both sources write ISO-8601, so
  the comparison is lexicographic on the day the two share.
- **The commit parser is total.** A subject that is not conventional becomes kind `Other`
  and still appears, with its subject kept whole. A changelog that drops what it cannot
  classify lies by omission.
- **The version keeps two statements and gains one writer.** `majordomus release bump`
  writes both lines — and only those two lines — taking by default the bump the conventional
  commits imply: breaking is major, a feature is minor, anything else is patch, no commits
  at all is none. `--level` and `--exact` override it for what the commits do not say.
  `scripts/release-version --check` is unchanged and now proves the work of one writer
  rather than the memory of one person.
- **Every surface is derived.** The two read halves are capabilities — `release.changelog`
  and `release.version` — so the command line, `GET /api/v1/changelog`,
  `GET /api/v1/release/version`, the MCP tool and resource, and the Cockpit all render one
  value. The generated document `docs/generated/changelog.{json,yaml,md}` is a target of
  `majordomus generate`, so the existing `generate --check` is what notices that the
  changelog has stopped describing the tree.
- **Raising the version is withheld from every machine surface, without being told to.** It
  is annotated `RepositoryMutation` beside the command line, and the exposure policy of
  ADR 0027 does the rest. No capability declares it and no exception was written.

## Alternatives rejected

**A hand-written `CHANGELOG.md`.** The conventional answer, and it fails here for the reason
every hand-maintained projection in this repository has failed: it is correct on the day it
is written and decays silently afterwards, with no gate able to tell a deliberate omission
from a forgotten one. It would also have been the only public document in the tree that
restates facts three canonical sources already hold — precisely what `derived-once` and
`interfaces-are-projections` exist to refuse. The honest version of this alternative is
"maintain a fourth copy of the release history by hand", and stating it that way settles it.

**Deriving the changelog from conventional commits alone.** This is what every off-the-shelf
generator does, it needs no ADR spine, and it would have been a third of the code. It also
answers the wrong question. A commit subject says what was done; it does not say what was
decided, and in this repository the decision is the durable artifact — an ADR is read months
later by someone judging whether a constraint still holds, and a changelog that cannot say
which release carried `adr-0027` cannot be used for that. The decisions are already dated
objects of the layer; leaving them out would mean the repository's most reviewed records
appear in no release history at all.

**Collapsing the version to a single source.** Attractive, and impossible without giving
something up. The shell tool must state its version in an installed tree that has no
`Cargo.toml`; a build script writing `MJ_VERSION` into `bin/majordomus` would make a tracked
file a build output, and the crate is compiled before the shell tool exists in any case.
Generating one from the other at run time trades a check that costs milliseconds for a
runtime dependency between two programs that ship separately. The duplication is a genuine
constraint of shipping two programs from one tree, so the right target was never one source
— it was one writer plus the check that already existed.

**Parsing GitHub's release notes back into the repository.** The notes are where the history
currently lives, so importing them looks like recovering it. It would make an external,
mutable, network-reachable service the authority for a repository fact: the notes are
editable after publication by anyone with write access, they are unavailable to a build with
no network, and a fork or a mirror has none. It would also invert the direction that already
works — the release pipeline writes `.ai/repo/releases/*.yaml` from what it actually
published, so the repository is upstream of the notes and reading them back would make a
projection feed its own source.

## Consequences

Every surface that states what shipped now renders one value, and a release record landing in
the tree puts its section into the command line, the API, the MCP resource, the Cockpit and
the generated document with no second registration. `docs/RELEASE.md` is the architecture
document; `docs/DISTRIBUTION.md` keeps the release procedure and is now the only place that
still describes raising the version by hand — its first step should become
`majordomus release bump`.

Three costs are real. The changelog's content depends on the git history the clone carries,
so a shallow checkout produces sections with no changes; that is reported in `diagnostics`
and rendered under "What could not be read" rather than silently omitted, which is the
correct behaviour but means the generated document is only reproducible from a full clone.
The generated changelog joins the set that `generate --check` gates, so every commit that
changes the history now changes a tracked file — the same merge cost every other generated
document in this repository already carries. And the bump's default is an inference from
commit subjects, so a breaking change committed as `feat` produces a minor bump; the
override exists for that, the report prints the commits it read as its evidence, and
`project.conventional-commits` is advisory rather than blocking, so the inference will
sometimes need overriding.

What holds it: the crate's unit tests over the window arithmetic, the parser, the bump
function and the narrowness of the write; `test/cases/103_release_projection.sh` over the
whole surface against the real executable in a repository built for the purpose, because
every question the case asks is a question about a history; `scripts/ci/release-check` over
what `generate --check` cannot see, which is the drift between the two declarations of where
the version is stated and the appearance of a hand-kept changelog; and
`scripts/release-version --check`, unchanged.

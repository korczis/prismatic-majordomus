+++
title = "The changelog is composed, and the version is authored once"
description = "The changelog is composed, and the version is authored once"
weight = 116
[extra]
kind = "rule"
slug = "project-release-is-a-projection-2"
identity = "project.release-is-a-projection@2"
status = "active"
source = ".ai/repo/rules/project/release-is-a-projection.v2.md"
+++
{% raw %}

## Rationale

A changelog is the classic file that is correct on the day it is written. Everything in one
is already in the repository that produced it — which tag was published and from which
commit, which decisions were taken and when, what each commit said it did — and the file
exists only because nothing joined those three sources. So it is written by hand, and then
it is not: the entry for the release nobody remembered is missing, and the reader cannot
tell a quiet release from a forgotten one, because a maintained changelog says nothing
about what it left out.

The version had the same shape one level down. It was the one fact both shipped programs
must state, and it was written twice — in the crate manifest and as a literal in the shell
tool — with a check comparing the two and, from version 1 of this rule, one writer raising
both. The reason given was real: an installed tree has no `Cargo.toml`, so the shell tool
could not read the manifest at run time. It could read a *projection* of it, though, and a
projection is how this repository states every other derived fact. Version 2 of this rule is
that: the version is authored in one place, and the shell tool reads a generated file its
own distribution carries (ADR 0085).

## Required behaviour

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

**The version is authored once, projected once for the shell tool, recorded by the
pipeline, and written by one command.** Every statement of it is one of these, and nothing
else:

<div class="overflow-x-auto" tabindex="0">

| Statement | Class | Written by |
|---|---|---|
| `apps/majordomus-cli/Cargo.toml` `[package] version` | **authored — the one place** | `majordomus release bump` |
| `crate::VERSION` and everything the executable prints | compiled | cargo |
| `share/version.txt` | generated projection, read by `bin/majordomus` at start-up | `majordomus generate` |
| `apps/majordomus-cli/Cargo.lock`, the crate's own entry | derived by cargo's rule | the writer, beside the manifest |
| generator stamps, the changelog, the site's version | generated | `majordomus generate`, `scripts/derive` |
| `.ai/repo/releases/*.yaml`, `latest.json`, `RELEASE.json` | records of a released version | the release pipeline |

</div>


- `bin/majordomus` states no version. It reads the `version=` line of
  `share/version.txt` beside itself, never from `MAJORDOMUS_SHARE`, and a distribution
  without the file refuses to start rather than guess.
- **One reader of the authority per language:** `release::version::declared` in Rust and
  `scripts/release-version` in shell. A gate, a script or a case that needs the version asks
  one of them, or asks `bin/majordomus version`; none parses the manifest again.
- `majordomus release bump` is the one writer. It rewrites the manifest's version line and
  the lock's own `majordomus-cli` entry — cargo's record, kept in step so the next
  `--locked` build survives — and nothing else; it is idempotent, reads both back, and
  declines to invent a version it was not given or cannot derive. The projection and every
  stamp follow from `scripts/derive`. What version it writes is not its own judgement:
  `project.the-version-is-measured` decides that from the public contract.
- `package.json` states no version: the private site package is not a product, and a second
  version string there contradicted the one place.

**Raising the version is a repository mutation.** It writes tracked files, so no capability
answers it and the exposure policy withholds it from every machine surface; the read half —
the changelog, the version report and the analysis — is a capability, and is therefore on
all of them.

## Failure behaviour

`release::version::diagnose`, in the release module, is what refuses a second statement. It
reports `version-stated-by-hand` (an error) for a version-shaped carrier written by hand in
`bin/`, `lib/`, `scripts/` or `share/` outside a generated artifact — an assignment to a name
ending in `version` (shell, TOML, JavaScript or Python), a `version:` or `"version":` member,
or the product's `majordomus X.Y.Z`;
`projection-stale` (a warning — the refusal is `generate --check`'s) when `share/version.txt`
is behind the manifest or missing; and `writers-disagree` (an error) when it is ahead of the
manifest or unrelated to it. `release analyze` carries these on every surface, and
`release version` prints them and exits 10. The gate `version-authored-once` runs
`bin/majordomus-cli release version` on every plan.

The gate `release-check` (`scripts/ci/release-check`) fails, exit 10, when:

- (1) the manifest the crate's writer names is not the one `scripts/release-version` reads;
- (2) `bin/majordomus version` does not print what the manifest declares
  (`scripts/release-version --check`);
- (2b) the lock records a different version for this crate than the manifest declares;
- (3) a changelog is authored at the repository root or under `docs/` outside
  `docs/generated/`;
- (4) the version the tree declares is behind the newest release the layer records;
- (5) a published tag has no release record in the layer.

`majordomus generate --check` is what proves the written changelog and `share/version.txt`
still match what the generator would write from this tree.

## Verification

```sh
scripts/ci/release-check                    # the gate
bin/majordomus-cli release version          # the version-authored-once gate
scripts/release-version --check             # the tool prints the authority
bash test/run.sh 484_the_version_is_authored_once 103_release_projection
majordomus release                          # the changelog, composed
majordomus release bump --dry-run           # what the one writer would write
majordomus generate --check                 # the written forms against the tree
```

`test/cases/484_the_version_is_authored_once.sh` proves the version half, each section first
shown to fail against the tree before version 2: the tool states no version, it prints the
authority because it reads its own projection, a hand-edited projection is refused, the
writer writes one line of the manifest and the lock's own entry, and a version written by
hand in `lib/` is refused. `test/cases/103_release_projection.sh` proves the changelog half
and the report, the writer and the gate against a repository with a real history.
`apps/majordomus-cli/src/release/` carries the unit half.
{% endraw %}

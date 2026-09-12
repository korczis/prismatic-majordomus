+++
title = "The release"
description = "the release: the changelog composed from the layer's release records, the decisions dated inside each release's window and the conventional commits in its range; the two statements of the version and the one command that writes them; the bump the commits imply and the evidence for it; and every surface derived from both"
weight = 25
[extra]
source = "docs/RELEASE.md"
+++

{% raw %}

What this project has shipped, what it would ship next, and the changelog that says so are
one typed value, `release::Changelog`, composed by one module under
[`apps/majordomus-cli/src/release/`](../apps/majordomus-cli/src/release/). Every surface
that states any of it renders that value: `majordomus release` on the command line, the HTTP
route `/api/v1/changelog`, the MCP tool `majordomus_changelog` and the resource
`majordomus://changelog`, and the generated document under `docs/generated/`. Nothing in it
is authored. There is no `CHANGELOG.md` in this repository and there is not meant to be.
Behaviour as implemented and tested; where this document and the executable disagree, the
document is wrong and changes in the same commit.

The commands, their arguments and their executable examples are in the generated reference
([`generated/cli.md`](generated/cli.md), under `majordomus release`); the capabilities, their
routes and their benchmark cases in [`generated/capabilities.md`](generated/capabilities.md),
module `release`. Neither is restated here.

## The version is measured, not claimed

The bump a release takes is decided by comparing the public surface of the last release
with this tree, not by the words in the commit messages. `feat:` meaning minor and `fix:`
meaning patch describes what the author believed; it says nothing about what a caller can
still call. Between `v0.3.1` and `0.4.0` the command `majordomus scope classify` left the
command line and every gate stayed green, because no gate was looking at the surface.

The surface is the capability registry — one declaration of which MCP, HTTP, OpenAPI and
the command line are projections — and `docs/generated/registry.json` is committed at every
commit and every tag, so two releases can be compared without building either. What a
caller can hold is a public capability's identity, its kind, the MCP tool and resource it
answers to, the HTTP method and path it is bound to, the command-line path that dispatches
it, **and both of its schemas**.

```text
something a caller held is gone    major implied   a caller who held it is broken
a contract narrowed under a caller major implied   a required field, a removed value, a type
something arrived                  minor implied   nothing broke, something is new
a contract widened                 minor implied   every existing caller still works
the surface is equal               nothing owed    most commits are here
```

Schemas are compared **as contracts, in both directions**, which is the comparison a textual
diff of two schemas gets wrong. An input is what a caller sends, so accepting more is
compatible and demanding more is breaking; an output is what a caller receives, so promising
more is compatible and promising less is breaking. The same edit — `x` becoming required —
is a minor in an output and a major in an input. Descriptions, titles and examples are
normalised away before anything is compared, so rewording a doc comment is not a release; a
constraint the comparator has no rule for, changed, is counted **breaking** rather than
guessed at, because a false major is an argument someone can win and a false patch is a
caller who finds out by breaking.

```bash
majordomus release analyze                     # the tree against the last release
majordomus release analyze --explain           # every movement, and why each counts
majordomus release analyze --format json       # the canonical VersionPlan every surface renders
scripts/ci/version-matches-surface             # the gate: the same thing, mapping exit codes
majordomus release bump                        # raise both writers to the measured minimum
```

There is **one** engine and it lives in `apps/majordomus-cli/src/release/compat.rs`, reading
`release/surface.rs`. The gate is an adapter over it; so is the Cockpit panel, the HTTP
route, the MCP tool and the writer. Until [ADR 0051](../.ai/repo/adrs/0051-the-minimum-release-version-is-measured-from-the-public-contract.md)
there were two answers — a shell comparison in CI and a commit-subject inference inside
`release bump` — and the inference won, because the writer runs before the gate does.

**Below 1.0.0 the floor is a minor release** for any surface change, gone or new. Semantic
versioning grants `0.y.z` a blanket exemption — anything may change — and Elm refuses that
exemption by starting every package at 1.0.0. This project has earned neither answer, so it
takes the strongest signal 0.x has rather than demanding 1.0.0 for a single removal. When
the major reaches 1 the shift ends and the implied bump is the required one.

**A removal is named whatever the verdict is.** Below 1.0.0 it does not refuse the release,
and it is still stated as a breaking change: a caller who held what is gone otherwise finds
out by breaking. The rule is `project.the-version-is-measured@2`; the gate is `version-surface`,
selected by the `rust`, `rust-generated` and `distribution` path classes so that it runs
before a change lands rather than only on a full plan.

### Where a removal is written down

Neither of the two documents that would hold it is authored, so the answer is not "write it
in one of them".

The **release record** cannot hold it at all today. `release/v1`
(`share/schemas/majordomus/release/release.v1.schema.json`) is `additionalProperties: false`
and declares no field for a breaking change, and a record is written by the release workflow
from the artifacts it published and by nobody else
([`.ai/repo/releases/README.md`](../.ai/repo/releases/README.md)) — so an unreleased version
has no record for anything to be written into. The smallest field that would close this is a
proposal, not a change made here: a schema change does not belong inside a documentation
change.

The **changelog** can, through the one authored input it has. Nothing in it is written by
hand, and the only authored text it reads *about a change* is the commit message.
`release/commits.rs` marks a change breaking on `type(scope)!:` or a `BREAKING CHANGE:`
trailer in the body, and `release/changelog.rs` renders such a change with a leading
`**BREAKING**`. So a removal is named in the **subject** of a commit inside the release that
carries it — the subject, because that is the text a reader of the changelog sees; the
trailer marks it and is not rendered.

Two consequences follow from `compose_published`, and both are intended:

- `majordomus release changelog` states it immediately, under `## Unreleased`;
- `docs/generated/changelog.*` does not, and will not until the version ships. The committed
  artifact carries only published releases, because a file inside a commit cannot describe
  the commit it is in. When the pipeline writes the record for that version, the section's
  changes are the commits in `<previous>..<this>` — the marked one among them — and the
  published changelog states the removal with nobody writing it there.

The consequence worth naming is that the mark has to be on a commit *inside* the release,
and cannot be added to a commit that has already landed. A removal noticed after the fact is
therefore stated by a later commit in the same window saying what left. That is what
`v0.3.1..0.4.0` needed: the atom `command scope classify` left the surface in
`427b73248d9`, whose subject marked nothing, and it is named by the commit that carries this
section.

## The gap it closes

Every other public fact in this repository has one canonical declaration and a set of
projections derived from it — a command, a capability, a schema, a page. The release did
not, and it failed in two different ways.

**The changelog did not exist.** What changed in a version lived in GitHub's release notes,
outside the repository that produced it, written by whoever cut the release and read by
nobody afterwards. The facts it would have contained were already in the tree, unread:

```text
  .ai/repo/releases/*.yaml   what shipped, when, from which commit   (kind release-record)
  .ai/repo/adrs/*.md         the decisions, dated                    (kind adr)
  git log <commit>..<commit> everything that has no object of its own
```

**Nothing raised the version.** It is stated in two files — `apps/majordomus-cli/Cargo.toml`
and `bin/majordomus` (`MJ_VERSION`) — and `scripts/release-version --check` compared them and
exited 10 when they disagreed. A check with no writer behind it verifies a person's memory:
it can say the two disagree, and it can say so only after someone has already edited one of
them and forgotten the other.

Neither gap needed a new source of truth. The changelog needed a join; the version needed a
writer.

## What a section is composed from

One section per release record, newest first by the date the record carries, with the
unreleased work leading:

```text
   release record  ──►  version, tag, date, commit, artifacts
   adr objects     ──►  the decisions dated inside this release's window
   git log A..B    ──►  the changes, as conventional commits
   issue·milestone ──►  the records those commits name, resolved against the layer
```

The window of a release is the same interval said in the two vocabularies its two sources
have. An ADR carries a date and no commit; a commit carries no date the layer indexes. So
the decisions of a release are those dated in `(previous release's date, this release's
date]`, and its changes are the commits in `previous..this`. The first release's range is
everything up to its commit — a clone that does not carry that history yields nothing, and
says so rather than showing an empty section.

Dates are compared as the strings they are: both sources write ISO-8601, where lexicographic
order is chronological order. A record's `published_at` carries a time and an ADR's `date`
does not, so the comparison is made on the day the two share.

A section's artifacts are the record's own evidence — the target, the file name and the
SHA-256 that `scripts/release-record` read off the file that was published. The changelog
copies them; it does not compute them and it does not reach the network.

### The unreleased section

Everything after the newest record, as `<last release's commit>..HEAD`. It leads the
document when it has any changes and is absent when it has none, because an empty
"Unreleased" heading says nothing a reader can use. A repository that has never published
has no records at all, and its whole history is one unreleased section — the right answer
for a project before its first release rather than an error.

## Conventional commits, read totally

The parse is small and deliberately total:

```text
  feat(commands): one canonical command graph
  ^^^^ ^^^^^^^^   ^^^^^^^^^^^^^^^^^^^^^^^^^^^
  kind scope      subject

  feat(api)!: the route moved      `!` marks it breaking
  BREAKING CHANGE: <why>           so does this trailer, in the body
```

A subject that does not parse is kind `Other` and still appears, with its subject kept
whole rather than split at a colon that was part of the sentence. This is the one design
decision in the parser worth arguing about, and it goes the other way from most changelog
generators: a changelog that silently drops what it cannot classify lies by omission, and
the commits it drops are exactly the ones nobody was paying attention to when they landed.
`project.conventional-commits` is blocking since version 2 and the `commit-msg` hook
refuses a subject that does not parse, so new commits arrive conventional. The history still
holds 54 that predate the policy — recorded in `.ai/repo/commit-policy-baseline.txt`, every
one of them published, none of them rewritable — and those are what the parser is lenient
for. See [COMMIT.md](@/docs/commit.md).

The kinds and the headings they render under, in the order a reader of a changelog wants
them — what is new, what is fixed, what is faster, then the rest:

```text
  feat      Added            docs           Documentation
  fix       Fixed            test           Tests
  perf      Performance      ci             Pipeline
  refactor  Changed          chore          Housekeeping
                             anything else  Other
```

A heading with no entries is absent; nothing is hidden. Merge commits are not read at all
(`git log --no-merges`), because a merge's subject describes the integration and its
contents are already in the range.

### The order is in the document, not in each renderer

A section does not carry a flat list of changes. It carries `groups` — one per kind that has
any change at all, each with its `kind`, the `heading` it is shown under, the `rank` it sorts
by, and its own `changes` — and the groups are already in rank order when the document is
composed:

```text
  section.groups[]  →  { kind: "feat", heading: "Added",   rank: 0, changes: [...] }
                       { kind: "fix",  heading: "Fixed",   rank: 1, changes: [...] }
                       { kind: "docs", heading: "Documentation", rank: 4, changes: [...] }
```

That order used to exist in exactly one place a projection could not reach: `ChangeKind::rank()`,
a Rust function the Markdown renderer called. The website cannot call a Rust function, so it
grouped what it was given alphabetically instead, and the same changelog was read in two
different orders depending on which surface showed it. The order is in the value now. Every
renderer reads `heading` and `rank` rather than deciding either — `site/templates/changelog.html`
neither groups nor sorts, and the Markdown renderer walks `groups` in the order it was handed
them. A presentation order stated once and carried is the only kind that survives a projection.

The ranks are the kinds' own and are not contiguous: a section with no refactors simply has no
group of rank 3. A group's absence means nothing of that kind happened in the range, never that
something was hidden.

## Where a fact can be read

Every address the changelog carries is derived from `about::REPOSITORY` — the crate manifest's
own `repository`, which the compiler passes in as `CARGO_PKG_REPOSITORY`. None of them is a
literal, so a fork or a move carries every link with it and nothing has to be told where the
project now lives:

```text
  section  ──►  notes_url    the published release, from the record's own notes_url
                compare_url  the range against the previous tag
                tree_url     the tree at that tag
  change   ──►  url          the commit
  decision ──►  url          the ADR file that states the decision
```

`forge()` in `changelog.rs` recognises one shape — `https://github.com/<owner>/<repo>` — and
returns nothing for anything else. When it returns nothing, **no links are produced at all**
rather than links guessed from a pattern that might hold. A wrong link cannot be told from a
right one until it is followed, so a reader with no link knows they have none and a reader with
a plausible wrong one does not.

Two of the section addresses are not simply the tag's. A record that names its own `notes_url`
keeps it, because the published release is where the record says it is; the fallback is the
tag's release page. The first release has no predecessor to compare against, so its
`compare_url` is the forge's list of commits up to the tag rather than a range. The unreleased
section compares the last tag against `master` and points at the tree there.

`Decision.route` was removed under the same argument. It named `/decisions/<id>/`, and nothing
publishes such a page: the honest destination for a decision is the ADR file in the repository,
which is what `url` now carries. The Markdown rendering keeps the plain shape it always had —
the addresses are in the value for the surfaces that can use them, and the site page renders
every one of them.

### The document names its own producer

`produced_by` is the capability that answered — its id, the command line that renders it,
the HTTP route, the MCP tool and the MCP resource — read off the registry entry by one
function, `capability::builtin::release::produced_by`, which the handler that serves the
document and the generator that commits it both call. So the committed
`docs/generated/changelog.json` and the live answer name the same surfaces, and a reader who
has one of them can find the others. A page that points at the same value elsewhere resolves
those names against the datasets the registry generates (`executable.json`, `cli.json`)
rather than typing a route: the site's changelog page does exactly that, because
`site-check`'s `registry` and `cli` assertions refuse a template that names a capability or a
command route by hand — and did, on the first draft of that page.

## What a commit names, resolved

A commit's subject and body are scanned together for the ids this repository's plan uses — an
`I` and four digits for an issue, an `M` and three for a milestone — and every id found is then
*looked up* in the layer. An id that matches the shape and names no object is dropped. What is
carried is a reference with the kind, the id, and the title from the record it resolved to, so
an entry reads as the issue it belongs to rather than as a code only its author can expand.

The resolution half is what makes the inference safe. Matching alone would put a link on every
string that looks like an id, including the ones nothing answers to, and that is the failure the
links above are built to avoid. The scan reads the body as well as the subject because a commit
that explains itself in its body is the one most worth linking, and a word that merely begins
with the letter (`Interesting`, `M1`) is not an id.

Counts of what that yields go stale; measure them instead:

```text
  majordomus release changelog --format json \
    | jq '[.sections[].groups[].changes[].references[]?] | length'
```

## The version: two statements, one writer

The version stays stated in two places, and should. `scripts/release-version` gives the
reason and it is a real one: an installed tree has no `Cargo.toml`, and the crate is
compiled before the shell tool exists, so neither program can read the other's copy at run
time. [`DISTRIBUTION.md`](DISTRIBUTION.md#the-two-versions-and-why-there-are-two) is where
that is argued. What was missing was not a single source — it was a single writer.

`majordomus release version` answers what both files state and whether they agree. It exits
10 when they do not, which is the same verdict and the same exit code
`scripts/release-version --check` gives, so a person and a pipeline get one answer.

`majordomus release bump` is the writer, and it **computes nothing**. It reads the same
`VersionPlan` that `release analyze`, the HTTP route, the MCP tool and the gate read, and
applies it:

```text
  plan  →  validate  →  apply(plan)
```

With no argument the version becomes the measured minimum: the baseline raised by what the
contract requires. `--level major|minor|patch` and `--exact 1.2.3` name a **higher** version
than that, for the cases where a maintainer means more than the contract did — reported with
its provenance rather than silently. Neither may name a lower one:

```text
  required minor, --level patch    REFUSED, nothing written
  required minor, --exact 0.4.0    REFUSED, a version does not go down
  required minor, --level major    allowed; "required 0.6.0, selected 1.0.0, explicit override"
```

An override that could undershoot would not be an override — it would be the hole that makes
the measurement decorative. `--dry-run` prints what would change and writes nothing.

Two properties of the write matter:

- **It is byte-narrow.** Only the `version` line inside `[package]` of the manifest and the
  `MJ_VERSION=` line of the shell tool are rewritten. A dependency pinned at the same
  version, or the string in a comment, is untouched.
- **It checks its own work.** After writing, the bump re-reads both files and reports a
  disagreement with exit 10 — so a bump that half-applied is caught by the thing built to
  catch it rather than by the release three commits later.

Zero-major is not special-cased in the *arithmetic* — `Version::raised` is arithmetic and
nothing else. Whether a breaking change below 1.0 costs a major or a minor is a policy
question, and this repository answers it in exactly one place: `compat::Policy`, carrying the
schema id `majordomus/version-policy/v1` and a mode decided by the baseline version alone. A
second answer in the arithmetic is the shape this subsystem was built to remove.

The version the crate declares is also the `current` field of the changelog, so a bump that
was made and a changelog that was not regenerated disagree, and `generate --check` says so.

## Which surface carries what

<div class="overflow-x-auto" tabindex="0">

| | command line | `generate` | HTTP · MCP | Cockpit |
|---|---|---|---|---|
| the changelog | `majordomus release [changelog [VERSION]]` | `docs/generated/changelog.{json,yaml,md}` | `GET /api/v1/changelog` · `majordomus_changelog` · `majordomus://changelog` | through the registry |
| the version report | `majordomus release version` | — | `GET /api/v1/release/version` · `majordomus_release_version` | through the registry |
| the compatibility analysis | `majordomus release analyze` | — | `GET /api/v1/release/analysis` · `majordomus_release_analysis` | through the registry |
| raising the version | `majordomus release bump` | — | withheld | withheld |

</div>


Nothing configures that last row. `release bump` writes tracked files, which
`command_graph/semantics.rs` annotates as `RepositoryMutation`, and the exposure policy of
[ADR 0027](../.ai/repo/adrs/0027-a-command-is-declared-once-and-every-surface-is-a-projection.md)
keeps repository mutations off every machine surface. No capability declares it, and
`majordomus commands explain executable.release.bump` prints the reason each surface
withholds it. The read half is two capabilities, and the command line renders them by
*executing* them rather than by calling the code underneath, so the terminal and the API
cannot drift apart.

Neither read capability declares a `cli` exposure, and that is deliberate: `majordomus release
changelog` is a *local* command that renders the capability for a person at a terminal, and
`cli::LOCAL` says so once — `RendersCapability("release.changelog")` — beside the reason it
is local. A capability that declared the same path would be the same command accounted for
twice, which `tests/quality.rs` refuses as `OPERATION_CLASSIFICATION_CONFLICT`. What reads
that one declaration is `produced_by` below, so the document can name its command line
without the crate stating it a second time.

The generated document is a generated artifact like any other — one value written as JSON
for a program, YAML beside it and Markdown for a reader, each declaring its schema
(`majordomus/changelog/v1`) and its source. That is the point of generating it at all:
`generate --check`, run by `scripts/rust-check`, is what notices that the changelog has
stopped describing the tree, so nobody has to remember.

## What could not be read

The changelog says what it could not read instead of hiding it. Every such finding appears
in `diagnostics` and, in the Markdown, under a final "What could not be read" heading:

<div class="overflow-x-auto" tabindex="0">

| finding | cause |
|---|---|
| a release record names no commit | the record is malformed; its section cannot be composed and is omitted |
| no commit was readable for a version | the clone does not carry that history — a shallow clone, or a rewritten range |

</div>


A `git log` that fails is not an error here: it yields no commits, and the caller reports
the gap. That is what lets the changelog render at all in a shallow CI checkout, where
refusing would make the document unavailable exactly where it is read from a machine.

## Releasing

The release procedure itself — the tag, the pipeline, the build matrix, the record written
from what was published, and recovery from a bad release — is
[`DISTRIBUTION.md`](DISTRIBUTION.md#releasing). This document owns only the version and the
changelog it produces. The first step of that procedure is now `majordomus release bump`
rather than an editor over two files, and `scripts/release-version --check` still runs
after it: the check proves the work of one writer instead of the memory of one person.

## What proves it

<div class="overflow-x-auto" tabindex="0">

| | |
|---|---|
| `apps/majordomus-cli/src/release/commits.rs` | the parser: the conventional shapes, both spellings of breaking, an unknown lowercase type, and a subject that is not conventional kept whole; and the resolution: an id the layer holds becomes a reference carrying that record's title, an id of the same shape that names nothing does not, a name mentioned twice is carried once, and a word that merely starts with the letter is not an id |
| `apps/majordomus-cli/src/release/version.rs` | the bump is a total function of the changes, raising zeroes what it supersedes, a version that is not three numbers is refused, and writing touches only the two lines that state the version |
| `apps/majordomus-cli/src/release/changelog.rs` | a decision belongs to the release whose window contains its date, the unreleased window opens after the last release, a timestamp and a date compare on the day they share, and the groups reach the renderer in rank order whatever order the commits arrived in |
| `test/cases/103_release_projection.sh` | the whole surface against the real executable, in a disposable repository with a real history: which commits fall in which range, which decision belongs to which window, a record added with nothing else edited, a subject that follows no convention carried rather than dropped, a fixture whose repository is not a forge the tool knows producing no link rather than a guessed one, a commit naming one id the layer holds and one it does not, and every exit code above |
| `scripts/ci/release-check` | the two declarations of where the version is stated have not drifted, the two sites agree on every plan rather than only at publication, no hand-kept changelog has appeared, and the tree is not behind the newest release the layer records |
| `scripts/rust-check` (gate `rust-check`) | `generate --check`: the committed changelog still describes the tree |
| `scripts/release-version --check` | the two writers agree — the same verdict `majordomus release version` gives, with the same exit code |

</div>


The behavioural case builds its own repository rather than reading this one, because every
question it asks is a question about a history. A fixture whose history is real is the only
thing those answers can be checked against.

## Related

- [ADR 0029](../.ai/repo/adrs/0029-the-changelog-is-a-projection-and-the-version-has-one-writer.md) — the decision behind this document, and the four alternatives it rejected
- [`DISTRIBUTION.md`](@/docs/distribution.md) — how the tool is packaged, published and installed, and the release pipeline that writes the records this reads
- [`COMMANDS.md`](@/docs/commands.md) — the effect model and the exposure policy that withhold `release bump`
- [`CAPABILITIES.md`](@/docs/capabilities.md) — the registry the two read capabilities are declared in
- [`DYNAMICITY.md`](@/docs/dynamicity.md) — the ownership rule this is an instance of
{% endraw %}

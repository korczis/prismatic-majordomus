# Release

How this project decides what version a release carries, what its changelog says, and how
the two reach every surface without anybody typing a version twice.

The rule is `project.release-identity`. The decision is ADR 0028. This document is the
operator's half: what to run, in what order, and what to do when a step fails.

## The shape

```text
  apps/majordomus-cli/Cargo.toml          the version, written once
            │
            ├──▶ crate::VERSION ──▶ OpenAPI info.version · MCP serverInfo · HTTP index
            │                       · Cockpit · generated provenance headers
            │
            └──▶ share/version.txt ──▶ `majordomus version` · site project.json

  capability registry ─┐
  command graph ───────┤
  document schemas ────┼──▶ docs/generated/contract.json ──┐
  distribution model ──┘        (committed, at every commit) │
                                                             ├──▶ ContractDiff
  the same file at the last published tag ───────────────────┘        │
                                                                      ▼
                                                        CompatibilityImpact
                                                                      │
                                                          release policy (below)
                                                                      ▼
                                                            minimum version
  .ai/repo/changes/*.md ──┐
                          ├──▶ CHANGELOG.md · release notes · release manifests
  .ai/repo/releases/*.yaml┘     · /cockpit/release · /api/v1/release/* · MCP
```

## The four versions

They are four different facts and nothing here collapses them.

| | what it is | where it comes from |
|---|---|---|
| **source** | what this tree would release | the crate manifest, compiled in |
| **published** | what an unpinned installation resolves to | the highest stable release record |
| **running** | what this process is | the crate manifest of the build that is running |
| **deployed** | what a hosted service reports | the deployment records |

`majordomus release version` prints all four and names every disagreement. The Cockpit's
topbar shows the source version with the release state beside it, and links to
`/cockpit/release`, where the four are laid out.

## The policy

A release's compatibility is measured from the public contract, not from commit messages.
A commit message says what somebody meant to do.

| impact | what it means | ≥ 1.0.0 | < 1.0.0 |
|---|---|---|---|
| `none` | nothing a caller can observe changed | no bump | no bump |
| `patch` | behaviour moved inside the contract | patch | patch |
| `additive` | the contract grew | minor | patch |
| `breaking` | the contract shrank or moved | major | **minor** |

Below 1.0.0 the specification binds nothing, so this project states its rule rather than
inheriting folklore: **a breaking change moves the minor**. That is the rule Cargo already
applies to a `0.x` dependency, so a repository that depends on this project by version
range gets the behaviour it already expects.

**A release may not declare a version below the minimum.** There is no flag that lifts it.

## What is in the public contract

`docs/generated/contract.json`, generated at every commit:

- **capabilities of this executable** — the kind, every input and output property with its
  type and whether it is required, and every projection declared (MCP tool, HTTP method and
  path, command path);
- **runnable commands** of the executable and the shell tool, with their arguments;
- **document kinds** — the schema of every `.ai/` object, field by field, because every
  repository's own files are validated against them;
- **distribution targets** — the platforms a release publishes for.

Not in it, deliberately: titles, descriptions, tags, stability, benchmark and cache policy,
the declarative capabilities a particular repository's own objects produce, and the workflow
runner's recipes. Each would make an ordinary edit look like a contract change, and a
subsystem that cries wolf about compatibility is one nobody reads.

## The workflow

```sh
majordomus release status      # where this stands
majordomus release explain     # why the next version must be what it must be
majordomus release plan        # what preparing would write
majordomus release prepare     # set the version, stamp the change records
majordomus generate            # regenerate every projection of it
majordomus release check       # prove the release may go out; exit 10 if not
git tag v0.4.0 && git push origin v0.4.0
```

The tag is what runs `.github/workflows/release.yml`: it builds every supported target,
verifies each archive, publishes the GitHub release, writes the release record, re-derives
everything and asks the Pages workflow to publish. Nothing after the tag is manual.

`majordomus release prepare` writes two kinds of file and no others: the crate's version,
and `released_in:` on every unreleased change record. Everything else a release moves is a
projection, and `majordomus generate` is the one thing that writes a projection.

### Answering for a release

```sh
majordomus release changelog                 # the whole changelog
majordomus release changelog --version 0.4.0 # one release's notes
majordomus release manifest --version 0.4.0  # the release, explained
majordomus release diff --impact breaking    # what a release costs, entry by entry
```

Every one of these is also `GET /api/v1/release/…`, an MCP tool, and a card on
`/cockpit/release`. One engine; four readings.

## Writing a change

Not every commit needs a record. A capability that appeared is already in the contract diff.
Write one when the change is not visible in the contract (a fix, a behaviour that moved
inside an unchanged signature), or when it **is** in the contract and is **breaking** — in
which case a record is required, and it must name a migration document.

The contract is `.ai/repo/changes/README.md`.

## Diagnostics

Every failure names the command that shows it and the command that changes it.

| code | what it means |
|---|---|
| `SEMVER_BUMP_TOO_LOW` | the version does not clear the minimum its change requires |
| `CHANGELOG_MISSING` | a breaking contract change no record names |
| `MIGRATION_GUIDE_REQUIRED` | a breaking record naming no migration document |
| `RELEASE_CHANGE_INVALID` | a record that could not be read |
| `RELEASE_MANIFEST_INVALID` | a release record disagreeing with the distribution model |
| `CONTRACT_SNAPSHOT_MISSING` | no committed contract, so this tree states none |
| `CONTRACT_SNAPSHOT_STALE` | the committed contract is not the one this build states |
| `CONTRACT_BASELINE_UNREADABLE` | the baseline's tag or contract cannot be read here |
| `CONTRACT_BASELINE_PREDATES` | informational: the baseline predates the contract snapshot |
| `LAYER_DEGRADED` | the layer did not read cleanly, so the verdict may be short |

## When something goes wrong

**The version was bumped and the tests fail.** Nothing was published. Fix, regenerate,
`release check` again. `prepare` is idempotent: running it twice on an unchanged tree writes
nothing the second time.

**A tag exists and no release record does.** The pipeline did not finish. Re-run the release
workflow for that tag; it adopts assets that already exist rather than republishing them,
because rebuilds are not bit-identical.

**A release is published and is wrong.** Set `yanked: true` on its record and regenerate.
The stable pointer moves to the release below it; artifacts anyone already has keep working,
and an installation pinned to that exact tag is untouched. Nothing is deleted.

**The published site shows an old version.** A push made with `GITHUB_TOKEN` starts no
workflow, which is why the release workflow asks for the Pages run explicitly. Check
`curl -s https://korczis.github.io/prismatic-majordomus/build.json` against the commit you
expect, and dispatch `pages.yml` by hand if they differ.

**A deployment reports a version that is neither.** That is drift, and it is shown as drift
wherever a version is shown rather than hidden. The deployment is running something the
repository did not publish.

## What is not built

`majordomus release publish` does not exist: publishing is the tag and the pipeline it
starts, and a command that pushed a tag would be a second way to do one thing. There is no
self-update (`docs/DISTRIBUTION.md` says what building it would reuse), and no
package-manager distribution. Deployment records exist and are validated locally; nothing
here contacts a provider, and no deployment of this service is active.

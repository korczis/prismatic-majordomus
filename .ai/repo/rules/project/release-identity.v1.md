---
id: project.release-identity
version: 1
kind: rule
title: A release has one identity
description: The project version is stated once and every other place it appears is derived from it or mechanically checked against it; the tag, the crate, the release record, the manifest, the published artifacts and the runtime describe the same release, and a deployment reports the release it is running.
statement: A release has one identity — one version, derived everywhere it is shown, and the same in the tag, the record, the manifest, the artifacts and the running process; nothing states a version of its own and nothing publishes a release whose identity cannot be proved.
status: active
class: blocking
depends_on: [project.distribution-canonical@1, project.interfaces-are-projections@1, project.derived-files-regenerated@1]
tags: [release, version, distribution, architecture]
---

# Rationale

A version is the fact a project restates in the most places and reconciles in the fewest.
This repository had two writers of it — the crate's manifest and a literal in the shell
entry point — kept in step by a script somebody had to remember to run, and two pipelines
downstream of them: the website's navigation bar took its number from the shell literal and
the API reference took it from the crate. They agreed on the day anyone looked, which is
exactly the condition under which a drift ships.

Worse than a drift is a version that is *wrong on purpose*. A number is not decoration: a
caller who pinned `0.3` is entitled to assume `0.3.2` did not remove the endpoint they call.
If nothing decides whether the number a release carries is allowed, the number stops
carrying information, and a person reading it learns only that somebody typed it.

And a release is not one number but four, which is the failure this rule exists to make
visible. What the tree would release, what is published, what this process is, and what a
deployed service is actually running are four different facts. A page that shows one of
them as though it were all four is how an operator concludes a fix is live when it is not.

# Required behaviour

**One authority.** `apps/majordomus-cli/Cargo.toml` states the version. Every other place it
appears is derived from it or mechanically checked against it:

- the executable compiles it in (`crate::VERSION`), and the OpenAPI `info.version`, the MCP
  `serverInfo.version`, the HTTP index, the generated artifacts' provenance headers, the
  Cockpit and the benchmark evidence all read that;
- the shell tool reads `share/version.txt`, generated from the crate's version by
  `majordomus generate --target release`; a release archive carries `share/` in full, so an
  installed tree reads the same file a checkout does;
- the site's `project.json` derives from the shell tool's, which derives from the crate's;
- `scripts/release-version --check` refuses a tree where the crate, the shell tool and a
  tag do not state one version.

Nothing else may state a version. A test fixture and prose that deliberately names a
historical release (`measured against v0.3.1`) are not statements of the current version
and are not covered by this.

**A version must be earned.** The public contract — every capability with its input and
output schemas and the projections it declares, every runnable command with its arguments,
every document schema a repository's files are validated against, every platform a release
publishes for — is normalised into `docs/generated/contract.json` at every commit. A
release's compatibility is the diff between that document and the same document at the last
published release, and the minimum version follows from the compatibility by the policy in
`docs/RELEASE.md`. **A release may not declare a version below that minimum.** There is no
flag that lifts it: a project that decides a break is worth making writes the change record
and takes the version the policy gives it.

**A break must be explicable.** Every breaking contract change is named by a record under
`.ai/repo/changes/`, and a breaking record names a migration document that exists. A
changelog that does not explain the thing its readers most need explained is not a
changelog.

**One release, one identity.** The tag is `v` followed by the version. The release record,
the release manifest, the published artifacts and the executable inside them state that
version and no other. A deployment states the version it is running, and a deployed version
that is neither the published release nor the source version is drift, reported as such
wherever a version is shown.

# Failure behaviour

`majordomus release check` exits 10 naming every violation, each with the command that
shows it and the command that changes it: `SEMVER_BUMP_TOO_LOW`, `CHANGELOG_MISSING`,
`MIGRATION_GUIDE_REQUIRED`, `RELEASE_MANIFEST_INVALID`, `CONTRACT_SNAPSHOT_STALE`,
`CONTRACT_BASELINE_UNREADABLE`, `RELEASE_CHANGE_INVALID`. `majordomus release prepare`
refuses a version below the minimum and refuses to write while any of those hold.
`majordomus generate --target release --check` exits 10 when the committed contract, the
changelog, a release manifest or `share/version.txt` has fallen behind. The CI gate
`release-identity` runs `scripts/ci/release-check`, which is all of the above plus the
version-agreement check, on every change to the crate, the layer or the release scripts.

# Verification

`test/cases/104_release_identity.sh` proves the invariants by mutation in a disposable
repository: a version below the minimum its contract change implies must be refused by
name; a breaking change with no record must fail the check; a breaking record with no
migration document must fail; a hand-edited `share/version.txt` must fail `generate
--check`; preparing the same release twice must write nothing the second time. The
compatibility engine's own behaviour is proved by the unit and property tests under
`apps/majordomus-cli/src/release/`, and the cross-surface agreement by
`apps/majordomus-cli/tests/release.rs`. The design is `docs/RELEASE.md` and ADR 0028.

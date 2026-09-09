<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `release` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.3.2 -->
# Module `release` — Release

What this repository would publish next and why it must carry the version it carries: the four versions and where they disagree, the public contract measured against the last published release, the compatibility that change implies, the minimum version the policy makes of it, the changelog every projection renders, and every reason a release is not ready. One engine; the command line, the API, MCP and the Cockpit are four readings of it.

Stability: behaviorally_verified. Capabilities: 8.

## `release.changelog` — The changelog

The changelog, as a model and as the Markdown CHANGELOG.md carries: the unreleased changes and every published version's, grouped by kind in the reader's order, with breaking changes marked and their migration documents linked. Every entry is one record under .ai/repo/changes/; this file, the release notes, the Cockpit, the site and this answer are five renderings of those records and of nothing else.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_changelog` |
| HTTP | `GET /api/v1/release/changelog` |
| CLI | `majordomus release changelog` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::release |
| tags | release, changelog |

| input | type | required | description |
|---|---|---|---|
| `version` | object | no | Only this version's section; the whole changelog when absent. |

Output: `ChangelogReport`.

## `release.check` — Whether a release may go out

Every release invariant this repository can decide locally: that the version clears the minimum its contract change requires, that every breaking change is named by a change record, that a breaking change carries migration guidance, that the change records agree with each other, that the release records agree with the distribution model, and that the committed contract is the contract this build states. Each failure names the command that shows it and the one that fixes it.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_release_check` |
| HTTP | `GET /api/v1/release/check` |
| CLI | `majordomus release check` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::release |
| tags | release, diagnostics |

Input: none.

Output: `ReleaseCheckReport`.

## `release.diff` — The public contract diff

Every difference between the public contract at the last published release and the contract this tree states, with what each one costs a caller and why. Optionally narrowed to one impact or one surface. The contract is the capabilities with their input and output schemas and every projection they declare, the runnable commands with their arguments, the document schemas a repository's own files are validated against, and the platforms a release publishes for.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_release_diff` |
| HTTP | `GET /api/v1/release/diff` |
| CLI | `majordomus release diff` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::release |
| tags | release, compatibility, contract |

| input | type | required | description |
|---|---|---|---|
| `impact` | object | no | Only the changes carrying this impact: `none`, `patch`, `additive` or `breaking`. |
| `surface` | string or null | no | Only the changes on this surface: `capability`, `command`, `document-kind`,
`target`. |

Output: `ContractDiffReport`.

## `release.explain` — Why this version

The reasoning behind the required bump, step by step: the baseline it was measured against, the contract changes that decided it, the impact they add up to, the policy sentence that maps that impact to a bump, and the version that comes out. Derived from the contract diff and the policy table; no sentence here is written by hand for a particular case.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_release_explain` |
| HTTP | `GET /api/v1/release/explain` |
| CLI | `majordomus release explain` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::release |
| tags | release, version, compatibility |

Input: none.

Output: `ReleaseExplanation`.

## `release.manifest` — A release manifest

One release explained: the version it carried, the release it succeeded, the commit, the compatibility, the contract fingerprints on both sides, the changes it published, the migration documents it requires and the artifacts it uploaded with their digests. Generated from the release state; enough to explain a release to somebody who was not there and to prove that the tag, the version, the artifacts and the runtime describe one release.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_release_manifest` |
| HTTP | `GET /api/v1/release/manifest` |
| CLI | `majordomus release manifest` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::release |
| tags | release, provenance |

| input | type | required | description |
|---|---|---|---|
| `version` | object | no | The version; the one this tree would publish when absent. |

Output: `ReleaseManifest`.

## `release.plan` — What a release would do

What `majordomus release prepare` would write and what it would publish: the target version, the bump that gets there, the change records it would stamp, every file it would rewrite, and everything that would stop it. The planning and the doing read the same engine, so a plan is a description of the act and not a simulation of it.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_release_plan` |
| HTTP | `GET /api/v1/release/plan` |
| CLI | `majordomus release plan` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::release |
| tags | release, planning |

| input | type | required | description |
|---|---|---|---|
| `version` | object | no | The version to plan for; the one this tree would publish when absent. A version
below the minimum the change requires is refused here rather than described, so
that a plan is never a description of something that cannot happen. |

Output: `ReleasePlanReport`.

## `release.status` — Release state

The whole release state: the four versions, the release the contract was measured against, the compatibility of the change, the minimum version it requires, the version this tree would publish, how far along the release is, and every diagnostic. This is the model the Cockpit's version display, the API and the release commands all read; none of them computes a release fact of its own.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_release_status` |
| MCP resource | `majordomus://release` |
| HTTP | `GET /api/v1/release/status` |
| CLI | `majordomus release status` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::release |
| tags | release, version, introspection |

Input: none.

Output: `ReleaseState`.

## `release.version` — The four versions

What this tree would release, what an unpinned installation resolves to, what this process is, and what each declared deployment reports — with every disagreement named. They are four different facts and this is the one place that refuses to collapse them: a page that shows one number while the machine serving it runs another is the failure this answer exists to make visible. No network is reached; a deployment that has reported nothing says so.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_version` |
| HTTP | `GET /api/v1/release/version` |
| CLI | `majordomus release version` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::release |
| tags | release, version, introspection |

Input: none.

Output: `VersionReport`.


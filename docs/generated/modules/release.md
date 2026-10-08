<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `release` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.18.0 -->
# Module `release` — Release

What this project has shipped and what it would ship next, derived rather than maintained: the changelog composes the layer's release records, the decisions dated inside each release's window and the conventional commits in its range; the version report reads the one place the version is authored and the projection the shell tool prints, and says what the public contract requires it to become — or that it cannot be measured, and so decides nothing — with what the commits since the last release imply beside it as evidence.

Stability: implemented. Capabilities: 3.

## `release.analysis` — What the public contract did, and the smallest version it allows

The public capability surface of this tree against the one the last release published, every movement of it named with the reason it counts for what it does, and the smallest version this tree may therefore declare. The compatibility level is measured from the contract rather than read off the commit subjects: a capability, route, MCP tool, command, or a required field of either schema that a caller could hold and cannot any more is breaking whatever the commit that removed it called itself. What the commits said about themselves is carried beside the verdict as evidence, and the answer says when the two disagree.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_release_analysis` |
| HTTP | `GET /api/v1/release/analysis` |
| cache | — |
| benchmark | waived (published_history) |
| provenance | builtin majordomus_cli::capability::builtin::release |
| tags | release, version, compatibility |

| input | type | required | description |
|---|---|---|---|
| `since` | string or null | no | A ref to compare with instead of the last release: `v0.4.0`, or any commit that
carries a committed registry. |

Output: `ReleaseVersionPlan`.

## `release.changelog` — The changelog

Every release the layer records, newest first, with the work that has not been released leading. A section's decisions are the ADRs dated inside that release's window, its changes the conventional commits in its range, its artifacts the record's own evidence. Nothing in it is authored, and a section that could not be read says so rather than appearing empty.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_changelog` |
| MCP resource | `majordomus://changelog` |
| HTTP | `GET /api/v1/changelog` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::release |
| tags | release, changelog |

| input | type | required | description |
|---|---|---|---|
| `version` | string or null | no | One version, or `unreleased`; every section when absent. |

Output: `ReleaseChangelog`.

## `release.version` — The version, and the one the contract requires next

The version the crate manifest declares — the one place it is authored — the version the shell tool prints from its projection `share/version.txt`, and whether that projection is current — the question `generate --check` refuses and `scripts/release-version --check` gates on. Then the next version, which is the public contract's answer: the version `release analyze` requires against the last release — the declared version when it already satisfies the contract, otherwise the smallest one it allows, and the smallest release above the last when the contract requires none over it — with `decided_by` naming who answered. It is the one release selection `release bump` also raises to when no target is named, so the report and the writer cannot disagree. When the contract cannot be measured — no published baseline, a version that is not three numbers, or any other error that makes the plan unsound, the plan `release bump` refuses to write from — the baseline is refused, not guessed: `next` is absent, `decided_by` is `undecided` and `contract_unreadable` carries every error, one per line. The bump the conventional commits since the last release imply, the version it would produce and the commits themselves are carried beside it as evidence, and never answer `next` in the contract's place.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_release_version` |
| HTTP | `GET /api/v1/release/version` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::release |
| tags | release, version |

Input: none.

Output: `ReleaseVersionReport`.


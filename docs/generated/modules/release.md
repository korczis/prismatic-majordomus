<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `release` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.4.0 -->
# Module `release` — Release

What this project has shipped and what it would ship next, derived rather than maintained: the changelog composes the layer's release records, the decisions dated inside each release's window and the conventional commits in its range; the version report reads the two places the version is stated and says what the commits since the last release imply it should become.

Stability: implemented. Capabilities: 2.

## `release.changelog` — The changelog

Every release the layer records, newest first, with the work that has not been released leading. A section's decisions are the ADRs dated inside that release's window, its changes the conventional commits in its range, its artifacts the record's own evidence. Nothing in it is authored, and a section that could not be read says so rather than appearing empty.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_changelog` |
| MCP resource | `majordomus://changelog` |
| HTTP | `GET /api/v1/changelog` |
| CLI | `majordomus release changelog` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::release |
| tags | release, changelog |

| input | type | required | description |
|---|---|---|---|
| `version` | string or null | no | One version, or `unreleased`; every section when absent. |

Output: `ReleaseChangelog`.

## `release.version` — The version, and the one the commits imply

The version the crate manifest declares, the version the shell tool prints, and whether they agree — the same question `scripts/release-version --check` gates on. Then the bump the conventional commits since the last release imply, the version it would produce, and the commits themselves as the evidence for it.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_release_version` |
| HTTP | `GET /api/v1/release/version` |
| CLI | `majordomus release version` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::release |
| tags | release, version |

Input: none.

Output: `ReleaseVersionReport`.


<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `distribution` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.2.0 -->
# Module `distribution` — Distribution

How this project is packaged, published and installed: the platforms a release builds, the artifact names the one naming function derives, the installer's canonical command, the releases that were published, and what this build itself is. Every answer comes from share/distribution.yaml and the release records; no surface here states a fact of its own.

Stability: behaviorally_verified. Capabilities: 3.

## `distribution.build` — This build

What this executable is: the version of the crate it was built from, the Rust target triple, the profile, and the commit — all compiled in at build time, so an installed binary answers without a repository, a toolchain or git.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_build` |
| HTTP | `GET /api/v1/distribution/build` |
| CLI | `majordomus distribution build` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::distribution |
| tags | distribution, introspection |

Input: none.

Output: `BuildReport`.

## `distribution.model` — The distribution model

The one-line install command, where an installation goes, and every declared target with the artifact name it derives. This is what the installation page, the landing page's install block and the cockpit's install card render; none of them holds a platform list of its own.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_distribution` |
| HTTP | `GET /api/v1/distribution` |
| CLI | `majordomus distribution show` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::distribution |
| tags | distribution, install, introspection |

Input: none.

Output: `DistributionReport`.

## `distribution.releases` — Published releases

Every release this repository recorded, newest first, and the one an unpinned installation resolves to: the highest version among the stable, unwithdrawn records. The pointer is derived here and never authored anywhere.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_releases` |
| HTTP | `GET /api/v1/distribution/releases` |
| CLI | `majordomus distribution releases` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::distribution |
| tags | distribution, release |

Input: none.

Output: `ReleasesReport`.


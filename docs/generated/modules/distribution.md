<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `distribution` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.6.0 -->
# Module `distribution` — Distribution

How this project is packaged, published and installed: the platforms a release builds, the artifact names the one naming function derives, the installer's canonical command, the releases that were published, and what this build itself is. Every answer comes from share/distribution.yaml and the release records; no surface here states a fact of its own.

Stability: behaviorally_verified. Capabilities: 5.

## `distribution.artifact` — The artifact of a target

The archive name a target and a tag derive, the directory it unpacks into, and where a release publishes it. The one naming function answers; the release pipeline asks it rather than composing a name in a workflow file.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_artifact` |
| HTTP | `GET /api/v1/distribution/artifact` |
| CLI | `majordomus distribution artifact` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::distribution |
| tags | distribution, release |

| input | type | required | description |
|---|---|---|---|
| `target` | string | yes | A target's id or its Rust target triple. |
| `tag` | string | yes | The tag, `v` and a version. `{tag}` asks for the name with the placeholder left in. |

Output: `ReleaseArtifactView`.

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

## `distribution.status` — Whether the published installation works

Whether a machine that has never seen this project can install it right now with the advertised one-line command, and when it cannot, which link in the chain is missing and what changes it. Derived from the distribution model and the release records alone: it reaches no network, so it is as fast as any other local query and answers offline. The served bytes are proved by the release pipeline's smoke phase, not guessed at here.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_install_status` |
| HTTP | `GET /api/v1/distribution/status` |
| CLI | `majordomus distribution status` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::distribution |
| tags | distribution, install, release, diagnostics |

Input: none.

Output: `InstallabilityReport`.


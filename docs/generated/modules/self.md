<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `self` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.0.0 -->
# Module `self` — Self-update

An installation keeps itself at the published stable release through the one installer: the running version against the version the published metadata names, forward only and stable only, and an apply that re-runs install.sh with the installation's own prefix rather than a second implementation of download, verification and swap (ADR 0161).

Stability: behaviorally_verified. Capabilities: 1.

## `self.update` — Report or apply an upgrade to the stable release

Reads the release metadata the installation's share/distribution.yaml names (releases/latest.json, or releases/<tag>.json for a named version; MAJORDOMUS_RELEASE_BASE_URL overrides the base as it does for install.sh) and judges it against the running version: an upgrade applies only to a newer, stable, listed release; a downgrade, a pre-release and an unlisted version are refused with the reason. Without --apply it writes nothing. With --apply it fetches install.sh from the same base and runs it with --version <tag>, --prefix and --install-dir of this installation, then reads the installed version back from the launcher. Exit 0 nothing to do or done, 10 refused or the installer failed, 12 not answered.

| | |
|---|---|
| kind | command |
| stability | behaviorally_verified |
| CLI | `majordomus self update` |
| cache | — |
| benchmark | waived (external_dependency) |
| provenance | builtin majordomus_cli::capability::builtin::self_update |
| tags | distribution, release, install |

| input | type | required | description |
|---|---|---|---|
| `apply` | boolean | no | Install the target. Without it the command reports and writes nothing. |
| `version` | string or null | no | The release to move to (`v1.0.0` or `1.0.0`); the stable pointer when absent. |
| `base_url` | string or null | no | The release base to read the metadata and the installer from; when absent,
`MAJORDOMUS_RELEASE_BASE_URL`, else the installer base of share/distribution.yaml. |
| `timeout_seconds` | integer or null | no | The bound on each metadata read, in seconds; twenty when absent. |

Output: `UpdateReport`.


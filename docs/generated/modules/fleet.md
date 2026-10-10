<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `fleet` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.20.0 -->
# Module `fleet` — Fleet

The machines that run this repository's mesh, as .ai/repo/fleet/ declares them, and the one operation that brings every one of them to the latest stable release the repository records: install it with the published installer, fast-forward each hub's checkout, write and restart each hub's service, and verify that each hub answers at the version and that the hubs see each other (ADR 0121).

Stability: experimental. Capabilities: 3.

## `fleet.plan` — What a rollout would do on every machine

Reads the fleet declaration and the distribution model and reaches nothing: the version every machine would be brought to, the installer, this machine's mesh node, and for each machine whether it is this one, the ssh destinations tried in order, the hub it serves and the steps a rollout takes there.

| | |
|---|---|
| kind | query |
| stability | experimental |
| CLI | `majordomus fleet plan` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::fleet |
| tags | fleet, mesh, distribution |

| input | type | required | description |
|---|---|---|---|
| `machines` | array | no | The machines to bring, by their names in the fleet; every machine when absent. |
| `version` | string or null | no | The release to install (`0.17.0` or `v0.17.0`); the latest stable release the repository
records when absent.
Named, it is installed even where a newer one is. |
| `keep_servers` | boolean | no | Leave running the servers a machine runs from an older installed tree. By default
each is stopped and started again at the version on the port it had; a session
attached to one reconnects. |

Output: `Plan`.

## `fleet.rollout` — Bring every machine of the fleet to this release, and form the mesh

On every machine named (every one by default), in parallel: reach it (this machine directly, the others by the first ssh destination that answers, never prompting); install the release with the published installer, which verifies the archive before touching anything, unless that version, or a newer one when none was named, is already there; for a hub, fast-forward its checkout to its remote's default branch (cloning it when absent, declining a dirty, diverged or off-branch one and leaving it as it is), write its service — a systemd user unit on Linux, a launchd agent on macOS — and restart it when anything changed or it does not answer at the version, then wait for it to answer at the version with its mesh active. Last, asks the converged hubs which nodes they see until every hub is seen by every other. Each machine's steps and verdict are reported; one machine's failure stops only that machine.

| | |
|---|---|
| kind | command |
| stability | experimental |
| CLI | `majordomus fleet rollout` |
| cache | — |
| benchmark | waived (destructive) |
| provenance | builtin majordomus_cli::capability::builtin::fleet |
| tags | fleet, mesh, distribution |

| input | type | required | description |
|---|---|---|---|
| `machines` | array | no | The machines to bring, by their names in the fleet; every machine when absent. |
| `version` | string or null | no | The release to install (`0.17.0` or `v0.17.0`); the latest stable release the repository
records when absent.
Named, it is installed even where a newer one is. |
| `keep_servers` | boolean | no | Leave running the servers a machine runs from an older installed tree. By default
each is stopped and started again at the version on the port it had; a session
attached to one reconnects. |

Output: `Rollout`.

## `fleet.status` — What every machine of the fleet runs

Asks every machine, in parallel and over ssh unless it is this one, what it is and runs: the destination that answered, its platform, the version its launcher runs, every majordomus serve running there, and for a hub its checkout's branch, commit and cleanliness and the version the hub answers at. A machine no destination reaches is reported with every destination's reason. Changes nothing.

| | |
|---|---|
| kind | query |
| stability | experimental |
| CLI | `majordomus fleet status` |
| cache | — |
| benchmark | waived (external_dependency) |
| provenance | builtin majordomus_cli::capability::builtin::fleet |
| tags | fleet, mesh, distribution |

Input: none.

Output: `Status`.


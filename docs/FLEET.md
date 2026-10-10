# Majordomus Fleet

The fleet is the set of machines that run a repository's mesh (docs/MESH.md), declared once
under `.ai/repo/fleet/`, and the one operation that brings every one of them to the release
this executable is: `majordomus fleet rollout` (ADR 0121).

The mesh declaration says who may link and where the hubs listen. It cannot say how a
machine gets there: which version it runs, which checkout its hub serves, what keeps that hub
running after a reboot. Before the fleet, that knowledge lived in an operator's notes and in
hand-written service files, and it drifted: on 2026-10-08 three machines ran 0.14.0, one ran
0.17.0 from a checkout build behind a hand-written systemd unit, one ran a hub started by hand
that a reboot would have ended, and the declared address of that hub was stale.

## The declaration

```yaml
# .ai/repo/fleet/majordomus.yaml
schema: fleet/v1
kind: fleet-declaration
id: majordomus
repository: https://github.com/korczis/prismatic-majordomus.git   # a hub's checkout is cloned from here
machines:
  - id: lundra
    title: lundra (Linux x86_64), a hub
    node: 25c9758fa667ea3e2dbb438c84cf4145      # majordomus mesh identity, on that machine
    ssh: [korczis@192.168.100.10, korczis@100.65.22.118]   # tried in order
    hub:
      checkout: dev/prismatic-majordomus-hub    # relative to the machine's home
      port: 8791
  - id: imac
    node: aaba18eaf51efe57bcacb03222a10e2d
    ssh: [korczis@100.122.86.48]
```

- `node` is the machine's mesh node id. The machine whose node is the operator's own is reached
  without ssh. A node id written only in digits must be quoted, or the layer's YAML reads it as
  a number.
- `ssh` holds destinations as `ssh` takes them: `user@address`, or a host alias of the
  operator's own ssh configuration. The keys stay in that configuration; nothing secret belongs
  in the declaration. An address must be private (RFC 1918), a tailnet's (100.64.0.0/10) or
  loopback; a public one is refused.
- `hub` makes the machine a rendezvous hub: a checkout used only by the hub, served on every
  interface by an always-on service. A session's checkout has a server of its own, and two
  servers never serve one checkout.

The schema is `share/schemas/majordomus/fleet-declaration/fleet-declaration.v1.schema.json`.

## The operation

```sh
majordomus fleet plan                    # what a rollout would do; reaches nothing
majordomus fleet status                  # what every machine runs; changes nothing; exits 10 if one is unreachable
majordomus fleet rollout                 # bring every machine to the latest stable release; exits 10 unless all converged
majordomus fleet rollout --machine lundra --machine imac
majordomus fleet rollout --version 0.17.0   # a named release, installed even where a newer one is
majordomus fleet rollout --keep-servers     # leave servers on an older installed tree running
```

Every machine is visited in parallel, and the results are reported in the declaration's
order. On each machine:

| step | what it does | declined when |
|---|---|---|
| `reach` | the first destination that answers `ssh -o BatchMode=yes` (this machine: directly) | none answers; no password is ever asked for |
| `probe` | platform, home, the launcher's version, the hub checkout's state, the version the hub answers at, every `majordomus serve` running | |
| `install` | the published installer, `--version` pinned; it verifies the archive's digest before touching anything | that version is installed, or a newer one is and no version was named |
| `checkout` | hub only: fast-forwards the checkout to its remote's default branch, or moves a detached one there; clones it when absent | it has uncommitted changes, commits the remote lacks, or sits on another branch; it is left as it was |
| `service` | hub only: writes `~/.config/systemd/user/majordomus-hub.service` (Linux, lingering enabled) or `~/Library/LaunchAgents/dev.majordomus.hub.plist` (macOS), runs the launcher with `serve --host 0.0.0.0 --port <port> --idle 0`, stops a server another starter left on the checkout, restarts; replaces the unit `scripts/mesh-hub` once wrote | the file already holds this content and the hub answers at the version |
| `verify` | hub: waits up to 90 s for `/api/v1/ready` at the version with the mesh active; otherwise: the launcher answers at the version | |
| `servers` | stops every server running from an installed tree of another version and starts it again through the launcher on its port; a server built from a checkout is that checkout's and is left alone | `--keep-servers` |

A machine that cannot be reached, or whose step is declined or fails, stops there and is
reported; the others go on. Last, the converged hubs are asked which nodes they see, for up
to 75 seconds, until every hub is seen by every other. The verdict is `converged` when every
machine converged and the hubs see each other, `partial` when some did, `failed` when none did.

## What it is allowed to do

`fleet.rollout` is the only capability whose effect is `remote_mutation`: it changes machines
this process does not run on. The registry admits that effect only on a command, and refuses
to build when such a capability is projected anywhere but the command line, so no agent over
MCP and no page over HTTP reaches another machine through this executable. `fleet.status`
reads over ssh and is on the command line only as well. `fleet.plan` reads the declaration
and nothing else.

Every program that runs on a machine is fixed in `apps/majordomus-cli/src/fleet/scripts.rs`
and receives the declaration's values as positional arguments, quoted for the remote shell;
nothing from the declaration is ever spliced into a program's text.

## Evidence

`test/cases/1003_the_fleet_is_rolled_out.sh` runs the real programs against machines that
are directories behind an `ssh` stub: the plan reaches nothing, the status reaches a machine
by its second destination and reports one no destination reaches, a rollout installs and
verifies, a second one changes nothing, one unreachable machine leaves the fleet `partial`,
the rollout is classified `remote_mutation`, and a public address is refused. The unit tests
in `src/fleet/mod.rs` hold the declaration's invariants and the service files' content.

The first rollout, on 2026-10-08, from the owner's MacBook Pro: four machines converged on
0.17.0, lundra's hand-written unit was replaced, the second MacBook Pro's hand-started hub
became a launchd agent, two servers on 0.14.0 were restarted at 0.17.0, and jetson was
reported unreachable on both of its addresses.

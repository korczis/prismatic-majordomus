# The build cache, as a person sees it

A local build of this repository reuses what the mesh already compiled, without anyone setting a
variable (ADR 0157, intent `a-build-reuses-what-the-mesh-already-built`). This page is what you
see; the derivation of the configuration is [MESH.md](MESH.md#the-sccache-client-of-a-node).

## Rust

Every script that builds Rust goes through `mj_cargo_bounded` in `lib/rust_bin.sh`
(`scripts/derive`, `scripts/rust-check`, the release scripts, `bin/majordomus-cli`). Before the
first cargo of a process it makes one decision, when no sccache server is listening:

| What it finds | What happens | What you see |
| --- | --- | --- |
| a server already listening on its port | nothing is started, nothing is asked | nothing |
| the store healthy, a private credential file, an sccache with the multi-level chain | the server is started with the local disk first and the store second; cargo is pointed at sccache | nothing |
| the store down, or any condition above missing | the build is local-only and succeeds | one line on stderr: `majordomus: build cache: local-only: <why>` |
| no majordomus executable yet (a cold node) | the build is what it was | one line saying so; the cache joins once the executable exists |

The store is asked once, when a server starts, and never per compilation: sccache fixes its levels
at that moment and fails a compile whose S3 level is down then, so a running server is never
handed a store later. The credential reaches the server as the path of a file; no key is ever
exported.

`MAJORDOMUS_BUILD_CACHE=off` skips all of it. A `RUSTC_WRAPPER` you set yourself, even an empty
one, is left alone.

## The gate

`scripts/ci/build-cache-check` (gate `build-cache-check`, job `structure`) refuses a tracked
script or `just` recipe that runs `cargo build|test|run`, `npm ci|install` or `mix deps.get`
outside the choke points unless the line, or the comment line before it, says why:

```sh
npm ci   # build-cache: the lockfile is generated here, nothing to reuse
```

GitHub workflows are a declared exemption: their runners cannot reach the store. Installing
JavaScript and Elixir dependencies through `majordomus mesh build-cache ensure` (`scripts/site-build`)
follows once that command lands.

## CI runner host

The build host (the policy's `local_node`, lundra) also runs the mesh's CI runners. On
2026-10-28 its root filesystem filled with the runners' cargo build directories, under homes
the disk guard did not search, and the guard itself had never been installed (I4352). The
runner units stayed active throughout: inside each one `runsvc.sh` kept relaunching a
listener that crashed, so systemd had nothing to restart while GitHub showed the runners
offline. Recovering a listener that crash-loops inside an active unit is I4345's watchdog,
not this provisioning. `majordomus mesh build-cache provision` now covers the runners from the
same declared policy (`apps/majordomus-cli/assets/mesh-cache-provider/policy.json`):

| Policy | What the provisioning installs |
| --- | --- |
| `runner_homes` (`/home/ghrunner`, `/home/gitlab-runner`) | `/etc/majordomus/disk-guard.homes`, which the disk guard reads: it reclaims cargo build output anywhere under a runner home, never a source, `.git`, registry or toolchain, only a directory whose `CACHEDIR.TAG` says cargo made it, only once nothing has written to it for its idle window and no process holds a file in it. A runner build directory over `RUNNER_CARGO_CAP_GIB` (40) is removed once it has been idle for `HARD_IDLE_MIN` |
| `runner_units` (`actions.runner.*.service`, `gitlab-runner.service`) | for every unit found on the host that matches and is not masked, `/etc/systemd/system/<unit>.d/majordomus-runner.conf`: `Restart=always`, `RestartSec=10`, no start limit, and no OOM protection, because a runner's jobs inherit it (the critical services' `50-build-host.conf` carries `OOMScoreAdjust=-900` and never reaches a GitHub runner). It brings back a runner whose `runsvc.sh` itself exits; it does not close the 2026-10-28 outage. The apply then starts every runner. `gitlab-runner`, also a critical service, keeps that drop-in beside this one, and this one's `RestartSec=10` overrides its `RestartSec=3` |

A runner home is written by the CI jobs it runs, so the guard trusts no path in it. It runs as
root, but there it renames and deletes only as the home's owner (`setpriv`), so the kernel
lets it take only what that runner could take itself, and it never takes the home, a directory
directly in it, a directory reached through a symlink, or one that holds a runner's
installation (`.runner`, `.credentials`, `config.sh`) or a source tree (`Cargo.toml`, `.git`).
An idle directory is renamed aside and checked again before it is deleted: one that came into
use in between is given back, never deleted.

`majordomus mesh doctor` on the build host shows a `build_host` row: `ok` when the host holds
the policy, `WARN` with the number of owed changes and the command when it does not. A warning
leaves the verdict as it is. The CLI prints every check that holds yet names a remedy as
`WARN`, so `mesh verify` now shows the loopback-only `endpoints` row as `WARN` too; JSON, MCP
and the UI still read a plain `ok: true` with a `remediation`. Nobody else applies the plan.
The owner runs it as root, from a checkout, with an executable that carries this change, after
a dry run of the guard over the rendered homes:

<!-- majordomus:unrun it plans and applies against the build host's own /etc and systemd as root; case 3956 runs the plan and the apply, twice, against a staging root -->
```sh
majordomus mesh build-cache provision          # the plan: what the host still owes
printf "HOMES=('/home/korczis')\nRUNNER_HOMES=('/home/ghrunner' '/home/gitlab-runner')\n" \
  > /tmp/disk-guard.homes
sudo DRY_RUN=1 HOMES_FILE=/tmp/disk-guard.homes LOCK=/tmp/disk-guard.lock \
  STATE=/tmp/disk-guard.json SCAN_STAMP=/tmp/disk-guard.scan \
  bash apps/majordomus-cli/assets/mesh-cache-provider/disk-guard   # every "would:" it would do
sudo "$(command -v majordomus)" mesh build-cache provision --apply
```

The apply enables `disk-guard.timer` at once, and its first run starts within minutes. Below
`SOFT_GIB` (80) free, every run is in the soft tier: it removes every unused `runner-` docker
volume and every cargo build directory idle for `IDLE_MIN` (two hours), in the owner's build
roots and in the runner homes alike, so the first pass on a full host is large. The host stays
in that tier until more than 80 GiB are free. A second apply changes nothing.

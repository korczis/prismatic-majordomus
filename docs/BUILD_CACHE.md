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

## The build host's own guard

The host that serves the store and runs CI (lundra) bounds its own disk with `disk-guard`, a
timer every five minutes; `build-host-observe.timer` runs `majordomus mesh doctor --build-host`
on the same cadence to prove the guard is still running rather than assume it (I4343). It fails,
naming each item, while disk-guard.timer or its own timer is not active, the ballast is missing,
disk-guard's state is older than 15 minutes, a declared critical-service drop-in is missing, or a
declared tenant (`build_host_tenants` in the policy) is over its bound. The verdict is written to
`/var/lib/majordomus/build-host.json` and, where a shared server runs, journaled as
`build_host_observed` for `mesh events` and `monitor`. See
[MESH.md](MESH.md#the-build-hosts-own-guard) for the mechanism.

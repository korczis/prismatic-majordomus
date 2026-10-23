+++
title = "The build cache, as a person sees it"
description = "what a person sees of the mesh build cache: how a Rust build starts sccache with the store as its second level, the one line a local-only build leaves, and the gate that refuses a build around the cache"
weight = 27
[extra]
source = "docs/BUILD_CACHE.md"
+++

{% raw %}

A local build of this repository reuses what the mesh already compiled, without anyone setting a
variable (ADR 0157, intent `a-build-reuses-what-the-mesh-already-built`). This page is what you
see; the derivation of the configuration is [MESH.md](@/docs/mesh.md#the-sccache-client-of-a-node).

## Rust

Every script that builds Rust goes through `mj_cargo_bounded` in `lib/rust_bin.sh`
(`scripts/derive`, `scripts/rust-check`, the release scripts, `bin/majordomus-cli`), and the
`just` recipes `build`, `build-release`, `test-rust` and `bench` call its `mj_build_cache_start`
on the line that runs cargo. Before the first cargo of a process it makes one decision, when no
sccache server is listening:

<div class="overflow-x-auto" tabindex="0">

| What it finds | What happens | What you see |
| --- | --- | --- |
| a server already listening on its port | nothing is started, nothing is asked | nothing |
| the store healthy, a private credential file, an sccache with the multi-level chain | the server is started with the local disk first and the store second; cargo is pointed at sccache | nothing |
| the store down, or any condition above missing | the build is local-only and succeeds | one line on stderr: `majordomus: build cache: local-only: <why>` |
| no majordomus executable yet (a cold node) | the build is what it was | one line saying so; the cache joins once the executable exists |

</div>


The store is asked once, when a server starts, and never per compilation: sccache fixes its levels
at that moment and fails a compile whose S3 level is down then, so a running server is never
handed a store later. The credential reaches the server as the path of a file; no key is ever
exported.

`MAJORDOMUS_BUILD_CACHE=off` skips all of it. A `RUSTC_WRAPPER` you set yourself, even an empty
one, is left alone.

## JavaScript

`scripts/site-build` and `scripts/cockpit-assets` install `node_modules` through
`mj_npm_modules_ensure` in `lib/rust_bin.sh`, which runs
`majordomus mesh build-cache ensure npm-modules --dir . -- npm ci --no-audit --no-fund`. The key
is the lockfile's digest with the platform and the node and npm versions:

<div class="overflow-x-auto" tabindex="0">

| What it finds | What happens | What you see |
| --- | --- | --- |
| no `node_modules` | restored from this machine's local level or the node's store when the key is held there; otherwise `npm ci` runs and what it left is saved to both | the ensure's report on stderr |
| a `node_modules` the cache wrote, with the lockfile's key | nothing is run | the report: `source in place` |
| a `node_modules` the cache wrote, with another key | brought to the lockfile's, as above | the report |
| a `node_modules` of your own (no key inside it), or a link to another checkout's | left alone, as before | nothing |
| no majordomus executable yet, or one older than the command | nothing is installed; the script's own check says `node_modules absent; run: npm ci`, as before | one line saying so |

</div>


`MAJORDOMUS_BUILD_CACHE=off` and a GitHub runner (`GITHUB_ACTIONS=true`, whose workflow installs
behind `actions/cache`) leave `node_modules` exactly as they were. The probes that only need
Playwright (`scripts/ui`, `scripts/cockpit-probe`, `scripts/interaction-probe`) do not install
anything: they still skip without it.

## The gate

`scripts/ci/build-cache-check` (gate `build-cache-check`, job `structure`) refuses a tracked
script or `just` recipe that compiles the crate (cargo's `build`, `test`, `run`, `bench`,
`check`, `clippy`, `doc`, `rustc`, `llvm-cov` and `nextest`, with a `+toolchain` or leading
flags) or installs dependencies (`npm ci|install|i|it|add` with or without options before them,
an `npx` that may install, `yarn install|add`, `pnpm install|i|add`, `mix deps.get`) outside
the choke points unless the line, or the comment line before it, says why:

```sh
npm ci   # build-cache: the lockfile is generated here, nothing to reuse
```

A file that defines `cargo()` as `mj_cargo_bounded`, a line that runs `mj_build_cache_start`
before its cargo, and the command a `mesh build-cache ensure` runs after its `--` are the choke
points themselves. `--version` and `--help`, and `npx --no`, build nothing and are not judged.

Declared today: the instrumented build of `scripts/rust-coverage` (`cargo llvm-cov`, in its own
target directory, whose objects no other build reuses and which a cached object must never stand
in for). GitHub workflows are a declared exemption: their runners cannot reach the store. The
behavioural cases under `test/cases/` are not read: their fixtures write build commands as text.
{% endraw %}

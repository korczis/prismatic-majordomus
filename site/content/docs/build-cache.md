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
(`scripts/derive`, `scripts/rust-check`, the release scripts, `bin/majordomus-cli`). Before the
first cargo of a process it makes one decision, when no sccache server is listening:

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
{% endraw %}

# Majordomus — the recipes a person runs here. `just` lists them; `just <recipe>` runs one.
#
# Two executables share the name `majordomus`: the shell tool `bin/majordomus` (the task
# lifecycle: init, start, check, finish, doctor, update, ...) and the Rust executable under
# apps/majordomus-cli (the read-only interfaces: MCP, HTTP, OpenAPI, Swagger UI, the Cockpit,
# introspection, generation). Everything the Rust executable can do is routed to it here;
# the shell tool keeps what only it does. Every recipe is a thin call: the source of truth
# for what a step does is the script or the command it names, never this file.

set shell := ["bash", "-euo", "pipefail", "-c"]

root      := justfile_directory()
crate     := root / "apps/majordomus-cli"
manifest  := crate / "Cargo.toml"
profile   := env("MAJORDOMUS_BUILD_PROFILE", "debug")
rust_bin  := crate / "target" / profile / "majordomus"
shell_bin := root / "bin/majordomus"

export MAJORDOMUS_SHARE := root / "share"

# List every recipe, by group.
[private]
default:
    @just --list --unsorted

# The recipes themselves live in .just/, one file per bounded context. `import` splices
# them into this one namespace, so every recipe keeps the name it always had: `just test`
# is `just test` whichever file it is written in. The order of the imports is the order
# `just --list` and `just --groups` report the groups in, and therefore the order the
# environment snapshot offers a newcomer their first commands in.
import '.just/test.just'
import '.just/build.just'
import '.just/serve.just'
import '.just/env.just'
import '.just/lifecycle.just'
import '.just/registry.just'
import '.just/bench.just'
import '.just/use-cases.just'
import '.just/ci.just'
import '.just/site.just'

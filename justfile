# Majordomus — the recipes a person runs here. `just` lists them; `just <recipe>` runs one.
#
# Two executables share the name `majordomus`: the shell tool `bin/majordomus` (the task
# lifecycle: init, start, check, finish, doctor, update, ...) and the Rust executable under
# apps/majordomus-cli (the read-only interfaces: MCP, HTTP, OpenAPI, Swagger UI, the Cockpit,
# introspection, generation). Everything the Rust executable can do is routed to it here;
# the shell tool keeps what only it does. Every recipe is a thin call: the source of truth
# for what a step does is the script or the command it names, never this file.

set shell := ["bash", "-euo", "pipefail", "-c"]

# A recipe's arguments reach its body as "$@" rather than as text spliced into it. Every
# generated bridge recipe forwards "$@", so a path with a space, a quoted string, a leading
# dash and a `$(...)` all arrive as the one argument they were typed as. Interpolating
# {{args}} would splice them, and no amount of escaping in a generator makes that safe.
set positional-arguments

root      := justfile_directory()
crate     := root / "apps/majordomus-cli"
manifest  := crate / "Cargo.toml"
profile   := env("MAJORDOMUS_BUILD_PROFILE", "debug")
# Cargo does not necessarily build into <crate>/target: CARGO_TARGET_DIR is how several
# worktrees of this repository share one build directory rather than each carrying its own
# multi-gigabyte copy, and a composed path then names an executable that is not there.
# The variable is read rather than `cargo metadata` asked, because every variable here is
# evaluated on every `just` invocation — including the bare `just` that only lists the
# recipes, which cannot spend a process on what an unset variable has already answered.
target_dir := env("CARGO_TARGET_DIR", crate / "target")
rust_bin  := target_dir / profile / "majordomus"
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
import '.just/ci.just'
import '.just/site.just'

# Every command of both programs, as a recipe, derived from the command graph: the whole of
# `majordomus <command>` and `bin/majordomus <command>`, with the descriptions their own
# declarations carry and the older recipe names kept as aliases. Nothing about a command is
# written here or in .just/ — `majordomus commands bridge` writes this file from the graph,
# entering the repository refreshes it when the graph has changed, and it is not tracked
# because it is a pure function of the tree it sits in.
#
# The import is optional so that a fresh clone, which has none, still has a justfile: `just
# bridge` writes it, and direnv does the same on the way in.
import? '.ai/local/cache/command-graph/bridge.just'

# Derive the workflow bridge from the command graph — the one recipe the bridge cannot write, because it is what writes the bridge.
[group('build')]
bridge *args:
    @bin/majordomus-cli commands bridge "$@"

# Majordomus — the recipes a person runs here. `just` lists them; `just <recipe>` runs one.
#
# Two executables share the name `majordomus`: the shell tool `bin/majordomus` (the task
# lifecycle: init, start, check, finish, doctor, update, ...) and the Rust executable under
# apps/majordomus-cli (the read-only interfaces: MCP, HTTP, OpenAPI, Swagger UI,
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

# ---------------------------------------------------------------- build

# Build the Rust executable (debug; MAJORDOMUS_BUILD_PROFILE=release for a release build).
[group('build')]
build:
    RUSTFLAGS='' cargo build --locked --manifest-path "{{manifest}}" {{ if profile == "release" { "--release" } else { "" } }}

# Build the release executable.
[group('build')]
build-release:
    RUSTFLAGS='' cargo build --locked --release --manifest-path "{{manifest}}"

# Remove the Rust build output.
[group('build')]
[confirm("Remove apps/majordomus-cli/target? [y/N]")]
clean:
    cargo clean --manifest-path "{{manifest}}"

# ---------------------------------------------------------------- serve (Rust executable)

# MCP on stdio for the client that spawned it, joining or starting the repository's one shared server (Swagger UI at /docs). Extra arguments pass through.
[group('serve')]
mcp *args: build
    "{{rust_bin}}" mcp {{args}}

# The shared server alone, on 127.0.0.1:8741 by default: Swagger UI /docs, /openapi.json, MCP over HTTP /mcp. Exits 0 if one already runs.
[group('serve')]
serve *args: build
    "{{rust_bin}}" serve {{args}}

# What `mcp` would serve, and every diagnostic; exit 10 when the layer is degraded.
[group('serve')]
inspect *args: build
    "{{rust_bin}}" mcp --inspect {{args}}

# Where the repository's shared server is, if one runs (from the lease under .ai/local/).
[group('serve')]
mcp-status:
    @f=".ai/local/state/mcp/server.json"; if [ -f "$f" ]; then cat "$f"; echo; else echo "no shared server lease at $f"; fi

# Open Swagger UI of the running shared server in the browser (macOS `open`, else xdg-open).
[group('serve')]
docs-ui:
    @f=".ai/local/state/mcp/server.json"; [ -f "$f" ] || { echo "no shared server is running (just serve, or start an MCP client)"; exit 1; }; \
    url="$(sed -n 's/.*"url":"\([^"]*\)".*/\1/p' "$f")"; echo "$url/docs"; (command -v open >/dev/null && open "$url/docs") || xdg-open "$url/docs"

# ---------------------------------------------------------------- registry (Rust executable)

# Every capability with its projections (Rust registry). Extra arguments pass through (--kind, --exposure, --format).
[group('registry')]
capabilities *args: build
    "{{rust_bin}}" capabilities list {{args}}

# One capability by id: schemas, provenance, every projection.
[group('registry')]
describe id *args: build
    "{{rust_bin}}" capabilities describe "{{id}}" {{args}}

# The registry's invariants and every projection; exit 10 with every violation named.
[group('registry')]
validate: build
    "{{rust_bin}}" capabilities validate

# Regenerate every projection: docs/generated/, share/allow/, AGENTS.md/CLAUDE.md/... from the policy, site/data/registry/ from the registry.
[group('registry')]
generate: build
    "{{rust_bin}}" generate

# Exit 10 naming every stale generated projection; writes nothing.
[group('registry')]
generate-check: build
    "{{rust_bin}}" generate --check

# ---------------------------------------------------------------- benchmarks (Rust executable)

# Time every externally callable operation (each capability directly, over MCP and over HTTP, and the transports' own operations). `just bench-run objects.search --transport mcp --profile full` narrows it.
[group('bench')]
bench-run *args: build
    "{{rust_bin}}" bench {{args}}

# Benchmark coverage: covered / required, the denominator generated from the registry; exit 10 when anything is missing or waived.
[group('bench')]
bench-coverage *args: build
    "{{rust_bin}}" bench coverage --check {{args}}

# Compare a run with this platform's accepted baseline under .ai/repo/benchmarks/rust/ (policy.yaml); exit 10 on a regression.
[group('bench')]
bench-check *args: build
    "{{rust_bin}}" bench --profile ci --check --no-write {{args}}

# Record this platform's baseline from a full run (a reviewable, tracked file); refuses a dirty tree.
[group('bench')]
bench-baseline *args: build
    "{{rust_bin}}" bench baseline update {{args}}

# ---------------------------------------------------------------- lifecycle (shell tool)

# What the next worker needs to know now, within budget.
[group('lifecycle')]
context *args:
    "{{shell_bin}}" context {{args}}

# Is Majordomus itself healthy and wired here?
[group('lifecycle')]
doctor *args:
    "{{shell_bin}}" doctor {{args}}

# What has drifted: state, policy, projections, retention.
[group('lifecycle')]
watch *args:
    "{{shell_bin}}" watch {{args}}

# Is the current task consistent with policy, scope and state?
[group('lifecycle')]
check *args:
    "{{shell_bin}}" check {{args}}

# Regenerate AGENTS.md, CLAUDE.md and the other projections from the policy.
[group('lifecycle')]
update *args:
    "{{shell_bin}}" update {{args}}

# The repository's skills: list, show <id>, or check every one against its contract.
[group('lifecycle')]
skills *args:
    "{{shell_bin}}" skills {{args}}

# ---------------------------------------------------------------- test

# Every gate: the shell suite, the Rust gate, the site data check.
[group('test')]
test: test-shell rust-check derive-check

# The behavioural suite of the shell tool and the cross-checks of the Rust executable (test/cases/*.sh), MJ_TEST_JOBS cases at a time (default 4 here; `MJ_TEST_JOBS=1 just test-shell` streams serially). `just test-shell 72_rust_mcp` runs one.
[group('test')]
test-shell *only:
    MJ_TEST_JOBS="${MJ_TEST_JOBS:-4}" bash test/run.sh {{only}}

# The Rust crate's own suites: unit, integration, doctests.
[group('test')]
test-rust *args:
    RUSTFLAGS='' cargo test --manifest-path "{{manifest}}" --no-fail-fast {{args}}

# Every Rust gate CI runs, in CI's order: fmt, clippy -D warnings, tests, rustdoc, benches compile, validate, generate --check, coverage threshold.
[group('test')]
rust-check:
    scripts/rust-check

# Line coverage of the Rust crate, against the threshold in scripts/rust-coverage-threshold.
[group('test')]
coverage:
    cd "{{crate}}" && RUSTFLAGS='' cargo llvm-cov --all-targets --summary-only --fail-under-lines "$(cat "{{root}}/scripts/rust-coverage-threshold")"

# The criterion microbenchmarks (benches/projections.rs, benches/shared.rs, benches/scaling.rs). `just bench scaling` runs one; `just bench-run` is the end-to-end measurement.
[group('test')]
bench *name:
    RUSTFLAGS='' cargo bench --manifest-path "{{manifest}}" {{ if name == "" { "" } else { "--bench " + name } }}

# bash -n and shellcheck over the shell tool, the scripts and every case (scripts/ci/shell-lint, the same gate CI runs).
[group('test')]
lint-shell:
    scripts/ci/shell-lint

# ---------------------------------------------------------------- ci (docs/CI.md)

# What CI would run for this working tree against master, from .ai/repo/ci/gates.yaml; extra arguments pass to scripts/ci-plan (--full, --base, --head, --files).
[group('ci')]
ci-plan *args:
    scripts/ci-plan --format text {{args}}

# The gates every plan runs: shell syntax and shellcheck, then doctor, watch, the context documents, the continuity commands, plan validate, the offline GitHub projection and the derived site data.
[group('ci')]
ci-structure:
    scripts/ci/shell-lint
    scripts/ci/core-check

# The gates the plan selects for this working tree, with the commands CI runs, in the plan's order.
[group('ci')]
ci-fast *args:
    scripts/ci/run-plan {{args}}

# Every gate, as a push to master runs them (the macOS gate only on macOS).
[group('ci')]
ci-full:
    scripts/ci/run-plan --full "just ci-full"

# What GitHub observed of recent runs, recorded into .ai/repo/ci/baseline.json (needs gh); `just ci-baseline --table` renders it.
[group('ci')]
ci-baseline *args:
    scripts/ci-baseline {{args}}

# ---------------------------------------------------------------- site and derived files

# Regenerate site/data/generated and the derived docs from README, docs/, the policy skeleton and CLAIMS.yaml.
[group('site')]
site-data:
    scripts/generate-site-data

# Exit 10 when the derived site data is stale.
[group('site')]
site-data-check:
    scripts/generate-site-data --check

# Deploy the site by hand: gate, build, check, push gh-pages (see .ai/repo/skills/deploy-site.md). `just site-deploy --dry-run` shows what it would push.
[group('site')]
site-deploy *args:
    scripts/site-deploy {{args}}

# Build the website (zola).
[group('site')]
site-build:
    scripts/site-build

# The static checks over the built site (scripts/site-check).
[group('site')]
site-check:
    scripts/site-check

# Every route at three widths in a real browser, SITE_PROBE_JOBS routes at a time (default 4 here); `just site-probe --quick` samples one route per template.
[group('site')]
site-probe *args:
    SITE_PROBE_JOBS="${SITE_PROBE_JOBS:-4}" scripts/site-probe {{args}}

# Serve the website locally in watch mode.
[group('site')]
site-serve:
    scripts/site-serve

# Every committed derived artifact, regenerated in dependency order (scripts/derive: generate, site data, generate again over the index the site data changed).
[group('site')]
derive:
    scripts/derive

# Exit 10 naming every stale derived artifact — the registry's projections and the site's data — and write nothing (scripts/derive-check).
[group('site')]
derive-check:
    scripts/derive-check
